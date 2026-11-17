use horizon_java::{host::RecordingHost, loader};

#[test]
fn transformation_requirements_are_not_silently_skipped() {
    use std::io::{Cursor, Write};
    let mut source =
        horizon_java::jar::Jar::open(include_bytes!("data/fabric-probe.jar").to_vec()).unwrap();
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options: zip::write::SimpleFileOptions = Default::default();
    for path in source.files().to_vec() {
        let mut body = source.read(&path).unwrap();
        if path == "fabric.mod.json" {
            let mut metadata: serde_json::Value = serde_json::from_slice(&body).unwrap();
            metadata["accessWidener"] = "probe.accesswidener".into();
            body = serde_json::to_vec(&metadata).unwrap();
        }
        zip.start_file(path, options).unwrap();
        zip.write_all(&body).unwrap();
    }
    zip.start_file("probe.accesswidener", options).unwrap();
    zip.write_all(b"accessWidener v2 named\naccessible class fixture/FabricProbe\n")
        .unwrap();
    let bytes = zip.finish().unwrap().into_inner();
    let error = loader::initialize(bytes, RecordingHost::default())
        .err()
        .unwrap();
    assert!(error.contains("unsupported transformations"), "{error}");
}

#[test]
fn fabric_jar_executes_its_own_entrypoint() {
    let mut loaded = loader::initialize(
        include_bytes!("data/fabric-probe.jar").to_vec(),
        RecordingHost::default(),
    )
    .unwrap();
    assert_eq!(
        loaded.vm.host().recorded.logs,
        ["[fabricprobe] fabric entrypoint executed"]
    );
    assert!(!loaded.report.verified);
    loaded.vm.tick().unwrap();
    loaded.vm.tick().unwrap();
    assert_eq!(
        &loaded.vm.host().recorded.logs[1..],
        ["[fabricprobe] tick=1", "[fabricprobe] tick=2"]
    );
}

#[test]
fn neoforge_jar_executes_annotated_constructor() {
    let loaded = loader::initialize(
        include_bytes!("data/neoforge-probe.jar").to_vec(),
        RecordingHost::default(),
    )
    .unwrap();
    assert_eq!(
        loaded.vm.host().recorded.logs,
        ["[neoprobe] neoforge constructor executed"]
    );
    assert!(!loaded.report.verified);
}

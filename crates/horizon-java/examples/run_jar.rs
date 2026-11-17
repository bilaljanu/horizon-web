//! Explicit development runner; does not load Horizon worlds or register content.
use horizon_java::{host::RecordingHost, loader};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("usage: run_jar <mod.jar>")?;
    let bytes = std::fs::read(path)?;
    let mut loaded = loader::initialize(bytes, RecordingHost::default())?;
    loaded.vm.tick()?;
    println!(
        "{} {}: initialization and one synthetic tick completed (gameplay unverified)",
        loaded.report.metadata.loader.name(),
        loaded.report.metadata.id
    );
    for line in &loaded.vm.host().recorded.logs {
        println!("{line}");
    }
    Ok(())
}

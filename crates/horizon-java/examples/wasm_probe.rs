//! Browser-callable regression probe. Exercises the interpreter, the jar
//! lifecycle, and the Java -> Horizon content registration pipeline; it does
//! not start the renderer. Codes are checked by probe.html.
use horizon_java::{host::RecordingHost, loader};
use std::sync::atomic::{AtomicU32, Ordering};

static CHECKPOINT: AtomicU32 = AtomicU32::new(0);
fn mark(n: u32) {
    CHECKPOINT.store(n, Ordering::Relaxed);
}

#[unsafe(no_mangle)]
pub extern "C" fn horizon_java_checkpoint() -> u32 {
    CHECKPOINT.load(Ordering::Relaxed)
}
use horizon_mods::{
    Capability, Content, Manifest, Side,
    content::{Block, BlockProperties, CreativeTab, Item, RandomTicks},
};

fn manifest(id: &str, name: &str) -> Manifest {
    Manifest {
        id: id.into(),
        name: name.into(),
        version: "0.1.0".into(),
        description: String::new(),
        authors: Vec::new(),
        api: "0.2".into(),
        side: Side::Both,
        capabilities: vec![Capability::Registries],
        namespaces: Vec::new(),
        experimental: Vec::new(),
    }
}

/// Build content from a Java jar exactly as the game's adapter does: read the
/// archive's assets/data, turn recorded Java registrations into Horizon
/// blocks/items, declare them and create a creative tab.
fn content_for(jar_bytes: &[u8], id: &str, name: &str) -> Result<Content, u32> {
    let loaded = loader::initialize(jar_bytes.to_vec(), RecordingHost::default())
        .map_err(|_| 10u32)?;
    let m = manifest(id, name);
    mark(30);
    let snapshot = std::path::Path::new("/tmp/java-probe.jar");
    let _ = horizon_os::fs::create_dir_all("/tmp");
    mark(31);
    horizon_os::fs::write(snapshot, jar_bytes).map_err(|_| 11u32)?;
    mark(32);
    let mut content = Content::read(snapshot, &m).map_err(|_| 12u32)?;
    mark(33);
    let recorded = &loaded.vm.host().recorded;
    let blocks: Vec<Block> = recorded
        .blocks
        .values()
        .map(|b| Block {
            id: b.id.clone(),
            template: b.template.clone(),
            item: false,
            properties: BlockProperties {
                hardness: b.hardness,
                explosion_resistance: b.resistance,
                light: b.light,
                requires_tool: b.requires_tool,
                random_ticks: b.random_ticks.map(RandomTicks::All),
                occludes: b.no_occlusion.map(|v| !v),
                ..Default::default()
            },
            ..Default::default()
        })
        .collect();
    let items: Vec<Item> = recorded
        .items
        .values()
        .map(|i| Item {
            id: i.id.clone(),
            template: i.template.clone(),
            max_stack_size: i.max_stack_size,
            place_block: i.block.clone(),
            ..Default::default()
        })
        .collect();
    content.declare(&blocks, &items).map_err(|_| 13u32)?;
    mark(34);
    if items.is_empty() {
        return Err(14);
    }
    content.creative_tabs.push(CreativeTab {
        id: format!("{id}:items"),
        title: name.into(),
        icon: items[0].id.clone(),
        items: items.iter().map(|i| i.id.clone()).collect(),
    });
    Ok(content)
}

#[unsafe(no_mangle)]
pub extern "C" fn horizon_java_probe() -> u32 {
    mark(1);
    let Ok(mut fabric) = loader::initialize(
        include_bytes!("../tests/data/fabric-probe.jar").to_vec(),
        RecordingHost::default(),
    ) else {
        return 1;
    };
    if fabric.vm.tick().is_err() || fabric.vm.tick().is_err() {
        return 2;
    }
    if fabric.vm.host().recorded.logs
        != [
            "[fabricprobe] fabric entrypoint executed",
            "[fabricprobe] tick=1",
            "[fabricprobe] tick=2",
        ]
    {
        return 3;
    }
    let Ok(neo) = loader::initialize(
        include_bytes!("../tests/data/neoforge-probe.jar").to_vec(),
        RecordingHost::default(),
    ) else {
        return 4;
    };
    if neo.vm.host().recorded.logs != ["[neoprobe] neoforge constructor executed"] {
        return 5;
    }
    mark(10);
    // Gameplay content: the same declarations the native game uses.
    for (bytes, id, name) in [
        (
            include_bytes!("../tests/data/fabric-probe.jar").as_slice(),
            "fabricprobe",
            "Fabric Probe",
        ),
        (
            include_bytes!("../tests/data/neoforge-probe.jar").as_slice(),
            "neoprobe",
            "NeoForge Probe",
        ),
    ] {
        let content = match content_for(bytes, id, name) {
            Ok(c) => c,
            Err(code) => return code,
        };
        if content.blocks.len() != 1 || content.items.len() != 2 {
            return 15;
        }
        let texture = format!("assets/{id}/textures/block/ruby_block.png");
        let Ok(png) = content.file(&texture) else {
            return 16;
        };
        if png.len() < 8 || png[..8] != *b"\x89PNG\r\n\x1a\n" {
            return 17;
        }
        let model = format!("assets/{id}/models/block/ruby_block.json");
        if content.file(&model).is_err() {
            return 18;
        }
        let loot = format!("data/{id}/loot_table/blocks/ruby_block.json");
        if content.file(&loot).is_err() {
            return 19;
        }
        let tab = content
            .creative_tabs
            .iter()
            .find(|t| t.id == format!("{id}:items"))
            .unwrap();
        if !tab.items.iter().any(|i| i == &format!("{id}:ruby_block"))
            || !tab.items.iter().any(|i| i == &format!("{id}:ruby"))
        {
            return 20;
        }
    }
    0
}

fn main() {
    assert_eq!(horizon_java_probe(), 0);
}

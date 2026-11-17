//! Java registrations enter the same validated content overlay as native mods.
//! A VM is retained per immutable jar snapshot. Gameplay callback dispatch is
//! intentionally separate from resource reloads: reloads never rerun Java code.
use horizon_java::{host::RecordingHost, loader, runtime::Vm};
use horizon_mods::{Content, Manifest, Capability, Side, content::{Block, BlockProperties, Item, CreativeTab, RandomTicks}};
use std::{collections::BTreeMap, path::Path, sync::Mutex};
use sha2::{Digest, Sha256};

struct Prepared {
    manifest: Manifest,
    content: Content,
    vm: Vm<RecordingHost>,
    active: bool,
    failed: bool,
}
static PREPARED: Mutex<BTreeMap<String, Result<Prepared, String>>> = Mutex::new(BTreeMap::new());

pub(super) fn load(path: &Path, cache: &Path) -> Result<(Manifest, Content), String> {
    let size = horizon_os::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > horizon_java::jar::MAX_JAR { return Err("Java archive exceeds size limit".into()); }
    let bytes = horizon_os::fs::read(path).map_err(|e| e.to_string())?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let mut prepared = PREPARED.lock().map_err(|e| e.to_string())?;
    let entry = prepared.entry(hash.clone()).or_insert_with(|| prepare(bytes, cache, &hash));
    match entry {
        Ok(p) => Ok((p.manifest.clone(), p.content.clone())),
        Err(e) => Err(e.clone()),
    }
}

fn prepare(bytes: Vec<u8>, cache: &Path, hash: &str) -> Result<Prepared, String> {
    let loaded = loader::initialize(bytes.clone(), RecordingHost::default())?;
    let m = &loaded.report.metadata;
    let manifest = Manifest {
        id: m.id.clone(), name: m.name.clone(), version: m.version.clone(), description: m.description.clone(),
        authors: m.authors.clone(), api: "0.2".into(), side: Side::Both,
        capabilities: vec![Capability::Registries], namespaces: Vec::new(), experimental: Vec::new(),
    };
    let directory = cache.join("java");
    horizon_os::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let snapshot = directory.join(format!("{hash}.jar"));
    horizon_os::fs::write(&snapshot, bytes).map_err(|e| e.to_string())?;
    let mut content = Content::read(&snapshot, &manifest)?;
    let recorded = &loaded.vm.host().recorded;
    let blocks: Vec<Block> = recorded.blocks.values().map(|b| Block {
        id: b.id.clone(), template: b.template.clone(), item: false,
        properties: BlockProperties { hardness: b.hardness, explosion_resistance: b.resistance,
            light: b.light, requires_tool: b.requires_tool, random_ticks: b.random_ticks.map(RandomTicks::All),
            occludes: b.no_occlusion.map(|v| !v), ..Default::default() }, ..Default::default()
    }).collect();
    let items: Vec<Item> = recorded.items.values().map(|i| Item {
        id: i.id.clone(), template: i.template.clone(), max_stack_size: i.max_stack_size,
        place_block: i.block.clone(), ..Default::default()
    }).collect();
    content.declare(&blocks, &items)?;
    // The first supported slice has a Horizon-created tab containing exactly
    // the Java-registered items. This does not claim Fabric creative-tab ABI.
    if !items.is_empty() && content.creative_tabs.is_empty() {
        content.creative_tabs.push(CreativeTab {
            id: format!("{}:items", m.id), title: m.name.clone(), icon: items[0].id.clone(),
            items: items.iter().map(|i| i.id.clone()).collect(),
        });
    }
    Ok(Prepared { manifest, content, vm: loaded.vm, active: false, failed: false })
}

pub(super) fn deactivate() {
    if let Ok(mut prepared) = PREPARED.lock() {
        for p in prepared.values_mut().filter_map(|p| p.as_mut().ok()) { p.active = false; }
    }
}

pub(super) fn activate(id: &str, source: &Path) {
    if let Ok(mut prepared) = PREPARED.lock() {
        for p in prepared.values_mut().filter_map(|p| p.as_mut().ok()) {
            if p.manifest.id == id { p.active = p.content.source() == source; }
        }
    }
}

/// Called exclusively from the authoritative server's end-tick hook.
pub(super) fn server_tick() {
    let Ok(mut prepared) = PREPARED.lock() else { return; };
    for p in prepared.values_mut().filter_map(|p| p.as_mut().ok()).filter(|p| p.active && !p.failed) {
        if let Err(error) = p.vm.tick() {
            p.failed = true;
            log::error!("Java callbacks disabled for {}: {error}", p.manifest.id);
        }
        for message in p.vm.host_mut().recorded.logs.drain(..) { log::info!("{message}"); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::*;
    use horizon_client::{chunk::{Chunk, LevelShape}, items::ItemStack, mod_content::{self, ContentMod}, save};
    use std::sync::Arc;

    #[test]
    #[ignore = "loads real prepared Minecraft resources from HORIZON_TEST_RESOURCES"]
    fn java_jars_place_break_and_save_in_real_game() {
        let base = std::path::PathBuf::from(std::env::var_os("HORIZON_TEST_RESOURCES").expect("prepared resources required"));
        let scratch = std::env::temp_dir().join(format!("horizon-java-game-{}", std::process::id()));
        std::fs::create_dir_all(&scratch).unwrap();
        for (id, bytes) in [
            ("fabricprobe", include_bytes!("../../../../horizon-java/tests/data/fabric-probe.jar").as_slice()),
            ("neoprobe", include_bytes!("../../../../horizon-java/tests/data/neoforge-probe.jar").as_slice()),
        ] {
            let jar = scratch.join(format!("{id}.jar"));
            std::fs::write(&jar, bytes).unwrap();
            let (manifest, content) = load(&jar, &scratch.join("cache")).unwrap();
            assert_eq!(content.blocks.len(), 1);
            assert_eq!(content.items.len(), 2);
            assert_eq!(content.file(&format!("assets/{id}/textures/block/ruby_block.png")).unwrap()[..8], *b"\x89PNG\r\n\x1a\n");
            let mut mods = Vec::new();
            mod_content::accept_mod(&mut mods, ContentMod { id: manifest.id, content }, &mod_content::known_components(&base).unwrap()).unwrap();
            let root = mod_content::resource_root(&base, &mods, &scratch.join(format!("overlay-{id}"))).unwrap();
            // The game's real PNG decoder reads the mod's texture from the overlay.
            let texture = root.join(format!("assets/{id}/textures/block/ruby_block.png"));
            let (w, h, pixels) = horizon_client::hud::decode(&texture).unwrap();
            assert_eq!((w, h), (16, 16));
            assert_eq!(pixels.len(), 16 * 16 * 4);
            for (n, name) in [
                (format!("assets/{id}/models/block/ruby_block.json"), "block model"),
                (format!("assets/{id}/items/ruby.json"), "item model"),
                (format!("data/{id}/loot_table/blocks/ruby_block.json"), "loot table"),
            ] {
                assert!(root.join(&n).exists(), "{name} missing from overlay: {n}");
            }
            let creative = horizon_client::creative::Creative::load(&root).unwrap();
            let tab = creative.tabs.iter().find(|tab| tab.id == format!("{id}:items")).unwrap();
            assert!(tab.items.iter().any(|item| &*item.item == format!("{id}:ruby_block")));
            assert!(tab.items.iter().any(|item| &*item.item == format!("{id}:ruby")));
            let settings = Settings::default();
            let loaded = load_resources(root.clone(), None, &root, &settings, std::time::Instant::now(), false, true, &Progress::new(), &mut |_, _| unreachable!()).unwrap();
            let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
            options.resources = root.clone();
            options.execution_role = execution_role::ExecutionRole::IntegratedServer;
            let mut game = Game::new(&loaded.resources, loaded.pack, loaded.gui.unwrap(), options, settings, Audio::silent(&root), None).unwrap();
            game.build_registries();
            game.world = World::new(game.res.facts().air, 1);
            for x in -1..=1 { for z in -1..=1 {
                game.world.chunks.insert((x, z), Arc::new(Chunk::empty(x, z)));
            } }
            game.sync_tick_chunks();
            game.set_mode_id(0);
            game.player = None;
            game.selected = 0;
            let item = format!("{id}:ruby_block");
            let target = game.pack.assets.registry.default_state(&item).unwrap();
            let pos = [1, 70, 1];
            let hit = Hit { pos, face: [0, 1, 0], point: [1.5, 70.0, 1.5] };
            *game.held_mut() = Some(ItemStack::new(&item, 2));
            assert!(game.use_hand(Some(&hit)).unwrap());
            assert_eq!(game.world.block(1, 70, 1), target);
            assert_eq!(game.held().unwrap().count, 1);
            let save_root = scratch.join(format!("world-{id}"));
            let save_dir = save::SaveDir::new(save_root.clone(), LevelShape::OVERWORLD);
            let chunk = game.world.chunks.get(&(0, 0)).unwrap().clone();
            save_dir.save(&[chunk], game.pack.assets.registry, game.res.biomes()).unwrap();
            save_dir.flush().unwrap();
            drop(save_dir);
            // New reader: this must load the on-disk region, not a pending-write cache.
            let reopened = save::SaveDir::new(save_root, LevelShape::OVERWORLD);
            let chunk = reopened.load(0, 0, game.pack.assets.registry, game.res.biomes()).unwrap();
            game.world.chunks.insert((0, 0), Arc::new(chunk));
            assert_eq!(game.world.block(1, 70, 1), target);
            let ruby = ItemStack::new(&format!("{id}:ruby"), 7);
            let inventory = scratch.join(format!("inventory-{id}.dat"));
            save::write_dat(&inventory, save::item_to_nbt(&ruby)).unwrap();
            let restored = save::item_from_nbt(&save::read_dat(&inventory).unwrap()).unwrap();
            assert_eq!((&*restored.item, restored.count), (&*ruby.item, 7));
            game.destroy_block_dropping(pos).unwrap();
            assert_eq!(game.world.block(1, 70, 1), game.res.facts().air);
        }
        std::fs::remove_dir_all(scratch).unwrap();
    }
}

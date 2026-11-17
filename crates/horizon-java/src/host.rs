//! What a Java mod asks of Horizon, independent of the JVM implementation.
//!
//! The browser and native builds use the same interface. The game supplies a
//! [`Host`] that records registrations in Horizon's content pipeline and
//! forwards chat, logging and world queries; tests supply a recording host.
//! Nothing here is a mock of Mojang's classes: these are the operations
//! Horizon actually performs for a mod.

use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

pub trait Host: Send {
    fn log(&mut self, level: LogLevel, mod_id: &str, message: &str);
    /// A block registered through `Registry.register(Registries.BLOCK, …)`.
    fn register_block(&mut self, mod_id: &str, spec: BlockSpec) -> Result<(), String>;
    /// An item registered through `Registry.register(Registries.ITEM, …)`.
    fn register_item(&mut self, mod_id: &str, spec: ItemSpec) -> Result<(), String>;
    fn register_creative_tab(&mut self, mod_id: &str, spec: CreativeTabSpec) -> Result<(), String>;
    /// A resource the mod asked the game to reload (a texture, model, sound
    /// or data file).
    fn register_resource(&mut self, mod_id: &str, path: &str, bytes: &[u8]) -> Result<(), String>;
    /// A line a mod showed the player.
    fn message(&mut self, mod_id: &str, text: &str);
    /// The integrated world's game time in ticks, if a world is open.
    fn world_time(&self) -> Option<i64>;
    /// The acting player's position, if there is one.
    fn player_position(&self) -> Option<[f64; 3]>;
    /// The acting player's name, if there is one.
    fn player_name(&self) -> Option<String>;
    /// Whether the player is in a world (a client or integrated server).
    fn in_world(&self) -> bool;
    /// Put a block into the world. `state` is a Horizon block id.
    fn set_block(&mut self, dimension: &str, pos: [i32; 3], block: &str) -> Result<(), String>;
    /// The block at a position, as a Horizon block id.
    fn block(&self, dimension: &str, pos: [i32; 3]) -> Option<String>;
    /// The dimension the acting player is in.
    fn dimension(&self) -> Option<String>;
    /// Send a custom payload to the integrated server or the connected one.
    fn send_payload(&mut self, channel: &str, data: &[u8]) -> Result<(), String>;
    /// A loaded resource from the mod's own jar, if the game exposes it.
    fn resource(&self, mod_id: &str, path: &str) -> Option<Vec<u8>>;
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BlockSpec {
    /// `<mod id>:<path>`.
    pub id: String,
    /// The vanilla block whose behaviour and model this copies.
    pub template: String,
    pub hardness: Option<f32>,
    pub resistance: Option<f32>,
    pub light: Option<u8>,
    pub requires_tool: Option<bool>,
    pub random_ticks: Option<bool>,
    pub no_occlusion: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemSpec {
    pub id: String,
    /// The vanilla item whose components this copies.
    pub template: String,
    pub max_stack_size: Option<u8>,
    pub rarity: Option<String>,
    /// The block this item places, when it is a `BlockItem`.
    pub block: Option<String>,
    pub creative_tab: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CreativeTabSpec {
    pub id: String,
    pub title: String,
    pub icon: String,
    pub items: Vec<String>,
}

/// A host that only logs. Used by the compatibility checker and by a game
/// with Java mods turned off.
#[derive(Default)]
pub struct LoggingHost {
    pub lines: Vec<String>,
}

impl Host for LoggingHost {
    fn log(&mut self, level: LogLevel, mod_id: &str, message: &str) {
        let line = format!("[{mod_id}] {message}");
        match level {
            LogLevel::Error => log::error!("{line}"),
            LogLevel::Warn => log::warn!("{line}"),
            LogLevel::Info => log::info!("{line}"),
            LogLevel::Debug => log::debug!("{line}"),
            LogLevel::Trace => log::trace!("{line}"),
        }
        self.lines.push(line);
    }
    fn register_block(&mut self, _: &str, spec: BlockSpec) -> Result<(), String> {
        self.lines.push(format!("block {}", spec.id));
        Ok(())
    }
    fn register_item(&mut self, _: &str, spec: ItemSpec) -> Result<(), String> {
        self.lines.push(format!("item {}", spec.id));
        Ok(())
    }
    fn register_creative_tab(&mut self, _: &str, spec: CreativeTabSpec) -> Result<(), String> {
        self.lines.push(format!("tab {}", spec.id));
        Ok(())
    }
    fn register_resource(&mut self, _: &str, path: &str, _: &[u8]) -> Result<(), String> {
        self.lines.push(format!("resource {path}"));
        Ok(())
    }
    fn message(&mut self, _: &str, text: &str) {
        self.lines.push(format!("message {text}"));
    }
    fn world_time(&self) -> Option<i64> {
        Some(0)
    }
    fn player_position(&self) -> Option<[f64; 3]> {
        Some([0.5, 64.0, 0.5])
    }
    fn player_name(&self) -> Option<String> {
        Some("Dev".into())
    }
    fn in_world(&self) -> bool {
        false
    }
    fn set_block(&mut self, _: &str, _: [i32; 3], _: &str) -> Result<(), String> {
        Ok(())
    }
    fn block(&self, _: &str, _: [i32; 3]) -> Option<String> {
        None
    }
    fn dimension(&self) -> Option<String> {
        None
    }
    fn send_payload(&mut self, _: &str, _: &[u8]) -> Result<(), String> {
        Ok(())
    }
    fn resource(&self, _: &str, _: &str) -> Option<Vec<u8>> {
        None
    }
}

/// Registrations a host recorded, for a caller that wants to turn them into
/// Horizon content files after the runtime has stopped.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recorded {
    pub blocks: BTreeMap<String, BlockSpec>,
    pub items: BTreeMap<String, ItemSpec>,
    pub tabs: BTreeMap<String, CreativeTabSpec>,
    pub resources: BTreeMap<String, Vec<u8>>,
    pub messages: Vec<String>,
    pub logs: Vec<String>,
}

/// A host that records everything, for tests and for the loader's first
/// pass (which only needs to know what a mod declares).
#[derive(Default)]
pub struct RecordingHost {
    pub recorded: Recorded,
    pub fail_block: Option<String>,
}

impl Host for RecordingHost {
    fn log(&mut self, _: LogLevel, mod_id: &str, message: &str) {
        self.recorded.logs.push(format!("[{mod_id}] {message}"));
    }
    fn register_block(&mut self, _: &str, spec: BlockSpec) -> Result<(), String> {
        if self.fail_block.as_deref() == Some(spec.id.as_str()) {
            return Err(format!("the host refused block {}", spec.id));
        }
        self.recorded.blocks.insert(spec.id.clone(), spec);
        Ok(())
    }
    fn register_item(&mut self, _: &str, spec: ItemSpec) -> Result<(), String> {
        self.recorded.items.insert(spec.id.clone(), spec);
        Ok(())
    }
    fn register_creative_tab(&mut self, _: &str, spec: CreativeTabSpec) -> Result<(), String> {
        self.recorded.tabs.insert(spec.id.clone(), spec);
        Ok(())
    }
    fn register_resource(&mut self, _: &str, path: &str, bytes: &[u8]) -> Result<(), String> {
        self.recorded
            .resources
            .insert(path.to_owned(), bytes.to_vec());
        Ok(())
    }
    fn message(&mut self, _: &str, text: &str) {
        self.recorded.messages.push(text.to_owned());
    }
    fn world_time(&self) -> Option<i64> {
        Some(0)
    }
    fn player_position(&self) -> Option<[f64; 3]> {
        Some([0.5, 64.0, 0.5])
    }
    fn player_name(&self) -> Option<String> {
        Some("Dev".into())
    }
    fn in_world(&self) -> bool {
        false
    }
    fn set_block(&mut self, _: &str, _: [i32; 3], _: &str) -> Result<(), String> {
        Ok(())
    }
    fn block(&self, _: &str, _: [i32; 3]) -> Option<String> {
        None
    }
    fn dimension(&self) -> Option<String> {
        None
    }
    fn send_payload(&mut self, _: &str, _: &[u8]) -> Result<(), String> {
        Ok(())
    }
    fn resource(&self, _: &str, _: &str) -> Option<Vec<u8>> {
        None
    }
}

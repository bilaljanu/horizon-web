//! Fabric and NeoForge mod support for Horizon.
//!
//! The crate is in two halves:
//!
//! * inspection — [`jar`], [`metadata`], [`access`], [`version`] and
//!   [`compat`] read an unmodified mod jar and say precisely what it declares
//!   and which parts of it this build can run;
//! * execution — [`classfile`] and [`runtime`] run the mod's own bytecode in a
//!   portable interpreter, with [`native`] supplying the Java, Minecraft,
//!   Fabric and NeoForge classes Horizon implements.
//!
//! Both halves are plain Rust with no JNI and no process spawning, so the same
//! code runs natively and in the browser. The game's side of the boundary is
//! [`host::Host`]: the runtime never links to Horizon's internals.

pub mod access;
mod bridge;
mod registry;
pub mod classfile;
pub mod compat;
pub mod host;
pub mod jar;
pub mod loader;
pub mod metadata;
mod native;
pub mod runtime;
pub mod version;

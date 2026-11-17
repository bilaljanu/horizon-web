//! Experimental entrypoint execution. Success means initialization completed,
//! not that a mod's gameplay or Minecraft API requirements are supported.
use crate::{
    classfile::ClassFile,
    compat::{self, GameInfo, Inspected},
    host::Host,
    jar::Jar,
    metadata::{Environment, Loader},
    runtime::Vm,
};
use std::collections::BTreeMap;

pub struct Loaded<H: Host> {
    pub report: Inspected,
    pub vm: Vm<H>,
}

/// Initialize one jar on the client. Dependencies must already be available;
/// cross-jar class loading and injected NeoForge constructors are not supported.
pub fn initialize<H: Host>(bytes: Vec<u8>, host: H) -> Result<Loaded<H>, String> {
    let mut jar = Jar::open(bytes.clone())?;
    let report = compat::inspect(&mut jar, &GameInfo::default(), &BTreeMap::new())?;
    if report.has_errors() {
        return Err(report
            .diagnostics
            .iter()
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
            .join("; "));
    }
    let m = &report.metadata;
    if m.environment == Environment::Server {
        return Err(format!(
            "{}: dedicated-server entrypoints are not implemented",
            m.id
        ));
    }
    if !m.mixins.is_empty()
        || m.access_widener.is_some()
        || m.access_transformer.is_some()
        || !m.nested_jars.is_empty()
    {
        return Err(format!(
            "{}: initialization requires unsupported transformations or nested jars",
            m.id
        ));
    }
    let mut vm = Vm::new(host, &m.id);
    vm.initializing = true;
    vm.add_jar(bytes)?;
    let result = (|| -> Result<(), String> {
        match m.loader {
            Loader::Fabric => {
                for entry in &m.entrypoints {
                    if entry.adapter != "default" || entry.class.contains("::") {
                        return Err(format!(
                            "unsupported entrypoint adapter/reference: {} {}",
                            entry.adapter, entry.class
                        ));
                    }
                    let method = match entry.kind.as_str() {
                        "main" => "onInitialize",
                        "client" => "onInitializeClient",
                        other => return Err(format!("unsupported entrypoint kind {other}")),
                    };
                    let object = vm
                        .new_instance(&entry.class.replace('.', "/"), "()V", vec![])
                        .map_err(|e| e.message)?;
                    vm.call_value(object, method, "()V", vec![])
                        .map_err(|e| e.message)?;
                }
            }
            Loader::NeoForge => {
                if report.all.len() != 1 {
                    return Err("multi-mod NeoForge jars are not implemented".into());
                }
                let mut found = false;
                for path in jar.files().to_vec() {
                    if !path.ends_with(".class") || path.starts_with("META-INF/") {
                        continue;
                    }
                    let class = ClassFile::parse(&jar.read(&path)?)?;
                    if class.annotations.iter().any(|a| {
                        a.type_descriptor == "Lnet/neoforged/fml/common/Mod;"
                            && a.string("value") == Some(m.id.as_str())
                    }) {
                        found = true;
                        vm.new_instance(&class.name, "()V", vec![])
                            .map_err(|e| e.message)?;
                    }
                }
                if !found {
                    return Err("no matching NeoForge @Mod class found".into());
                }
            }
            other => return Err(format!("{} execution is not implemented", other.name())),
        }
        Ok(())
    })();
    result.map_err(|e| format!("{} initialization: {e}", m.id))?;
    vm.initializing = false;
    Ok(Loaded { report, vm })
}

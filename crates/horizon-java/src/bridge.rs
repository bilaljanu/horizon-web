//! Explicitly implemented Minecraft/loader ABI. Unknown calls fall through to
//! the interpreter's diagnostic; they must never become successful no-ops.
use crate::{
    host::Host,
    runtime::{Callback, Event, Fault, Kind, Object, Value, Vm},
};

pub(crate) fn builtin_names() -> Vec<&'static str> {
    let mut names = vec![
        "net/fabricmc/api/ModInitializer",
        "net/fabricmc/api/ClientModInitializer",
        "net/minecraft/network/chat/Component",
        "net/minecraft/network/chat/MutableComponent",
        "net/minecraft/server/MinecraftServer",
        "net/fabricmc/fabric/api/event/Event",
        "net/fabricmc/fabric/api/event/lifecycle/v1/ServerTickEvents",
        "net/fabricmc/fabric/api/event/lifecycle/v1/ServerTickEvents$EndTick",
    ];
    names.extend_from_slice(crate::registry::CLASSES);
    names
}
pub(crate) fn builtin_super(name: &str) -> Option<Option<&'static str>> {
    if name == "net/minecraft/world/item/BlockItem" { return Some(Some("net/minecraft/world/item/Item")); }
    builtin_names()
        .contains(&name)
        .then_some(Some("java/lang/Object"))
}
pub(crate) fn builtin_fields(_: &str) -> &'static [(&'static str, &'static str)] {
    &[]
}
pub(crate) fn builtin_interfaces(name: &str) -> Vec<String> {
    if name == "net/minecraft/network/chat/MutableComponent" {
        vec!["net/minecraft/network/chat/Component".into()]
    } else {
        Vec::new()
    }
}
pub(crate) fn has_native(class: &str, name: &str, descriptor: &str) -> bool {
    if crate::registry::supports(class, name, descriptor) { return true; }
    matches!(
        (class, name, descriptor),
        (
            "net/fabricmc/fabric/api/event/Event",
            "register",
            "(Ljava/lang/Object;)V"
        ) | (
            "net/minecraft/network/chat/Component",
            "literal",
            "(Ljava/lang/String;)Lnet/minecraft/network/chat/MutableComponent;"
        ) | (
            "net/minecraft/network/chat/Component" | "net/minecraft/network/chat/MutableComponent",
            "getString",
            "()Ljava/lang/String;"
        )
    )
}
pub(crate) fn static_field<H: Host>(
    vm: &mut Vm<H>,
    class: &str,
    name: &str,
    descriptor: &str,
) -> Option<Value> {
    if let Some(value) = crate::registry::static_field(vm, class, name, descriptor) { return Some(value); }
    if (class, name, descriptor)
        == (
            "net/fabricmc/fabric/api/event/lifecycle/v1/ServerTickEvents",
            "END_SERVER_TICK",
            "Lnet/fabricmc/fabric/api/event/Event;",
        )
    {
        return vm
            .alloc(Object {
                class: "net/fabricmc/fabric/api/event/Event".into(),
                fields: Vec::new(),
                kind: Kind::EventHandle("end_server_tick".into()),
            })
            .ok()
            .map(Value::obj);
    }
    None
}
pub(crate) fn init_statics<H: Host>(_: &mut Vm<H>, _: &str) -> Result<(), Fault> {
    Ok(())
}
pub(crate) fn call<H: Host>(
    vm: &mut Vm<H>,
    class: &str,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Option<Value>, Fault> {
    if let Some(value) = crate::registry::call(vm, class, name, descriptor, args)? { return Ok(Some(value)); }
    if !has_native(class, name, descriptor) {
        return Ok(None);
    }
    let value = *args
        .first()
        .ok_or_else(|| Fault::Vm("missing component argument".into()))?;
    if class == "net/fabricmc/fabric/api/event/Event" {
        let reference = vm.null_check(value)?;
        if !matches!(&vm.object(reference)?.kind, Kind::EventHandle(event) if event == "end_server_tick")
        {
            return Err(Fault::Vm("unsupported Fabric event handle".into()));
        }
        let target = *args
            .get(1)
            .ok_or_else(|| Fault::Vm("missing event listener".into()))?;
        vm.null_check(target)?;
        vm.add_callback(Callback {
            event: Event::ServerTick,
            target,
            method: "onEndTick".into(),
            descriptor: "(Lnet/minecraft/server/MinecraftServer;)V".into(),
        });
        return Ok(Some(Value::Void));
    }
    if name == "literal" {
        let text = vm.string_of(value)?;
        return vm
            .alloc(Object {
                class: "net/minecraft/network/chat/MutableComponent".into(),
                fields: Vec::new(),
                kind: Kind::Component(text),
            })
            .map(|r| Some(Value::obj(r)));
    }
    let reference = vm.null_check(value)?;
    let Kind::Component(text) = &vm.object(reference)?.kind else {
        return Err(Fault::Vm("invalid component receiver".into()));
    };
    let text = text.clone();
    vm.make_string(&text).map(Some)
}

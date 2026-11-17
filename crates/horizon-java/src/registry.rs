//! The small registry ABI supported by the gameplay content adapter.
use crate::{host::{BlockSpec, Host, ItemSpec}, runtime::{Fault, Kind, Object, Value, Vm}};

pub(crate) const CLASSES: &[&str] = &[
    "net/minecraft/resources/ResourceLocation", "net/minecraft/core/Registry",
    "net/minecraft/core/registries/BuiltInRegistries", "net/minecraft/world/item/Item",
    "net/minecraft/world/item/BlockItem", "net/minecraft/world/item/Item$Properties",
    "net/minecraft/world/level/block/Block", "net/minecraft/world/level/block/state/BlockBehaviour$Properties",
];

pub(crate) fn supports(class: &str, name: &str, desc: &str) -> bool {
    matches!((class, name, desc),
        ("net/minecraft/resources/ResourceLocation", "fromNamespaceAndPath", "(Ljava/lang/String;Ljava/lang/String;)Lnet/minecraft/resources/ResourceLocation;") |
        ("net/minecraft/core/Registry", "register", "(Lnet/minecraft/core/Registry;Lnet/minecraft/resources/ResourceLocation;Ljava/lang/Object;)Ljava/lang/Object;") |
        ("net/minecraft/world/item/Item$Properties", "<init>", "()V") |
        ("net/minecraft/world/item/Item$Properties", "stacksTo", "(I)Lnet/minecraft/world/item/Item$Properties;") |
        ("net/minecraft/world/level/block/state/BlockBehaviour$Properties", "of", "()Lnet/minecraft/world/level/block/state/BlockBehaviour$Properties;") |
        ("net/minecraft/world/level/block/state/BlockBehaviour$Properties", "strength", "(F)Lnet/minecraft/world/level/block/state/BlockBehaviour$Properties;") |
        ("net/minecraft/world/level/block/Block", "<init>", "(Lnet/minecraft/world/level/block/state/BlockBehaviour$Properties;)V") |
        ("net/minecraft/world/item/Item", "<init>", "(Lnet/minecraft/world/item/Item$Properties;)V") |
        ("net/minecraft/world/item/BlockItem", "<init>", "(Lnet/minecraft/world/level/block/Block;Lnet/minecraft/world/item/Item$Properties;)V"))
}

pub(crate) fn static_field<H: Host>(vm: &mut Vm<H>, class: &str, name: &str, desc: &str) -> Option<Value> {
    if class != "net/minecraft/core/registries/BuiltInRegistries" || desc != "Lnet/minecraft/core/Registry;" { return None; }
    let id = match name { "BLOCK" => "block", "ITEM" => "item", _ => return None };
    vm.alloc(Object { class: "net/minecraft/core/Registry".into(), fields: Vec::new(), kind: Kind::Registry(id.into()) }).ok().map(Value::obj)
}

fn kind<H: Host>(vm: &mut Vm<H>, value: Value) -> Result<Kind, Fault> {
    let reference = vm.null_check(value)?;
    Ok(vm.object(reference)?.kind.clone())
}

pub(crate) fn call<H: Host>(vm: &mut Vm<H>, class: &str, name: &str, desc: &str, args: &[Value]) -> Result<Option<Value>, Fault> {
    if !supports(class, name, desc) { return Ok(None); }
    let arg = |i: usize| args.get(i).copied().ok_or_else(|| Fault::Vm(format!("missing argument {i} for {class}.{name}{desc}")));
    if name == "fromNamespaceAndPath" {
        let namespace = vm.string_of(arg(0)?)?;
        let path = vm.string_of(arg(1)?)?;
        let valid = |s: &str, slash: bool| !s.is_empty() && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b) || (slash && b == b'/'));
        if !valid(&namespace, false) || !valid(&path, true) { return Err(Fault::Vm(format!("invalid resource location {namespace}:{path}"))); }
        return vm.alloc(Object { class: class.into(), fields: Vec::new(), kind: Kind::ResourceLocation { namespace, path } }).map(|r| Some(Value::obj(r)));
    }
    if name == "register" {
        if !vm.initializing { return Err(Fault::Vm("registries are frozen after mod initialization".into())); }
        let Kind::Registry(registry) = kind(vm, arg(0)?)? else { return Err(Fault::Vm("expected registry".into())); };
        let Kind::ResourceLocation { namespace, path } = kind(vm, arg(1)?)? else { return Err(Fault::Vm("expected resource location".into())); };
        if namespace != vm.mod_id { return Err(Fault::Vm(format!("mod {} cannot register namespace {namespace}", vm.mod_id))); }
        let id = format!("{namespace}:{path}");
        let key = (registry.clone(), id.clone());
        if vm.registry_ids.contains_key(&key) { return Err(Fault::Vm(format!("duplicate {registry} registration {id}"))); }
        let value = arg(2)?;
        let reference = vm.null_check(value)?;
        let mod_id = vm.mod_id.clone();
        match (registry.as_str(), kind(vm, value)?) {
            ("block", Kind::Block { template, hardness, resistance, light, requires_tool, random_ticks, no_occlusion }) => {
                vm.host.register_block(&mod_id, BlockSpec { id: id.clone(), template, hardness, resistance, light, requires_tool, random_ticks, no_occlusion })?;
            }
            ("item", Kind::Item { template, max_stack, rarity, block }) => {
                vm.host.register_item(&mod_id, ItemSpec { id: id.clone(), template, max_stack_size: max_stack, rarity, block, creative_tab: None })?;
            }
            _ => return Err(Fault::Vm(format!("unsupported value for registry {registry}"))),
        }
        vm.registry_ids.insert(key, reference);
        vm.registered.insert(reference, id);
        return Ok(Some(value));
    }
    if name == "of" {
        return vm.alloc(Object { class: class.into(), fields: Vec::new(), kind: Kind::BlockProperties { template: "minecraft:stone".into(), hardness: None, resistance: None, light: None, requires_tool: None, random_ticks: None, no_occlusion: None } }).map(|r| Some(Value::obj(r)));
    }
    let receiver = vm.null_check(arg(0)?)?;
    if name == "stacksTo" {
        let n = arg(1)?.as_int().ok_or_else(|| Fault::Vm("stacksTo requires int".into()))?;
        if !(1..=99).contains(&n) { return Err(Fault::Vm("stack size must be 1..99".into())); }
        let Kind::ItemProperties { max_stack, .. } = &mut vm.object_mut(receiver)?.kind else { return Err(Fault::Vm("invalid item properties".into())); };
        *max_stack = Some(n as u8);
        return Ok(Some(arg(0)?));
    }
    if name == "strength" {
        let n = arg(1)?.as_float().ok_or_else(|| Fault::Vm("strength requires float".into()))?;
        if !n.is_finite() || n < 0.0 { return Err(Fault::Vm("strength must be finite and nonnegative".into())); }
        let Kind::BlockProperties { hardness, resistance, .. } = &mut vm.object_mut(receiver)?.kind else { return Err(Fault::Vm("invalid block properties".into())); };
        *hardness = Some(n); *resistance = Some(n);
        return Ok(Some(arg(0)?));
    }
    let new_kind = match class {
        "net/minecraft/world/item/Item$Properties" => Kind::ItemProperties { max_stack: None, rarity: None },
        "net/minecraft/world/level/block/Block" => {
            let Kind::BlockProperties { template, hardness, resistance, light, requires_tool, random_ticks, no_occlusion } = kind(vm, arg(1)?)? else { return Err(Fault::Vm("invalid block properties".into())); };
            Kind::Block { template, hardness, resistance, light, requires_tool, random_ticks, no_occlusion }
        }
        _ => {
            let block_item = class == "net/minecraft/world/item/BlockItem";
            let Kind::ItemProperties { max_stack, rarity } = kind(vm, arg(if block_item { 2 } else { 1 })?)? else { return Err(Fault::Vm("invalid item properties".into())); };
            let block = if block_item {
                let r = vm.null_check(arg(1)?)?;
                Some(vm.registered.get(&r).cloned().ok_or_else(|| Fault::Vm("BlockItem requires a registered block".into()))?)
            } else { None };
            Kind::Item { template: if block_item { "minecraft:stone" } else { "minecraft:stick" }.into(), max_stack, rarity, block }
        }
    };
    vm.object_mut(receiver)?.kind = new_kind;
    Ok(Some(Value::Void))
}

//! A portable JVM for Fabric and NeoForge mods.
//!
//! This is an interpreter for the class-file instruction set, not a wrapper
//! around a system JVM: it is plain Rust with no JNI and no child process, so
//! the same runtime executes a mod's own bytecode natively and inside a
//! WebAssembly worker. Java library classes, Minecraft and the loader APIs are
//! supplied by [`crate::native`] and [`crate::bridge`]; a mod's classes come
//! out of its jar unchanged.
//!
//! The runtime is deliberately bounded. A mod cannot run forever, allocate
//! without limit or recurse without limit; the budgets are visible in
//! [`Limits`]. A construct the interpreter does not implement is a precise
//! error naming the class, method and bytecode offset, never a silent skip.

use crate::classfile::{self, ACC_STATIC, ClassFile, Code, Constant, MemberRef, Method};
use crate::host::Host;
use std::collections::BTreeMap;
use std::fmt;

/// The largest number of bytecode instructions one entry call may execute.
pub const DEFAULT_STEPS: u64 = 8_000_000;
/// The largest Java call depth.
pub const DEFAULT_DEPTH: usize = 512;
/// The most live heap objects.
pub const DEFAULT_HEAP: usize = 1_000_000;

/// Limits that keep a broken or hostile mod from hanging the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub steps: u64,
    pub depth: usize,
    pub objects: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            steps: DEFAULT_STEPS,
            depth: DEFAULT_DEPTH,
            objects: DEFAULT_HEAP,
        }
    }
}

/// A JVM value. Category-2 values (`long`, `double`) are one value with two
/// local slots, as the class-file format describes them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Value {
    #[default]
    Void,
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    /// A heap reference; `None` is Java's `null`.
    Ref(Option<u32>),
}

impl Value {
    pub fn null() -> Self {
        Value::Ref(None)
    }
    pub fn obj(reference: u32) -> Self {
        Value::Ref(Some(reference))
    }
    /// The two's-complement `int` a boolean uses in bytecode.
    pub fn boolean(value: bool) -> Self {
        Value::Int(i32::from(value))
    }
    pub fn as_int(self) -> Option<i32> {
        match self {
            Value::Int(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_long(self) -> Option<i64> {
        match self {
            Value::Long(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_double(self) -> Option<f64> {
        match self {
            Value::Double(v) => Some(v),
            Value::Float(v) => Some(f64::from(v)),
            _ => None,
        }
    }
    pub fn as_float(self) -> Option<f32> {
        match self {
            Value::Float(v) => Some(v),
            Value::Double(v) => Some(v as f32),
            _ => None,
        }
    }
    pub fn as_ref(self) -> Option<Option<u32>> {
        match self {
            Value::Ref(r) => Some(r),
            _ => None,
        }
    }
    pub fn is_null(self) -> bool {
        matches!(self, Value::Ref(None))
    }
    /// Local-variable slots this value occupies.
    fn slots(self) -> usize {
        match self {
            Value::Long(_) | Value::Double(_) => 2,
            _ => 1,
        }
    }
    fn tag(self) -> &'static str {
        match self {
            Value::Void => "void",
            Value::Int(_) => "int",
            Value::Long(_) => "long",
            Value::Float(_) => "float",
            Value::Double(_) => "double",
            Value::Ref(_) => "reference",
        }
    }
}

/// A Java object. The class name is always the runtime class, so virtual
/// dispatch and `instanceof` never depend on the static type.
#[derive(Clone, Debug)]
pub struct Object {
    pub class: String,
    pub fields: Vec<Value>,
    pub kind: Kind,
}

/// The state a class of ours keeps outside its fields.
#[derive(Clone, Debug)]
pub enum Kind {
    Plain,
    Array {
        /// The array's component descriptor (`I`, `Ljava/lang/String;`, `[I`).
        element: String,
        values: Vec<Value>,
    },
    String(String),
    Class(String),
    StringBuilder(String),
    Throwable {
        message: Option<String>,
    },
    Lambda(Lambda),
    Boxed(Value),
    List(Vec<Value>),
    Map(Vec<(Value, Value)>),
    Set(Vec<Value>),
    Iterator {
        values: Vec<Value>,
        index: usize,
    },
    Optional(Option<Value>),
    Random(u64),
    Uuid(u128),
    Registry(String),
    RegistryKey(String),
    Item {
        template: String,
        max_stack: Option<u8>,
        rarity: Option<String>,
        block: Option<String>,
    },
    Block {
        template: String,
        hardness: Option<f32>,
        resistance: Option<f32>,
        light: Option<u8>,
        requires_tool: Option<bool>,
        random_ticks: Option<bool>,
        no_occlusion: Option<bool>,
    },
    ItemProperties {
        max_stack: Option<u8>,
        rarity: Option<String>,
    },
    BlockProperties {
        template: String,
        hardness: Option<f32>,
        resistance: Option<f32>,
        light: Option<u8>,
        requires_tool: Option<bool>,
        random_ticks: Option<bool>,
        no_occlusion: Option<bool>,
    },
    Component(String),
    ItemStack {
        item: Option<u32>,
        count: i32,
    },
    EventBus,
    EventHandle(String),
    Entries {
        tab: String,
        items: Vec<String>,
    },
    ResourceLocation {
        namespace: String,
        path: String,
    },
    Logger(String),
    PrintStream,
    ActionResult(String),
    Hand(String),
    HitResult {
        dimension: String,
        pos: [i32; 3],
        block: Option<String>,
    },
    Server,
    ClientLevel,
    Level {
        dimension: String,
    },
    BlockState {
        block: Option<u32>,
        id: String,
    },
    Player {
        name: String,
    },
    ItemStackHolder,
    NumberFormat,
    Pattern(String),
    Matcher {
        text: String,
        pattern: String,
    },
}

/// A captured lambda or method reference produced by `invokedynamic`.
#[derive(Clone, Debug)]
pub struct Lambda {
    /// The runtime class that implements the functional interface (may be a
    /// built-in such as `java/lang/Runnable`).
    pub interface: String,
    pub method: String,
    pub descriptor: String,
    pub implementation: MemberRef,
    /// JVM method-handle kind (5 invokevirtual, 6 invokestatic, 7
    /// invokespecial, 8 newInvokeSpecial, 9 invokeinterface).
    pub handle_kind: u8,
    pub captured: Vec<Value>,
}

/// An event a mod subscribed to during initialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    ServerTick,
    ClientTick,
    UseBlock,
    Register,
    CommonSetup,
}

#[derive(Clone, Debug)]
pub struct Callback {
    pub event: Event,
    /// The lambda object or object implementing the listener interface.
    pub target: Value,
    pub method: String,
    pub descriptor: String,
}

/// A Java exception that has not been caught yet, or a fault in the runtime.
#[derive(Debug)]
pub enum Fault {
    /// A live Java throwable object.
    Java(u32),
    /// An error in the mod's bytecode or an unimplemented construct.
    Vm(String),
}

impl From<String> for Fault {
    fn from(value: String) -> Self {
        Fault::Vm(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VmError {
    pub message: String,
}

impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for VmError {}

struct ClassData {
    file: Option<ClassFile>,
    super_name: Option<String>,
    interfaces: Vec<String>,
    /// Instance fields, superclass first, as `(declaring class, name,
    /// descriptor)`.
    layout: Vec<(String, String, String)>,
    /// Static fields declared by this class.
    statics: BTreeMap<(String, String), Value>,
    initialized: bool,
    initializing: bool,
}

/// A method found by resolving a symbolic reference.
#[derive(Clone)]
struct ResolvedMethod {
    owner: String,
    method: Method,
}

/// The interpreter. One per mod (or one shared by every Java mod); the host
/// receives everything the mod asks the game to do.
pub struct Vm<H: Host> {
    pub(crate) host: H,
    pub(crate) mod_id: String,
    limits: Limits,
    steps: u64,
    depth: usize,
    classes: BTreeMap<String, ClassData>,
    raw: BTreeMap<String, Vec<u8>>,
    heap: Vec<Option<Object>>,
    free: Vec<u32>,
    interned: BTreeMap<String, u32>,
    pub(crate) resources: BTreeMap<String, Vec<u8>>,
    pub(crate) callbacks: Vec<Callback>,
    pub(crate) registered: BTreeMap<u32, String>,
    pub(crate) registry_ids: BTreeMap<(String, String), u32>,
    server: Option<u32>,
    client: Option<u32>,
    /// Set while a mod's initialization runs; diagnostics use it to name the
    /// mod that failed.
    pub(crate) initializing: bool,
}

impl<H: Host> Vm<H> {
    pub fn new(host: H, mod_id: impl Into<String>) -> Self {
        Self {
            host,
            mod_id: mod_id.into(),
            limits: Limits::default(),
            steps: DEFAULT_STEPS,
            depth: 0,
            classes: BTreeMap::new(),
            raw: BTreeMap::new(),
            heap: Vec::new(),
            free: Vec::new(),
            interned: BTreeMap::new(),
            resources: BTreeMap::new(),
            callbacks: Vec::new(),
            registered: BTreeMap::new(),
            registry_ids: BTreeMap::new(),
            server: None,
            client: None,
            initializing: false,
        }
    }

    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self.steps = limits.steps;
        self
    }

    pub fn limits(&self) -> Limits {
        self.limits
    }

    pub fn host(&self) -> &H {
        &self.host
    }

    pub fn host_mut(&mut self) -> &mut H {
        &mut self.host
    }

    pub fn into_host(self) -> H {
        self.host
    }

    /// Add a class from its bytes (a jar entry or a directory). The name is
    /// the binary name (`com/example/Mod`).
    pub fn add_class(&mut self, name: impl Into<String>, bytes: Vec<u8>) {
        self.raw.insert(name.into(), bytes);
    }

    /// Add every class and resource in a jar.
    pub fn add_jar(&mut self, bytes: Vec<u8>) -> Result<Vec<String>, String> {
        let mut jar = crate::jar::Jar::open(bytes)?;
        let files = jar.files().to_vec();
        let mut added = Vec::new();
        for name in files {
            let Some(binary) = name.strip_suffix(".class") else {
                if !name.starts_with("META-INF/") {
                    let body = jar.read(&name)?;
                    self.resources.insert(name.clone(), body);
                }
                continue;
            };
            if binary.starts_with("META-INF/versions/") {
                // Multi-release jars are read only for the base classes: the
                // interpreter targets the newest platform we implement.
                continue;
            }
            let body = jar.read(&name)?;
            self.add_class(binary.to_owned(), body);
            added.push(binary.to_owned());
        }
        Ok(added)
    }

    pub fn add_resource(&mut self, path: impl Into<String>, bytes: Vec<u8>) {
        self.resources.insert(path.into(), bytes);
    }

    /// Classes present in the runtime, sorted: the mod's classes and every
    /// built-in the vm knows.
    pub fn class_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.raw.keys().cloned().collect();
        names.extend(self.classes.keys().cloned());
        names.extend(
            crate::native::builtin_names()
                .iter()
                .map(|s| (*s).to_owned()),
        );
        names.sort();
        names.dedup();
        names
    }

    /// Whether a class can be resolved, without running its initializer.
    pub fn has_class(&mut self, name: &str) -> bool {
        self.ensure_loaded(name).is_ok()
    }

    // ---- heap -----------------------------------------------------------

    pub(crate) fn alloc(&mut self, object: Object) -> Result<u32, Fault> {
        if self.heap.len() - self.free.len() >= self.limits.objects {
            return Err(Fault::Vm(format!(
                "the mod allocated more than {} objects",
                self.limits.objects
            )));
        }
        if let Some(index) = self.free.pop() {
            self.heap[index as usize] = Some(object);
            return Ok(index);
        }
        let index = u32::try_from(self.heap.len())
            .map_err(|_| Fault::Vm("the heap index overflowed".to_owned()))?;
        self.heap.push(Some(object));
        Ok(index)
    }

    pub(crate) fn object(&self, reference: u32) -> Result<&Object, Fault> {
        self.heap
            .get(reference as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| Fault::Vm(format!("dangling reference {reference}")))
    }

    pub(crate) fn object_mut(&mut self, reference: u32) -> Result<&mut Object, Fault> {
        self.heap
            .get_mut(reference as usize)
            .and_then(Option::as_mut)
            .ok_or_else(|| Fault::Vm(format!("dangling reference {reference}")))
    }

    /// The object a value refers to, or a `NullPointerException`, the way a
    /// field access or method call would.
    pub(crate) fn deref(&self, value: Value) -> Result<u32, Fault> {
        match value {
            Value::Ref(Some(reference))
                if self
                    .heap
                    .get(reference as usize)
                    .is_some_and(Option::is_some) =>
            {
                Ok(reference)
            }
            Value::Ref(_) => Err(Fault::Vm("null reference".to_owned())),
            other => Err(Fault::Vm(format!(
                "expected a reference, found {}",
                other.tag()
            ))),
        }
    }

    pub(crate) fn null_check(&mut self, value: Value) -> Result<u32, Fault> {
        match value {
            Value::Ref(Some(reference))
                if self
                    .heap
                    .get(reference as usize)
                    .is_some_and(Option::is_some) =>
            {
                Ok(reference)
            }
            Value::Ref(_) => Err(self.throw("java/lang/NullPointerException", "")),
            other => Err(Fault::Vm(format!(
                "expected a reference, found {}",
                other.tag()
            ))),
        }
    }

    pub(crate) fn class_of(&self, value: Value) -> Result<String, Fault> {
        let Some(Some(reference)) = value.as_ref() else {
            return Err(Fault::Vm("null has no class".to_owned()));
        };
        Ok(self.object(reference)?.class.clone())
    }

    /// Intern a Java string, the way a class-file string constant is interned.
    pub(crate) fn make_string(&mut self, text: &str) -> Result<Value, Fault> {
        if let Some(&reference) = self.interned.get(text) {
            return Ok(Value::obj(reference));
        }
        let reference = self.alloc(Object {
            class: "java/lang/String".into(),
            fields: Vec::new(),
            kind: Kind::String(text.to_owned()),
        })?;
        self.interned.insert(text.to_owned(), reference);
        Ok(Value::obj(reference))
    }

    /// The Java string a value holds. `null` is an error only where Java would
    /// throw; callers that accept null pass [`Value::Ref`] themselves.
    pub(crate) fn string_of(&self, value: Value) -> Result<String, Fault> {
        let reference = self.deref(value)?;
        match &self.object(reference)?.kind {
            Kind::String(text) => Ok(text.clone()),
            other => Err(Fault::Vm(format!(
                "expected java/lang/String, found {} ({other:?})",
                self.object(reference)?.class
            ))),
        }
    }

    pub(crate) fn string_or_null(&self, value: Value) -> Result<Option<String>, Fault> {
        if value.is_null() {
            return Ok(None);
        }
        self.string_of(value).map(Some)
    }

    /// A Java `toString` for values the host shows (`Component.literal`, log
    /// messages, string concatenation).
    pub(crate) fn java_to_string(&mut self, value: Value) -> Result<String, Fault> {
        match value {
            Value::Void => Ok("void".into()),
            Value::Int(v) => Ok(v.to_string()),
            Value::Long(v) => Ok(v.to_string()),
            Value::Float(v) => Ok(java_float(v)),
            Value::Double(v) => Ok(java_double(v)),
            Value::Ref(None) => Ok("null".into()),
            Value::Ref(Some(reference)) => {
                let object = self.object(reference)?;
                let class = object.class.clone();
                match &object.kind {
                    Kind::String(text) => Ok(text.clone()),
                    Kind::StringBuilder(text) => Ok(text.clone()),
                    Kind::Boxed(value) => self.java_to_string(*value),
                    Kind::Class(name) => Ok(format!("class {}", name.replace('/', "."))),
                    _ => {
                        if class == "java/lang/Object" {
                            return Ok(format!("java.lang.Object@{}", format!("{:x}", reference)));
                        }
                        if self.method_exists(&class, "toString", "()Ljava/lang/String;") {
                            let result = self.invoke_method(
                                &class,
                                "toString",
                                "()Ljava/lang/String;",
                                vec![Value::obj(reference)],
                            )?;
                            if let Ok(text) = self.string_of(result) {
                                return Ok(text);
                            }
                        }
                        Ok(format!(
                            "{}.{}@{}",
                            class.rsplit('/').next().unwrap_or(&class),
                            class.replace('/', "."),
                            format!("{:x}", reference)
                        ))
                    }
                }
            }
        }
    }

    /// Allocate a throwable and return it as [`Fault::Java`].
    pub(crate) fn throw(&mut self, class: &str, message: &str) -> Fault {
        match self.alloc(Object {
            class: class.to_owned(),
            fields: Vec::new(),
            kind: Kind::Throwable {
                message: Some(message.to_owned()),
            },
        }) {
            Ok(reference) => Fault::Java(reference),
            Err(fault) => fault,
        }
    }

    /// Run a constructor for a new object.
    pub(crate) fn instantiate(
        &mut self,
        class: &str,
        descriptor: &str,
        args: Vec<Value>,
    ) -> Result<Value, Fault> {
        let reference = self.new_object(class)?;
        let mut all = Vec::with_capacity(args.len() + 1);
        all.push(Value::obj(reference));
        all.extend(args);
        self.invoke_method(class, "<init>", descriptor, all)?;
        Ok(Value::obj(reference))
    }

    /// Allocate an object without running a constructor (the bytecode's `new`
    /// leaves it uninitialized until `<init>`).
    pub(crate) fn new_object(&mut self, class: &str) -> Result<u32, Fault> {
        self.ensure_loaded(class)?;
        let layout = self.resolve_layout(class)?;
        let mut fields = Vec::with_capacity(layout.len());
        for (_, _, descriptor) in &layout {
            fields.push(default_value(descriptor));
        }
        self.alloc(Object {
            class: class.to_owned(),
            fields,
            kind: Kind::Plain,
        })
    }

    // ---- classes --------------------------------------------------------

    fn ensure_loaded(&mut self, name: &str) -> Result<(), Fault> {
        if self.classes.contains_key(name) {
            return Ok(());
        }
        if let Some(bytes) = self.raw.remove(name) {
            let file = ClassFile::parse(&bytes).map_err(|e| Fault::Vm(format!("{name}: {e}")))?;
            let super_name = file.super_name.clone();
            let interfaces = file.interfaces.clone();
            if let Some(super_name) = &super_name
                && super_name != name
            {
                self.ensure_loaded(super_name)?;
            }
            for interface in &interfaces {
                self.ensure_loaded(interface)?;
            }
            let mut layout = super_name
                .as_deref()
                .and_then(|s| self.classes.get(s))
                .map(|c| c.layout.clone())
                .unwrap_or_default();
            for field in &file.fields {
                if field.access & ACC_STATIC == 0 {
                    layout.push((
                        name.to_owned(),
                        field.name.clone(),
                        field.descriptor.clone(),
                    ));
                }
            }
            let mut statics = BTreeMap::new();
            for field in &file.fields {
                if field.access & ACC_STATIC != 0 {
                    let value = field
                        .constant
                        .clone()
                        .map(const_value)
                        .unwrap_or_else(|| default_value(&field.descriptor));
                    statics.insert((field.name.clone(), field.descriptor.clone()), value);
                }
            }
            self.classes.insert(
                name.to_owned(),
                ClassData {
                    file: Some(file),
                    super_name,
                    interfaces,
                    layout,
                    statics,
                    initialized: false,
                    initializing: false,
                },
            );
            return Ok(());
        }
        let Some(super_name) = crate::native::builtin_super(name) else {
            return Err(Fault::Vm(format!(
                "class {name} is not in the mod or the Java subset"
            )));
        };
        if let Some(super_name) = super_name {
            self.ensure_loaded(super_name)?;
        }
        let mut layout = super_name
            .and_then(|s| self.classes.get(s))
            .map(|c| c.layout.clone())
            .unwrap_or_default();
        for (field_name, descriptor) in crate::native::builtin_fields(name) {
            layout.push((
                name.to_owned(),
                (*field_name).to_owned(),
                (*descriptor).to_owned(),
            ));
        }
        self.classes.insert(
            name.to_owned(),
            ClassData {
                file: None,
                super_name: super_name.map(str::to_owned),
                interfaces: crate::native::builtin_interfaces(name),
                layout,
                statics: BTreeMap::new(),
                initialized: false,
                initializing: false,
            },
        );
        Ok(())
    }

    fn resolve_layout(&mut self, class: &str) -> Result<Vec<(String, String, String)>, Fault> {
        self.ensure_loaded(class)?;
        Ok(self
            .classes
            .get(class)
            .map(|c| c.layout.clone())
            .unwrap_or_default())
    }

    fn super_of(&mut self, class: &str) -> Option<String> {
        self.ensure_loaded(class).ok()?;
        self.classes.get(class)?.super_name.clone()
    }

    pub(crate) fn super_of_public(&mut self, class: &str) -> Option<String> {
        self.super_of(class)
    }

    pub(crate) fn classes_get_file(&self, class: &str) -> Option<&ClassFile> {
        self.classes.get(class)?.file.as_ref()
    }

    fn interfaces_of(&mut self, class: &str) -> Vec<String> {
        if self.ensure_loaded(class).is_err() {
            return Vec::new();
        }
        self.classes
            .get(class)
            .map(|c| c.interfaces.clone())
            .unwrap_or_default()
    }

    /// The class and its superclasses, most specific first.
    fn superclasses(&mut self, class: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut current = Some(class.to_owned());
        let mut guard = 0;
        while let Some(name) = current {
            guard += 1;
            if guard > 128 {
                break;
            }
            out.push(name.clone());
            current = self.super_of(&name);
        }
        out
    }

    /// `name` is `target` or a subclass/implementation of it.
    pub(crate) fn is_assignable(&mut self, target: &str, name: &str) -> bool {
        if target == name || target == "java/lang/Object" {
            return true;
        }
        if name.starts_with('[') || target.starts_with('[') {
            return self.array_assignable(target, name);
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut stack = vec![name.to_owned()];
        while let Some(current) = stack.pop() {
            if !seen.insert(current.clone()) {
                continue;
            }
            if current == target {
                return true;
            }
            if let Some(super_name) = self.super_of(&current) {
                stack.push(super_name);
            }
            stack.extend(self.interfaces_of(&current));
        }
        false
    }

    fn array_assignable(&mut self, target: &str, name: &str) -> bool {
        if target == name || target == "java/lang/Object" {
            return true;
        }
        if target == "java/lang/Cloneable" || target == "java/io/Serializable" {
            return name.starts_with('[');
        }
        let (Some(target_element), Some(name_element)) =
            (array_element(target), array_element(name))
        else {
            return false;
        };
        if target_element == name_element {
            return true;
        }
        if target_element.starts_with('L') && name_element.starts_with('L') {
            return self.is_assignable(
                class_of_descriptor(target_element),
                class_of_descriptor(name_element),
            );
        }
        false
    }

    /// Resolve a method the way `invokevirtual` does, walking the class and
    /// then its interfaces.
    fn find_method(&mut self, class: &str, name: &str, descriptor: &str) -> Option<ResolvedMethod> {
        let mut seen = std::collections::BTreeSet::new();
        let mut queue = vec![class.to_owned()];
        while !queue.is_empty() {
            let current = queue.remove(0);
            if !seen.insert(current.clone()) {
                continue;
            }
            if self.ensure_loaded(&current).is_err() {
                continue;
            }
            if let Some(data) = self.classes.get(&current)
                && let Some(file) = &data.file
                && let Some(method) = file.method(name, descriptor)
            {
                return Some(ResolvedMethod {
                    owner: current,
                    method: method.clone(),
                });
            }
            if let Some(super_name) = self.super_of(&current) {
                queue.push(super_name);
            }
            queue.extend(self.interfaces_of(&current));
        }
        None
    }

    fn method_exists(&mut self, class: &str, name: &str, descriptor: &str) -> bool {
        self.find_method(class, name, descriptor).is_some()
    }

    /// Resolve a field to its declaring class and whether it is static.
    fn find_field(&mut self, class: &str, name: &str, descriptor: &str) -> Option<(String, bool)> {
        for current in self.superclasses(class) {
            if let Some(data) = self.classes.get(&current) {
                if data
                    .layout
                    .iter()
                    .any(|(_, n, d)| n == name && d == descriptor)
                {
                    return Some((current, false));
                }
                if let Some(file) = &data.file
                    && file
                        .fields
                        .iter()
                        .any(|f| f.name == name && f.descriptor == descriptor)
                {
                    return Some((
                        current,
                        file.fields.iter().any(|f| {
                            f.name == name
                                && f.descriptor == descriptor
                                && f.access & ACC_STATIC != 0
                        }),
                    ));
                }
                if data
                    .statics
                    .contains_key(&(name.to_owned(), descriptor.to_owned()))
                {
                    return Some((current, true));
                }
            }
        }
        // Built-in fields (for example `System.out`) are resolved by natives,
        // but report them here so diagnostic messages name a declaration.
        None
    }

    fn field_index(
        &self,
        class: &str,
        declaring: &str,
        name: &str,
        descriptor: &str,
    ) -> Option<usize> {
        self.classes
            .get(class)?
            .layout
            .iter()
            .position(|(c, n, d)| c == declaring && n == name && d == descriptor)
    }

    pub(crate) fn get_instance_field(
        &self,
        reference: u32,
        name: &str,
        descriptor: &str,
    ) -> Result<Value, Fault> {
        let object = self.object(reference)?;
        let index = self
            .classes
            .get(&object.class)
            .and_then(|data| {
                data.layout
                    .iter()
                    .position(|(_, n, d)| n == name && d == descriptor)
            })
            .ok_or_else(|| {
                Fault::Vm(format!("{} has no field {name}:{descriptor}", object.class))
            })?;
        Ok(object.fields.get(index).copied().unwrap_or_default())
    }

    pub(crate) fn set_instance_field(
        &mut self,
        reference: u32,
        name: &str,
        descriptor: &str,
        value: Value,
    ) -> Result<(), Fault> {
        let class = self.object(reference)?.class.clone();
        let index = self
            .classes
            .get(&class)
            .and_then(|data| {
                data.layout
                    .iter()
                    .position(|(_, n, d)| n == name && d == descriptor)
            })
            .ok_or_else(|| Fault::Vm(format!("{class} has no field {name}:{descriptor}")))?;
        if let Some(slot) = self.object_mut(reference)?.fields.get_mut(index) {
            *slot = value;
        }
        Ok(())
    }

    pub(crate) fn get_static_raw(
        &mut self,
        class: &str,
        name: &str,
        descriptor: &str,
    ) -> Option<Value> {
        self.ensure_loaded(class).ok()?;
        if let Some((declaring, true)) = self.find_field(class, name, descriptor) {
            return self
                .classes
                .get(&declaring)?
                .statics
                .get(&(name.to_owned(), descriptor.to_owned()))
                .copied();
        }
        // Some built-ins (Registries.ITEM, InteractionResult.SUCCESS) are
        // answered by the bridge on demand.
        let value = crate::native::static_value(self, class, name, descriptor)?;
        self.classes
            .get_mut(class)?
            .statics
            .insert((name.to_owned(), descriptor.to_owned()), value);
        Some(value)
    }

    pub(crate) fn set_static_raw(
        &mut self,
        class: &str,
        name: &str,
        descriptor: &str,
        value: Value,
    ) -> Result<(), Fault> {
        self.ensure_loaded(class)?;
        let (declaring, is_static) = self
            .find_field(class, name, descriptor)
            .ok_or_else(|| Fault::Vm(format!("{class} has no static field {name}:{descriptor}")))?;
        if !is_static {
            return Err(Fault::Vm(format!("{declaring}.{name} is not static")));
        }
        if let Some(data) = self.classes.get_mut(&declaring) {
            data.statics
                .insert((name.to_owned(), descriptor.to_owned()), value);
        }
        Ok(())
    }

    /// Run a class initializer if it has not run yet. The JVM's re-entrancy
    /// guard is preserved: a class being initialized does not initialize
    /// itself again.
    pub(crate) fn ensure_initialized(&mut self, class: &str) -> Result<(), Fault> {
        self.ensure_loaded(class)?;
        let (initialized, initializing) = {
            let data = self
                .classes
                .get(class)
                .ok_or_else(|| Fault::Vm(format!("no class {class}")))?;
            (data.initialized, data.initializing)
        };
        if initialized || initializing {
            return Ok(());
        }
        if let Some(super_name) = self.super_of(class) {
            self.ensure_initialized(&super_name)?;
        }
        crate::bridge::init_statics(self, class)?;
        let method = self
            .classes
            .get(class)
            .and_then(|d| d.file.as_ref())
            .and_then(|f| f.method("<clinit>", "()V"))
            .cloned();
        if let Some(data) = self.classes.get_mut(class) {
            data.initializing = true;
        }
        let result = if let Some(method) = method {
            self.run_method(class, &method, Vec::new())
        } else {
            Ok(Value::Void)
        };
        match result {
            Ok(_) => {
                if let Some(data) = self.classes.get_mut(class) {
                    data.initializing = false;
                    data.initialized = true;
                }
                Ok(())
            }
            Err(fault) => {
                if let Some(data) = self.classes.get_mut(class) {
                    data.initializing = false;
                }
                Err(fault)
            }
        }
    }

    // ---- calls ----------------------------------------------------------

    fn check_budget(&mut self, owner: &str, name: &str, descriptor: &str) -> Result<(), Fault> {
        if self.steps == 0 {
            return Err(Fault::Vm(format!(
                "instruction budget exhausted in {owner}.{name}{descriptor}"
            )));
        }
        self.steps -= 1;
        Ok(())
    }

    fn push_frame(&mut self) -> Result<(), Fault> {
        if self.depth >= self.limits.depth {
            return Err(Fault::Vm(format!(
                "Java call depth limit {} exceeded",
                self.limits.depth
            )));
        }
        self.depth += 1;
        Ok(())
    }

    fn pop_frame(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    /// Invoke a method by its runtime dispatch class. `args` includes the
    /// receiver for instance methods.
    pub(crate) fn invoke_method(
        &mut self,
        class: &str,
        name: &str,
        descriptor: &str,
        args: Vec<Value>,
    ) -> Result<Value, Fault> {
        if name == "<clinit>" {
            self.ensure_initialized(class)?;
            return Ok(Value::Void);
        }
        if name != "<init>" {
            // Static methods initialize their class first. Instance methods
            // have already had it initialized by `new`.
        }
        let resolved = self.find_method(class, name, descriptor);
        if name == "<init>"
            && self.classes_get_file(class).is_some()
            && resolved.as_ref().is_none_or(|m| m.owner != class)
        {
            return Err(Fault::Vm(format!(
                "missing constructor {class}.{name}{descriptor}"
            )));
        }
        if let Some(resolved) = resolved {
            if resolved.method.code.is_some() {
                return self.run_method(&resolved.owner, &resolved.method, args);
            }
            if let Some(result) =
                crate::native::call(self, &resolved.owner, name, descriptor, &args)?
            {
                return Ok(result);
            }
            return Err(Fault::Vm(format!(
                "{} has no implementation for {}.{name}{descriptor} (native or abstract)",
                resolved.owner, class
            )));
        }
        // A method on a class we synthesize but do not list in a class file:
        // walk the chain so `Object.toString` and `Enum.name` are reachable.
        for current in self.superclasses(class) {
            if let Some(result) = crate::native::call(self, &current, name, descriptor, &args)? {
                return Ok(result);
            }
        }
        Err(Fault::Vm(format!(
            "unimplemented Java method {}.{name}{descriptor}",
            class
        )))
    }

    /// Call a method on a value, for `invokevirtual` and `invokeinterface`.
    pub(crate) fn call_ref(
        &mut self,
        receiver: Value,
        name: &str,
        descriptor: &str,
        args: Vec<Value>,
    ) -> Result<Value, Fault> {
        if receiver.is_null() {
            return Err(self.throw("java/lang/NullPointerException", ""));
        }
        let reference = self.deref(receiver)?;
        let object = self.object(reference)?;
        if let Kind::Lambda(lambda) = &object.kind
            && name == lambda.method
            && descriptor == lambda.descriptor
        {
            return self.call_lambda(lambda.clone(), args);
        }
        let class = object.class.clone();
        let mut all = Vec::with_capacity(args.len() + 1);
        all.push(receiver);
        all.extend(args);
        self.invoke_method(&class, name, descriptor, all)
    }

    fn call_lambda(&mut self, lambda: Lambda, args: Vec<Value>) -> Result<Value, Fault> {
        let mut all = lambda.captured.clone();
        all.extend(args);
        let implementation = lambda.implementation.clone();
        match lambda.handle_kind {
            5 | 9 => {
                let receiver = all.first().copied().unwrap_or(Value::null());
                let rest = all.into_iter().skip(1).collect();
                self.call_ref(
                    receiver,
                    &implementation.name,
                    &implementation.descriptor,
                    rest,
                )
            }
            7 => self.invoke_method(
                &implementation.class,
                &implementation.name,
                &implementation.descriptor,
                all,
            ),
            6 => self.invoke_method(
                &implementation.class,
                &implementation.name,
                &implementation.descriptor,
                all,
            ),
            8 => {
                let object = self.new_object(&implementation.class)?;
                let mut ctor_args = vec![Value::obj(object)];
                ctor_args.extend(all);
                self.invoke_method(
                    &implementation.class,
                    "<init>",
                    &implementation.descriptor,
                    ctor_args,
                )?;
                Ok(Value::obj(object))
            }
            other => Err(Fault::Vm(format!(
                "unsupported method-handle kind {other} in a lambda"
            ))),
        }
    }

    // ---- public entry points -------------------------------------------

    /// A string value, interned like a constant.
    pub fn string(&mut self, text: &str) -> Value {
        self.make_string(text).unwrap_or(Value::null())
    }

    pub fn invoke_static(
        &mut self,
        class: &str,
        name: &str,
        descriptor: &str,
        args: Vec<Value>,
    ) -> Result<Value, VmError> {
        self.steps = self.limits.steps;
        self.depth = 0;
        self.ensure_initialized(class)
            .map_err(|e| self.public_error(e))?;
        self.invoke_method(class, name, descriptor, args)
            .map_err(|e| self.public_error(e))
    }

    /// Instantiate a class and run its constructor.
    pub fn new_instance(
        &mut self,
        class: &str,
        descriptor: &str,
        args: Vec<Value>,
    ) -> Result<Value, VmError> {
        self.steps = self.limits.steps;
        self.depth = 0;
        self.ensure_initialized(class)
            .map_err(|e| self.public_error(e))?;
        self.instantiate(class, descriptor, args)
            .map_err(|e| self.public_error(e))
    }

    /// Call a method on a value (the entrypoint interface, an event listener).
    pub fn call_value(
        &mut self,
        value: Value,
        name: &str,
        descriptor: &str,
        args: Vec<Value>,
    ) -> Result<Value, VmError> {
        self.steps = self.limits.steps;
        self.depth = 0;
        self.call_ref(value, name, descriptor, args)
            .map_err(|e| self.public_error(e))
    }

    pub fn read_string(&self, value: Value) -> Result<String, VmError> {
        self.string_of(value).map_err(|e| self.public_error(e))
    }

    fn public_error(&self, fault: Fault) -> VmError {
        match fault {
            Fault::Vm(message) => VmError { message },
            Fault::Java(reference) => {
                let text = self.describe_throwable(reference);
                VmError {
                    message: format!("uncaught Java exception: {text}"),
                }
            }
        }
    }

    pub(crate) fn describe_throwable(&self, reference: u32) -> String {
        let (class, message) = match self.object(reference) {
            Ok(object) => {
                let class = object.class.clone();
                let message = match &object.kind {
                    Kind::Throwable { message } => message.clone(),
                    _ => match self.get_instance_field(
                        reference,
                        "detailMessage",
                        "Ljava/lang/String;",
                    ) {
                        Ok(Value::Ref(Some(text))) => {
                            self.string_or_null(Value::obj(text)).ok().flatten()
                        }
                        _ => None,
                    },
                };
                (class, message)
            }
            Err(_) => return "a dangling throwable".into(),
        };
        let name = class.replace('/', ".");
        match message {
            Some(message) if !message.is_empty() => format!("{name}: {message}"),
            _ => name,
        }
    }

    // ---- events ---------------------------------------------------------

    pub(crate) fn add_callback(&mut self, callback: Callback) {
        self.callbacks.push(callback);
    }

    /// Fire every callback of an event with freshly built arguments.
    pub(crate) fn fire<F>(&mut self, event: Event, mut make_args: F) -> Result<(), VmError>
    where
        F: FnMut(&mut Self) -> Vec<Value>,
    {
        let callbacks: Vec<Callback> = self
            .callbacks
            .iter()
            .filter(|c| c.event == event)
            .cloned()
            .collect();
        for callback in callbacks {
            let args = make_args(self);
            let value = callback.target;
            self.steps = self.limits.steps;
            self.call_ref(value, &callback.method, &callback.descriptor, args)
                .map_err(|e| self.public_error(e))?;
        }
        Ok(())
    }

    /// Run one server and one client tick.
    pub fn tick(&mut self) -> Result<(), VmError> {
        let server = self.server_object().map_err(|e| self.public_error(e))?;
        self.fire(Event::ServerTick, |_| vec![Value::obj(server)])?;
        self.fire(Event::ClientTick, |vm| {
            let client = vm.client_object();
            vec![client]
        })?;
        Ok(())
    }

    pub(crate) fn server_object(&mut self) -> Result<u32, Fault> {
        if let Some(reference) = self.server {
            return Ok(reference);
        }
        let reference = self.alloc(Object {
            class: "net/minecraft/server/MinecraftServer".into(),
            fields: Vec::new(),
            kind: Kind::Server,
        })?;
        self.server = Some(reference);
        Ok(reference)
    }

    pub(crate) fn client_object(&mut self) -> Value {
        if let Some(reference) = self.client {
            return Value::obj(reference);
        }
        let reference = self.alloc(Object {
            class: "net/minecraft/client/Minecraft".into(),
            fields: Vec::new(),
            kind: Kind::ClientLevel,
        });
        match reference {
            Ok(reference) => {
                self.client = Some(reference);
                Value::obj(reference)
            }
            Err(_) => Value::null(),
        }
    }

    /// Let the mod's `UseBlockCallback` handlers see a block interaction.
    /// Returns true when one of them used the click.
    pub fn use_block(
        &mut self,
        dimension: &str,
        pos: [i32; 3],
        block: Option<&str>,
    ) -> Result<bool, VmError> {
        let hit = self
            .alloc(Object {
                class: "net/minecraft/world/phys/BlockHitResult".into(),
                fields: Vec::new(),
                kind: Kind::HitResult {
                    dimension: dimension.to_owned(),
                    pos,
                    block: block.map(str::to_owned),
                },
            })
            .map_err(|e| self.public_error(e))?;
        let world = self
            .alloc(Object {
                class: "net/minecraft/world/level/Level".into(),
                fields: Vec::new(),
                kind: Kind::Level {
                    dimension: dimension.to_owned(),
                },
            })
            .map_err(|e| self.public_error(e))?;
        let player_name = self.host.player_name().unwrap_or_else(|| "Player".into());
        let player = self
            .alloc(Object {
                class: "net/minecraft/world/entity/player/Player".into(),
                fields: Vec::new(),
                kind: Kind::Player { name: player_name },
            })
            .map_err(|e| self.public_error(e))?;
        let hand = self
            .alloc(Object {
                class: "net/minecraft/world/InteractionHand".into(),
                fields: Vec::new(),
                kind: Kind::Hand("MAIN_HAND".into()),
            })
            .map_err(|e| self.public_error(e))?;
        let mut used = false;
        let callbacks: Vec<Callback> = self
            .callbacks
            .iter()
            .filter(|c| c.event == Event::UseBlock)
            .cloned()
            .collect();
        for callback in callbacks {
            let args = vec![
                Value::obj(player),
                Value::obj(world),
                Value::obj(hand),
                Value::obj(hit),
            ];
            let result = self.call_ref(
                callback.target,
                &callback.method,
                &callback.descriptor,
                args,
            );
            let result = result.map_err(|e| self.public_error(e))?;
            let verdict = match result {
                Value::Ref(Some(reference)) => match &self
                    .object(reference)
                    .map_err(|e| self.public_error(e))?
                    .kind
                {
                    Kind::ActionResult(name) => name.clone(),
                    _ => "PASS".into(),
                },
                Value::Int(value) => match value {
                    // `InteractionResult` ordinals: SUCCESS (0/1) in the
                    // versions that use an enum, PASS (1) in the boolean form.
                    0 | 1 => "SUCCESS".into(),
                    _ => "PASS".into(),
                },
                _ => "PASS".into(),
            };
            if verdict == "SUCCESS" || verdict == "CONSUME" {
                used = true;
            }
        }
        Ok(used)
    }

    /// Fire the registration and setup events a NeoForge mod subscribes to.
    pub fn fire_register(&mut self) -> Result<(), VmError> {
        self.fire(Event::Register, |_| Vec::new())?;
        self.fire(Event::CommonSetup, |_| Vec::new())
    }

    /// Whether a class or method a mod needs is known, for the loader's
    /// preflight report.
    pub fn supports_method(&mut self, class: &str, name: &str, descriptor: &str) -> bool {
        if self.find_method(class, name, descriptor).is_some() {
            return true;
        }
        self.superclasses(class)
            .into_iter()
            .any(|c| crate::native::has_native(&c, name, descriptor))
    }
}

// ---- descriptors -------------------------------------------------------

fn default_value(descriptor: &str) -> Value {
    match descriptor.as_bytes().first().copied() {
        Some(b'J') => Value::Long(0),
        Some(b'F') => Value::Float(0.0),
        Some(b'D') => Value::Double(0.0),
        Some(b'L') | Some(b'[') => Value::null(),
        _ => Value::Int(0),
    }
}

pub(crate) fn const_value(value: classfile::ConstValue) -> Value {
    match value {
        classfile::ConstValue::Int(v) => Value::Int(v),
        classfile::ConstValue::Long(v) => Value::Long(v),
        classfile::ConstValue::Float(v) => Value::Float(v),
        classfile::ConstValue::Double(v) => Value::Double(v),
        classfile::ConstValue::String(_) | classfile::ConstValue::Class(_) => Value::null(),
    }
}

/// The element descriptor of an array descriptor (`[I` → `I`).
fn array_element(descriptor: &str) -> Option<&str> {
    descriptor.strip_prefix('[')
}

/// `Ljava/lang/String;` → `java/lang/String`; other descriptors unchanged.
pub(crate) fn class_of_descriptor(descriptor: &str) -> &str {
    descriptor
        .strip_prefix('L')
        .and_then(|s| s.strip_suffix(';'))
        .unwrap_or(descriptor)
}

/// Split a method descriptor into argument and return descriptors.
pub(crate) fn parse_descriptor(descriptor: &str) -> Result<(Vec<String>, String), Fault> {
    let mut chars = descriptor.chars().peekable();
    if chars.next() != Some('(') {
        return Err(Fault::Vm(format!("bad method descriptor {descriptor:?}")));
    }
    let mut args = Vec::new();
    loop {
        match chars.peek() {
            Some(')') => {
                chars.next();
                break;
            }
            None => return Err(Fault::Vm(format!("bad method descriptor {descriptor:?}"))),
            _ => args.push(take_field_descriptor(&mut chars)?),
        }
    }
    let return_type = if chars.peek() == Some(&'V') {
        chars.next();
        "V".to_owned()
    } else {
        take_field_descriptor(&mut chars)?
    };
    if chars.next().is_some() {
        return Err(Fault::Vm(format!("trailing characters in {descriptor:?}")));
    }
    Ok((args, return_type))
}

fn take_field_descriptor(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Result<String, Fault> {
    let mut out = String::new();
    let mut depth = 0;
    loop {
        let Some(ch) = chars.next() else {
            return Err(Fault::Vm("truncated field descriptor".into()));
        };
        out.push(ch);
        match ch {
            '[' => depth += 1,
            'L' => {
                for ch in chars.by_ref() {
                    out.push(ch);
                    if ch == ';' {
                        break;
                    }
                }
                if !out.ends_with(';') {
                    return Err(Fault::Vm("unterminated class descriptor".into()));
                }
                break;
            }
            _ => break,
        }
    }
    let _ = depth;
    Ok(out)
}

/// The JVM's single-slot count for these descriptors.
pub(crate) fn slot_count(descriptors: &[String]) -> usize {
    descriptors
        .iter()
        .map(|d| match d.as_str() {
            "J" | "D" => 2,
            _ => 1,
        })
        .sum()
}

/// Java's `Float.toString`, close enough for log lines and components.
pub(crate) fn java_float(value: f32) -> String {
    if value.is_nan() {
        "NaN".into()
    } else if value.is_infinite() {
        if value > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        }
    } else if value == value.trunc() && value.abs() < 1e7 {
        format!("{value:.1}")
    } else {
        format!("{value}")
    }
}

/// Java's `Double.toString`, close enough for log lines and components.
pub(crate) fn java_double(value: f64) -> String {
    if value.is_nan() {
        "NaN".into()
    } else if value.is_infinite() {
        if value > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        }
    } else if value == value.trunc() && value.abs() < 1e7 {
        format!("{value:.1}")
    } else {
        format!("{value}")
    }
}
pub(crate) fn value_is_long(value: Value) -> bool {
    matches!(value, Value::Long(_))
}

pub(crate) fn value_is_double(value: Value) -> bool {
    matches!(value, Value::Double(_))
}

// ---- interpreter -------------------------------------------------------

impl<H: Host> Vm<H> {
    fn run_method(
        &mut self,
        owner: &str,
        method: &Method,
        args: Vec<Value>,
    ) -> Result<Value, Fault> {
        let Some(code) = &method.code else {
            return Err(Fault::Vm(format!(
                "{owner}.{}{} has no Code attribute",
                method.name, method.descriptor
            )));
        };
        let (argument_types, return_type) = parse_descriptor(&method.descriptor)?;
        let mut locals = vec![Value::Int(0); code.max_locals as usize];
        let mut slot = 0usize;
        if method.access & ACC_STATIC == 0 {
            locals[0] = args.first().copied().unwrap_or(Value::null());
            slot = 1;
        }
        let mut given = args
            .iter()
            .skip(usize::from(method.access & ACC_STATIC == 0));
        for descriptor in &argument_types {
            let Some(value) = given.next().copied() else {
                return Err(Fault::Vm(format!(
                    "{owner}.{}{}: too few arguments",
                    method.name, method.descriptor
                )));
            };
            if slot >= locals.len() {
                return Err(Fault::Vm(format!(
                    "{owner}.{}{}: local slot overflow",
                    method.name, method.descriptor
                )));
            }
            locals[slot] = value;
            slot += if slot_count(std::slice::from_ref(descriptor)) == 2 {
                2
            } else {
                1
            };
        }
        self.push_frame()?;
        let result = self.run_code(owner, method, code.clone(), locals, return_type);
        self.pop_frame();
        result
    }

    fn run_code(
        &mut self,
        owner: &str,
        method: &Method,
        code: Code,
        mut locals: Vec<Value>,
        return_type: String,
    ) -> Result<Value, Fault> {
        let mut stack: Vec<Value> = Vec::with_capacity(code.max_stack as usize);
        let mut pc = 0usize;
        loop {
            self.check_budget(owner, &method.name, &method.descriptor)?;
            let instruction = pc;
            let fault = self.step(
                owner,
                method,
                &code,
                &mut pc,
                &mut locals,
                &mut stack,
                &return_type,
            );
            match fault {
                Ok(Step::Value(value)) => return Ok(value),
                Ok(Step::Continue) => {}
                Err(Fault::Vm(message)) => {
                    return Err(Fault::Vm(format!(
                        "{owner}.{}{} at byte {instruction}: {message}",
                        method.name, method.descriptor
                    )));
                }
                Err(Fault::Java(exception)) => {
                    if let Some(handler) = self.find_handler(&code, instruction, exception) {
                        stack.clear();
                        stack.push(Value::obj(exception));
                        pc = handler;
                        continue;
                    }
                    return Err(Fault::Java(exception));
                }
            }
        }
    }

    fn step(
        &mut self,
        owner: &str,
        method: &Method,
        code: &Code,
        pc: &mut usize,
        locals: &mut [Value],
        stack: &mut Vec<Value>,
        return_type: &str,
    ) -> Result<Step, Fault> {
        let bytes = &code.code;
        let instruction = *pc;
        let opcode = read_u8(bytes, pc)?;
        macro_rules! push {
            ($v:expr) => {
                stack.push($v)
            };
        }
        macro_rules! pop {
            () => {
                stack
                    .pop()
                    .ok_or_else(|| Fault::Vm("operand stack underflow".to_owned()))?
            };
        }
        macro_rules! pop_int {
            () => {
                match pop!() {
                    Value::Int(v) => v,
                    other => return Err(Fault::Vm(format!("expected int, found {}", other.tag()))),
                }
            };
        }
        macro_rules! pop_long {
            () => {
                match pop!() {
                    Value::Long(v) => v,
                    other => return Err(Fault::Vm(format!("expected long, found {}", other.tag()))),
                }
            };
        }
        macro_rules! pop_float {
            () => {
                match pop!() {
                    Value::Float(v) => v,
                    other => {
                        return Err(Fault::Vm(format!("expected float, found {}", other.tag())))
                    }
                }
            };
        }
        macro_rules! pop_double {
            () => {
                match pop!() {
                    Value::Double(v) => v,
                    other => {
                        return Err(Fault::Vm(format!("expected double, found {}", other.tag())))
                    }
                }
            };
        }
        macro_rules! pop_ref {
            () => {
                match pop!() {
                    Value::Ref(v) => v,
                    other => {
                        return Err(Fault::Vm(format!(
                            "expected reference, found {}",
                            other.tag()
                        )))
                    }
                }
            };
        }
        match opcode {
            0x00 => {}
            0x01 => push!(Value::null()),
            0x02..=0x08 => push!(Value::Int(i32::from(opcode) - 3)),
            0x09..=0x0a => push!(Value::Long(i64::from(opcode) - 9)),
            0x0b..=0x0d => push!(Value::Float(f32::from(opcode) - 11.0)),
            0x0e..=0x0f => push!(Value::Double(f64::from(opcode) - 14.0)),
            0x10 => push!(Value::Int(i32::from(read_u8(bytes, pc)? as i8))),
            0x11 => push!(Value::Int(i32::from(read_i16(bytes, pc)?))),
            0x12 => {
                let index = u16::from(read_u8(bytes, pc)?);
                let value = self.load_constant(owner, index)?;
                push!(value);
            }
            0x13 | 0x14 => {
                let index = read_u16(bytes, pc)?;
                let value = self.load_constant(owner, index)?;
                push!(value);
            }
            0x15 | 0x17 | 0x19 => {
                let index = read_u8(bytes, pc)? as usize;
                push!(local(locals, index)?);
            }
            0x16 | 0x18 => {
                let index = read_u8(bytes, pc)? as usize;
                push!(local(locals, index)?);
            }
            0x1a..=0x1d => push!(local(locals, usize::from(opcode - 0x1a))?),
            0x1e..=0x21 => push!(local(locals, usize::from(opcode - 0x1e))?),
            0x22..=0x25 => push!(local(locals, usize::from(opcode - 0x22))?),
            0x26..=0x29 => push!(local(locals, usize::from(opcode - 0x26))?),
            0x2a..=0x2d => push!(local(locals, usize::from(opcode - 0x2a))?),
            0x2e..=0x35 => {
                let index = pop_int!();
                let reference =
                    pop_ref!().ok_or_else(|| self.throw("java/lang/NullPointerException", ""))?;
                let value = self.array_load(reference, index, opcode)?;
                push!(value);
            }
            0x36 | 0x38 | 0x3a => {
                let index = read_u8(bytes, pc)? as usize;
                let value = pop!();
                store_local(locals, index, value)?;
            }
            0x37 | 0x39 => {
                let index = read_u8(bytes, pc)? as usize;
                let value = pop!();
                store_local(locals, index, value)?;
            }
            0x3b..=0x3e => store_local(locals, usize::from(opcode - 0x3b), pop!())?,
            0x3f..=0x42 => store_local(locals, usize::from(opcode - 0x3f), pop!())?,
            0x43..=0x46 => store_local(locals, usize::from(opcode - 0x43), pop!())?,
            0x47..=0x4a => store_local(locals, usize::from(opcode - 0x47), pop!())?,
            0x4b..=0x4e => store_local(locals, usize::from(opcode - 0x4b), pop!())?,
            0x4f..=0x56 => {
                let value = pop!();
                let index = pop_int!();
                let reference =
                    pop_ref!().ok_or_else(|| self.throw("java/lang/NullPointerException", ""))?;
                self.array_store(reference, index, opcode, value)?;
            }
            0x57 => {
                pop!();
            }
            0x58 => {
                let value = pop!();
                if value.slots() == 1 {
                    pop!();
                }
            }
            0x59 => {
                let value = *stack
                    .last()
                    .ok_or_else(|| Fault::Vm("stack underflow".into()))?;
                push!(value);
            }
            0x5a => {
                let a = pop!();
                let b = pop!();
                push!(a);
                push!(b);
                push!(a);
            }
            0x5b => {
                let a = pop!();
                let b = pop!();
                if b.slots() == 2 {
                    push!(a);
                    push!(b);
                    push!(a);
                } else {
                    let c = pop!();
                    push!(a);
                    push!(c);
                    push!(b);
                    push!(a);
                }
            }
            0x5c => {
                let a = pop!();
                if a.slots() == 2 {
                    push!(a);
                    push!(a);
                } else {
                    let b = pop!();
                    push!(b);
                    push!(a);
                    push!(b);
                    push!(a);
                }
            }
            0x5d => {
                let a = pop!();
                let b = pop!();
                if a.slots() == 2 {
                    push!(a);
                    push!(b);
                    push!(a);
                } else {
                    let c = pop!();
                    push!(b);
                    push!(a);
                    push!(c);
                    push!(b);
                    push!(a);
                }
            }
            0x5e => {
                let a = pop!();
                let b = pop!();
                if a.slots() == 2 {
                    if b.slots() == 2 {
                        push!(a);
                        push!(b);
                        push!(a);
                    } else {
                        let c = pop!();
                        push!(a);
                        push!(c);
                        push!(b);
                        push!(a);
                    }
                } else {
                    let c = pop!();
                    if c.slots() == 2 {
                        push!(b);
                        push!(a);
                        push!(c);
                        push!(b);
                        push!(a);
                    } else {
                        let d = pop!();
                        push!(b);
                        push!(a);
                        push!(d);
                        push!(c);
                        push!(b);
                        push!(a);
                    }
                }
            }
            0x5f => {
                let a = pop!();
                let b = pop!();
                push!(a);
                push!(b);
            }
            0x60..=0x83 => arithmetic(self, opcode, stack)?,
            0x84 => {
                let index = read_u8(bytes, pc)? as usize;
                let increment = i32::from(read_u8(bytes, pc)? as i8);
                let value = local(locals, index)?;
                match value {
                    Value::Int(v) => locals[index] = Value::Int(v.wrapping_add(increment)),
                    other => return Err(Fault::Vm(format!("iinc on {}", other.tag()))),
                }
            }
            0x85..=0x93 => convert(self, opcode, stack)?,
            0x94 => {
                let b = pop_long!();
                let a = pop_long!();
                push!(Value::Int(match a.cmp(&b) {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Equal => 0,
                    std::cmp::Ordering::Greater => 1,
                }));
            }
            0x95 | 0x96 => {
                let b = pop_float!();
                let a = pop_float!();
                push!(Value::Int(compare_float(a, b, opcode == 0x96)));
            }
            0x97 | 0x98 => {
                let b = pop_double!();
                let a = pop_double!();
                push!(Value::Int(compare_double(a, b, opcode == 0x98)));
            }
            0x99..=0x9e => {
                let value = pop_int!();
                let offset = read_i16(bytes, pc)?;
                let take = match opcode {
                    0x99 => value == 0,
                    0x9a => value != 0,
                    0x9b => value < 0,
                    0x9c => value >= 0,
                    0x9d => value > 0,
                    _ => value <= 0,
                };
                if take {
                    *pc = branch(instruction, i32::from(offset))?;
                }
            }
            0x9f..=0xa4 => {
                let b = pop_int!();
                let a = pop_int!();
                let offset = read_i16(bytes, pc)?;
                let take = match opcode {
                    0x9f => a == b,
                    0xa0 => a != b,
                    0xa1 => a < b,
                    0xa2 => a >= b,
                    0xa3 => a > b,
                    0xa4 => a <= b,
                    0xa5 => a == b,
                    _ => a != b,
                };
                if take {
                    *pc = branch(instruction, i32::from(offset))?;
                }
            }
            0xa5 | 0xa6 => {
                let b = pop_ref!();
                let a = pop_ref!();
                let offset = read_i16(bytes, pc)?;
                if (a == b) == (opcode == 0xa5) {
                    *pc = branch(instruction, i32::from(offset))?;
                }
            }
            0xa7 => {
                let offset = read_i16(bytes, pc)?;
                *pc = branch(instruction, i32::from(offset))?;
            }
            0xa8 | 0xa9 | 0xc9 => {
                return Err(Fault::Vm(
                    "jsr/ret bytecode from Java 1.4 is not supported".into(),
                ));
            }
            0xaa => {
                while !pc.is_multiple_of(4) {
                    read_u8(bytes, pc)?;
                }
                let default = read_i32(bytes, pc)?;
                let low = read_i32(bytes, pc)?;
                let high = read_i32(bytes, pc)?;
                let key = pop_int!();
                let offset = if key >= low && key <= high {
                    let n = (high - low + 1) as usize;
                    let mut chosen = default;
                    for i in 0..n {
                        let candidate = read_i32(bytes, pc)?;
                        if i as i32 == key - low {
                            chosen = candidate;
                        }
                    }
                    chosen
                } else {
                    default
                };
                *pc = branch(instruction, offset)?;
            }
            0xab => {
                while !pc.is_multiple_of(4) {
                    read_u8(bytes, pc)?;
                }
                let default = read_i32(bytes, pc)?;
                let pairs = read_i32(bytes, pc)?;
                let key = pop_int!();
                let mut chosen = default;
                for _ in 0..pairs {
                    let case = read_i32(bytes, pc)?;
                    let offset = read_i32(bytes, pc)?;
                    if case == key {
                        chosen = offset;
                    }
                }
                *pc = branch(instruction, chosen)?;
            }
            0xac => return Ok(Step::Value(Value::Int(pop_int!()))),
            0xad => return Ok(Step::Value(Value::Long(pop_long!()))),
            0xae => return Ok(Step::Value(Value::Float(pop_float!()))),
            0xaf => return Ok(Step::Value(Value::Double(pop_double!()))),
            0xb0 => return Ok(Step::Value(Value::Ref(pop_ref!()))),
            0xb1 => {
                let _ = return_type;
                return Ok(Step::Value(Value::Void));
            }
            0xb2 => {
                let index = read_u16(bytes, pc)?;
                let reference = pool_member(self, owner, index)?;
                self.ensure_initialized(&reference.class)?;
                let value = self
                    .get_static_raw(&reference.class, &reference.name, &reference.descriptor)
                    .ok_or_else(|| {
                        Fault::Vm(format!(
                            "no static field {}.{}:{}",
                            reference.class, reference.name, reference.descriptor
                        ))
                    })?;
                push!(value);
            }
            0xb3 => {
                let index = read_u16(bytes, pc)?;
                let reference = pool_member(self, owner, index)?;
                let value = pop!();
                self.ensure_initialized(&reference.class)?;
                self.set_static_raw(
                    &reference.class,
                    &reference.name,
                    &reference.descriptor,
                    value,
                )?;
            }
            0xb4 => {
                let index = read_u16(bytes, pc)?;
                let reference = pool_member(self, owner, index)?;
                let object =
                    pop_ref!().ok_or_else(|| self.throw("java/lang/NullPointerException", ""))?;
                let value =
                    self.get_instance_field(object, &reference.name, &reference.descriptor)?;
                push!(value);
            }
            0xb5 => {
                let index = read_u16(bytes, pc)?;
                let reference = pool_member(self, owner, index)?;
                let value = pop!();
                let object =
                    pop_ref!().ok_or_else(|| self.throw("java/lang/NullPointerException", ""))?;
                self.set_instance_field(object, &reference.name, &reference.descriptor, value)?;
            }
            0xb6 | 0xb7 | 0xb8 | 0xb9 => {
                let index = read_u16(bytes, pc)?;
                if opcode == 0xb9 {
                    read_u8(bytes, pc)?;
                    read_u8(bytes, pc)?;
                }
                let reference = pool_member(self, owner, index)?;
                let (types, _) = parse_descriptor(&reference.descriptor)?;
                let mut args = Vec::with_capacity(types.len() + 1);
                for _ in 0..types.len() {
                    args.push(pop!());
                }
                args.reverse();
                if opcode == 0xb6 || opcode == 0xb7 || opcode == 0xb9 {
                    let receiver = pop_ref!()
                        .ok_or_else(|| self.throw("java/lang/NullPointerException", ""))?;
                    args.insert(0, Value::obj(receiver));
                }
                if opcode == 0xb8 {
                    self.ensure_initialized(&reference.class)?;
                }
                let value = if opcode == 0xb6 || opcode == 0xb9 {
                    let receiver = args.remove(0);
                    self.call_ref(receiver, &reference.name, &reference.descriptor, args)?
                } else {
                    self.invoke_method(
                        &reference.class,
                        &reference.name,
                        &reference.descriptor,
                        args,
                    )?
                };
                if !matches!(value, Value::Void) {
                    push!(value);
                }
            }
            0xba => {
                let index = read_u16(bytes, pc)?;
                read_u16(bytes, pc)?;
                let value = self.invoke_dynamic(owner, index, stack)?;
                push!(value);
            }
            0xbb => {
                let index = read_u16(bytes, pc)?;
                let class = self.pool(owner)?.class_name(index)?;
                self.ensure_initialized(&class)?;
                let reference = self.new_object(&class)?;
                push!(Value::obj(reference));
            }
            0xbc => {
                let atype = read_u8(bytes, pc)?;
                let element = match atype {
                    4 => "Z",
                    5 => "C",
                    6 => "F",
                    7 => "D",
                    8 => "B",
                    9 => "S",
                    10 => "I",
                    11 => "J",
                    other => return Err(Fault::Vm(format!("bad newarray type {other}"))),
                };
                let count = pop_int!();
                let values = self.array_values(element, count)?;
                let reference = self.alloc(Object {
                    class: format!("[{element}"),
                    fields: Vec::new(),
                    kind: Kind::Array {
                        element: element.to_owned(),
                        values,
                    },
                })?;
                push!(Value::obj(reference));
            }
            0xbd => {
                let index = read_u16(bytes, pc)?;
                let class = self.pool(owner)?.class_name(index)?;
                let element = format!("L{class};");
                let count = pop_int!();
                let values = self.array_values(&element, count)?;
                let reference = self.alloc(Object {
                    class: format!("[L{class};"),
                    fields: Vec::new(),
                    kind: Kind::Array { element, values },
                })?;
                push!(Value::obj(reference));
            }
            0xbe => {
                let reference =
                    pop_ref!().ok_or_else(|| self.throw("java/lang/NullPointerException", ""))?;
                let length = match &self.object(reference)?.kind {
                    Kind::Array { values, .. } => values.len(),
                    _ => return Err(Fault::Vm("arraylength on a non-array".into())),
                };
                push!(Value::Int(length as i32));
            }
            0xbf => {
                let reference =
                    pop_ref!().ok_or_else(|| self.throw("java/lang/NullPointerException", ""))?;
                return Err(Fault::Java(reference));
            }
            0xc0 => {
                let index = read_u16(bytes, pc)?;
                let class = self.pool(owner)?.class_name(index)?;
                let value = pop!();
                if !value.is_null() {
                    let actual = self.class_of(value)?;
                    if !self.is_assignable(&class, &actual) {
                        return Err(self.throw(
                            "java/lang/ClassCastException",
                            &format!(
                                "class {} cannot be cast to {}",
                                actual.replace('/', "."),
                                class.replace('/', ".")
                            ),
                        ));
                    }
                }
                push!(value);
            }
            0xc1 => {
                let index = read_u16(bytes, pc)?;
                let class = self.pool(owner)?.class_name(index)?;
                let value = pop!();
                let yes = if value.is_null() {
                    false
                } else {
                    let actual = self.class_of(value)?;
                    self.is_assignable(&class, &actual)
                };
                push!(Value::boolean(yes));
            }
            0xc2 | 0xc3 => {
                // Monitors do not exist in a single-threaded interpreter;
                // synchronized blocks still behave atomically here.
                pop_ref!().ok_or_else(|| self.throw("java/lang/NullPointerException", ""))?;
            }
            0xc4 => {
                let op = read_u8(bytes, pc)?;
                let index = read_u16(bytes, pc)? as usize;
                match op {
                    0x15 | 0x17 | 0x19 => push!(local(locals, index)?),
                    0x16 | 0x18 => push!(local(locals, index)?),
                    0x36 | 0x38 | 0x3a => {
                        let value = pop!();
                        store_local(locals, index, value)?;
                    }
                    0x37 | 0x39 => {
                        let value = pop!();
                        store_local(locals, index, value)?;
                    }
                    0x84 => {
                        let increment = read_i16(bytes, pc)?;
                        let value = local(locals, index)?;
                        match value {
                            Value::Int(v) => {
                                locals[index] = Value::Int(v.wrapping_add(i32::from(increment)))
                            }
                            other => {
                                return Err(Fault::Vm(format!("wide iinc on {}", other.tag())));
                            }
                        }
                    }
                    other => return Err(Fault::Vm(format!("bad wide opcode {other:#x}"))),
                }
            }
            0xc5 => {
                let index = read_u16(bytes, pc)?;
                let class = self.pool(owner)?.class_name(index)?;
                let dimensions = read_u8(bytes, pc)? as usize;
                let mut lengths = Vec::with_capacity(dimensions);
                for _ in 0..dimensions {
                    lengths.push(pop_int!());
                }
                lengths.reverse();
                let value = self.multianewarray(&class, &lengths)?;
                push!(value);
            }
            0xc6 | 0xc7 => {
                let reference = pop_ref!();
                let offset = read_i16(bytes, pc)?;
                let take = match opcode {
                    0xc6 => reference.is_none(),
                    _ => reference.is_some(),
                };
                if take {
                    *pc = branch(instruction, i32::from(offset))?;
                }
            }
            0xc8 => {
                let offset = read_i32(bytes, pc)?;
                *pc = branch(instruction, offset)?;
            }
            other => {
                return Err(Fault::Vm(format!(
                    "unsupported bytecode {other:#04x} in {owner}.{}{}",
                    method.name, method.descriptor
                )));
            }
        }
        Ok(Step::Continue)
    }
}

enum Step {
    Continue,
    Value(Value),
}

fn local(locals: &[Value], index: usize) -> Result<Value, Fault> {
    locals
        .get(index)
        .copied()
        .ok_or_else(|| Fault::Vm(format!("local {index} is outside the frame")))
}

fn store_local(locals: &mut [Value], index: usize, value: Value) -> Result<(), Fault> {
    let slot = locals
        .get_mut(index)
        .ok_or_else(|| Fault::Vm(format!("local {index} is outside the frame")))?;
    *slot = value;
    if value.slots() == 2
        && let Some(next) = locals.get_mut(index + 1)
    {
        *next = Value::Void;
    }
    Ok(())
}

fn branch(pc: usize, offset: i32) -> Result<usize, Fault> {
    let target = i64::try_from(pc).unwrap_or(i64::MAX) + i64::from(offset);
    usize::try_from(target)
        .map_err(|_| Fault::Vm(format!("branch to {target} is outside the method")))
}

fn compare_float(a: f32, b: f32, nan_is_greater: bool) -> i32 {
    if a < b {
        -1
    } else if a > b {
        1
    } else if a == b {
        0
    } else if nan_is_greater {
        1
    } else {
        -1
    }
}

fn compare_double(a: f64, b: f64, nan_is_greater: bool) -> i32 {
    if a < b {
        -1
    } else if a > b {
        1
    } else if a == b {
        0
    } else if nan_is_greater {
        1
    } else {
        -1
    }
}

fn read_u8(bytes: &[u8], pc: &mut usize) -> Result<u8, Fault> {
    let value = *bytes
        .get(*pc)
        .ok_or_else(|| Fault::Vm("truncated instruction".to_owned()))?;
    *pc += 1;
    Ok(value)
}

fn read_u16(bytes: &[u8], pc: &mut usize) -> Result<u16, Fault> {
    let hi = read_u8(bytes, pc)?;
    let lo = read_u8(bytes, pc)?;
    Ok(u16::from_be_bytes([hi, lo]))
}

fn read_i16(bytes: &[u8], pc: &mut usize) -> Result<i16, Fault> {
    Ok(read_u16(bytes, pc)? as i16)
}

fn read_i32(bytes: &[u8], pc: &mut usize) -> Result<i32, Fault> {
    let mut value = [0u8; 4];
    for byte in &mut value {
        *byte = read_u8(bytes, pc)?;
    }
    Ok(i32::from_be_bytes(value))
}

impl<H: Host> Vm<H> {
    fn pool(&self, owner: &str) -> Result<crate::classfile::Pool, Fault> {
        self.classes
            .get(owner)
            .and_then(|c| c.file.as_ref())
            .map(ClassFile::pool)
            .ok_or_else(|| Fault::Vm(format!("{owner} has no constant pool")))
    }

    fn load_constant(&mut self, owner: &str, index: u16) -> Result<Value, Fault> {
        let constant = self.pool(owner)?.get(index)?.clone();
        Ok(match constant {
            Constant::Integer(v) => Value::Int(v),
            Constant::Float(v) => Value::Float(v),
            Constant::Long(v) => Value::Long(v),
            Constant::Double(v) => Value::Double(v),
            Constant::String(text_index) => {
                let text = self.pool(owner)?.utf8(text_index)?;
                self.make_string(&text)?
            }
            Constant::Class(name_index) => {
                let name = self.pool(owner)?.utf8(name_index)?;
                let reference = self.alloc(Object {
                    class: "java/lang/Class".into(),
                    fields: Vec::new(),
                    kind: Kind::Class(name),
                })?;
                Value::obj(reference)
            }
            other => return Err(Fault::Vm(format!("ldc of {other:?}"))),
        })
    }

    fn find_handler(&mut self, code: &Code, pc: usize, exception: u32) -> Option<usize> {
        let actual = self.object(exception).ok()?.class.clone();
        for handler in &code.exception_table {
            if pc < usize::from(handler.start_pc) || pc >= usize::from(handler.end_pc) {
                continue;
            }
            let matches = match &handler.catch_type {
                None => true,
                Some(class) => self.is_assignable(class, &actual),
            };
            if matches {
                return Some(usize::from(handler.handler_pc));
            }
        }
        None
    }

    fn invoke_dynamic(
        &mut self,
        owner: &str,
        index: u16,
        stack: &mut Vec<Value>,
    ) -> Result<Value, Fault> {
        let (bootstrap_index, name_and_type) = self.pool(owner)?.invoke_dynamic(index)?;
        let bootstrap = self
            .classes
            .get(owner)
            .and_then(|c| c.file.as_ref())
            .and_then(|f| f.bootstrap_methods.get(usize::from(bootstrap_index)))
            .cloned()
            .ok_or_else(|| {
                Fault::Vm(format!(
                    "{owner}: missing bootstrap method {bootstrap_index}"
                ))
            })?;
        let pool = self.pool(owner)?;
        let (_, handle) = pool.method_handle(bootstrap.method_ref)?;
        let (argument_types, return_type) = parse_descriptor(&name_and_type.descriptor)?;
        match handle.class.as_str() {
            "java/lang/invoke/StringConcatFactory" => {
                let mut values = Vec::with_capacity(argument_types.len());
                for _ in 0..argument_types.len() {
                    values.push(
                        stack
                            .pop()
                            .ok_or_else(|| Fault::Vm("operand stack underflow".into()))?,
                    );
                }
                values.reverse();
                let recipe = bootstrap
                    .arguments
                    .first()
                    .and_then(|index| pool.constant_value(*index).ok())
                    .and_then(|value| match value {
                        classfile::ConstValue::String(text) => Some(text),
                        _ => None,
                    })
                    .unwrap_or_default();
                let mut out = String::new();
                let mut dynamic = 0usize;
                let mut constants = 1usize;
                for ch in recipe.chars() {
                    match ch {
                        '\u{1}' => {
                            if let Some(value) = values.get(dynamic).copied() {
                                out.push_str(&self.java_to_string(value)?);
                                dynamic += 1;
                            }
                        }
                        '\u{2}' => {
                            if let Some(index) = bootstrap.arguments.get(constants).copied() {
                                constants += 1;
                                if let Ok(classfile::ConstValue::String(text)) =
                                    pool.constant_value(index)
                                {
                                    out.push_str(&text);
                                }
                            }
                        }
                        other => out.push(other),
                    }
                }
                let _ = return_type;
                self.make_string(&out)
            }
            "java/lang/invoke/LambdaMetafactory" => {
                let implementation_index = *bootstrap.arguments.get(1).ok_or_else(|| {
                    Fault::Vm("lambda bootstrap lacks implementation handle".into())
                })?;
                let (handle_kind, implementation) = pool.method_handle(implementation_index)?;
                let sam_descriptor = bootstrap
                    .arguments
                    .first()
                    .and_then(|index| match pool.get(*index) {
                        Ok(Constant::MethodType(text)) => pool.utf8(*text).ok(),
                        _ => None,
                    })
                    .unwrap_or_else(|| name_and_type.descriptor.clone());
                let captured = argument_types.len();
                let mut values = Vec::with_capacity(captured);
                for _ in 0..captured {
                    values.push(
                        stack
                            .pop()
                            .ok_or_else(|| Fault::Vm("operand stack underflow".into()))?,
                    );
                }
                values.reverse();
                let interface = class_of_descriptor(&return_type).to_owned();
                let reference = self.alloc(Object {
                    class: interface.clone(),
                    fields: Vec::new(),
                    kind: Kind::Lambda(Lambda {
                        interface,
                        method: name_and_type.name.clone(),
                        descriptor: sam_descriptor,
                        implementation,
                        handle_kind,
                        captured: values,
                    }),
                })?;
                Ok(Value::obj(reference))
            }
            other => Err(Fault::Vm(format!(
                "invokedynamic bootstrap {other} is not implemented"
            ))),
        }
    }

    fn array_values(&mut self, element: &str, count: i32) -> Result<Vec<Value>, Fault> {
        if count < 0 {
            return Err(self.throw("java/lang/NegativeArraySizeException", &count.to_string()));
        }
        if count as usize > self.limits.objects {
            return Err(Fault::Vm(format!(
                "array length {count} exceeds allocation limit {}",
                self.limits.objects
            )));
        }
        let default = default_value(element);
        Ok(vec![default; count as usize])
    }

    fn array_load(&mut self, reference: u32, index: i32, _opcode: u8) -> Result<Value, Fault> {
        let (value, length) = {
            let object = self.object(reference)?;
            let Kind::Array { values, .. } = &object.kind else {
                return Err(Fault::Vm("array load on a non-array".into()));
            };
            (values.get(index as usize).copied(), values.len())
        };
        match value {
            Some(value) => Ok(value),
            None => Err(self.throw(
                "java/lang/ArrayIndexOutOfBoundsException",
                &format!("index {index}, length {length}"),
            )),
        }
    }

    fn array_store(
        &mut self,
        reference: u32,
        index: i32,
        opcode: u8,
        value: Value,
    ) -> Result<(), Fault> {
        let length = {
            let object = self.object(reference)?;
            match &object.kind {
                Kind::Array { values, .. } => values.len(),
                _ => return Err(Fault::Vm("array store on a non-array".into())),
            }
        };
        if index < 0 || index as usize >= length {
            return Err(self.throw(
                "java/lang/ArrayIndexOutOfBoundsException",
                &format!("index {index}, length {length}"),
            ));
        }
        let converted = match opcode {
            0x4f | 0x54 | 0x55 | 0x56 => Value::Int(value.as_int().unwrap_or(0)),
            _ => value,
        };
        if let Kind::Array { values, .. } = &mut self.object_mut(reference)?.kind {
            values[index as usize] = converted;
        }
        Ok(())
    }

    fn multianewarray(&mut self, class: &str, lengths: &[i32]) -> Result<Value, Fault> {
        let Some(element) = array_element(class) else {
            return Err(Fault::Vm(format!("multianewarray of {class}")));
        };
        let count = *lengths.first().unwrap_or(&0);
        let mut values = self.array_values(element, count)?;
        if lengths.len() > 1 {
            for slot in &mut values {
                *slot = self.multianewarray(element, &lengths[1..])?;
            }
        }
        let reference = self.alloc(Object {
            class: class.to_owned(),
            fields: Vec::new(),
            kind: Kind::Array {
                element: element.to_owned(),
                values,
            },
        })?;
        Ok(Value::obj(reference))
    }
}

fn pool_member<H: Host>(vm: &Vm<H>, owner: &str, index: u16) -> Result<MemberRef, Fault> {
    Ok(vm.pool(owner)?.member(index)?)
}

fn arithmetic<H: Host>(vm: &mut Vm<H>, opcode: u8, stack: &mut Vec<Value>) -> Result<(), Fault> {
    macro_rules! pop_int {
        () => {
            match stack.pop() {
                Some(Value::Int(v)) => v,
                other => return Err(Fault::Vm(format!("expected int, found {other:?}"))),
            }
        };
    }
    macro_rules! pop_long {
        () => {
            match stack.pop() {
                Some(Value::Long(v)) => v,
                other => return Err(Fault::Vm(format!("expected long, found {other:?}"))),
            }
        };
    }
    macro_rules! pop_float {
        () => {
            match stack.pop() {
                Some(Value::Float(v)) => v,
                other => return Err(Fault::Vm(format!("expected float, found {other:?}"))),
            }
        };
    }
    macro_rules! pop_double {
        () => {
            match stack.pop() {
                Some(Value::Double(v)) => v,
                other => return Err(Fault::Vm(format!("expected double, found {other:?}"))),
            }
        };
    }
    match opcode {
        0x60 => {
            let b = pop_int!();
            let a = pop_int!();
            stack.push(Value::Int(a.wrapping_add(b)));
        }
        0x61 => {
            let b = pop_long!();
            let a = pop_long!();
            stack.push(Value::Long(a.wrapping_add(b)));
        }
        0x62 => {
            let b = pop_float!();
            let a = pop_float!();
            stack.push(Value::Float(a + b));
        }
        0x63 => {
            let b = pop_double!();
            let a = pop_double!();
            stack.push(Value::Double(a + b));
        }
        0x64 => {
            let b = pop_int!();
            let a = pop_int!();
            stack.push(Value::Int(a.wrapping_sub(b)));
        }
        0x65 => {
            let b = pop_long!();
            let a = pop_long!();
            stack.push(Value::Long(a.wrapping_sub(b)));
        }
        0x66 => {
            let b = pop_float!();
            let a = pop_float!();
            stack.push(Value::Float(a - b));
        }
        0x67 => {
            let b = pop_double!();
            let a = pop_double!();
            stack.push(Value::Double(a - b));
        }
        0x68 => {
            let b = pop_int!();
            let a = pop_int!();
            stack.push(Value::Int(a.wrapping_mul(b)));
        }
        0x69 => {
            let b = pop_long!();
            let a = pop_long!();
            stack.push(Value::Long(a.wrapping_mul(b)));
        }
        0x6a => {
            let b = pop_float!();
            let a = pop_float!();
            stack.push(Value::Float(a * b));
        }
        0x6b => {
            let b = pop_double!();
            let a = pop_double!();
            stack.push(Value::Double(a * b));
        }
        0x6c | 0x70 => {
            let b = pop_int!();
            let a = pop_int!();
            if b == 0 {
                return Err(vm.throw("java/lang/ArithmeticException", "/ by zero"));
            }
            if opcode == 0x6c {
                stack.push(Value::Int(a.wrapping_div(b)));
            } else {
                stack.push(Value::Int(a.wrapping_rem(b)));
            }
        }
        0x6d | 0x71 => {
            let b = pop_long!();
            let a = pop_long!();
            if b == 0 {
                return Err(vm.throw("java/lang/ArithmeticException", "/ by zero"));
            }
            if opcode == 0x6d {
                stack.push(Value::Long(a.wrapping_div(b)));
            } else {
                stack.push(Value::Long(a.wrapping_rem(b)));
            }
        }
        0x6e | 0x72 => {
            let b = pop_float!();
            let a = pop_float!();
            if opcode == 0x6e {
                stack.push(Value::Float(a / b));
            } else {
                stack.push(Value::Float(a % b));
            }
        }
        0x6f | 0x73 => {
            let b = pop_double!();
            let a = pop_double!();
            if opcode == 0x6f {
                stack.push(Value::Double(a / b));
            } else {
                stack.push(Value::Double(a % b));
            }
        }
        0x74 => {
            let v = pop_int!();
            stack.push(Value::Int(v.wrapping_neg()));
        }
        0x75 => {
            let v = pop_long!();
            stack.push(Value::Long(v.wrapping_neg()));
        }
        0x76 => {
            let v = pop_float!();
            stack.push(Value::Float(-v));
        }
        0x77 => {
            let v = pop_double!();
            stack.push(Value::Double(-v));
        }
        0x78 => {
            let b = pop_int!() & 0x1f;
            let a = pop_int!();
            stack.push(Value::Int(a.wrapping_shl(b as u32)));
        }
        0x79 => {
            let b = pop_int!() & 0x3f;
            let a = pop_long!();
            stack.push(Value::Long(a.wrapping_shl(b as u32)));
        }
        0x7a => {
            let b = pop_int!() & 0x1f;
            let a = pop_int!();
            stack.push(Value::Int(a.wrapping_shr(b as u32)));
        }
        0x7b => {
            let b = pop_int!() & 0x3f;
            let a = pop_long!();
            stack.push(Value::Long(a.wrapping_shr(b as u32)));
        }
        0x7c => {
            let b = pop_int!() & 0x1f;
            let a = pop_int!();
            stack.push(Value::Int(((a as u32) >> b) as i32));
        }
        0x7d => {
            let b = pop_int!() & 0x3f;
            let a = pop_long!();
            stack.push(Value::Long(((a as u64) >> b) as i64));
        }
        0x7e => {
            let b = pop_int!();
            let a = pop_int!();
            stack.push(Value::Int(a & b));
        }
        0x7f => {
            let b = pop_long!();
            let a = pop_long!();
            stack.push(Value::Long(a & b));
        }
        0x80 => {
            let b = pop_int!();
            let a = pop_int!();
            stack.push(Value::Int(a | b));
        }
        0x81 => {
            let b = pop_long!();
            let a = pop_long!();
            stack.push(Value::Long(a | b));
        }
        0x82 => {
            let b = pop_int!();
            let a = pop_int!();
            stack.push(Value::Int(a ^ b));
        }
        0x83 => {
            let b = pop_long!();
            let a = pop_long!();
            stack.push(Value::Long(a ^ b));
        }
        other => return Err(Fault::Vm(format!("bad arithmetic opcode {other:#x}"))),
    }
    Ok(())
}

fn convert<H: Host>(_vm: &mut Vm<H>, opcode: u8, stack: &mut Vec<Value>) -> Result<(), Fault> {
    let value = stack
        .pop()
        .ok_or_else(|| Fault::Vm("operand stack underflow".into()))?;
    let converted = match opcode {
        0x85 => Value::Long(value.as_int().unwrap_or(0) as i64),
        0x86 => Value::Float(value.as_int().unwrap_or(0) as f32),
        0x87 => Value::Double(value.as_int().unwrap_or(0) as f64),
        0x88 => Value::Int(value.as_long().unwrap_or(0) as i32),
        0x89 => Value::Float(value.as_long().unwrap_or(0) as f32),
        0x8a => Value::Double(value.as_long().unwrap_or(0) as f64),
        0x8b => Value::Int(value.as_float().unwrap_or(0.0) as i32),
        0x8c => Value::Long(value.as_float().unwrap_or(0.0) as i64),
        0x8d => Value::Double(f64::from(value.as_float().unwrap_or(0.0))),
        0x8e => Value::Int(value.as_double().unwrap_or(0.0) as i32),
        0x8f => Value::Long(value.as_double().unwrap_or(0.0) as i64),
        0x90 => Value::Float(value.as_double().unwrap_or(0.0) as f32),
        0x91 => Value::Int(value.as_int().unwrap_or(0) as i8 as i32),
        0x92 => Value::Int(value.as_int().unwrap_or(0) as u16 as i32),
        0x93 => Value::Int(value.as_int().unwrap_or(0) as i16 as i32),
        other => return Err(Fault::Vm(format!("bad conversion opcode {other:#x}"))),
    };
    stack.push(converted);
    Ok(())
}

//! A reader for the JVM class-file format (JVMS §4).
//!
//! The interpreter consumes this directly: method bodies, the constant pool,
//! exception tables, bootstrap methods and runtime annotations. Unsupported
//! or malformed input is rejected with the class and byte offset, never
//! guessed around.

use std::collections::BTreeSet;

pub const MAGIC: u32 = 0xCAFE_BABE;

#[derive(Clone, Debug, PartialEq)]
pub enum Constant {
    Utf8(String),
    Integer(i32),
    Float(f32),
    Long(i64),
    Double(f64),
    Class(u16),
    String(u16),
    Fieldref {
        class: u16,
        name_type: u16,
    },
    Methodref {
        class: u16,
        name_type: u16,
    },
    InterfaceMethodref {
        class: u16,
        name_type: u16,
    },
    NameAndType {
        name: u16,
        descriptor: u16,
    },
    MethodHandle {
        kind: u8,
        reference: u16,
    },
    MethodType(u16),
    Dynamic {
        bootstrap: u16,
        name_type: u16,
    },
    InvokeDynamic {
        bootstrap: u16,
        name_type: u16,
    },
    Module(u16),
    Package(u16),
    /// An entry the reader does not model (`Unused`); never referenced by
    /// valid bytecode.
    Unusable,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NameAndType {
    pub name: String,
    pub descriptor: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MemberRef {
    pub class: String,
    pub name: String,
    pub descriptor: String,
    pub interface: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConstValue {
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    String(String),
    Class(String),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Annotation {
    /// The annotation type descriptor (`Lnet/neoforged/fml/common/Mod;`).
    pub type_descriptor: String,
    pub values: Vec<(String, ElementValue)>,
}

impl Annotation {
    pub fn type_name(&self) -> String {
        descriptor_class(&self.type_descriptor)
    }
    pub fn get(&self, name: &str) -> Option<&ElementValue> {
        self.values.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }
    pub fn string(&self, name: &str) -> Option<&str> {
        match self.get(name)? {
            ElementValue::Const(ConstValue::String(s)) => Some(s),
            _ => None,
        }
    }
    pub fn bool(&self, name: &str) -> Option<bool> {
        match self.get(name)? {
            ElementValue::Const(ConstValue::Int(i)) => Some(*i != 0),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ElementValue {
    Const(ConstValue),
    Enum {
        type_descriptor: String,
        name: String,
    },
    Class(String),
    Annotation(Box<Annotation>),
    Array(Vec<ElementValue>),
}

impl ElementValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            ElementValue::Const(ConstValue::String(s)) => Some(s),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExceptionHandler {
    pub start_pc: u16,
    pub end_pc: u16,
    pub handler_pc: u16,
    /// The caught class (internal name); `None` catches everything.
    pub catch_type: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Code {
    pub max_stack: u16,
    pub max_locals: u16,
    pub code: Vec<u8>,
    pub exception_table: Vec<ExceptionHandler>,
    /// `(start_pc, line)` pairs, for diagnostics.
    pub line_numbers: Vec<(u16, u16)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub access: u16,
    pub name: String,
    pub descriptor: String,
    pub constant: Option<ConstValue>,
    pub annotations: Vec<Annotation>,
    pub signature: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Method {
    pub access: u16,
    pub name: String,
    pub descriptor: String,
    pub code: Option<Code>,
    pub exceptions: Vec<String>,
    pub annotations: Vec<Annotation>,
    pub signature: Option<String>,
    pub annotation_default: Option<ElementValue>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BootstrapMethod {
    pub method_ref: u16,
    pub arguments: Vec<u16>,
}

#[derive(Clone, Debug)]
pub struct ClassFile {
    pub major: u16,
    pub minor: u16,
    pub access: u16,
    pub name: String,
    pub super_name: Option<String>,
    pub interfaces: Vec<String>,
    pub fields: Vec<Field>,
    pub methods: Vec<Method>,
    pub constants: Vec<Constant>,
    pub bootstrap_methods: Vec<BootstrapMethod>,
    pub annotations: Vec<Annotation>,
    pub signature: Option<String>,
    pub source_file: Option<String>,
    /// Record component names, when the class is a record.
    pub record_components: Vec<(String, String)>,
    pub nest_host: Option<String>,
}

// Access flags.
pub const ACC_PUBLIC: u16 = 0x0001;
pub const ACC_PRIVATE: u16 = 0x0002;
pub const ACC_PROTECTED: u16 = 0x0004;
pub const ACC_STATIC: u16 = 0x0008;
pub const ACC_FINAL: u16 = 0x0010;
pub const ACC_SUPER: u16 = 0x0020;
pub const ACC_VOLATILE: u16 = 0x0040;
pub const ACC_TRANSIENT: u16 = 0x0080;
pub const ACC_NATIVE: u16 = 0x0100;
pub const ACC_INTERFACE: u16 = 0x0200;
pub const ACC_ABSTRACT: u16 = 0x0400;
pub const ACC_SYNTHETIC: u16 = 0x1000;
pub const ACC_ANNOTATION: u16 = 0x2000;
pub const ACC_ENUM: u16 = 0x4000;

impl ClassFile {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let mut r = Reader::new(bytes);
        if r.u4()? != MAGIC {
            return Err("not a class file (bad magic)".into());
        }
        let minor = r.u2()?;
        let major = r.u2()?;
        if major < 45 || major > 69 {
            return Err(format!("unsupported class-file version {major}.{minor}"));
        }
        let count = r.u2()? as usize;
        if count == 0 {
            return Err("constant pool is empty".into());
        }
        let mut constants = Vec::with_capacity(count);
        constants.push(Constant::Unusable);
        let mut i = 1;
        while i < count {
            let tag = r.u1()?;
            let constant = match tag {
                1 => Constant::Utf8(r.utf8()?),
                3 => Constant::Integer(r.u4()? as i32),
                4 => Constant::Float(f32::from_bits(r.u4()?)),
                5 => {
                    let v = Constant::Long(r.u8()? as i64);
                    constants.push(v);
                    constants.push(Constant::Unusable);
                    i += 2;
                    continue;
                }
                6 => {
                    let v = Constant::Double(f64::from_bits(r.u8()?));
                    constants.push(v);
                    constants.push(Constant::Unusable);
                    i += 2;
                    continue;
                }
                7 => Constant::Class(r.u2()?),
                8 => Constant::String(r.u2()?),
                9 => Constant::Fieldref {
                    class: r.u2()?,
                    name_type: r.u2()?,
                },
                10 => Constant::Methodref {
                    class: r.u2()?,
                    name_type: r.u2()?,
                },
                11 => Constant::InterfaceMethodref {
                    class: r.u2()?,
                    name_type: r.u2()?,
                },
                12 => Constant::NameAndType {
                    name: r.u2()?,
                    descriptor: r.u2()?,
                },
                15 => Constant::MethodHandle {
                    kind: r.u1()?,
                    reference: r.u2()?,
                },
                16 => Constant::MethodType(r.u2()?),
                17 => Constant::Dynamic {
                    bootstrap: r.u2()?,
                    name_type: r.u2()?,
                },
                18 => Constant::InvokeDynamic {
                    bootstrap: r.u2()?,
                    name_type: r.u2()?,
                },
                19 => Constant::Module(r.u2()?),
                20 => Constant::Package(r.u2()?),
                other => return Err(format!("constant pool entry {i}: unknown tag {other}")),
            };
            constants.push(constant);
            i += 1;
        }
        let access = r.u2()?;
        let this_class = r.u2()?;
        let super_class = r.u2()?;
        let mut pool = Pool { entries: constants };
        let name = pool.class_name(this_class)?;
        let super_name = match super_class {
            0 => None,
            index => Some(pool.class_name(index)?),
        };
        let interface_count = r.u2()? as usize;
        let mut interfaces = Vec::with_capacity(interface_count);
        for _ in 0..interface_count {
            interfaces.push(pool.class_name(r.u2()?)?);
        }
        let field_count = r.u2()? as usize;
        let mut fields = Vec::with_capacity(field_count);
        for _ in 0..field_count {
            fields.push(read_field(&mut r, &pool)?);
        }
        let method_count = r.u2()? as usize;
        let mut methods = Vec::with_capacity(method_count);
        for _ in 0..method_count {
            methods.push(read_method(&mut r, &pool)?);
        }
        let mut annotations = Vec::new();
        let mut signature = None;
        let mut source_file = None;
        let mut bootstrap_methods = Vec::new();
        let mut record_components = Vec::new();
        let mut nest_host = None;
        let attribute_count = r.u2()? as usize;
        for _ in 0..attribute_count {
            let attr_name = pool.utf8(r.u2()?)?;
            let length = r.u4()? as usize;
            let body = r.take(length)?;
            match attr_name.as_str() {
                "BootstrapMethods" => {
                    bootstrap_methods = read_bootstrap_methods(body)?;
                }
                "RuntimeVisibleAnnotations" | "RuntimeInvisibleAnnotations" => {
                    let mut ar = Reader::new(body);
                    annotations.extend(read_annotations(&mut ar, &pool)?);
                }
                "Signature" => {
                    let mut ar = Reader::new(body);
                    signature = Some(pool.utf8(ar.u2()?)?);
                }
                "SourceFile" => {
                    let mut ar = Reader::new(body);
                    source_file = Some(pool.utf8(ar.u2()?)?);
                }
                "NestHost" => {
                    let mut ar = Reader::new(body);
                    nest_host = Some(pool.class_name(ar.u2()?)?);
                }
                "Record" => {
                    let mut ar = Reader::new(body);
                    let n = ar.u2()? as usize;
                    for _ in 0..n {
                        let comp_name = pool.utf8(ar.u2()?)?;
                        let descriptor = pool.utf8(ar.u2()?)?;
                        let attrs = ar.u2()? as usize;
                        for _ in 0..attrs {
                            let _ = ar.u2()?;
                            let len = ar.u4()? as usize;
                            ar.take(len)?;
                        }
                        record_components.push((comp_name, descriptor));
                    }
                }
                _ => {}
            }
        }
        if r.remaining() != 0 {
            return Err(format!("{} trailing bytes after the class", r.remaining()));
        }
        Ok(Self {
            major,
            minor,
            access,
            name,
            super_name,
            interfaces,
            fields,
            methods,
            constants: pool.entries,
            bootstrap_methods,
            annotations,
            signature,
            source_file,
            record_components,
            nest_host,
        })
    }

    pub fn pool(&self) -> Pool {
        Pool {
            entries: self.constants.clone(),
        }
    }

    pub fn method(&self, name: &str, descriptor: &str) -> Option<&Method> {
        self.methods
            .iter()
            .find(|m| m.name == name && m.descriptor == descriptor)
    }

    pub fn method_named(&self, name: &str) -> Option<&Method> {
        self.methods.iter().find(|m| m.name == name)
    }

    pub fn field(&self, name: &str, descriptor: &str) -> Option<&Field> {
        self.fields
            .iter()
            .find(|f| f.name == name && f.descriptor == descriptor)
    }

    pub fn is_interface(&self) -> bool {
        self.access & ACC_INTERFACE != 0
    }

    /// Every class name in the constant pool that is not this class: the
    /// classes the bytecode names, whether or not an executed path reaches
    /// them.
    pub fn referenced_classes(&self) -> BTreeSet<String> {
        let pool = self.pool();
        let mut out = BTreeSet::new();
        for constant in &self.constants {
            if let Constant::Class(index) = constant
                && let Ok(name) = pool.class_name(*index)
            {
                if name != self.name {
                    out.insert(name);
                }
            }
        }
        out
    }

    /// The Java release this class needs (major 52 = Java 8).
    pub fn java_release(&self) -> u32 {
        u32::from(self.major).saturating_sub(44)
    }
}

#[derive(Clone, Debug)]
pub struct Pool {
    pub entries: Vec<Constant>,
}

impl Pool {
    pub fn get(&self, index: u16) -> Result<&Constant, String> {
        self.entries
            .get(index as usize)
            .ok_or_else(|| format!("constant pool index {index} out of range"))
    }

    pub fn utf8(&self, index: u16) -> Result<String, String> {
        match self.get(index)? {
            Constant::Utf8(s) => Ok(s.clone()),
            other => Err(format!("constant {index} is not Utf8: {other:?}")),
        }
    }

    pub fn class_name(&self, index: u16) -> Result<String, String> {
        match self.get(index)? {
            Constant::Class(name) => Ok(descriptor_class(&self.utf8(*name)?)),
            other => Err(format!("constant {index} is not a Class: {other:?}")),
        }
    }

    /// The internal class name a `Class` constant names, keeping an array
    /// descriptor as is.
    pub fn class_descriptor(&self, index: u16) -> Result<String, String> {
        match self.get(index)? {
            Constant::Class(name) => self.utf8(*name),
            other => Err(format!("constant {index} is not a Class: {other:?}")),
        }
    }

    pub fn name_and_type(&self, index: u16) -> Result<NameAndType, String> {
        match self.get(index)? {
            Constant::NameAndType { name, descriptor } => Ok(NameAndType {
                name: self.utf8(*name)?,
                descriptor: self.utf8(*descriptor)?,
            }),
            other => Err(format!("constant {index} is not a NameAndType: {other:?}")),
        }
    }

    pub fn member(&self, index: u16) -> Result<MemberRef, String> {
        let (class, name_type, interface) = match self.get(index)? {
            Constant::Fieldref { class, name_type } => (*class, *name_type, false),
            Constant::Methodref { class, name_type } => (*class, *name_type, false),
            Constant::InterfaceMethodref { class, name_type } => (*class, *name_type, true),
            other => {
                return Err(format!(
                    "constant {index} is not a member reference: {other:?}"
                ));
            }
        };
        let nt = self.name_and_type(name_type)?;
        Ok(MemberRef {
            class: self.class_name(class)?,
            name: nt.name,
            descriptor: nt.descriptor,
            interface,
        })
    }

    pub fn invoke_dynamic(&self, index: u16) -> Result<(u16, NameAndType), String> {
        match self.get(index)? {
            Constant::InvokeDynamic {
                bootstrap,
                name_type,
            } => Ok((*bootstrap, self.name_and_type(*name_type)?)),
            other => Err(format!(
                "constant {index} is not an InvokeDynamic: {other:?}"
            )),
        }
    }

    pub fn dynamic(&self, index: u16) -> Result<(u16, NameAndType), String> {
        match self.get(index)? {
            Constant::Dynamic {
                bootstrap,
                name_type,
            } => Ok((*bootstrap, self.name_and_type(*name_type)?)),
            other => Err(format!("constant {index} is not a Dynamic: {other:?}")),
        }
    }

    pub fn method_handle(&self, index: u16) -> Result<(u8, MemberRef), String> {
        match self.get(index)? {
            Constant::MethodHandle { kind, reference } => Ok((*kind, self.member(*reference)?)),
            other => Err(format!("constant {index} is not a MethodHandle: {other:?}")),
        }
    }

    pub fn constant_value(&self, index: u16) -> Result<ConstValue, String> {
        match self.get(index)? {
            Constant::Integer(v) => Ok(ConstValue::Int(*v)),
            Constant::Long(v) => Ok(ConstValue::Long(*v)),
            Constant::Float(v) => Ok(ConstValue::Float(*v)),
            Constant::Double(v) => Ok(ConstValue::Double(*v)),
            Constant::String(s) => Ok(ConstValue::String(self.utf8(*s)?)),
            Constant::Class(c) => Ok(ConstValue::Class(self.class_name(*c)?)),
            other => Err(format!("constant {index} is not a literal: {other:?}")),
        }
    }
}

/// `Ljava/lang/String;` → `java/lang/String`; an array descriptor is kept
/// (`[Ljava/lang/String;`, `[I`).
pub fn descriptor_class(descriptor: &str) -> String {
    let d = descriptor.trim();
    if let Some(inner) = d.strip_prefix('L').and_then(|s| s.strip_suffix(';')) {
        inner.to_owned()
    } else if d.starts_with('[') {
        d.to_owned()
    } else {
        d.to_owned()
    }
}

/// The JVM descriptor of a class name (`java/lang/String` →
/// `Ljava/lang/String;`).
pub fn class_descriptor_of(name: &str) -> String {
    if name.starts_with('[') {
        name.to_owned()
    } else {
        format!("L{name};")
    }
}

fn read_field(r: &mut Reader<'_>, pool: &Pool) -> Result<Field, String> {
    let access = r.u2()?;
    let name = pool.utf8(r.u2()?)?;
    let descriptor = pool.utf8(r.u2()?)?;
    let mut constant = None;
    let mut annotations = Vec::new();
    let mut signature = None;
    for _ in 0..r.u2()? {
        let attr = pool.utf8(r.u2()?)?;
        let len = r.u4()? as usize;
        let body = r.take(len)?;
        match attr.as_str() {
            "ConstantValue" => {
                let mut ar = Reader::new(body);
                constant = Some(pool.constant_value(ar.u2()?)?);
            }
            "RuntimeVisibleAnnotations" | "RuntimeInvisibleAnnotations" => {
                let mut ar = Reader::new(body);
                annotations.extend(read_annotations(&mut ar, pool)?);
            }
            "Signature" => {
                let mut ar = Reader::new(body);
                signature = Some(pool.utf8(ar.u2()?)?);
            }
            _ => {}
        }
    }
    Ok(Field {
        access,
        name,
        descriptor,
        constant,
        annotations,
        signature,
    })
}

fn read_method(r: &mut Reader<'_>, pool: &Pool) -> Result<Method, String> {
    let access = r.u2()?;
    let name = pool.utf8(r.u2()?)?;
    let descriptor = pool.utf8(r.u2()?)?;
    let mut code = None;
    let mut exceptions = Vec::new();
    let mut annotations = Vec::new();
    let mut signature = None;
    let mut annotation_default = None;
    for _ in 0..r.u2()? {
        let attr = pool.utf8(r.u2()?)?;
        let len = r.u4()? as usize;
        let body = r.take(len)?;
        match attr.as_str() {
            "Code" => code = Some(read_code(body, pool)?),
            "Exceptions" => {
                let mut ar = Reader::new(body);
                for _ in 0..ar.u2()? {
                    exceptions.push(pool.class_name(ar.u2()?)?);
                }
            }
            "RuntimeVisibleAnnotations" | "RuntimeInvisibleAnnotations" => {
                let mut ar = Reader::new(body);
                annotations.extend(read_annotations(&mut ar, pool)?);
            }
            "Signature" => {
                let mut ar = Reader::new(body);
                signature = Some(pool.utf8(ar.u2()?)?);
            }
            "AnnotationDefault" => {
                let mut ar = Reader::new(body);
                annotation_default = Some(read_element_value(&mut ar, pool)?);
            }
            _ => {}
        }
    }
    Ok(Method {
        access,
        name,
        descriptor,
        code,
        exceptions,
        annotations,
        signature,
        annotation_default,
    })
}

fn read_code(body: &[u8], pool: &Pool) -> Result<Code, String> {
    let mut r = Reader::new(body);
    let max_stack = r.u2()?;
    let max_locals = r.u2()?;
    let len = r.u4()? as usize;
    let code = r.take(len)?.to_vec();
    let mut exception_table = Vec::new();
    for _ in 0..r.u2()? {
        let start_pc = r.u2()?;
        let end_pc = r.u2()?;
        let handler_pc = r.u2()?;
        let catch_type = r.u2()?;
        exception_table.push(ExceptionHandler {
            start_pc,
            end_pc,
            handler_pc,
            catch_type: match catch_type {
                0 => None,
                index => Some(pool.class_name(index)?),
            },
        });
    }
    let mut line_numbers = Vec::new();
    for _ in 0..r.u2()? {
        let attr = pool.utf8(r.u2()?)?;
        let len = r.u4()? as usize;
        let body = r.take(len)?;
        if attr == "LineNumberTable" {
            let mut ar = Reader::new(body);
            for _ in 0..ar.u2()? {
                line_numbers.push((ar.u2()?, ar.u2()?));
            }
        }
    }
    if code.is_empty() {
        return Err("a Code attribute has no instructions".into());
    }
    Ok(Code {
        max_stack,
        max_locals,
        code,
        exception_table,
        line_numbers,
    })
}

fn read_bootstrap_methods(body: &[u8]) -> Result<Vec<BootstrapMethod>, String> {
    let mut r = Reader::new(body);
    let mut out = Vec::new();
    for _ in 0..r.u2()? {
        out.push(BootstrapMethod {
            method_ref: r.u2()?,
            arguments: (0..r.u2()?).map(|_| r.u2()).collect::<Result<_, _>>()?,
        });
    }
    Ok(out)
}

fn read_annotations(r: &mut Reader<'_>, pool: &Pool) -> Result<Vec<Annotation>, String> {
    let mut out = Vec::new();
    for _ in 0..r.u2()? {
        out.push(read_annotation(r, pool)?);
    }
    Ok(out)
}

fn read_annotation(r: &mut Reader<'_>, pool: &Pool) -> Result<Annotation, String> {
    let type_descriptor = pool.utf8(r.u2()?)?;
    let mut values = Vec::new();
    for _ in 0..r.u2()? {
        let name = pool.utf8(r.u2()?)?;
        values.push((name, read_element_value(r, pool)?));
    }
    Ok(Annotation {
        type_descriptor,
        values,
    })
}

fn read_element_value(r: &mut Reader<'_>, pool: &Pool) -> Result<ElementValue, String> {
    let tag = r.u1()? as char;
    Ok(match tag {
        'B' | 'C' | 'I' | 'S' | 'Z' => {
            ElementValue::Const(ConstValue::Int(match pool.get(r.u2()?)? {
                Constant::Integer(v) => *v,
                other => return Err(format!("annotation integer is {other:?}")),
            }))
        }
        'J' => ElementValue::Const(ConstValue::Long(match pool.get(r.u2()?)? {
            Constant::Long(v) => *v,
            other => return Err(format!("annotation long is {other:?}")),
        })),
        'F' => ElementValue::Const(ConstValue::Float(match pool.get(r.u2()?)? {
            Constant::Float(v) => *v,
            other => return Err(format!("annotation float is {other:?}")),
        })),
        'D' => ElementValue::Const(ConstValue::Double(match pool.get(r.u2()?)? {
            Constant::Double(v) => *v,
            other => return Err(format!("annotation double is {other:?}")),
        })),
        's' => ElementValue::Const(ConstValue::String(pool.utf8(r.u2()?)?)),
        'e' => {
            let type_descriptor = pool.utf8(r.u2()?)?;
            let name = pool.utf8(r.u2()?)?;
            ElementValue::Enum {
                type_descriptor,
                name,
            }
        }
        'c' => ElementValue::Class(pool.class_descriptor(r.u2()?)?),
        '@' => ElementValue::Annotation(Box::new(read_annotation(r, pool)?)),
        '[' => {
            let n = r.u2()?;
            let mut items = Vec::with_capacity(n as usize);
            for _ in 0..n {
                items.push(read_element_value(r, pool)?);
            }
            ElementValue::Array(items)
        }
        other => return Err(format!("unknown annotation element tag {other:?}")),
    })
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }
    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.at)
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self
            .at
            .checked_add(n)
            .ok_or_else(|| format!("offset {} overflows", self.at))?;
        if end > self.bytes.len() {
            return Err(format!(
                "truncated at byte {}: need {n}, have {}",
                self.at,
                self.bytes.len() - self.at
            ));
        }
        let out = &self.bytes[self.at..end];
        self.at = end;
        Ok(out)
    }
    fn u1(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u2(&mut self) -> Result<u16, String> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }
    fn u4(&mut self) -> Result<u32, String> {
        let b = self.take(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn u8(&mut self) -> Result<u64, String> {
        let b = self.take(8)?;
        Ok(u64::from_be_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }
    fn utf8(&mut self) -> Result<String, String> {
        let len = self.u2()? as usize;
        let bytes = self.take(len)?;
        // Modified UTF-8: NUL is encoded as C0 80, supplementary characters
        // as surrogate pairs. The common case is ordinary UTF-8.
        if !bytes.contains(&0xC0) && !bytes.contains(&0xED) {
            return String::from_utf8(bytes.to_vec()).map_err(|e| e.to_string());
        }
        let mut out = String::with_capacity(len);
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            if b == 0xC0 && bytes.get(i + 1) == Some(&0x80) {
                out.push('\0');
                i += 2;
            } else if (0xE0..=0xEF).contains(&b) && i + 2 < bytes.len() {
                let cp = ((b as u32 & 0x0F) << 12)
                    | ((bytes[i + 1] as u32 & 0x3F) << 6)
                    | (bytes[i + 2] as u32 & 0x3F);
                if (0xD800..=0xDBFF).contains(&cp)
                    && bytes.get(i + 3) == Some(&0xED)
                    && i + 5 < bytes.len()
                {
                    let low = (((bytes[i + 3] as u32) & 0x0F) << 12)
                        | ((bytes[i + 4] as u32 & 0x3F) << 6)
                        | (bytes[i + 5] as u32 & 0x3F);
                    let joined = 0x10000 + ((cp - 0xD800) << 10) + (low - 0xDC00);
                    if let Some(c) = char::from_u32(joined) {
                        out.push(c);
                    }
                    i += 6;
                } else {
                    if let Some(c) = char::from_u32(cp) {
                        out.push(c);
                    }
                    i += 3;
                }
            } else if (0xC0..=0xDF).contains(&b) && i + 1 < bytes.len() {
                let cp = ((b as u32 & 0x1F) << 6) | (bytes[i + 1] as u32 & 0x3F);
                if let Some(c) = char::from_u32(cp) {
                    out.push(c);
                }
                i += 2;
            } else {
                out.push(b as char);
                i += 1;
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A class compiled by javac 21 from `testdata/src/t/Test.java`:
    ///
    /// ```java
    /// @Mark("ruby")
    /// public class Test {
    ///     public static final int x = 7;
    ///     public void run() {
    ///         if (x < 0) throw new IllegalStateException("negative");
    ///     }
    /// }
    /// ```
    fn fixture() -> &'static [u8] {
        include_bytes!("../tests/data/Test.class")
    }

    #[test]
    fn parses_a_class() {
        let class = ClassFile::parse(fixture()).unwrap();
        assert_eq!(class.name, "t/Test");
        assert_eq!(class.super_name.as_deref(), Some("java/lang/Object"));
        assert_eq!(class.fields.len(), 1);
        assert_eq!(class.fields[0].name, "x");
        assert_eq!(class.fields[0].constant, Some(ConstValue::Int(7)));
        assert_eq!(class.methods.len(), 2);
        assert!(class.method("<init>", "()V").is_some());
        let run = class.method("run", "()V").unwrap();
        assert_eq!(run.code.as_ref().unwrap().code.last(), Some(&0xb1));
        let annotation = &class.annotations[0];
        assert_eq!(annotation.type_name(), "t/Mark");
        assert_eq!(annotation.string("value"), Some("ruby"));
    }

    #[test]
    fn rejects_truncation() {
        let bytes = fixture();
        for cut in [0, 4, 8, 20, bytes.len() - 1] {
            assert!(
                ClassFile::parse(&bytes[..cut]).is_err(),
                "accepted a class cut at {cut}"
            );
        }
    }

    #[test]
    fn rejects_junk() {
        assert!(ClassFile::parse(b"not a class").is_err());
        assert!(ClassFile::parse(&[0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 0]).is_err());
    }
}

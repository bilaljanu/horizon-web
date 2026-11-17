//! The Java library, Minecraft and loader methods Horizon implements.
//!
//! A native is a Rust function with a Java signature. Every method that is
//! visible to a mod is listed here or in [`crate::bridge`]; a class or method
//! that is not is an explicit "unimplemented Java method" error. Nothing is
//! skipped silently.

use crate::host::Host;
use crate::host::LogLevel;
use crate::runtime::{
    Fault, Kind, Object, Value, Vm, class_of_descriptor, java_double, java_float, parse_descriptor,
};
use std::collections::BTreeMap;

/// Methods `java/lang/Object` gives every class.
pub(crate) fn builtin_super(name: &str) -> Option<Option<&'static str>> {
    if let Some(super_name) = crate::bridge::builtin_super(name) {
        return Some(super_name);
    }
    Some(match name {
        "java/lang/Object" => None,
        "java/lang/String" => Some("java/lang/Object"),
        "java/lang/StringBuilder" | "java/lang/StringBuffer" => Some("java/lang/Object"),
        "java/lang/Number" => Some("java/lang/Object"),
        "java/lang/Throwable" => Some("java/lang/Object"),
        "java/lang/Exception" | "java/lang/Error" => Some("java/lang/Throwable"),
        "java/lang/RuntimeException" => Some("java/lang/Exception"),
        "java/lang/IllegalArgumentException"
        | "java/lang/IllegalStateException"
        | "java/lang/NullPointerException"
        | "java/lang/UnsupportedOperationException"
        | "java/lang/IndexOutOfBoundsException"
        | "java/lang/NumberFormatException"
        | "java/lang/ArithmeticException"
        | "java/lang/ClassCastException"
        | "java/lang/SecurityException"
        | "java/util/NoSuchElementException"
        | "java/util/ConcurrentModificationException" => Some("java/lang/RuntimeException"),
        "java/lang/ArrayIndexOutOfBoundsException" => Some("java/lang/IndexOutOfBoundsException"),
        "java/lang/NegativeArraySizeException" | "java/lang/ClassNotFoundException" => {
            Some("java/lang/RuntimeException")
        }
        "java/io/IOException" => Some("java/lang/Exception"),
        "java/io/UncheckedIOException" => Some("java/lang/RuntimeException"),
        "java/io/PrintStream" => Some("java/lang/Object"),
        "java/lang/Integer" | "java/lang/Long" | "java/lang/Short" | "java/lang/Byte"
        | "java/lang/Float" | "java/lang/Double" => Some("java/lang/Number"),
        "java/lang/Boolean" | "java/lang/Character" | "java/lang/Enum" | "java/lang/Class" => {
            Some("java/lang/Object")
        }
        "java/lang/Math" | "java/lang/StrictMath" | "java/lang/System" => Some("java/lang/Object"),
        "java/lang/CharSequence"
        | "java/lang/Comparable"
        | "java/lang/Runnable"
        | "java/lang/Iterable"
        | "java/lang/AutoCloseable"
        | "java/lang/Cloneable"
        | "java/io/Serializable"
        | "java/lang/annotation/Annotation" => Some("java/lang/Object"),
        "java/util/Objects"
        | "java/util/Arrays"
        | "java/util/Collections"
        | "java/util/Optional"
        | "java/util/Random"
        | "java/util/UUID"
        | "java/util/regex/Pattern"
        | "java/util/regex/Matcher" => Some("java/lang/Object"),
        "java/util/List" | "java/util/Collection" => Some("java/lang/Iterable"),
        "java/util/ArrayList"
        | "java/util/HashMap"
        | "java/util/HashSet"
        | "java/util/Iterator" => Some("java/lang/Object"),
        "java/util/Map" | "java/util/Set" => Some("java/lang/Object"),
        "java/util/function/Supplier"
        | "java/util/function/Consumer"
        | "java/util/function/BiConsumer"
        | "java/util/function/Function"
        | "java/util/function/BiFunction"
        | "java/util/function/Predicate"
        | "java/util/function/BiPredicate"
        | "java/util/function/UnaryOperator"
        | "java/util/function/BinaryOperator"
        | "java/util/function/BooleanSupplier"
        | "java/util/function/IntSupplier"
        | "java/util/function/LongSupplier"
        | "java/util/function/DoubleSupplier"
        | "java/util/function/IntFunction"
        | "java/util/function/ToIntFunction"
        | "java/util/function/ToLongFunction" => Some("java/lang/Object"),
        "org/slf4j/Logger" | "org/slf4j/LoggerFactory" => Some("java/lang/Object"),
        _ => return None,
    })
}

pub(crate) fn builtin_names() -> Vec<&'static str> {
    let mut names = crate::bridge::builtin_names();
    names.extend_from_slice(&[
        "java/lang/Object",
        "java/lang/String",
        "java/lang/StringBuilder",
        "java/lang/StringBuffer",
        "java/lang/Number",
        "java/lang/Integer",
        "java/lang/Long",
        "java/lang/Short",
        "java/lang/Byte",
        "java/lang/Float",
        "java/lang/Double",
        "java/lang/Boolean",
        "java/lang/Character",
        "java/lang/Math",
        "java/lang/StrictMath",
        "java/lang/System",
        "java/lang/Class",
        "java/lang/Enum",
        "java/lang/Throwable",
        "java/lang/Exception",
        "java/lang/Error",
        "java/lang/RuntimeException",
        "java/lang/IllegalArgumentException",
        "java/lang/IllegalStateException",
        "java/lang/NullPointerException",
        "java/lang/UnsupportedOperationException",
        "java/lang/IndexOutOfBoundsException",
        "java/lang/ArrayIndexOutOfBoundsException",
        "java/lang/NegativeArraySizeException",
        "java/lang/NumberFormatException",
        "java/lang/ArithmeticException",
        "java/lang/ClassCastException",
        "java/lang/ClassNotFoundException",
        "java/util/Objects",
        "java/util/Arrays",
        "java/util/Collections",
        "java/util/List",
        "java/util/ArrayList",
        "java/util/Collection",
        "java/util/Map",
        "java/util/HashMap",
        "java/util/Set",
        "java/util/HashSet",
        "java/util/Iterator",
        "java/util/Optional",
        "java/util/Random",
        "java/util/UUID",
        "java/util/function/Supplier",
        "java/util/function/Consumer",
        "java/util/function/Function",
        "java/util/function/Predicate",
        "java/util/function/BooleanSupplier",
        "java/lang/Runnable",
        "java/lang/Comparable",
        "java/lang/Iterable",
        "java/lang/CharSequence",
        "java/io/PrintStream",
        "java/io/IOException",
        "org/slf4j/Logger",
        "org/slf4j/LoggerFactory",
    ]);
    names
}

pub(crate) fn builtin_fields(name: &str) -> &'static [(&'static str, &'static str)] {
    match name {
        "java/lang/Enum" => &[("name", "Ljava/lang/String;"), ("ordinal", "I")],
        "java/lang/Throwable" => &[
            ("detailMessage", "Ljava/lang/String;"),
            ("cause", "Ljava/lang/Throwable;"),
        ],
        _ => crate::bridge::builtin_fields(name),
    }
}

pub(crate) fn builtin_interfaces(name: &str) -> Vec<String> {
    crate::bridge::builtin_interfaces(name)
}

/// Whether `call` knows this exact signature, without running it.
pub(crate) fn has_native(class: &str, name: &str, descriptor: &str) -> bool {
    if crate::bridge::has_native(class, name, descriptor) {
        return true;
    }
    let key = (class, name, descriptor);
    matches!(
        key,
        ("java/io/PrintStream", "println", "(Ljava/lang/String;)V")
            | ("java/lang/Class", "getName", "()Ljava/lang/String;")
            | ("java/lang/Object", "<init>", "()V")
            | ("java/lang/Object", "getClass", "()Ljava/lang/Class;")
            | ("java/lang/Object", "hashCode", "()I")
            | ("java/lang/Object", "equals", "(Ljava/lang/Object;)Z")
            | ("java/lang/Object", "toString", "()Ljava/lang/String;")
            | ("java/lang/String", "<init>", "()V")
            | ("java/lang/String", "<init>", "(Ljava/lang/String;)V")
            | ("java/lang/String", "length", "()I")
            | ("java/lang/String", "isEmpty", "()Z")
            | ("java/lang/String", "charAt", "(I)C")
            | ("java/lang/String", "equals", "(Ljava/lang/Object;)Z")
            | ("java/lang/String", "hashCode", "()I")
            | ("java/lang/String", "toString", "()Ljava/lang/String;")
            | ("java/lang/String", "substring", "(I)Ljava/lang/String;")
            | ("java/lang/String", "substring", "(II)Ljava/lang/String;")
            | ("java/lang/String", "indexOf", "(Ljava/lang/String;)I")
            | (
                "java/lang/String",
                "contains",
                "(Ljava/lang/CharSequence;)Z"
            )
            | ("java/lang/String", "startsWith", "(Ljava/lang/String;)Z")
            | ("java/lang/String", "endsWith", "(Ljava/lang/String;)Z")
            | ("java/lang/String", "toLowerCase", "()Ljava/lang/String;")
            | ("java/lang/String", "toUpperCase", "()Ljava/lang/String;")
            | ("java/lang/String", "trim", "()Ljava/lang/String;")
            | ("java/lang/String", "strip", "()Ljava/lang/String;")
            | (
                "java/lang/String",
                "split",
                "(Ljava/lang/String;)[Ljava/lang/String;"
            )
            | ("java/lang/String", "getBytes", "()[B")
            | ("java/lang/String", "compareTo", "(Ljava/lang/String;)I")
            | (
                "java/lang/String",
                "concat",
                "(Ljava/lang/String;)Ljava/lang/String;"
            )
            | ("java/lang/String", "isBlank", "()Z")
            | ("java/lang/String", "repeat", "(I)Ljava/lang/String;")
            | (
                "java/lang/String",
                "replace",
                "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;)Ljava/lang/String;"
            )
            | (
                "java/lang/String",
                "valueOf",
                "(Ljava/lang/Object;)Ljava/lang/String;"
            )
            | ("java/lang/String", "valueOf", "(I)Ljava/lang/String;")
            | ("java/lang/String", "valueOf", "(J)Ljava/lang/String;")
            | ("java/lang/String", "valueOf", "(Z)Ljava/lang/String;")
            | ("java/lang/String", "valueOf", "(C)Ljava/lang/String;")
            | (
                "java/lang/String",
                "format",
                "(Ljava/lang/String;[Ljava/lang/Object;)Ljava/lang/String;"
            )
            | ("java/lang/StringBuilder", "<init>", "()V")
            | ("java/lang/StringBuilder", "<init>", "(Ljava/lang/String;)V")
            | ("java/lang/StringBuilder", "<init>", "(I)V")
            | (
                "java/lang/StringBuilder",
                "append",
                "(Ljava/lang/String;)Ljava/lang/StringBuilder;"
            )
            | (
                "java/lang/StringBuilder",
                "append",
                "(Ljava/lang/Object;)Ljava/lang/StringBuilder;"
            )
            | (
                "java/lang/StringBuilder",
                "append",
                "(I)Ljava/lang/StringBuilder;"
            )
            | (
                "java/lang/StringBuilder",
                "append",
                "(J)Ljava/lang/StringBuilder;"
            )
            | (
                "java/lang/StringBuilder",
                "append",
                "(F)Ljava/lang/StringBuilder;"
            )
            | (
                "java/lang/StringBuilder",
                "append",
                "(D)Ljava/lang/StringBuilder;"
            )
            | (
                "java/lang/StringBuilder",
                "append",
                "(C)Ljava/lang/StringBuilder;"
            )
            | (
                "java/lang/StringBuilder",
                "append",
                "(Z)Ljava/lang/StringBuilder;"
            )
            | (
                "java/lang/StringBuilder",
                "toString",
                "()Ljava/lang/String;"
            )
            | ("java/lang/StringBuilder", "length", "()I")
            | (
                "java/lang/StringBuilder",
                "reverse",
                "()Ljava/lang/StringBuilder;"
            )
            | ("java/lang/Math", "abs", "(I)I")
            | ("java/lang/Math", "abs", "(J)J")
            | ("java/lang/Math", "abs", "(F)F")
            | ("java/lang/Math", "abs", "(D)D")
            | ("java/lang/Math", "max", "(II)I")
            | ("java/lang/Math", "max", "(JJ)J")
            | ("java/lang/Math", "max", "(FF)F")
            | ("java/lang/Math", "max", "(DD)D")
            | ("java/lang/Math", "min", "(II)I")
            | ("java/lang/Math", "min", "(JJ)J")
            | ("java/lang/Math", "min", "(FF)F")
            | ("java/lang/Math", "min", "(DD)D")
            | ("java/lang/Math", "sqrt", "(D)D")
            | ("java/lang/Math", "floor", "(D)D")
            | ("java/lang/Math", "ceil", "(D)D")
            | ("java/lang/Math", "round", "(F)I")
            | ("java/lang/Math", "round", "(D)J")
            | ("java/lang/Math", "pow", "(DD)D")
            | (
                "java/lang/System",
                "arraycopy",
                "(Ljava/lang/Object;ILjava/lang/Object;II)V"
            )
            | (
                "java/lang/System",
                "getProperty",
                "(Ljava/lang/String;)Ljava/lang/String;"
            )
            | ("java/lang/System", "lineSeparator", "()Ljava/lang/String;")
    )
}

/// A static field whose value is a constant object or a bridge handle.
pub(crate) fn static_value<H: Host>(
    vm: &mut Vm<H>,
    class: &str,
    name: &str,
    descriptor: &str,
) -> Option<Value> {
    if let Some(value) = crate::bridge::static_field(vm, class, name, descriptor) {
        return Some(value);
    }
    let value = match (class, name) {
        ("java/lang/System", "out" | "err") if descriptor == "Ljava/io/PrintStream;" => Value::obj(
            vm.alloc(Object {
                class: "java/io/PrintStream".into(),
                fields: Vec::new(),
                kind: Kind::PrintStream,
            })
            .ok()?,
        ),
        ("java/lang/Integer", "MAX_VALUE") => Value::Int(i32::MAX),
        ("java/lang/Integer", "MIN_VALUE") => Value::Int(i32::MIN),
        ("java/lang/Long", "MAX_VALUE") => Value::Long(i64::MAX),
        ("java/lang/Long", "MIN_VALUE") => Value::Long(i64::MIN),
        ("java/lang/Short", "MAX_VALUE") => Value::Int(i32::from(i16::MAX)),
        ("java/lang/Short", "MIN_VALUE") => Value::Int(i32::from(i16::MIN)),
        ("java/lang/Byte", "MAX_VALUE") => Value::Int(i32::from(i8::MAX)),
        ("java/lang/Byte", "MIN_VALUE") => Value::Int(i32::from(i8::MIN)),
        ("java/lang/Float", "MAX_VALUE") => Value::Float(f32::MAX),
        ("java/lang/Float", "MIN_VALUE") => Value::Float(f32::from_bits(1)),
        ("java/lang/Float", "NaN") => Value::Float(f32::NAN),
        ("java/lang/Float", "POSITIVE_INFINITY") => Value::Float(f32::INFINITY),
        ("java/lang/Float", "NEGATIVE_INFINITY") => Value::Float(f32::NEG_INFINITY),
        ("java/lang/Double", "MAX_VALUE") => Value::Double(f64::MAX),
        ("java/lang/Double", "MIN_VALUE") => Value::Double(f64::from_bits(1)),
        ("java/lang/Double", "NaN") => Value::Double(f64::NAN),
        ("java/lang/Double", "POSITIVE_INFINITY") => Value::Double(f64::INFINITY),
        ("java/lang/Double", "NEGATIVE_INFINITY") => Value::Double(f64::NEG_INFINITY),
        ("java/lang/Math", "PI") | ("java/lang/StrictMath", "PI") => {
            Value::Double(std::f64::consts::PI)
        }
        ("java/lang/Math", "E") | ("java/lang/StrictMath", "E") => {
            Value::Double(std::f64::consts::E)
        }
        ("java/lang/Boolean", "TRUE") => boxed(vm, Value::Int(1), "java/lang/Boolean")?,
        ("java/lang/Boolean", "FALSE") => boxed(vm, Value::Int(0), "java/lang/Boolean")?,
        ("java/util/Collections", "EMPTY_LIST") => list(vm, Vec::new())?,
        ("java/util/Collections", "EMPTY_SET") => set(vm, Vec::new())?,
        ("java/util/Collections", "EMPTY_MAP") => map(vm, Vec::new())?,
        _ => return None,
    };
    Some(value)
}

fn boxed<H: Host>(vm: &mut Vm<H>, value: Value, class: &str) -> Option<Value> {
    vm.alloc(Object {
        class: class.to_owned(),
        fields: Vec::new(),
        kind: Kind::Boxed(value),
    })
    .ok()
    .map(Value::obj)
}

fn list<H: Host>(vm: &mut Vm<H>, values: Vec<Value>) -> Option<Value> {
    vm.alloc(Object {
        class: "java/util/ArrayList".into(),
        fields: Vec::new(),
        kind: Kind::List(values),
    })
    .ok()
    .map(Value::obj)
}

fn set<H: Host>(vm: &mut Vm<H>, values: Vec<Value>) -> Option<Value> {
    vm.alloc(Object {
        class: "java/util/HashSet".into(),
        fields: Vec::new(),
        kind: Kind::Set(values),
    })
    .ok()
    .map(Value::obj)
}

fn map<H: Host>(vm: &mut Vm<H>, values: Vec<(Value, Value)>) -> Option<Value> {
    vm.alloc(Object {
        class: "java/util/HashMap".into(),
        fields: Vec::new(),
        kind: Kind::Map(values),
    })
    .ok()
    .map(Value::obj)
}

fn int(args: &[Value], index: usize) -> Result<i32, Fault> {
    match args.get(index) {
        Some(Value::Int(v)) => Ok(*v),
        Some(other) => Err(Fault::Vm(format!(
            "argument {index}: expected int, found {other:?}"
        ))),
        None => Err(Fault::Vm(format!("missing argument {index}"))),
    }
}

fn long(args: &[Value], index: usize) -> Result<i64, Fault> {
    match args.get(index) {
        Some(Value::Long(v)) => Ok(*v),
        Some(Value::Int(v)) => Ok(i64::from(*v)),
        Some(other) => Err(Fault::Vm(format!(
            "argument {index}: expected long, found {other:?}"
        ))),
        None => Err(Fault::Vm(format!("missing argument {index}"))),
    }
}

fn float(args: &[Value], index: usize) -> Result<f32, Fault> {
    match args.get(index) {
        Some(Value::Float(v)) => Ok(*v),
        Some(other) => Err(Fault::Vm(format!(
            "argument {index}: expected float, found {other:?}"
        ))),
        None => Err(Fault::Vm(format!("missing argument {index}"))),
    }
}

fn double(args: &[Value], index: usize) -> Result<f64, Fault> {
    match args.get(index) {
        Some(Value::Double(v)) => Ok(*v),
        Some(Value::Float(v)) => Ok(f64::from(*v)),
        Some(other) => Err(Fault::Vm(format!(
            "argument {index}: expected double, found {other:?}"
        ))),
        None => Err(Fault::Vm(format!("missing argument {index}"))),
    }
}

fn bool_arg(args: &[Value], index: usize) -> Result<bool, Fault> {
    Ok(int(args, index)? != 0)
}

fn reference(args: &[Value], index: usize) -> Result<Option<u32>, Fault> {
    match args.get(index) {
        Some(Value::Ref(value)) => Ok(*value),
        Some(other) => Err(Fault::Vm(format!(
            "argument {index}: expected reference, found {other:?}"
        ))),
        None => Err(Fault::Vm(format!("missing argument {index}"))),
    }
}

fn text<H: Host>(vm: &Vm<H>, args: &[Value], index: usize) -> Result<String, Fault> {
    let value = args
        .get(index)
        .copied()
        .ok_or_else(|| Fault::Vm(format!("missing argument {index}")))?;
    vm.string_of(value)
}

fn opt_text<H: Host>(vm: &Vm<H>, args: &[Value], index: usize) -> Result<Option<String>, Fault> {
    let value = args
        .get(index)
        .copied()
        .ok_or_else(|| Fault::Vm(format!("missing argument {index}")))?;
    vm.string_or_null(value)
}

fn receiver(args: &[Value]) -> Result<u32, Fault> {
    reference(args, 0)?.ok_or_else(|| Fault::Vm("null receiver".into()))
}

fn string_result<H: Host>(vm: &mut Vm<H>, text: String) -> Result<Value, Fault> {
    vm.make_string(&text)
}

fn log<H: Host>(vm: &mut Vm<H>, level: LogLevel, message: &str) {
    let mod_id = vm.mod_id.clone();
    vm.host.log(level, &mod_id, message);
}

/// Dispatch one native call. `None` means "not a native": the interpreter
/// resolves the method or reports it.
pub(crate) fn call<H: Host>(
    vm: &mut Vm<H>,
    class: &str,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Option<Value>, Fault> {
    if let Some(value) = crate::bridge::call(vm, class, name, descriptor, args)? {
        return Ok(Some(value));
    }
    // The dispatch helpers contain incomplete families. Only enter an exact
    // registered signature; an unknown overload must not return their default
    // void value and appear to have executed successfully.
    if !has_native(class, name, descriptor) {
        return Ok(None);
    }
    let value = match class {
        "java/lang/Object" => object(vm, name, descriptor, args)?,
        "java/lang/String" => string(vm, name, descriptor, args)?,
        "java/lang/StringBuilder" | "java/lang/StringBuffer" => {
            builder(vm, name, descriptor, args)?
        }
        "java/lang/Integer" => wrapper_int(vm, name, descriptor, args)?,
        "java/lang/Long" => wrapper_long(vm, name, descriptor, args)?,
        "java/lang/Short" | "java/lang/Byte" => wrapper_small(vm, class, name, descriptor, args)?,
        "java/lang/Float" => wrapper_float(vm, name, descriptor, args)?,
        "java/lang/Double" => wrapper_double(vm, name, descriptor, args)?,
        "java/lang/Boolean" => wrapper_bool(vm, name, descriptor, args)?,
        "java/lang/Character" => wrapper_char(vm, name, descriptor, args)?,
        "java/lang/Math" | "java/lang/StrictMath" => math(name, descriptor, args)?,
        "java/lang/System" => system(vm, name, descriptor, args)?,
        "java/lang/Class" => class_native(vm, name, descriptor, args)?,
        "java/lang/Enum" => enum_native(vm, name, descriptor, args)?,
        "java/lang/Throwable"
        | "java/lang/Exception"
        | "java/lang/Error"
        | "java/lang/RuntimeException"
        | "java/lang/IllegalArgumentException"
        | "java/lang/IllegalStateException"
        | "java/lang/NullPointerException"
        | "java/lang/UnsupportedOperationException"
        | "java/lang/IndexOutOfBoundsException"
        | "java/lang/ArrayIndexOutOfBoundsException"
        | "java/lang/NumberFormatException"
        | "java/lang/ArithmeticException"
        | "java/lang/ClassCastException"
        | "java/lang/NegativeArraySizeException"
        | "java/util/NoSuchElementException" => throwable(vm, name, descriptor, args)?,
        "java/util/Objects" => objects(vm, name, descriptor, args)?,
        "java/util/Arrays" => arrays(vm, name, descriptor, args)?,
        "java/util/Collections" => collections(vm, name, descriptor, args)?,
        "java/util/List" | "java/util/ArrayList" | "java/util/Collection" => {
            list_native(vm, name, descriptor, args)?
        }
        "java/util/Map" | "java/util/HashMap" => map_native(vm, name, descriptor, args)?,
        "java/util/Set" | "java/util/HashSet" => set_native(vm, name, descriptor, args)?,
        "java/util/Iterator" => iterator(vm, name, descriptor, args)?,
        "java/util/Optional" => optional(vm, name, descriptor, args)?,
        "java/util/Random" => random(vm, name, descriptor, args)?,
        "java/util/UUID" => uuid(vm, name, descriptor, args)?,
        "java/io/PrintStream" => print_stream(vm, name, descriptor, args)?,
        "org/slf4j/Logger" => logger(vm, name, descriptor, args)?,
        "org/slf4j/LoggerFactory" => logger_factory(vm, name, descriptor, args)?,
        _ => return Ok(None),
    };
    Ok(Some(value))
}

// ---- java.lang.Object --------------------------------------------------

fn object<H: Host>(
    vm: &mut Vm<H>,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Value, Fault> {
    match (name, descriptor) {
        ("<init>", "()V") => Ok(Value::Void),
        ("getClass", "()Ljava/lang/Class;") => {
            let class = vm.class_of(args[0])?;
            let reference = vm.alloc(Object {
                class: "java/lang/Class".into(),
                fields: Vec::new(),
                kind: Kind::Class(class),
            })?;
            Ok(Value::obj(reference))
        }
        ("hashCode", "()I") => Ok(Value::Int(match args[0] {
            Value::Ref(Some(reference)) => reference as i32,
            _ => 0,
        })),
        ("equals", "(Ljava/lang/Object;)Z") => {
            let left = args[0];
            let right = args[1];
            let equal = match (left.as_ref(), right.as_ref()) {
                (Some(None), Some(None)) => true,
                (Some(Some(a)), Some(Some(b))) => {
                    let left_object = vm.object(a)?;
                    let right_object = vm.object(b)?;
                    match (&left_object.kind, &right_object.kind) {
                        (Kind::String(a), Kind::String(b)) => a == b,
                        (Kind::Boxed(a), Kind::Boxed(b)) => a == b,
                        (
                            Kind::ResourceLocation {
                                namespace: an,
                                path: ap,
                            },
                            Kind::ResourceLocation {
                                namespace: bn,
                                path: bp,
                            },
                        ) => an == bn && ap == bp,
                        _ => a == b,
                    }
                }
                _ => false,
            };
            Ok(Value::boolean(equal))
        }
        ("toString", "()Ljava/lang/String;") => {
            let class = vm.class_of(args[0])?;
            let reference = receiver(args)?;
            let text = format!("{}@{}", class.replace('/', "."), format!("{reference:x}"));
            string_result(vm, text)
        }
        ("notify", "()V") | ("notifyAll", "()V") | ("wait", "()V") => Ok(Value::Void),
        _ => Ok(Value::Void),
    }
}

// ---- java.lang.String --------------------------------------------------

pub(crate) fn java_hash(text: &str) -> i32 {
    let mut hash = 0i32;
    for c in text.encode_utf16() {
        hash = hash.wrapping_mul(31).wrapping_add(i32::from(c));
    }
    hash
}

fn string<H: Host>(
    vm: &mut Vm<H>,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Value, Fault> {
    match (name, descriptor) {
        ("<init>", "()V") => {
            let reference = receiver(args)?;
            if let Kind::Plain = vm.object(reference)?.kind {
                vm.object_mut(reference)?.kind = Kind::String(String::new());
            }
            Ok(Value::Void)
        }
        ("<init>", "(Ljava/lang/String;)V") => {
            let value = text(vm, args, 1)?;
            let reference = receiver(args)?;
            vm.object_mut(reference)?.kind = Kind::String(value);
            Ok(Value::Void)
        }
        ("<init>", "([C)V") => {
            let Some(array) = reference(args, 1)? else {
                return Err(vm.throw("java/lang/NullPointerException", ""));
            };
            let chars = char_array(vm, array)?;
            let value: String = chars.into_iter().collect();
            let reference = receiver(args)?;
            vm.object_mut(reference)?.kind = Kind::String(value);
            Ok(Value::Void)
        }
        ("<init>", "([B)V") => {
            let Some(array) = reference(args, 1)? else {
                return Err(vm.throw("java/lang/NullPointerException", ""));
            };
            let bytes = byte_array(vm, array)?;
            let value = String::from_utf8_lossy(&bytes).into_owned();
            let reference = receiver(args)?;
            vm.object_mut(reference)?.kind = Kind::String(value);
            Ok(Value::Void)
        }
        ("length", "()I") => {
            let value = text(vm, args, 0)?;
            Ok(Value::Int(value.chars().count() as i32))
        }
        ("isEmpty", "()Z") => {
            let value = text(vm, args, 0)?;
            Ok(Value::boolean(value.is_empty()))
        }
        ("isBlank", "()Z") => {
            let value = text(vm, args, 0)?;
            Ok(Value::boolean(value.trim().is_empty()))
        }
        ("charAt", "(I)C") => {
            let value = text(vm, args, 0)?;
            let index = int(args, 1)?;
            let mut chars = value.chars();
            match usize::try_from(index)
                .ok()
                .and_then(|i| value.chars().nth(i))
            {
                Some(c) => Ok(Value::Int(c as i32)),
                None => {
                    let _ = &mut chars;
                    Err(vm.throw(
                        "java/lang/StringIndexOutOfBoundsException",
                        &format!("index {index}, length {}", value.chars().count()),
                    ))
                }
            }
        }
        ("equals", "(Ljava/lang/Object;)Z") => {
            let left = text(vm, args, 0)?;
            let equal = match reference(args, 1)? {
                None => false,
                Some(other) => match &vm.object(other)?.kind {
                    Kind::String(right) => &left == right,
                    _ => false,
                },
            };
            Ok(Value::boolean(equal))
        }
        ("equalsIgnoreCase", "(Ljava/lang/String;)Z") => {
            let left = text(vm, args, 0)?;
            let right = opt_text(vm, args, 1)?.unwrap_or_default();
            Ok(Value::boolean(left.eq_ignore_ascii_case(&right)))
        }
        ("hashCode", "()I") => {
            let value = text(vm, args, 0)?;
            Ok(Value::Int(java_hash(&value)))
        }
        ("toString", "()Ljava/lang/String;") => Ok(args[0]),
        ("intern", "()Ljava/lang/String;") => {
            let value = text(vm, args, 0)?;
            string_result(vm, value)
        }
        ("substring", "(I)Ljava/lang/String;") => {
            let value = text(vm, args, 0)?;
            let start = int(args, 1)?.max(0) as usize;
            if start > value.chars().count() {
                return Err(vm.throw(
                    "java/lang/StringIndexOutOfBoundsException",
                    &format!("begin {start}, length {}", value.chars().count()),
                ));
            }
            string_result(vm, value.chars().skip(start).collect())
        }
        ("substring", "(II)Ljava/lang/String;") => {
            let value = text(vm, args, 0)?;
            let start = int(args, 1)?.max(0) as usize;
            let end = int(args, 2)?.max(0) as usize;
            let chars: Vec<char> = value.chars().collect();
            if start > end || end > chars.len() {
                return Err(vm.throw(
                    "java/lang/StringIndexOutOfBoundsException",
                    &format!("begin {start}, end {end}, length {}", chars.len()),
                ));
            }
            string_result(vm, chars[start..end].iter().collect())
        }
        ("indexOf", "(Ljava/lang/String;)I") => {
            let value = text(vm, args, 0)?;
            let needle = text(vm, args, 1)?;
            Ok(Value::Int(match value.find(&needle) {
                Some(byte) => value[..byte].chars().count() as i32,
                None => -1,
            }))
        }
        ("lastIndexOf", "(Ljava/lang/String;)I") => {
            let value = text(vm, args, 0)?;
            let needle = text(vm, args, 1)?;
            Ok(Value::Int(match value.rfind(&needle) {
                Some(byte) => value[..byte].chars().count() as i32,
                None => -1,
            }))
        }
        ("contains", "(Ljava/lang/CharSequence;)Z") => {
            let value = text(vm, args, 0)?;
            let needle = opt_text(vm, args, 1)?.unwrap_or_default();
            Ok(Value::boolean(value.contains(&needle)))
        }
        ("startsWith", "(Ljava/lang/String;)Z") => {
            let value = text(vm, args, 0)?;
            Ok(Value::boolean(value.starts_with(&text(vm, args, 1)?)))
        }
        ("endsWith", "(Ljava/lang/String;)Z") => {
            let value = text(vm, args, 0)?;
            Ok(Value::boolean(value.ends_with(&text(vm, args, 1)?)))
        }
        ("toLowerCase", "()Ljava/lang/String;") => {
            string_result(vm, text(vm, args, 0)?.to_lowercase())
        }
        ("toUpperCase", "()Ljava/lang/String;") => {
            string_result(vm, text(vm, args, 0)?.to_uppercase())
        }
        ("trim", "()Ljava/lang/String;") | ("strip", "()Ljava/lang/String;") => {
            string_result(vm, text(vm, args, 0)?.trim().to_owned())
        }
        ("repeat", "(I)Ljava/lang/String;") => {
            let value = text(vm, args, 0)?;
            let n = int(args, 1)?;
            if n < 0 {
                return Err(vm.throw(
                    "java/lang/IllegalArgumentException",
                    &format!("count is negative: {n}"),
                ));
            }
            string_result(vm, value.repeat(n as usize))
        }
        ("split", "(Ljava/lang/String;)[Ljava/lang/String;") => {
            let value = text(vm, args, 0)?;
            let separator = text(vm, args, 1)?;
            let parts: Vec<Value> = if separator.is_empty() {
                value
                    .chars()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .into_iter()
                    .map(|part| string_result(vm, part))
                    .collect::<Result<_, _>>()?
            } else if separator.chars().all(|c| !"\\^$.|?*+()[]{}".contains(c)) {
                value
                    .split(&separator)
                    .map(|part| string_result(vm, part.to_owned()))
                    .collect::<Result<_, _>>()?
            } else {
                return Err(Fault::Vm(format!(
                    "String.split with a regular expression {separator:?} is not implemented; use a literal separator"
                )));
            };
            let mut parts = parts;
            while parts.len() > 1 && text(vm, &parts, parts.len() - 1)?.is_empty() {
                parts.pop();
            }
            let reference = vm.alloc(Object {
                class: "[Ljava/lang/String;".into(),
                fields: Vec::new(),
                kind: Kind::Array {
                    element: "Ljava/lang/String;".into(),
                    values: parts,
                },
            })?;
            Ok(Value::obj(reference))
        }
        ("getBytes", "()[B") => {
            let value = text(vm, args, 0)?;
            let bytes = value.into_bytes();
            let reference = vm.alloc(Object {
                class: "[B".into(),
                fields: Vec::new(),
                kind: Kind::Array {
                    element: "B".into(),
                    values: bytes
                        .into_iter()
                        .map(|b| Value::Int(i32::from(b as i8)))
                        .collect(),
                },
            })?;
            Ok(Value::obj(reference))
        }
        ("compareTo", "(Ljava/lang/String;)I") => {
            let left = text(vm, args, 0)?;
            let right = text(vm, args, 1)?;
            Ok(Value::Int(match left.cmp(&right) {
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            }))
        }
        ("concat", "(Ljava/lang/String;)Ljava/lang/String;") => {
            let mut value = text(vm, args, 0)?;
            value.push_str(&text(vm, args, 1)?);
            string_result(vm, value)
        }
        ("replace", "(CC)Ljava/lang/String;") => {
            let value = text(vm, args, 0)?;
            let from = char::from_u32(int(args, 1)? as u32).unwrap_or('\0');
            let to = char::from_u32(int(args, 2)? as u32).unwrap_or('\0');
            string_result(vm, value.replace(from, &to.to_string()))
        }
        ("replace", "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;)Ljava/lang/String;") => {
            let value = text(vm, args, 0)?;
            let from = text(vm, args, 1)?;
            let to = text(vm, args, 2)?;
            string_result(vm, value.replace(&from, &to))
        }
        ("valueOf", "(Ljava/lang/Object;)Ljava/lang/String;") => {
            let value = args[1];
            let text = vm.java_to_string(value)?;
            string_result(vm, text)
        }
        ("valueOf", "(I)Ljava/lang/String;") => string_result(vm, int(args, 1)?.to_string()),
        ("valueOf", "(J)Ljava/lang/String;") => string_result(vm, long(args, 1)?.to_string()),
        ("valueOf", "(Z)Ljava/lang/String;") => string_result(vm, bool_arg(args, 1)?.to_string()),
        ("valueOf", "(C)Ljava/lang/String;") => string_result(
            vm,
            char::from_u32(int(args, 1)? as u32)
                .unwrap_or('\0')
                .to_string(),
        ),
        ("valueOf", "(F)Ljava/lang/String;") => string_result(vm, java_float(float(args, 1)?)),
        ("valueOf", "(D)Ljava/lang/String;") => string_result(vm, java_double(double(args, 1)?)),
        ("format", "(Ljava/lang/String;[Ljava/lang/Object;)Ljava/lang/String;") => {
            let format = text(vm, args, 0)?;
            let Some(array) = reference(args, 1)? else {
                return Err(vm.throw("java/lang/NullPointerException", ""));
            };
            let values = array_values(vm, array)?;
            let text = self::format(&mut |value| vm.java_to_string(value), &format, &values)?;
            string_result(vm, text)
        }
        _ => Ok(Value::Void),
    }
}

fn char_array<H: Host>(vm: &Vm<H>, reference: u32) -> Result<Vec<char>, Fault> {
    let object = vm.object(reference)?;
    let Kind::Array { values, .. } = &object.kind else {
        return Err(Fault::Vm("expected a char array".into()));
    };
    Ok(values
        .iter()
        .filter_map(|v| v.as_int().and_then(|i| char::from_u32(i as u32)))
        .collect())
}

fn byte_array<H: Host>(vm: &Vm<H>, reference: u32) -> Result<Vec<u8>, Fault> {
    let object = vm.object(reference)?;
    let Kind::Array { values, .. } = &object.kind else {
        return Err(Fault::Vm("expected a byte array".into()));
    };
    Ok(values
        .iter()
        .filter_map(|v| v.as_int().map(|i| i as u8))
        .collect())
}

fn array_values<H: Host>(vm: &Vm<H>, reference: u32) -> Result<Vec<Value>, Fault> {
    let object = vm.object(reference)?;
    match &object.kind {
        Kind::Array { values, .. } => Ok(values.clone()),
        _ => Err(Fault::Vm("expected an array".into())),
    }
}

fn format<F: FnMut(Value) -> Result<String, Fault>>(
    to_string: &mut F,
    format: &str,
    values: &[Value],
) -> Result<String, Fault> {
    let mut out = String::new();
    let mut chars = format.chars().peekable();
    let mut next = 0usize;
    while let Some(ch) = chars.next() {
        if ch != '%' {
            out.push(ch);
            continue;
        }
        if chars.peek() == Some(&'%') {
            chars.next();
            out.push('%');
            continue;
        }
        let mut spec = String::new();
        while let Some(c) = chars.peek().copied() {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '+' || c == '<' {
                spec.push(c);
                chars.next();
            } else {
                break;
            }
        }
        let Some(conversion) = spec.pop() else {
            return Err(Fault::Vm("incomplete format specifier".into()));
        };
        let value = values.get(next).copied().unwrap_or(Value::Void);
        next += 1;
        let rendered = match conversion {
            's' | 'S' => to_string(value)?,
            'd' => value.as_int().unwrap_or(0).to_string(),
            'x' => format!("{:x}", value.as_int().unwrap_or(0)),
            'X' => format!("{:X}", value.as_int().unwrap_or(0)),
            'f' => {
                let precision: usize = spec
                    .rsplit_once('.')
                    .and_then(|(_, p)| p.parse().ok())
                    .unwrap_or(6);
                format!("{:.*}", precision, value.as_double().unwrap_or(0.0))
            }
            'b' => (value.as_int().unwrap_or(0) != 0).to_string(),
            'c' => char::from_u32(value.as_int().unwrap_or(0) as u32)
                .unwrap_or('\0')
                .to_string(),
            other => {
                return Err(Fault::Vm(format!(
                    "format conversion %{other} is not implemented"
                )));
            }
        };
        out.push_str(&rendered);
    }
    Ok(out)
}

// ---- StringBuilder -----------------------------------------------------

fn builder<H: Host>(
    vm: &mut Vm<H>,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Value, Fault> {
    match (name, descriptor) {
        ("<init>", "()V") => {
            let reference = receiver(args)?;
            vm.object_mut(reference)?.kind = Kind::StringBuilder(String::new());
            Ok(Value::Void)
        }
        ("<init>", "(Ljava/lang/String;)V") => {
            let value = opt_text(vm, args, 1)?.unwrap_or_default();
            let reference = receiver(args)?;
            vm.object_mut(reference)?.kind = Kind::StringBuilder(value);
            Ok(Value::Void)
        }
        ("<init>", "(I)V") => {
            let reference = receiver(args)?;
            vm.object_mut(reference)?.kind = Kind::StringBuilder(String::new());
            Ok(Value::Void)
        }
        ("append", _) => {
            let reference = receiver(args)?;
            let add = match descriptor {
                "(C)Ljava/lang/StringBuilder;" => char::from_u32(int(args, 1)? as u32)
                    .unwrap_or('\0')
                    .to_string(),
                "(Z)Ljava/lang/StringBuilder;" => bool_arg(args, 1)?.to_string(),
                "(I)Ljava/lang/StringBuilder;" => int(args, 1)?.to_string(),
                "(J)Ljava/lang/StringBuilder;" => long(args, 1)?.to_string(),
                "(F)Ljava/lang/StringBuilder;" => java_float(float(args, 1)?),
                "(D)Ljava/lang/StringBuilder;" => java_double(double(args, 1)?),
                _ => vm.java_to_string(args[1])?,
            };
            if let Kind::StringBuilder(buffer) = &mut vm.object_mut(reference)?.kind {
                buffer.push_str(&add);
            }
            Ok(args[0])
        }
        ("toString", "()Ljava/lang/String;") => {
            let value = match &vm.object(receiver(args)?)?.kind {
                Kind::StringBuilder(buffer) => buffer.clone(),
                _ => String::new(),
            };
            string_result(vm, value)
        }
        ("length", "()I") => {
            let value = match &vm.object(receiver(args)?)?.kind {
                Kind::StringBuilder(buffer) => buffer.chars().count(),
                _ => 0,
            };
            Ok(Value::Int(value as i32))
        }
        ("setLength", "(I)V") => {
            let n = int(args, 1)?.max(0) as usize;
            let reference = receiver(args)?;
            if let Kind::StringBuilder(buffer) = &mut vm.object_mut(reference)?.kind {
                let mut chars: Vec<char> = buffer.chars().take(n).collect();
                chars.resize(n, '\0');
                *buffer = chars.into_iter().collect();
            }
            Ok(Value::Void)
        }
        ("reverse", "()Ljava/lang/StringBuilder;") => {
            let reference = receiver(args)?;
            if let Kind::StringBuilder(buffer) = &mut vm.object_mut(reference)?.kind {
                *buffer = buffer.chars().rev().collect();
            }
            Ok(args[0])
        }
        ("charAt", "(I)C") => {
            let value = match &vm.object(receiver(args)?)?.kind {
                Kind::StringBuilder(buffer) => buffer.clone(),
                _ => String::new(),
            };
            let index = int(args, 1)? as usize;
            match value.chars().nth(index) {
                Some(c) => Ok(Value::Int(c as i32)),
                None => Err(vm.throw(
                    "java/lang/StringIndexOutOfBoundsException",
                    &format!("index {index}, length {}", value.chars().count()),
                )),
            }
        }
        ("insert", "(ILjava/lang/String;)Ljava/lang/StringBuilder;") => {
            let at = int(args, 1)?.max(0) as usize;
            let add = opt_text(vm, args, 2)?.unwrap_or_default();
            let reference = receiver(args)?;
            if let Kind::StringBuilder(buffer) = &mut vm.object_mut(reference)?.kind {
                let mut chars: Vec<char> = buffer.chars().collect();
                let at = at.min(chars.len());
                chars.splice(at..at, add.chars());
                *buffer = chars.into_iter().collect();
            }
            Ok(args[0])
        }
        _ => Ok(Value::Void),
    }
}

// ---- wrappers ----------------------------------------------------------

fn wrapper_int<H: Host>(
    vm: &mut Vm<H>,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Value, Fault> {
    match (name, descriptor) {
        ("<init>", "(I)V") => {
            let reference = receiver(args)?;
            vm.object_mut(reference)?.kind = Kind::Boxed(Value::Int(int(args, 1)?));
            Ok(Value::Void)
        }
        ("valueOf", "(I)Ljava/lang/Integer;") => {
            let value = Value::Int(int(args, 0)?);
            boxed(vm, value, "java/lang/Integer").ok_or_else(|| Fault::Vm("heap exhausted".into()))
        }
        ("parseInt", "(Ljava/lang/String;)I") => {
            let text = text(vm, args, 0)?;
            text.trim().parse::<i32>().map(Value::Int).map_err(|_| {
                vm.throw(
                    "java/lang/NumberFormatException",
                    &format!("For input string: {text:?}"),
                )
            })
        }
        ("parseInt", "(Ljava/lang/String;I)I") => {
            let text = text(vm, args, 0)?;
            let radix = int(args, 1)? as u32;
            i32::from_str_radix(text.trim(), radix)
                .map(Value::Int)
                .map_err(|_| {
                    vm.throw(
                        "java/lang/NumberFormatException",
                        &format!("For input string: {text:?}"),
                    )
                })
        }
        ("intValue", "()I") => Ok(boxed_value(vm, args[0], Value::Int(0))?),
        ("longValue", "()J") => Ok(Value::Long(i64::from(unbox_int(vm, args[0])?))),
        ("doubleValue", "()D") => Ok(Value::Double(f64::from(unbox_int(vm, args[0])?))),
        ("floatValue", "()F") => Ok(Value::Float(unbox_int(vm, args[0])? as f32)),
        ("shortValue", "()S") => Ok(Value::Int(unbox_int(vm, args[0])?)),
        ("byteValue", "()B") => Ok(Value::Int(unbox_int(vm, args[0])?)),
        ("toString", "()Ljava/lang/String;") => {
            string_result(vm, unbox_int(vm, args[0])?.to_string())
        }
        ("toString", "(I)Ljava/lang/String;") => string_result(vm, int(args, 0)?.to_string()),
        ("equals", "(Ljava/lang/Object;)Z") => {
            let equal = match reference(args, 1)? {
                Some(other) => match &vm.object(other)?.kind {
                    Kind::Boxed(Value::Int(value)) => *value == unbox_int(vm, args[0])?,
                    _ => false,
                },
                None => false,
            };
            Ok(Value::boolean(equal))
        }
        ("hashCode", "()I") => Ok(Value::Int(unbox_int(vm, args[0])?)),
        ("compare", "(II)I") => Ok(Value::Int(int(args, 0)?.cmp(&int(args, 1)?) as i32)),
        ("compareTo", "(Ljava/lang/Integer;)I") => Ok(Value::Int(
            unbox_int(vm, args[0])?.cmp(&unbox_int(vm, args[1])?) as i32,
        )),
        _ => Ok(Value::Void),
    }
}

fn wrapper_long<H: Host>(
    vm: &mut Vm<H>,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Value, Fault> {
    match (name, descriptor) {
        ("<init>", "(J)V") => {
            let reference = receiver(args)?;
            vm.object_mut(reference)?.kind = Kind::Boxed(Value::Long(long(args, 1)?));
            Ok(Value::Void)
        }
        ("valueOf", "(J)Ljava/lang/Long;") => {
            boxed(vm, Value::Long(long(args, 0)?), "java/lang/Long")
                .ok_or_else(|| Fault::Vm("heap exhausted".into()))
        }
        ("parseLong", "(Ljava/lang/String;)J") => {
            let text = text(vm, args, 0)?;
            text.trim().parse::<i64>().map(Value::Long).map_err(|_| {
                vm.throw(
                    "java/lang/NumberFormatException",
                    &format!("For input string: {text:?}"),
                )
            })
        }
        ("longValue", "()J") => Ok(boxed_value(vm, args[0], Value::Long(0))?),
        ("intValue", "()I") => Ok(Value::Int(unbox_long(vm, args[0])? as i32)),
        ("doubleValue", "()D") => Ok(Value::Double(unbox_long(vm, args[0])? as f64)),
        ("floatValue", "()F") => Ok(Value::Float(unbox_long(vm, args[0])? as f32)),
        ("toString", "()Ljava/lang/String;") => {
            string_result(vm, unbox_long(vm, args[0])?.to_string())
        }
        ("toString", "(J)Ljava/lang/String;") => string_result(vm, long(args, 0)?.to_string()),
        ("hashCode", "()I") => {
            let value = unbox_long(vm, args[0])?;
            Ok(Value::Int((value ^ (value >> 32)) as i32))
        }
        ("compare", "(JJ)I") => Ok(Value::Int(int(args, 0)?.cmp(&int(args, 1)?) as i32)),
        _ => Ok(Value::Void),
    }
}

fn wrapper_small<H: Host>(
    vm: &mut Vm<H>,
    class: &str,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Value, Fault> {
    match (name, descriptor) {
        ("<init>", "(S)V") | ("<init>", "(B)V") => {
            let reference = receiver(args)?;
            let value = int(args, 1)?;
            let value = if class.ends_with("Byte") {
                value as i8 as i32
            } else {
                value as i16 as i32
            };
            vm.object_mut(reference)?.kind = Kind::Boxed(Value::Int(value));
            Ok(Value::Void)
        }
        ("valueOf", _) => {
            let value = int(args, 0)?;
            let value = if class.ends_with("Byte") {
                value as i8 as i32
            } else {
                value as i16 as i32
            };
            boxed(vm, Value::Int(value), class).ok_or_else(|| Fault::Vm("heap exhausted".into()))
        }
        ("parseByte", "(Ljava/lang/String;)B") | ("parseShort", "(Ljava/lang/String;)S") => {
            let text = text(vm, args, 0)?;
            text.trim()
                .parse::<i32>()
                .map(Value::Int)
                .map_err(|_| vm.throw("java/lang/NumberFormatException", &text))
        }
        ("intValue", "()I") | ("byteValue", "()B") | ("shortValue", "()S") => {
            boxed_value(vm, args[0], Value::Int(0))
        }
        ("toString", "()Ljava/lang/String;") => {
            string_result(vm, unbox_int(vm, args[0])?.to_string())
        }
        ("hashCode", "()I") => Ok(Value::Int(unbox_int(vm, args[0])?)),
        _ => Ok(Value::Void),
    }
}

fn wrapper_float<H: Host>(
    vm: &mut Vm<H>,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Value, Fault> {
    match (name, descriptor) {
        ("<init>", "(F)V") => {
            let reference = receiver(args)?;
            vm.object_mut(reference)?.kind = Kind::Boxed(Value::Float(float(args, 1)?));
            Ok(Value::Void)
        }
        ("valueOf", "(F)Ljava/lang/Float;") => {
            boxed(vm, Value::Float(float(args, 0)?), "java/lang/Float")
                .ok_or_else(|| Fault::Vm("heap exhausted".into()))
        }
        ("parseFloat", "(Ljava/lang/String;)F") => {
            let text = text(vm, args, 0)?;
            text.trim()
                .parse::<f32>()
                .map(Value::Float)
                .map_err(|_| vm.throw("java/lang/NumberFormatException", &text))
        }
        ("floatValue", "()F") => Ok(boxed_value(vm, args[0], Value::Float(0.0))?),
        ("doubleValue", "()D") => Ok(Value::Double(f64::from(unbox_float(vm, args[0])?))),
        ("toString", "()Ljava/lang/String;") => {
            string_result(vm, java_float(unbox_float(vm, args[0])?))
        }
        _ => Ok(Value::Void),
    }
}

fn wrapper_double<H: Host>(
    vm: &mut Vm<H>,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Value, Fault> {
    match (name, descriptor) {
        ("<init>", "(D)V") => {
            let reference = receiver(args)?;
            vm.object_mut(reference)?.kind = Kind::Boxed(Value::Double(double(args, 1)?));
            Ok(Value::Void)
        }
        ("valueOf", "(D)Ljava/lang/Double;") => {
            boxed(vm, Value::Double(double(args, 0)?), "java/lang/Double")
                .ok_or_else(|| Fault::Vm("heap exhausted".into()))
        }
        ("parseDouble", "(Ljava/lang/String;)D") => {
            let text = text(vm, args, 0)?;
            text.trim()
                .parse::<f64>()
                .map(Value::Double)
                .map_err(|_| vm.throw("java/lang/NumberFormatException", &text))
        }
        ("doubleValue", "()D") => Ok(boxed_value(vm, args[0], Value::Double(0.0))?),
        ("floatValue", "()F") => Ok(Value::Float(unbox_double(vm, args[0])? as f32)),
        ("toString", "()Ljava/lang/String;") => {
            string_result(vm, java_double(unbox_double(vm, args[0])?))
        }
        _ => Ok(Value::Void),
    }
}

fn wrapper_bool<H: Host>(
    vm: &mut Vm<H>,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Value, Fault> {
    match (name, descriptor) {
        ("<init>", "(Z)V") => {
            let reference = receiver(args)?;
            vm.object_mut(reference)?.kind = Kind::Boxed(Value::Int(i32::from(bool_arg(args, 1)?)));
            Ok(Value::Void)
        }
        ("valueOf", "(Z)Ljava/lang/Boolean;") => boxed(
            vm,
            Value::Int(i32::from(bool_arg(args, 0)?)),
            "java/lang/Boolean",
        )
        .ok_or_else(|| Fault::Vm("heap exhausted".into())),
        ("parseBoolean", "(Ljava/lang/String;)Z") => Ok(Value::boolean(
            opt_text(vm, args, 0)?.is_some_and(|s| s.eq_ignore_ascii_case("true")),
        )),
        ("booleanValue", "()Z") => Ok(Value::boolean(unbox_int(vm, args[0])? != 0)),
        ("toString", _) => string_result(vm, (unbox_int(vm, args[0])? != 0).to_string()),
        ("hashCode", "()I") => Ok(Value::Int(if unbox_int(vm, args[0])? != 0 {
            1231
        } else {
            1237
        })),
        _ => Ok(Value::Void),
    }
}

fn wrapper_char<H: Host>(
    vm: &mut Vm<H>,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Value, Fault> {
    match (name, descriptor) {
        ("<init>", "(C)V") => {
            let reference = receiver(args)?;
            vm.object_mut(reference)?.kind = Kind::Boxed(Value::Int(int(args, 1)?));
            Ok(Value::Void)
        }
        ("valueOf", "(C)Ljava/lang/Character;") => {
            boxed(vm, Value::Int(int(args, 0)?), "java/lang/Character")
                .ok_or_else(|| Fault::Vm("heap exhausted".into()))
        }
        ("charValue", "()C") => boxed_value(vm, args[0], Value::Int(0)),
        ("isDigit", "(C)Z") => Ok(Value::boolean(
            char::from_u32(int(args, 0)? as u32).is_some_and(|c| c.is_ascii_digit()),
        )),
        ("isLetter", "(C)Z") => Ok(Value::boolean(
            char::from_u32(int(args, 0)? as u32).is_some_and(|c| c.is_alphabetic()),
        )),
        ("isWhitespace", "(C)Z") => Ok(Value::boolean(
            char::from_u32(int(args, 0)? as u32).is_some_and(|c| c.is_whitespace()),
        )),
        ("toLowerCase", "(C)C") => Ok(Value::Int(
            char::from_u32(int(args, 0)? as u32)
                .map(|c| c.to_ascii_lowercase() as i32)
                .unwrap_or(0),
        )),
        ("toUpperCase", "(C)C") => Ok(Value::Int(
            char::from_u32(int(args, 0)? as u32)
                .map(|c| c.to_ascii_uppercase() as i32)
                .unwrap_or(0),
        )),
        ("toString", "()Ljava/lang/String;") => string_result(
            vm,
            char::from_u32(unbox_int(vm, args[0])? as u32)
                .unwrap_or('\0')
                .to_string(),
        ),
        _ => Ok(Value::Void),
    }
}

fn boxed_value<H: Host>(vm: &Vm<H>, value: Value, default: Value) -> Result<Value, Fault> {
    let Some(reference) = value.as_ref().and_then(|r| r) else {
        return Ok(default);
    };
    match &vm.object(reference)?.kind {
        Kind::Boxed(value) => Ok(*value),
        _ => Ok(default),
    }
}

fn unbox_int<H: Host>(vm: &Vm<H>, value: Value) -> Result<i32, Fault> {
    match boxed_value(vm, value, Value::Int(0))? {
        Value::Int(v) => Ok(v),
        Value::Long(v) => Ok(v as i32),
        other => Err(Fault::Vm(format!("expected an integer, found {other:?}"))),
    }
}

fn unbox_long<H: Host>(vm: &Vm<H>, value: Value) -> Result<i64, Fault> {
    match boxed_value(vm, value, Value::Long(0))? {
        Value::Long(v) => Ok(v),
        Value::Int(v) => Ok(i64::from(v)),
        other => Err(Fault::Vm(format!("expected a long, found {other:?}"))),
    }
}

fn unbox_float<H: Host>(vm: &Vm<H>, value: Value) -> Result<f32, Fault> {
    match boxed_value(vm, value, Value::Float(0.0))? {
        Value::Float(v) => Ok(v),
        Value::Int(v) => Ok(v as f32),
        other => Err(Fault::Vm(format!("expected a float, found {other:?}"))),
    }
}

fn unbox_double<H: Host>(vm: &Vm<H>, value: Value) -> Result<f64, Fault> {
    match boxed_value(vm, value, Value::Double(0.0))? {
        Value::Double(v) => Ok(v),
        Value::Float(v) => Ok(f64::from(v)),
        Value::Int(v) => Ok(f64::from(v)),
        other => Err(Fault::Vm(format!("expected a double, found {other:?}"))),
    }
}

// ---- Math and System ---------------------------------------------------

fn math(name: &str, descriptor: &str, args: &[Value]) -> Result<Value, Fault> {
    Ok(match (name, descriptor) {
        ("abs", "(I)I") => Value::Int(int(args, 0)?.wrapping_abs()),
        ("abs", "(J)J") => Value::Long(long(args, 0)?.wrapping_abs()),
        ("abs", "(F)F") => Value::Float(float(args, 0)?.abs()),
        ("abs", "(D)D") => Value::Double(double(args, 0)?.abs()),
        ("max", "(II)I") => Value::Int(int(args, 0)?.max(int(args, 1)?)),
        ("max", "(JJ)J") => Value::Long(long(args, 0)?.max(long(args, 1)?)),
        ("max", "(FF)F") => Value::Float(float(args, 0)?.max(float(args, 1)?)),
        ("max", "(DD)D") => Value::Double(double(args, 0)?.max(double(args, 1)?)),
        ("min", "(II)I") => Value::Int(int(args, 0)?.min(int(args, 1)?)),
        ("min", "(JJ)J") => Value::Long(long(args, 0)?.min(long(args, 1)?)),
        ("min", "(FF)F") => Value::Float(float(args, 0)?.min(float(args, 1)?)),
        ("min", "(DD)D") => Value::Double(double(args, 0)?.min(double(args, 1)?)),
        ("sqrt", "(D)D") => Value::Double(double(args, 0)?.sqrt()),
        ("floor", "(D)D") => Value::Double(double(args, 0)?.floor()),
        ("ceil", "(D)D") => Value::Double(double(args, 0)?.ceil()),
        ("round", "(F)I") => Value::Int(double(args, 0)?.round() as i32),
        ("round", "(D)J") => Value::Long(double(args, 0)?.round() as i64),
        ("pow", "(DD)D") => Value::Double(double(args, 0)?.powf(double(args, 1)?)),
        ("sin", "(D)D") => Value::Double(double(args, 0)?.sin()),
        ("cos", "(D)D") => Value::Double(double(args, 0)?.cos()),
        ("tan", "(D)D") => Value::Double(double(args, 0)?.tan()),
        ("log", "(D)D") => Value::Double(double(args, 0)?.ln()),
        ("exp", "(D)D") => Value::Double(double(args, 0)?.exp()),
        ("clamp", "(III)I") => Value::Int(int(args, 0)?.clamp(int(args, 1)?, int(args, 2)?)),
        ("random", "()D") => {
            return Err(Fault::Vm("Math.random requires an entropy provider".into()));
        }
        ("signum", "(D)D") => Value::Double(double(args, 0)?.signum()),
        ("floorDiv", "(II)I") => Value::Int(int(args, 0)?.div_euclid(int(args, 1)?)),
        ("floorMod", "(II)I") => Value::Int(int(args, 0)?.rem_euclid(int(args, 1)?)),
        _ => Value::Void,
    })
}

fn system<H: Host>(
    vm: &mut Vm<H>,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Value, Fault> {
    match (name, descriptor) {
        ("currentTimeMillis", "()J") => Ok(Value::Long(now_millis())),
        ("nanoTime", "()J") => Ok(Value::Long(now_millis().saturating_mul(1_000_000))),
        ("arraycopy", "(Ljava/lang/Object;ILjava/lang/Object;II)V") => {
            let source = reference(args, 0)?
                .ok_or_else(|| vm.throw("java/lang/NullPointerException", ""))?;
            let source_pos = int(args, 1)?;
            let destination = reference(args, 2)?
                .ok_or_else(|| vm.throw("java/lang/NullPointerException", ""))?;
            let destination_pos = int(args, 3)?;
            let length = int(args, 4)?;
            if length < 0 || source_pos < 0 || destination_pos < 0 {
                return Err(vm.throw("java/lang/IndexOutOfBoundsException", "negative arraycopy"));
            }
            let values = {
                let object = vm.object(source)?;
                match &object.kind {
                    Kind::Array { values, .. } => values.clone(),
                    _ => {
                        return Err(
                            vm.throw("java/lang/ArrayStoreException", "source is not an array")
                        );
                    }
                }
            };
            let start = source_pos as usize;
            let end = start.saturating_add(length as usize);
            if end > values.len() {
                return Err(vm.throw(
                    "java/lang/ArrayIndexOutOfBoundsException",
                    &format!("arraycopy source {start}..{end} of {}", values.len()),
                ));
            }
            let copied = values[start..end].to_vec();
            let destination_len = {
                let object = vm.object(destination)?;
                match &object.kind {
                    Kind::Array { values, .. } => values.len(),
                    _ => {
                        return Err(vm.throw(
                            "java/lang/ArrayStoreException",
                            "destination is not an array",
                        ));
                    }
                }
            };
            let dstart = destination_pos as usize;
            if dstart + copied.len() > destination_len {
                return Err(vm.throw(
                    "java/lang/ArrayIndexOutOfBoundsException",
                    &format!(
                        "arraycopy destination {dstart}..{} of {destination_len}",
                        dstart + copied.len()
                    ),
                ));
            }
            if let Kind::Array { values, .. } = &mut vm.object_mut(destination)?.kind {
                values[dstart..dstart + copied.len()].copy_from_slice(&copied);
            }
            Ok(Value::Void)
        }
        ("getProperty", "(Ljava/lang/String;)Ljava/lang/String;") => {
            let key = text(vm, args, 0)?;
            let value = match key.as_str() {
                "java.version" => "25",
                "os.name" => "Linux",
                "java.awt.headless" => "true",
                "user.home" => "/",
                "line.separator" => "\n",
                "file.separator" => "/",
                _ => return string_result(vm, String::new()),
            };
            string_result(vm, value.to_owned())
        }
        ("getenv", "(Ljava/lang/String;)Ljava/lang/String;") | ("getenv", "()Ljava/util/Map;") => {
            Ok(Value::null())
        }
        ("lineSeparator", "()Ljava/lang/String;") => string_result(vm, "\n".into()),
        ("exit", "(I)V") => Ok(Value::Void),
        _ => Ok(Value::Void),
    }
}

fn now_millis() -> i64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }
    #[cfg(target_arch = "wasm32")]
    {
        0
    }
}

// ---- Class, Enum, Throwable -------------------------------------------

fn class_native<H: Host>(
    vm: &mut Vm<H>,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Value, Fault> {
    let this_class = match &vm.object(receiver(args)?)?.kind {
        Kind::Class(name) => name.clone(),
        _ => return Err(Fault::Vm("Class method on a non-Class".into())),
    };
    match (name, descriptor) {
        ("getName", "()Ljava/lang/String;") => string_result(vm, this_class.replace('/', ".")),
        ("getSimpleName", "()Ljava/lang/String;") => {
            let simple = this_class.rsplit(['/', '$']).next().unwrap_or(&this_class);
            string_result(vm, simple.to_owned())
        }
        ("isInterface", "()Z") => {
            let is_interface = vm
                .classes_get_file(&this_class)
                .map(|f| f.is_interface())
                .unwrap_or(false);
            Ok(Value::boolean(is_interface))
        }
        ("isAssignableFrom", "(Ljava/lang/Class;)Z") => {
            let other = match reference(args, 1)? {
                Some(reference) => match &vm.object(reference)?.kind {
                    Kind::Class(name) => name.clone(),
                    _ => return Ok(Value::boolean(false)),
                },
                None => return Err(vm.throw("java/lang/NullPointerException", "")),
            };
            Ok(Value::boolean(vm.is_assignable(&this_class, &other)))
        }
        ("isInstance", "(Ljava/lang/Object;)Z") => {
            if args[1].is_null() {
                return Ok(Value::boolean(false));
            }
            let actual = vm.class_of(args[1])?;
            Ok(Value::boolean(vm.is_assignable(&this_class, &actual)))
        }
        ("cast", "(Ljava/lang/Object;)Ljava/lang/Object;") => Ok(args[1]),
        ("getSuperclass", "()Ljava/lang/Class;") => {
            let super_name = vm.super_of_public(&this_class);
            match super_name {
                Some(super_name) => {
                    let reference = vm.alloc(Object {
                        class: "java/lang/Class".into(),
                        fields: Vec::new(),
                        kind: Kind::Class(super_name),
                    })?;
                    Ok(Value::obj(reference))
                }
                None => Ok(Value::null()),
            }
        }
        ("toString", "()Ljava/lang/String;") => {
            string_result(vm, format!("class {}", this_class.replace('/', ".")))
        }
        _ => Ok(Value::Void),
    }
}

fn enum_native<H: Host>(
    vm: &mut Vm<H>,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Value, Fault> {
    match (name, descriptor) {
        ("<init>", "(Ljava/lang/String;I)V") => {
            let reference = receiver(args)?;
            let label = text(vm, args, 1)?;
            let ordinal = int(args, 2)?;
            let label = vm.make_string(&label)?;
            vm.set_instance_field(reference, "name", "Ljava/lang/String;", label)?;
            vm.set_instance_field(reference, "ordinal", "I", Value::Int(ordinal))?;
            Ok(Value::Void)
        }
        ("name", "()Ljava/lang/String;") => {
            vm.get_instance_field(receiver(args)?, "name", "Ljava/lang/String;")
        }
        ("ordinal", "()I") => vm.get_instance_field(receiver(args)?, "ordinal", "I"),
        ("toString", "()Ljava/lang/String;") => {
            vm.get_instance_field(receiver(args)?, "name", "Ljava/lang/String;")
        }
        ("hashCode", "()I") => Ok(Value::Int(match args[0] {
            Value::Ref(Some(reference)) => reference as i32,
            _ => 0,
        })),
        ("equals", "(Ljava/lang/Object;)Z") => Ok(Value::boolean(args[0] == args[1])),
        ("compareTo", "(Ljava/lang/Enum;)I") => {
            let left = vm.get_instance_field(receiver(args)?, "ordinal", "I")?;
            let right = match reference(args, 1)? {
                Some(reference) => vm.get_instance_field(reference, "ordinal", "I")?,
                None => return Err(vm.throw("java/lang/NullPointerException", "")),
            };
            Ok(Value::Int(
                left.as_int().unwrap_or(0).cmp(&right.as_int().unwrap_or(0)) as i32,
            ))
        }
        ("valueOf", "(Ljava/lang/Class;Ljava/lang/String;)Ljava/lang/Enum;") => {
            let class = match reference(args, 0)? {
                Some(reference) => match &vm.object(reference)?.kind {
                    Kind::Class(name) => name.clone(),
                    _ => return Err(Fault::Vm("valueOf needs a class".into())),
                },
                None => return Err(vm.throw("java/lang/NullPointerException", "")),
            };
            let label = text(vm, args, 1)?;
            let descriptor = format!("L{class};");
            vm.get_static_raw(&class, &label, &descriptor)
                .ok_or_else(|| vm.throw("java/lang/IllegalArgumentException", &label))
        }
        _ => Ok(Value::Void),
    }
}

fn throwable<H: Host>(
    vm: &mut Vm<H>,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Value, Fault> {
    match (name, descriptor) {
        ("<init>", "()V") => {
            let reference = receiver(args)?;
            vm.object_mut(reference)?.kind = Kind::Throwable { message: None };
            Ok(Value::Void)
        }
        ("<init>", "(Ljava/lang/String;)V") => {
            let message = opt_text(vm, args, 1)?;
            let reference = receiver(args)?;
            vm.object_mut(reference)?.kind = Kind::Throwable {
                message: message.clone(),
            };
            if let Some(message) = message {
                let message = vm.make_string(&message)?;
                vm.set_instance_field(reference, "detailMessage", "Ljava/lang/String;", message)?;
            }
            Ok(Value::Void)
        }
        ("<init>", "(Ljava/lang/String;Ljava/lang/Throwable;)V") => {
            let message = opt_text(vm, args, 1)?;
            let reference = receiver(args)?;
            vm.object_mut(reference)?.kind = Kind::Throwable { message };
            vm.set_instance_field(reference, "cause", "Ljava/lang/Throwable;", args[2])?;
            Ok(Value::Void)
        }
        ("<init>", "(Ljava/lang/Throwable;)V") => {
            let reference = receiver(args)?;
            vm.object_mut(reference)?.kind = Kind::Throwable { message: None };
            vm.set_instance_field(reference, "cause", "Ljava/lang/Throwable;", args[1])?;
            Ok(Value::Void)
        }
        ("getMessage", "()Ljava/lang/String;") => {
            let message = match &vm.object(receiver(args)?)?.kind {
                Kind::Throwable { message } => message.clone(),
                _ => None,
            };
            match message {
                Some(message) => string_result(vm, message),
                None => Ok(Value::null()),
            }
        }
        ("getLocalizedMessage", "()Ljava/lang/String;") => {
            let message = match &vm.object(receiver(args)?)?.kind {
                Kind::Throwable { message } => message.clone(),
                _ => None,
            };
            match message {
                Some(message) => string_result(vm, message),
                None => Ok(Value::null()),
            }
        }
        ("getCause", "()Ljava/lang/Throwable;") => {
            vm.get_instance_field(receiver(args)?, "cause", "Ljava/lang/Throwable;")
        }
        ("initCause", "(Ljava/lang/Throwable;)Ljava/lang/Throwable;") => {
            let reference = receiver(args)?;
            vm.set_instance_field(reference, "cause", "Ljava/lang/Throwable;", args[1])?;
            Ok(args[0])
        }
        ("toString", "()Ljava/lang/String;") => {
            let reference = receiver(args)?;
            let class = vm.class_of(args[0])?.replace('/', ".");
            let message = match &vm.object(reference)?.kind {
                Kind::Throwable { message } => message.clone(),
                _ => None,
            };
            string_result(
                vm,
                match message {
                    Some(message) if !message.is_empty() => format!("{class}: {message}"),
                    _ => class,
                },
            )
        }
        ("printStackTrace", "()V") => {
            let reference = receiver(args)?;
            let message = vm.describe_throwable(reference);
            log(vm, LogLevel::Error, &message);
            Ok(Value::Void)
        }
        ("addSuppressed", "(Ljava/lang/Throwable;)V") => Ok(Value::Void),
        _ => Ok(Value::Void),
    }
}

// ---- Objects and Arrays ------------------------------------------------

fn objects<H: Host>(
    vm: &mut Vm<H>,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Value, Fault> {
    match (name, descriptor) {
        ("requireNonNull", "(Ljava/lang/Object;)Ljava/lang/Object;") => {
            if args[0].is_null() {
                Err(vm.throw("java/lang/NullPointerException", ""))
            } else {
                Ok(args[0])
            }
        }
        ("requireNonNull", "(Ljava/lang/Object;Ljava/lang/String;)Ljava/lang/Object;") => {
            if args[0].is_null() {
                let message = opt_text(vm, args, 1)?.unwrap_or_default();
                Err(vm.throw("java/lang/NullPointerException", &message))
            } else {
                Ok(args[0])
            }
        }
        ("requireNonNullElse", "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;") => {
            Ok(if args[0].is_null() { args[1] } else { args[0] })
        }
        ("equals", "(Ljava/lang/Object;Ljava/lang/Object;)Z") => {
            Ok(Value::boolean(equals_value(vm, args[0], args[1])?))
        }
        ("hashCode", "(Ljava/lang/Object;)I") => Ok(Value::Int(hash_value(vm, args[0])?)),
        ("toString", "(Ljava/lang/Object;)Ljava/lang/String;") => {
            let text = vm.java_to_string(args[0])?;
            string_result(vm, text)
        }
        ("toString", "(Ljava/lang/Object;Ljava/lang/String;)Ljava/lang/String;") => {
            if args[0].is_null() {
                string_result(vm, opt_text(vm, args, 1)?.unwrap_or_default())
            } else {
                let text = vm.java_to_string(args[0])?;
                string_result(vm, text)
            }
        }
        ("isNull", "(Ljava/lang/Object;)Z") => Ok(Value::boolean(args[0].is_null())),
        ("nonNull", "(Ljava/lang/Object;)Z") => Ok(Value::boolean(!args[0].is_null())),
        _ => Ok(Value::Void),
    }
}

fn equals_value<H: Host>(vm: &Vm<H>, a: Value, b: Value) -> Result<bool, Fault> {
    match (a.as_ref(), b.as_ref()) {
        (Some(None), Some(None)) => Ok(true),
        (Some(Some(ra)), Some(Some(rb))) => {
            let left = &vm.object(ra)?.kind;
            let right = &vm.object(rb)?.kind;
            Ok(match (left, right) {
                (Kind::String(a), Kind::String(b)) => a == b,
                (Kind::Boxed(a), Kind::Boxed(b)) => a == b,
                (
                    Kind::ResourceLocation {
                        namespace: a,
                        path: b,
                    },
                    Kind::ResourceLocation {
                        namespace: c,
                        path: d,
                    },
                ) => a == c && b == d,
                _ => ra == rb,
            })
        }
        _ => Ok(false),
    }
}

fn hash_value<H: Host>(vm: &mut Vm<H>, value: Value) -> Result<i32, Fault> {
    Ok(match value.as_ref() {
        Some(Some(reference)) => match &vm.object(reference)?.kind {
            Kind::String(text) => java_hash(text),
            Kind::Boxed(Value::Int(v)) => *v,
            Kind::Boxed(Value::Long(v)) => (*v ^ (*v >> 32)) as i32,
            Kind::ResourceLocation { namespace, path } => java_hash(&format!("{namespace}:{path}")),
            _ => reference as i32,
        },
        _ => 0,
    })
}

fn arrays<H: Host>(
    vm: &mut Vm<H>,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Value, Fault> {
    match (name, descriptor) {
        ("asList", "([Ljava/lang/Object;)Ljava/util/List;") => {
            let Some(array) = reference(args, 0)? else {
                return Err(vm.throw("java/lang/NullPointerException", ""));
            };
            let values = array_values(vm, array)?;
            list(vm, values).ok_or_else(|| Fault::Vm("heap exhausted".into()))
        }
        ("toString", _) => {
            let Some(array) = reference(args, 0)? else {
                return string_result(vm, "null".into());
            };
            let values = array_values(vm, array)?;
            let mut parts = Vec::with_capacity(values.len());
            for value in values {
                parts.push(vm.java_to_string(value)?);
            }
            string_result(vm, format!("[{}]", parts.join(", ")))
        }
        ("equals", _) => {
            let left = reference(args, 0)?
                .map(|r| array_values(vm, r))
                .transpose()?;
            let right = reference(args, 1)?
                .map(|r| array_values(vm, r))
                .transpose()?;
            Ok(Value::boolean(left == right))
        }
        ("hashCode", _) => {
            let Some(array) = reference(args, 0)? else {
                return Ok(Value::Int(0));
            };
            let values = array_values(vm, array)?;
            let mut hash = 1i32;
            for value in values {
                hash = hash.wrapping_mul(31).wrapping_add(hash_value(vm, value)?);
            }
            Ok(Value::Int(hash))
        }
        ("fill", _) => {
            let Some(array) = reference(args, 0)? else {
                return Err(vm.throw("java/lang/NullPointerException", ""));
            };
            let value = args[1];
            if let Kind::Array { values, .. } = &mut vm.object_mut(array)?.kind {
                values.fill(value);
            }
            Ok(Value::Void)
        }
        ("copyOf", _) => {
            let Some(array) = reference(args, 0)? else {
                return Err(vm.throw("java/lang/NullPointerException", ""));
            };
            let length = int(args, 1)?.max(0) as usize;
            let (element, mut values) = {
                let object = vm.object(array)?;
                match &object.kind {
                    Kind::Array { element, values } => (element.clone(), values.clone()),
                    _ => return Err(Fault::Vm("copyOf on a non-array".into())),
                }
            };
            values.resize(length, Value::Int(0));
            let reference = vm.alloc(Object {
                class: format!("[{element}"),
                fields: Vec::new(),
                kind: Kind::Array { element, values },
            })?;
            Ok(Value::obj(reference))
        }
        ("copyOfRange", _) => {
            let Some(array) = reference(args, 0)? else {
                return Err(vm.throw("java/lang/NullPointerException", ""));
            };
            let from = int(args, 1)?.max(0) as usize;
            let to = int(args, 2)?.max(0) as usize;
            let (element, values) = {
                let object = vm.object(array)?;
                match &object.kind {
                    Kind::Array { element, values } => (element.clone(), values.clone()),
                    _ => return Err(Fault::Vm("copyOfRange on a non-array".into())),
                }
            };
            if from > to || to > values.len() {
                return Err(vm.throw(
                    "java/lang/ArrayIndexOutOfBoundsException",
                    &format!("{from}..{to} of {}", values.len()),
                ));
            }
            let reference = vm.alloc(Object {
                class: format!("[{element}"),
                fields: Vec::new(),
                kind: Kind::Array {
                    element,
                    values: values[from..to].to_vec(),
                },
            })?;
            Ok(Value::obj(reference))
        }
        ("sort", _) => {
            let Some(array) = reference(args, 0)? else {
                return Err(vm.throw("java/lang/NullPointerException", ""));
            };
            let mut values = array_values(vm, array)?;
            values.sort_by(|a, b| a.as_int().unwrap_or(0).cmp(&b.as_int().unwrap_or(0)));
            if let Kind::Array { values: target, .. } = &mut vm.object_mut(array)?.kind {
                *target = values;
            }
            Ok(Value::Void)
        }
        _ => Ok(Value::Void),
    }
}

fn collections<H: Host>(
    vm: &mut Vm<H>,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Value, Fault> {
    match (name, descriptor) {
        ("emptyList", "()Ljava/util/List;") => {
            list(vm, Vec::new()).ok_or_else(|| Fault::Vm("heap exhausted".into()))
        }
        ("emptySet", "()Ljava/util/Set;") => {
            set(vm, Vec::new()).ok_or_else(|| Fault::Vm("heap exhausted".into()))
        }
        ("emptyMap", "()Ljava/util/Map;") => {
            map(vm, Vec::new()).ok_or_else(|| Fault::Vm("heap exhausted".into()))
        }
        ("singletonList", "(Ljava/lang/Object;)Ljava/util/List;") => {
            list(vm, vec![args[0]]).ok_or_else(|| Fault::Vm("heap exhausted".into()))
        }
        ("unmodifiableList", "(Ljava/util/List;)Ljava/util/List;") => Ok(args[0]),
        ("unmodifiableMap", "(Ljava/util/Map;)Ljava/util/Map;") => Ok(args[0]),
        ("unmodifiableSet", "(Ljava/util/Set;)Ljava/util/Set;") => Ok(args[0]),
        ("reverse", "(Ljava/util/List;)V") => {
            if let Some(reference) = reference(args, 0)? {
                if let Kind::List(values) = &mut vm.object_mut(reference)?.kind {
                    values.reverse();
                }
            }
            Ok(Value::Void)
        }
        _ => Ok(Value::Void),
    }
}

// ---- collections -------------------------------------------------------

fn list_native<H: Host>(
    vm: &mut Vm<H>,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Value, Fault> {
    match (name, descriptor) {
        ("<init>", "()V") | ("<init>", "(I)V") => {
            let reference = receiver(args)?;
            vm.object_mut(reference)?.kind = Kind::List(Vec::new());
            Ok(Value::Void)
        }
        ("<init>", "(Ljava/util/Collection;)V") => {
            let values = match reference(args, 1)? {
                Some(other) => match &vm.object(other)?.kind {
                    Kind::List(values) | Kind::Set(values) => values.clone(),
                    _ => Vec::new(),
                },
                None => return Err(vm.throw("java/lang/NullPointerException", "")),
            };
            let reference = receiver(args)?;
            vm.object_mut(reference)?.kind = Kind::List(values);
            Ok(Value::Void)
        }
        ("add", "(Ljava/lang/Object;)Z") => {
            let reference = receiver(args)?;
            if let Kind::List(values) = &mut vm.object_mut(reference)?.kind {
                values.push(args[1]);
                Ok(Value::boolean(true))
            } else {
                Ok(Value::boolean(false))
            }
        }
        ("add", "(ILjava/lang/Object;)V") => {
            let index = int(args, 1)?.max(0) as usize;
            let reference = receiver(args)?;
            if let Kind::List(values) = &mut vm.object_mut(reference)?.kind {
                let index = index.min(values.len());
                values.insert(index, args[2]);
            }
            Ok(Value::Void)
        }
        ("addAll", "(Ljava/util/Collection;)Z") => {
            let additions = match reference(args, 1)? {
                Some(other) => match &vm.object(other)?.kind {
                    Kind::List(values) | Kind::Set(values) => values.clone(),
                    _ => Vec::new(),
                },
                None => return Err(vm.throw("java/lang/NullPointerException", "")),
            };
            let reference = receiver(args)?;
            let mut changed = false;
            if let Kind::List(values) = &mut vm.object_mut(reference)?.kind {
                changed = !additions.is_empty();
                values.extend(additions);
            }
            Ok(Value::boolean(changed))
        }
        ("get", "(I)Ljava/lang/Object;") => {
            let values = list_values(vm, args[0])?;
            let index = int(args, 1)?;
            values.get(index as usize).copied().ok_or_else(|| {
                vm.throw(
                    "java/lang/IndexOutOfBoundsException",
                    &format!("Index {index} out of bounds for length {}", values.len()),
                )
            })
        }
        ("set", "(ILjava/lang/Object;)Ljava/lang/Object;") => {
            let index = int(args, 1)?;
            let reference = receiver(args)?;
            if let Kind::List(values) = &mut vm.object_mut(reference)?.kind {
                if index < 0 || index as usize >= values.len() {
                    let len = values.len();
                    return Err(vm.throw(
                        "java/lang/IndexOutOfBoundsException",
                        &format!("Index {index} out of bounds for length {len}"),
                    ));
                }
                let old = values[index as usize];
                values[index as usize] = args[2];
                Ok(old)
            } else {
                Ok(Value::null())
            }
        }
        ("size", "()I") => Ok(Value::Int(list_values(vm, args[0])?.len() as i32)),
        ("isEmpty", "()Z") => Ok(Value::boolean(list_values(vm, args[0])?.is_empty())),
        ("clear", "()V") => {
            let reference = receiver(args)?;
            if let Kind::List(values) = &mut vm.object_mut(reference)?.kind {
                values.clear();
            }
            Ok(Value::Void)
        }
        ("contains", "(Ljava/lang/Object;)Z") => {
            let values = list_values(vm, args[0])?;
            let mut found = false;
            for value in values {
                if equals_value(vm, value, args[1])? {
                    found = true;
                    break;
                }
            }
            Ok(Value::boolean(found))
        }
        ("indexOf", "(Ljava/lang/Object;)I") => {
            let values = list_values(vm, args[0])?;
            let mut found = -1;
            for (index, value) in values.iter().enumerate() {
                if equals_value(vm, *value, args[1])? {
                    found = index as i32;
                    break;
                }
            }
            Ok(Value::Int(found))
        }
        ("remove", "(I)Ljava/lang/Object;") => {
            let index = int(args, 1)?;
            let reference = receiver(args)?;
            if let Kind::List(values) = &mut vm.object_mut(reference)?.kind {
                if index < 0 || index as usize >= values.len() {
                    let len = values.len();
                    return Err(vm.throw(
                        "java/lang/IndexOutOfBoundsException",
                        &format!("Index {index} out of bounds for length {len}"),
                    ));
                }
                Ok(values.remove(index as usize))
            } else {
                Ok(Value::null())
            }
        }
        ("remove", "(Ljava/lang/Object;)Z") => {
            let values = list_values(vm, args[0])?;
            let mut position = None;
            for (index, value) in values.iter().enumerate() {
                if equals_value(vm, *value, args[1])? {
                    position = Some(index);
                    break;
                }
            }
            if let Some(position) = position
                && let Kind::List(values) = &mut vm.object_mut(receiver(args)?)?.kind
            {
                values.remove(position);
                return Ok(Value::boolean(true));
            }
            Ok(Value::boolean(false))
        }
        ("iterator", "()Ljava/util/Iterator;") => {
            let values = list_values(vm, args[0])?;
            iterator_object(vm, values)
        }
        ("toArray", "()[Ljava/lang/Object;") => {
            let values = list_values(vm, args[0])?;
            let reference = vm.alloc(Object {
                class: "[Ljava/lang/Object;".into(),
                fields: Vec::new(),
                kind: Kind::Array {
                    element: "Ljava/lang/Object;".into(),
                    values,
                },
            })?;
            Ok(Value::obj(reference))
        }
        ("stream", _) => Err(Fault::Vm("java.util.stream is not implemented yet".into())),
        _ => Ok(Value::Void),
    }
}

fn list_values<H: Host>(vm: &mut Vm<H>, value: Value) -> Result<Vec<Value>, Fault> {
    let Some(reference) = value.as_ref().and_then(|r| r) else {
        return Err(vm.throw("java/lang/NullPointerException", "").into());
    };
    match &vm.object(reference)?.kind {
        Kind::List(values) | Kind::Set(values) => Ok(values.clone()),
        _ => Err(Fault::Vm("expected a list".into())),
    }
}

fn iterator_object<H: Host>(vm: &mut Vm<H>, values: Vec<Value>) -> Result<Value, Fault> {
    let reference = vm.alloc(Object {
        class: "java/util/Iterator".into(),
        fields: Vec::new(),
        kind: Kind::Iterator { values, index: 0 },
    })?;
    Ok(Value::obj(reference))
}

fn iterator<H: Host>(
    vm: &mut Vm<H>,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Value, Fault> {
    match (name, descriptor) {
        ("hasNext", "()Z") => {
            let reference = receiver(args)?;
            let has = match &vm.object(reference)?.kind {
                Kind::Iterator { values, index } => *index < values.len(),
                _ => false,
            };
            Ok(Value::boolean(has))
        }
        ("next", "()Ljava/lang/Object;") => {
            let reference = receiver(args)?;
            let value = match &mut vm.object_mut(reference)?.kind {
                Kind::Iterator { values, index } => {
                    let value = values.get(*index).copied();
                    if value.is_some() {
                        *index += 1;
                    }
                    value
                }
                _ => None,
            };
            match value {
                Some(value) => Ok(value),
                None => Err(vm.throw("java/util/NoSuchElementException", "")),
            }
        }
        _ => Ok(Value::Void),
    }
}

fn map_native<H: Host>(
    vm: &mut Vm<H>,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Value, Fault> {
    match (name, descriptor) {
        ("<init>", "()V") | ("<init>", "(I)V") => {
            let reference = receiver(args)?;
            vm.object_mut(reference)?.kind = Kind::Map(Vec::new());
            Ok(Value::Void)
        }
        ("put", "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;") => {
            let reference = receiver(args)?;
            let key = args[1];
            let value = args[2];
            let snapshot = map_values(vm, args[0])?;
            let mut found = None;
            for (index, (existing, _)) in snapshot.iter().enumerate() {
                if equals_value(vm, *existing, key)? {
                    found = Some(index);
                    break;
                }
            }
            if let Kind::Map(entries) = &mut vm.object_mut(reference)?.kind {
                if let Some(index) = found {
                    return Ok(std::mem::replace(&mut entries[index].1, value));
                }
                entries.push((key, value));
            }
            Ok(Value::null())
        }
        ("get", "(Ljava/lang/Object;)Ljava/lang/Object;") => {
            let entries = map_values(vm, args[0])?;
            for (key, value) in entries {
                if equals_value(vm, key, args[1])? {
                    return Ok(value);
                }
            }
            Ok(Value::null())
        }
        ("getOrDefault", "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;") => {
            let entries = map_values(vm, args[0])?;
            for (key, value) in entries {
                if equals_value(vm, key, args[1])? {
                    return Ok(value);
                }
            }
            Ok(args[2])
        }
        ("containsKey", "(Ljava/lang/Object;)Z") => {
            let entries = map_values(vm, args[0])?;
            for (key, _) in entries {
                if equals_value(vm, key, args[1])? {
                    return Ok(Value::boolean(true));
                }
            }
            Ok(Value::boolean(false))
        }
        ("containsValue", "(Ljava/lang/Object;)Z") => {
            let entries = map_values(vm, args[0])?;
            for (_, value) in entries {
                if equals_value(vm, value, args[1])? {
                    return Ok(Value::boolean(true));
                }
            }
            Ok(Value::boolean(false))
        }
        ("remove", "(Ljava/lang/Object;)Ljava/lang/Object;") => {
            let reference = receiver(args)?;
            let mut index = None;
            let key = args[1];
            if let Kind::Map(entries) = &vm.object(reference)?.kind {
                for (position, (existing, _)) in entries.iter().enumerate() {
                    if equals_value(vm, *existing, key)? {
                        index = Some(position);
                        break;
                    }
                }
            }
            match index {
                Some(position) => {
                    if let Kind::Map(entries) = &mut vm.object_mut(reference)?.kind {
                        Ok(entries.remove(position).1)
                    } else {
                        Ok(Value::null())
                    }
                }
                None => Ok(Value::null()),
            }
        }
        ("size", "()I") => Ok(Value::Int(map_values(vm, args[0])?.len() as i32)),
        ("isEmpty", "()Z") => Ok(Value::boolean(map_values(vm, args[0])?.is_empty())),
        ("clear", "()V") => {
            let reference = receiver(args)?;
            if let Kind::Map(entries) = &mut vm.object_mut(reference)?.kind {
                entries.clear();
            }
            Ok(Value::Void)
        }
        ("keySet", "()Ljava/util/Set;") => {
            let entries = map_values(vm, args[0])?;
            set(vm, entries.into_iter().map(|(key, _)| key).collect())
                .ok_or_else(|| Fault::Vm("heap exhausted".into()))
        }
        ("values", "()Ljava/util/Collection;") => {
            let entries = map_values(vm, args[0])?;
            list(vm, entries.into_iter().map(|(_, value)| value).collect())
                .ok_or_else(|| Fault::Vm("heap exhausted".into()))
        }
        _ => Ok(Value::Void),
    }
}

fn map_values<H: Host>(vm: &mut Vm<H>, value: Value) -> Result<Vec<(Value, Value)>, Fault> {
    let Some(reference) = value.as_ref().and_then(|r| r) else {
        return Err(vm.throw("java/lang/NullPointerException", "").into());
    };
    match &vm.object(reference)?.kind {
        Kind::Map(entries) => Ok(entries.clone()),
        _ => Err(Fault::Vm("expected a map".into())),
    }
}

fn set_native<H: Host>(
    vm: &mut Vm<H>,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Value, Fault> {
    match (name, descriptor) {
        ("<init>", "()V") | ("<init>", "(I)V") => {
            let reference = receiver(args)?;
            vm.object_mut(reference)?.kind = Kind::Set(Vec::new());
            Ok(Value::Void)
        }
        ("add", "(Ljava/lang/Object;)Z") => {
            let reference = receiver(args)?;
            let exists = {
                let object = vm.object(reference)?;
                match &object.kind {
                    Kind::Set(values) => {
                        let mut found = false;
                        for value in values {
                            if equals_value(vm, *value, args[1])? {
                                found = true;
                                break;
                            }
                        }
                        found
                    }
                    _ => false,
                }
            };
            if !exists && let Kind::Set(values) = &mut vm.object_mut(reference)?.kind {
                values.push(args[1]);
            }
            Ok(Value::boolean(!exists))
        }
        ("contains", "(Ljava/lang/Object;)Z") => {
            let values = list_values(vm, args[0])?;
            let mut found = false;
            for value in values {
                if equals_value(vm, value, args[1])? {
                    found = true;
                    break;
                }
            }
            Ok(Value::boolean(found))
        }
        ("size", "()I") => Ok(Value::Int(list_values(vm, args[0])?.len() as i32)),
        ("isEmpty", "()Z") => Ok(Value::boolean(list_values(vm, args[0])?.is_empty())),
        ("iterator", "()Ljava/util/Iterator;") => {
            let values = list_values(vm, args[0])?;
            iterator_object(vm, values)
        }
        _ => Ok(Value::Void),
    }
}

fn optional<H: Host>(
    vm: &mut Vm<H>,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Value, Fault> {
    match (name, descriptor) {
        ("of", "(Ljava/lang/Object;)Ljava/util/Optional;") => {
            if args[0].is_null() {
                return Err(vm.throw("java/lang/NullPointerException", ""));
            }
            optional_value(vm, Some(args[0]))
        }
        ("ofNullable", "(Ljava/lang/Object;)Ljava/util/Optional;") => optional_value(
            vm,
            if args[0].is_null() {
                None
            } else {
                Some(args[0])
            },
        ),
        ("empty", "()Ljava/util/Optional;") => optional_value(vm, None),
        ("isPresent", "()Z") => Ok(Value::boolean(optional_inner(vm, args[0])?.is_some())),
        ("isEmpty", "()Z") => Ok(Value::boolean(optional_inner(vm, args[0])?.is_none())),
        ("get", "()Ljava/lang/Object;") => match optional_inner(vm, args[0])? {
            Some(value) => Ok(value),
            None => Err(vm.throw("java/util/NoSuchElementException", "No value present")),
        },
        ("orElse", "(Ljava/lang/Object;)Ljava/lang/Object;") => {
            Ok(optional_inner(vm, args[0])?.unwrap_or(args[1]))
        }
        ("orElseGet", "(Ljava/util/function/Supplier;)Ljava/lang/Object;") => {
            match optional_inner(vm, args[0])? {
                Some(value) => Ok(value),
                None => vm.call_ref(args[1], "get", "()Ljava/lang/Object;", Vec::new()),
            }
        }
        ("ifPresent", "(Ljava/util/function/Consumer;)V") => {
            if let Some(value) = optional_inner(vm, args[0])? {
                vm.call_ref(args[1], "accept", "(Ljava/lang/Object;)V", vec![value])?;
            }
            Ok(Value::Void)
        }
        ("map", "(Ljava/util/function/Function;)Ljava/util/Optional;") => {
            match optional_inner(vm, args[0])? {
                Some(value) => {
                    let mapped = vm.call_ref(
                        args[1],
                        "apply",
                        "(Ljava/lang/Object;)Ljava/lang/Object;",
                        vec![value],
                    )?;
                    optional_value(vm, if mapped.is_null() { None } else { Some(mapped) })
                }
                None => optional_value(vm, None),
            }
        }
        ("toString", "()Ljava/lang/String;") => match optional_inner(vm, args[0])? {
            Some(value) => {
                let text = vm.java_to_string(value)?;
                string_result(vm, format!("Optional[{text}]"))
            }
            None => string_result(vm, "Optional.empty".into()),
        },
        _ => Ok(Value::Void),
    }
}

fn optional_value<H: Host>(vm: &mut Vm<H>, value: Option<Value>) -> Result<Value, Fault> {
    let reference = vm.alloc(Object {
        class: "java/util/Optional".into(),
        fields: Vec::new(),
        kind: Kind::Optional(value),
    })?;
    Ok(Value::obj(reference))
}

fn optional_inner<H: Host>(vm: &mut Vm<H>, value: Value) -> Result<Option<Value>, Fault> {
    let Some(reference) = value.as_ref().and_then(|r| r) else {
        return Err(vm.throw("java/lang/NullPointerException", "").into());
    };
    match &vm.object(reference)?.kind {
        Kind::Optional(value) => Ok(*value),
        _ => Err(Fault::Vm("expected an Optional".into())),
    }
}

// ---- Random and UUID ---------------------------------------------------

fn random<H: Host>(
    vm: &mut Vm<H>,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Value, Fault> {
    let reference = receiver(args)?;
    if name == "<init>" {
        let seed = if descriptor == "(J)V" {
            long(args, 1)? as u64
        } else {
            0x5DEECE66D
        };
        if let Kind::Random(state) = &mut vm.object_mut(reference)?.kind {
            *state = seed ^ 0x5DEECE66D;
        }
        vm.object_mut(reference)?.kind = Kind::Random(seed ^ 0x5DEECE66D);
        return Ok(Value::Void);
    }
    let mut state = match &vm.object(reference)?.kind {
        Kind::Random(state) => *state,
        _ => 0x5DEECE66D,
    };
    if state == 0 {
        state = 0x5DEECE66D;
    }
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let value = match (name, descriptor) {
        ("nextInt", "()I") => Value::Int(next() as i32),
        ("nextInt", "(I)I") => {
            let bound = int(args, 1)?.max(1) as u64;
            Value::Int((next() % bound) as i32)
        }
        ("nextLong", "()J") => Value::Long(next() as i64),
        ("nextDouble", "()D") => Value::Double((next() >> 11) as f64 / (1u64 << 53) as f64),
        ("nextFloat", "()F") => Value::Float((next() >> 40) as f32 / (1u64 << 24) as f32),
        ("nextBoolean", "()Z") => Value::boolean(next() & 1 == 0),
        ("setSeed", "(J)V") => {
            state = long(args, 1)? as u64 ^ 0x5DEECE66D;
            Value::Void
        }
        _ => Value::Void,
    };
    if let Kind::Random(slot) = &mut vm.object_mut(reference)?.kind {
        *slot = state;
    }
    Ok(value)
}

fn uuid<H: Host>(
    vm: &mut Vm<H>,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Value, Fault> {
    match (name, descriptor) {
        ("randomUUID", "()Ljava/util/UUID;") => Err(Fault::Vm(
            "java/util/UUID.randomUUID requires a secure entropy provider".into(),
        )),
        ("fromString", "(Ljava/lang/String;)Ljava/util/UUID;") => {
            let text = text(vm, args, 0)?;
            let parsed = parse_uuid(&text)
                .ok_or_else(|| vm.throw("java/lang/IllegalArgumentException", &text))?;
            uuid_value(vm, parsed)
        }
        ("toString", "()Ljava/lang/String;") => {
            let value = match &vm.object(receiver(args)?)?.kind {
                Kind::Uuid(value) => *value,
                _ => 0,
            };
            string_result(vm, format_uuid(value))
        }
        ("equals", "(Ljava/lang/Object;)Z") => {
            let left = match &vm.object(receiver(args)?)?.kind {
                Kind::Uuid(value) => *value,
                _ => 0,
            };
            let right = match reference(args, 1)?.and_then(|r| match &vm.object(r) {
                Ok(object) => match &object.kind {
                    Kind::Uuid(value) => Some(*value),
                    _ => None,
                },
                Err(_) => None,
            }) {
                Some(value) => value,
                None => return Ok(Value::boolean(false)),
            };
            Ok(Value::boolean(left == right))
        }
        ("hashCode", "()I") => {
            let value = match &vm.object(receiver(args)?)?.kind {
                Kind::Uuid(value) => *value,
                _ => 0,
            };
            Ok(Value::Int((value >> 64) as i32 ^ (value as i32)))
        }
        ("getMostSignificantBits", "()J") => {
            let value = match &vm.object(receiver(args)?)?.kind {
                Kind::Uuid(value) => *value,
                _ => 0,
            };
            Ok(Value::Long((value >> 64) as i64))
        }
        ("getLeastSignificantBits", "()J") => {
            let value = match &vm.object(receiver(args)?)?.kind {
                Kind::Uuid(value) => *value,
                _ => 0,
            };
            Ok(Value::Long(value as i64))
        }
        _ => Ok(Value::Void),
    }
}

fn uuid_value<H: Host>(vm: &mut Vm<H>, value: u128) -> Result<Value, Fault> {
    let reference = vm.alloc(Object {
        class: "java/util/UUID".into(),
        fields: Vec::new(),
        kind: Kind::Uuid(value),
    })?;
    Ok(Value::obj(reference))
}

fn parse_uuid(text: &str) -> Option<u128> {
    let hex: String = text.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 {
        return None;
    }
    u128::from_str_radix(&hex, 16).ok()
}

fn format_uuid(value: u128) -> String {
    let hex = format!("{value:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

// ---- stdout and logging ------------------------------------------------

fn print_stream<H: Host>(
    vm: &mut Vm<H>,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Value, Fault> {
    match name {
        "println" | "print" | "printf" => {
            let message = if descriptor == "()V" {
                String::new()
            } else if name == "printf" {
                let format = opt_text(vm, args, 1)?.unwrap_or_default();
                let values = match reference(args, 2)? {
                    Some(array) => array_values(vm, array)?,
                    None => Vec::new(),
                };
                self::format(&mut |value| vm.java_to_string(value), &format, &values)?
            } else {
                vm.java_to_string(args[1])?
            };
            log(vm, LogLevel::Info, &message);
            Ok(Value::Void)
        }
        "flush" | "close" => Ok(Value::Void),
        _ => Ok(Value::Void),
    }
}

fn logger<H: Host>(
    vm: &mut Vm<H>,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<Value, Fault> {
    let level = match name {
        "error" => LogLevel::Error,
        "warn" => LogLevel::Warn,
        "info" => LogLevel::Info,
        "debug" => LogLevel::Debug,
        "trace" => LogLevel::Trace,
        "isErrorEnabled" => return Ok(Value::boolean(true)),
        "isWarnEnabled" => return Ok(Value::boolean(true)),
        "isInfoEnabled" => return Ok(Value::boolean(true)),
        "isDebugEnabled" => return Ok(Value::boolean(true)),
        "isTraceEnabled" => return Ok(Value::boolean(true)),
        _ => return Ok(Value::Void),
    };
    let message = if descriptor.ends_with("[Ljava/lang/Object;)V")
        && descriptor.starts_with("(Ljava/lang/String;")
    {
        let format = opt_text(vm, args, 1)?.unwrap_or_default();
        let values = match reference(args, 2)? {
            Some(array) => array_values(vm, array)?,
            None => Vec::new(),
        };
        self::format(&mut |value| vm.java_to_string(value), &format, &values)?
    } else if descriptor == "(Ljava/lang/String;Ljava/lang/Throwable;)V" {
        let format = opt_text(vm, args, 1)?.unwrap_or_default();
        let throwable = match reference(args, 2)? {
            Some(reference) => vm.describe_throwable(reference),
            None => String::new(),
        };
        format!("{format}: {throwable}")
    } else if descriptor == "()V" {
        String::new()
    } else {
        vm.java_to_string(args[1])?
    };
    log(vm, level, &message);
    Ok(Value::Void)
}

fn logger_factory<H: Host>(
    vm: &mut Vm<H>,
    name: &str,
    _descriptor: &str,
    _args: &[Value],
) -> Result<Value, Fault> {
    if name != "getLogger" {
        return Ok(Value::Void);
    }
    let reference = vm.alloc(Object {
        class: "org/slf4j/Logger".into(),
        fields: Vec::new(),
        kind: Kind::Logger(vm.mod_id.clone()),
    })?;
    Ok(Value::obj(reference))
}

// ---- file resources ----------------------------------------------------

/// `getResourceAsStream` and friends: read from the mod's own archive.
pub(crate) fn resource_bytes<H: Host>(vm: &Vm<H>, path: &str) -> Option<Vec<u8>> {
    vm.resources
        .get(path)
        .cloned()
        .or_else(|| vm.host.resource(&vm.mod_id, path))
}

#[allow(dead_code)]
pub(crate) fn empty_map() -> BTreeMap<String, Vec<u8>> {
    BTreeMap::new()
}

/// Parse a class descriptor into its internal name (used by annotation
/// scanning in the loader).
pub(crate) fn descriptor_name(descriptor: &str) -> &str {
    class_of_descriptor(descriptor)
}

#[allow(dead_code)]
fn _unused(_: &str) -> (Vec<String>, String) {
    parse_descriptor("()V").unwrap_or_default()
}

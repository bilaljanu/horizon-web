use horizon_java::{
    host::RecordingHost,
    runtime::{Limits, Value, Vm},
};

fn vm() -> Vm<RecordingHost> {
    let mut vm = Vm::new(RecordingHost::default(), "execution-test");
    for (name, bytes) in [
        (
            "t/Execution",
            include_bytes!("data/t/Execution.class").as_slice(),
        ),
        (
            "t/Execution$Counter",
            include_bytes!("data/t/Execution$Counter.class").as_slice(),
        ),
        (
            "t/Execution$Base",
            include_bytes!("data/t/Execution$Base.class").as_slice(),
        ),
        (
            "t/Execution$Child",
            include_bytes!("data/t/Execution$Child.class").as_slice(),
        ),
        (
            "t/Execution$OnlyArg",
            include_bytes!("data/t/Execution$OnlyArg.class").as_slice(),
        ),
    ] {
        vm.add_class(name, bytes.to_vec());
    }
    vm
}

#[test]
fn javac_control_flow_and_dispatch() {
    let mut vm = vm();
    for (method, expected) in [
        ("arithmetic", 45),
        ("arrays", 17),
        ("dispatch", 9),
        ("exception", 42),
        ("nullException", 43),
    ] {
        assert_eq!(
            vm.invoke_static("t/Execution", method, "()I", vec![])
                .unwrap(),
            Value::Int(expected),
            "{method}"
        );
    }
    assert_eq!(
        vm.invoke_static("t/Execution", "identity", "()Z", vec![])
            .unwrap(),
        Value::Int(1)
    );
    for (method, input, expected) in [
        ("dense", 0, 10),
        ("dense", 2, 30),
        ("dense", 9, -1),
        ("sparse", 1000, 5),
        ("sparse", 0, -2),
    ] {
        assert_eq!(
            vm.invoke_static("t/Execution", method, "(I)I", vec![Value::Int(input)])
                .unwrap(),
            Value::Int(expected)
        );
    }
}

#[test]
fn javac_lambda_concat_and_static_initialization() {
    let mut vm = vm();
    assert_eq!(
        vm.invoke_static("t/Execution", "lambda", "()I", vec![])
            .unwrap(),
        Value::Int(12)
    );
    assert_eq!(
        vm.invoke_static("t/Execution", "lambda", "()I", vec![])
            .unwrap(),
        Value::Int(17)
    );
    let text = vm
        .invoke_static("t/Execution", "concat", "()Ljava/lang/String;", vec![])
        .unwrap();
    assert_eq!(vm.read_string(text).unwrap(), "tick=17");
}

#[test]
fn runaway_and_unsupported_calls_fail_explicitly() {
    let mut vm = vm().with_limits(Limits {
        steps: 1000,
        ..Limits::default()
    });
    let error = vm
        .invoke_static("t/Execution", "forever", "()V", vec![])
        .unwrap_err();
    assert!(
        error.message.contains("instruction budget exhausted"),
        "{error}"
    );
    let error = vm
        .invoke_static("t/Execution", "unsupported", "()V", vec![])
        .unwrap_err();
    assert!(error.message.contains("java/lang/System.gc()V"), "{error}");
    assert_eq!(
        vm.invoke_static("t/Execution", "arithmetic", "()I", vec![])
            .unwrap(),
        Value::Int(45)
    );
}

#[test]
fn javac_category_two_stack_operations() {
    assert_eq!(
        vm().invoke_static("t/Execution", "category2", "()J", vec![])
            .unwrap(),
        Value::Long(3)
    );
}

#[test]
fn allocation_and_constructor_failures_are_not_silent() {
    let mut vm = vm();
    let error = vm
        .invoke_static("t/Execution", "oversizedArray", "()I", vec![])
        .unwrap_err();
    assert!(error.message.contains("allocation limit"), "{error}");
    let error = vm
        .new_instance("t/Execution$OnlyArg", "()V", vec![])
        .unwrap_err();
    assert!(error.message.contains("missing constructor"), "{error}");
    assert_eq!(
        vm.invoke_static("t/Execution", "staticIdentity", "()Z", vec![])
            .unwrap(),
        Value::Int(1)
    );
    let name = vm
        .invoke_static("t/Execution", "className", "()Ljava/lang/String;", vec![])
        .unwrap();
    assert_eq!(vm.read_string(name).unwrap(), "t.Execution");
}

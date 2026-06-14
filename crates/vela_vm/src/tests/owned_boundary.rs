use super::*;
use crate::owned_value::OwnedValue;

#[test]
fn public_program_entrypoint_roundtrips_nested_owned_values() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn identity(value) {
    return value;
}
"#,
    )
    .expect("compile identity program");
    let value = OwnedValue::map([
        ("enabled".to_owned(), OwnedValue::Bool(true)),
        (
            "items".to_owned(),
            OwnedValue::Array(vec![
                OwnedValue::String("gold".to_owned()),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(3)),
            ]),
        ),
    ]);
    let linked = link_test_program(&program);

    let result = Vm::new()
        .run_linked_program(&linked, "identity", std::slice::from_ref(&value))
        .expect("run public owned boundary");

    assert_eq!(result, value);
}

#[test]
fn public_program_entrypoint_preserves_owned_non_string_map_keys() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn identity(value) {
    return value;
}
"#,
    )
    .expect("compile identity program");
    let value = OwnedValue::map([(1_i64, "one"), (2_i64, "two")]);
    let linked = link_test_program(&program);

    let result = Vm::new()
        .run_linked_program(&linked, "identity", std::slice::from_ref(&value))
        .expect("run public owned boundary");

    assert_eq!(result, value);
}

#[test]
fn public_program_entrypoint_rejects_owned_map_non_keyable_keys() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn identity(value) {
    return value;
}
"#,
    )
    .expect("compile identity program");
    let value = OwnedValue::map([(OwnedValue::PathProxy(path_proxy()), "level")]);
    let linked = link_test_program(&program);

    let error = Vm::new()
        .run_linked_program(&linked, "identity", &[value])
        .expect_err("owned map with non-keyable key should be rejected");

    assert!(matches!(
        error.kind(),
        VmErrorKind::TypeMismatch { operation } if operation == "owned map"
    ));
}

#[test]
fn public_program_entrypoint_rejects_owned_set_non_keyable_elements() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn identity(value) {
    return value;
}
"#,
    )
    .expect("compile identity program");
    let value = OwnedValue::set([OwnedValue::PathProxy(path_proxy())]);
    let linked = link_test_program(&program);

    let error = Vm::new()
        .run_linked_program(&linked, "identity", &[value])
        .expect_err("owned set with non-keyable element should be rejected");

    assert!(matches!(
        error.kind(),
        VmErrorKind::TypeMismatch { operation } if operation == "owned set"
    ));
}

#[test]
fn budgeted_public_program_entrypoint_releases_boundary_heap_memory() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn identity(value) {
    return value;
}
"#,
    )
    .expect("compile identity program");
    let value = OwnedValue::Array(vec![
        OwnedValue::String("quest".to_owned()),
        OwnedValue::String("reward".to_owned()),
    ]);
    let mut budget = ExecutionBudget::unbounded();
    let linked = link_test_program(&program);

    let result = Vm::new()
        .run_linked_program_with_budget(
            &linked,
            "identity",
            std::slice::from_ref(&value),
            &mut budget,
        )
        .expect("run budgeted public owned boundary");

    assert_eq!(result, value);
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

fn path_proxy() -> PathProxy {
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 1);
    PathProxy::from_diagnostic_path(HostPath::new(host_ref).field(FieldId::new(2)))
}

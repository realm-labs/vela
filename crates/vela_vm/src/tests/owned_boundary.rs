use super::*;
use crate::owned_value::OwnedValue;

#[test]
fn public_program_entrypoint_roundtrips_nested_owned_values() {
    let program = compile_test_program(
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
fn script_mutation_of_owned_argument_has_no_implicit_rust_writeback() {
    let program = compile_test_program(
        SourceId::new(96),
        "fn update(value) { value[0] = 42; return value; }",
    )
    .expect("compile owned argument mutation");
    let linked = link_test_program(&program);
    let input = OwnedValue::array([1_i64]);
    let result = Vm::new()
        .run_linked_program(&linked, "update", std::slice::from_ref(&input))
        .expect("mutate the script-owned copy");
    assert_eq!(result, OwnedValue::array([42_i64]));
    assert_eq!(input, OwnedValue::array([1_i64]));
}

#[test]
fn public_program_entrypoint_preserves_owned_non_string_map_keys() {
    let program = compile_test_program(
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
fn public_program_entrypoint_roundtrips_owned_tuple_values() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
fn identity(value) {
    return value;
}
"#,
    )
    .expect("compile identity program");
    let value = OwnedValue::tuple([
        OwnedValue::String("reward".to_owned()),
        OwnedValue::Scalar(vela_common::ScalarValue::I64(3)),
        OwnedValue::Array(vec![OwnedValue::Bool(true)]),
    ]);
    let linked = link_test_program(&program);

    let result = Vm::new()
        .run_linked_program(&linked, "identity", std::slice::from_ref(&value))
        .expect("run public owned tuple boundary");

    assert_eq!(result, value);
}

#[test]
fn public_program_entrypoint_rejects_owned_map_non_keyable_keys() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
fn identity(value) {
    return value;
}
"#,
    )
    .expect("compile identity program");
    let value = OwnedValue::map([(OwnedValue::tuple([] as [OwnedValue; 0]), "level")]);
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
    let program = compile_test_program(
        SourceId::new(1),
        r#"
fn identity(value) {
    return value;
}
"#,
    )
    .expect("compile identity program");
    let value = OwnedValue::set([OwnedValue::tuple([] as [OwnedValue; 0])]);
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
    let program = compile_test_program(
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

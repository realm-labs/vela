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
    let value = OwnedValue::Map(BTreeMap::from([
        (
            "items".to_owned(),
            OwnedValue::Array(vec![
                OwnedValue::String("gold".to_owned()),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(3)),
            ]),
        ),
        ("enabled".to_owned(), OwnedValue::Bool(true)),
    ]));
    let linked = link_test_program(&program);

    let result = Vm::new()
        .run_linked_program(&linked, "identity", std::slice::from_ref(&value))
        .expect("run public owned boundary");

    assert_eq!(result, value);
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

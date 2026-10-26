use super::standard_id_dispatch::std_method_id;
use super::*;

#[test]
fn call_method_uses_standard_map_transform_ids_before_name_fallback() {
    let mut program = compile_standard_program_source(
        SourceId::new(1),
        include_str!("../../tests/fixtures/dispatch/map_transform_ids.vela"),
    )
    .expect("standard map transform source should compile")
    .into_bytecode();
    replace_call_method_debug_names(
        &mut program,
        &[
            (std_method_id("Map", "extend"), "missing_map_extend"),
            (std_method_id("Map", "merge"), "missing_map_merge"),
            (std_method_id("Map", "keys"), "missing_map_keys"),
            (std_method_id("Map", "values"), "missing_map_values"),
            (std_method_id("Map", "entries"), "missing_map_entries"),
        ],
    );

    let mut budget = ExecutionBudget::unbounded();
    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(9)))
    );
}

#[test]
fn call_method_uses_standard_set_transform_ids_before_name_fallback() {
    let mut program = compile_standard_program_source(
        SourceId::new(1),
        include_str!("../../tests/fixtures/dispatch/set_transform_ids.vela"),
    )
    .expect("standard set transform source should compile")
    .into_bytecode();
    replace_call_method_debug_names(
        &mut program,
        &[
            (std_method_id("Set", "extend"), "missing_set_extend"),
            (std_method_id("Set", "union"), "missing_set_union"),
            (std_method_id("Set", "values"), "missing_set_values"),
            (
                std_method_id("Set", "intersection"),
                "missing_set_intersection",
            ),
            (std_method_id("Set", "difference"), "missing_set_difference"),
            (
                std_method_id("Set", "symmetric_difference"),
                "missing_set_symmetric_difference",
            ),
        ],
    );

    let mut vm = Vm::new();
    vm.register_standard_natives();
    let mut budget = ExecutionBudget::unbounded();
    assert_eq!(
        run_linked_test_program_with_budget(&vm, &program, "main", &[], &mut budget),
        Ok(OwnedValue::String("bonus,daily,quest,raid".to_owned()))
    );
}

fn replace_call_method_debug_names(
    program: &mut UnlinkedProgram,
    replacements: &[(MethodId, &str)],
) {
    let code = program
        .function_mut("main")
        .expect("test function should exist");
    for instruction in &mut code.instructions {
        if let UnlinkedInstructionKind::CallMethodId {
            method, method_id, ..
        } = &mut instruction.kind
            && let Some((_, replacement)) = replacements
                .iter()
                .find(|(expected_method, _)| *expected_method == *method_id)
        {
            *method = (*replacement).to_owned();
        }
    }
}

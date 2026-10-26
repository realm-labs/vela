use super::standard_id_dispatch::std_method_id;
use super::*;

#[test]
fn call_method_uses_standard_option_result_ids_before_name_fallback() {
    let mut program = compile_standard_program_source(
        SourceId::new(1),
        include_str!("../../tests/fixtures/dispatch/option_result_ids.vela"),
    )
    .expect("standard option/result method source should compile")
    .into_bytecode();
    replace_call_method_debug_names(
        &mut program,
        &[
            (
                std_method_id("Option", "unwrap_or"),
                "missing_option_unwrap_or",
            ),
            (std_method_id("Option", "ok_or"), "missing_option_ok_or"),
            (std_method_id("Option", "flatten"), "missing_option_flatten"),
            (
                std_method_id("Result", "unwrap_or"),
                "missing_result_unwrap_or",
            ),
            (
                std_method_id("Result", "to_option"),
                "missing_result_to_option",
            ),
            (
                std_method_id("Result", "to_error_option"),
                "missing_result_to_error_option",
            ),
            (std_method_id("Result", "flatten"), "missing_result_flatten"),
        ],
    );

    let mut vm = Vm::new();
    vm.register_standard_natives();
    let mut budget = ExecutionBudget::unbounded();
    assert_eq!(
        run_linked_test_program_with_budget(&vm, &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
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

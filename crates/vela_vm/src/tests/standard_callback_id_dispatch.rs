use super::standard_id_dispatch::std_method_id;
use super::*;

#[test]
fn linked_callback_method_id_rejects_receiver_owner_mismatch() {
    let mut program = compile_standard_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let mapped = [1, 2, 3].map(|value| value + 1);
    return mapped[0];
}
"#,
    )
    .expect("standard callback method source should compile");
    replace_call_method_id(
        &mut program,
        std_method_id("Array", "map"),
        std_method_id("Set", "map"),
    );

    let mut budget = ExecutionBudget::unbounded();
    let error = run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget)
        .expect_err("linked callback dispatch must reject owner-mismatched method ids");

    assert_eq!(
        error.kind(),
        VmErrorKind::UnknownMethod {
            method: "map".to_owned()
        }
    );
}

fn replace_call_method_id(
    program: &mut UnlinkedProgram,
    expected_method: MethodId,
    replacement_method: MethodId,
) {
    let code = program
        .function_mut("main")
        .expect("test function should exist");
    for instruction in &mut code.instructions {
        if let UnlinkedInstructionKind::CallMethodId { method_id, .. } = &mut instruction.kind
            && *method_id == expected_method
        {
            *method_id = replacement_method;
            return;
        }
    }
    panic!("test method call should exist");
}

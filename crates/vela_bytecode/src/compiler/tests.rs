use super::*;
use crate::verification::VerificationErrorKind;
use crate::{CallArgument, CodeObject, Instruction, InstructionKind, Program, Register};
use vela_common::MethodId;
fn semantic_diagnostic_codes(error: CompileError) -> Vec<String> {
    let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
        panic!("expected semantic diagnostics");
    };
    diagnostics
        .into_iter()
        .filter_map(|diagnostic| diagnostic.code)
        .collect()
}

fn stable_test_trait_method_id(trait_name: &str, method_name: &str) -> MethodId {
    MethodId::new(vela_common::stable_id(
        "trait_method",
        trait_name,
        method_name,
    ))
}

#[test]
fn compiler_boundary_rejects_invalid_program_bytecode() {
    let mut code = CodeObject::new("main", 1);
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(2),
    }));
    let mut program = Program::new();
    program.insert_function(code);

    let error = verify_program(program).expect_err("invalid bytecode should fail verification");
    let CompileErrorKind::BytecodeVerification(error) = error.kind else {
        panic!("expected bytecode verification error");
    };
    assert_eq!(error.function, "main");
    assert_eq!(error.instruction, Some(0));
    assert_eq!(
        error.kind,
        VerificationErrorKind::RegisterOutOfBounds {
            register: Register(2),
            register_count: 1,
        }
    );
}

#[test]
fn compiler_boundary_rejects_invalid_function_bytecode() {
    let mut code = CodeObject::new("main", 1);
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(2),
    }));

    let error = verify_code_object(code).expect_err("invalid bytecode should fail verification");
    let CompileErrorKind::BytecodeVerification(error) = error.kind else {
        panic!("expected bytecode verification error");
    };
    assert_eq!(error.function, "main");
    assert_eq!(error.instruction, Some(0));
}

mod closures_and_bindings;
mod diagnostics;
mod expressions;
mod host_paths;
mod literals_and_calls;
mod loops_and_errors;
mod module_resolution;
mod script_methods;

use vela_bytecode::LinkedProgram;

use crate::{ExecutionBudget, OwnedValue, Vm, VmError};

#[derive(Clone, Debug, PartialEq)]
pub struct BackendConformanceResult {
    pub result: Result<OwnedValue, VmError>,
    pub execution_units_consumed: u64,
}

/// Runs the reference linked interpreter contract used by future backend
/// equivalence tests. A JIT backend must produce the same value/error and
/// execution-unit count for the same immutable generation and limit.
#[must_use]
pub fn run_linked_interpreter_case(
    vm: &Vm,
    program: &LinkedProgram,
    entry: &str,
    args: &[OwnedValue],
    execution_unit_limit: u64,
) -> BackendConformanceResult {
    let mut budget = ExecutionBudget::new(execution_unit_limit, usize::MAX, usize::MAX);
    let result = vm.run_linked_program_with_budget(program, entry, args, &mut budget);
    BackendConformanceResult {
        result,
        execution_units_consumed: budget.execution_units_consumed(),
    }
}

#[cfg(test)]
mod tests {
    use vela_bytecode::Linker;
    use vela_bytecode::compiler::compile_program_source;
    use vela_common::{ScalarValue, SourceId};

    use super::*;

    #[test]
    fn reference_case_reports_backend_neutral_result_and_units() {
        let compiled = compile_program_source(
            SourceId::new(1),
            r#"
fn helper(value) { return value + 1; }
fn main() { return helper(41); }
"#,
        )
        .expect("source should compile");
        let artifact = Linker::new()
            .link_compiled_program(&compiled)
            .expect("program should link");
        let vm = Vm::new();

        let complete = run_linked_interpreter_case(&vm, artifact.program(), "main", &[], 1);
        assert_eq!(
            complete.result,
            Ok(OwnedValue::Scalar(ScalarValue::I64(42)))
        );
        assert_eq!(complete.execution_units_consumed, 1);

        let exhausted = run_linked_interpreter_case(&vm, artifact.program(), "main", &[], 0);
        assert!(exhausted.result.is_err());
        assert_eq!(exhausted.execution_units_consumed, 0);
    }
}

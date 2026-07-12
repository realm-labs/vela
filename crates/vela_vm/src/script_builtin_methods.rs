use crate::{
    ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult, map_methods, set_methods,
};
use vela_def::MethodId;

pub(crate) use crate::standard_method_cache::{
    call_standard_cached, standard_cache_entry, standard_cache_entry_matches_method_id,
};

pub(crate) fn call_by_id(
    receiver: &mut Value,
    method_id: MethodId,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> Option<VmResult<Value>> {
    let cache = standard_cache_entry(method_id, receiver, heap.as_deref())?;
    call_standard_cached(receiver, cache, args, heap, budget)
}

pub(crate) fn has(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    if set_methods::is_set(receiver, heap) {
        set_methods::has(receiver, args, heap)
    } else {
        map_methods::has(receiver, args, heap)
    }
}

pub(crate) fn expect_no_args(method: &str, args: &[Value]) -> VmResult<()> {
    expect_arity(method, args, 0)
}

fn expect_arity(method: &str, args: &[Value], expected: usize) -> VmResult<()> {
    if args.len() == expected {
        return Ok(());
    }
    Err(VmError::new(VmErrorKind::ArityMismatch {
        name: method.to_owned(),
        expected,
        actual: args.len(),
    }))
}

#[cfg(test)]
mod tests {
    use vela_bytecode::compiler::compile_function_source_with_registry;
    use vela_bytecode::{Linker, UnlinkedCodeObject, UnlinkedProgram};
    use vela_common::SourceId;

    use crate::{ExecutionBudget, OwnedValue, Vm, VmResult};

    fn compile_standard_function_source(
        source: SourceId,
        text: &str,
        function_name: &str,
    ) -> vela_bytecode::compiler::error::CompileResult<UnlinkedCodeObject> {
        let registry = vela_stdlib::standard_registry().expect("standard registry should build");
        compile_function_source_with_registry(source, text, function_name, registry.compile_view())
    }

    fn run_linked_builtin_test_code(
        code: UnlinkedCodeObject,
        budget: &mut ExecutionBudget,
    ) -> VmResult<OwnedValue> {
        let entry = code.name.clone();
        let mut program = UnlinkedProgram::new();
        program.insert_function(code);
        let linked = Linker::new()
            .link_test_program(&program)
            .expect("builtin method test program should link");
        Vm::new().run_linked_program_with_budget(&linked, &entry, &[], budget)
    }

    #[test]
    fn string_len_counts_bytes() {
        let source = r#"
fn main() {
    return "quest".len() * 100 + "é日".len();
}
"#;
        let code = compile_standard_function_source(SourceId::new(1), source, "main")
            .expect("string len source should compile");
        let mut budget = ExecutionBudget::unbounded();

        let result =
            run_linked_builtin_test_code(code, &mut budget).expect("string len should run");
        assert_eq!(
            result,
            OwnedValue::Scalar(vela_common::ScalarValue::I64(505))
        );
    }

    #[test]
    fn managed_heap_string_len_counts_bytes() {
        let source = r#"
fn main() {
    let ascii = "quest";
    let unicode = "é日";
    return ascii.len() * 100 + unicode.len();
}
"#;
        let code = compile_standard_function_source(SourceId::new(1), source, "main")
            .expect("managed heap string len source should compile");
        let mut budget = ExecutionBudget::unbounded();

        let result = run_linked_builtin_test_code(code, &mut budget)
            .expect("managed heap string len should run");
        assert_eq!(
            result,
            OwnedValue::Scalar(vela_common::ScalarValue::I64(505))
        );
    }
}

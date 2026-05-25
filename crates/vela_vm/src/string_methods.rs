use crate::heap::HeapValue;
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult};

pub(crate) fn contains(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    predicate(
        receiver,
        "contains",
        "method contains",
        args,
        heap,
        |value, needle| value.contains(needle),
    )
}

pub(crate) fn starts_with(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    predicate(
        receiver,
        "starts_with",
        "method starts_with",
        args,
        heap,
        |value, prefix| value.starts_with(prefix),
    )
}

pub(crate) fn ends_with(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    predicate(
        receiver,
        "ends_with",
        "method ends_with",
        args,
        heap,
        |value, suffix| value.ends_with(suffix),
    )
}

pub(crate) fn to_upper(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_no_args("to_upper", args)?;
    string_value(receiver, heap, "method to_upper")
        .map(str::to_uppercase)
        .map(Value::String)
}

pub(crate) fn to_lower(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_no_args("to_lower", args)?;
    string_value(receiver, heap, "method to_lower")
        .map(str::to_lowercase)
        .map(Value::String)
}

pub(crate) fn trim(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_no_args("trim", args)?;
    string_value(receiver, heap, "method trim")
        .map(str::trim)
        .map(str::to_owned)
        .map(Value::String)
}

pub(crate) fn split(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("split", args, 1)?;
    let value = string_value(receiver, heap, "method split")?;
    let separator = string_value(&args[0], heap, "method split")?;
    Ok(Value::Array(
        value
            .split(separator)
            .map(|part| Value::String(part.to_owned()))
            .collect(),
    ))
}

pub(crate) fn string_value<'a>(
    value: &'a Value,
    heap: Option<&'a HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<&'a str> {
    match value {
        Value::String(value) => Ok(value),
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(value)) => Ok(value),
            _ => type_error(operation),
        },
        _ => type_error(operation),
    }
}

fn predicate(
    receiver: &Value,
    method: &str,
    operation: &'static str,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
    predicate: impl FnOnce(&str, &str) -> bool,
) -> VmResult<bool> {
    expect_arity(method, args, 1)?;
    let receiver = string_value(receiver, heap, operation)?;
    let needle = string_value(&args[0], heap, operation)?;
    Ok(predicate(receiver, needle))
}

fn expect_no_args(method: &str, args: &[Value]) -> VmResult<()> {
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

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}

#[cfg(test)]
mod tests {
    use vela_bytecode::compiler::compile_function_source;
    use vela_common::SourceId;

    use crate::{ExecutionBudget, Value, Vm};

    #[test]
    fn runs_compiled_string_utility_methods() {
        let source = r#"
fn main() {
    let label = "  Quest.Log ";
    let parts = label.trim().to_lower().split(".");
    if parts.len() == 2
        && parts[0] == "quest"
        && parts[1] == "log"
        && "wolf".to_upper() == "WOLF"
    {
        return parts[0];
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("string utility method source should compile");

        let result = Vm::new()
            .run(&code)
            .expect("string utility methods should run");
        assert_eq!(result, Value::String("quest".to_owned()));
    }

    #[test]
    fn managed_heap_execution_runs_string_utility_methods() {
        let source = r#"
fn main() {
    let event = " Player.LevelUp ";
    let pieces = event.trim().to_lower().split(".");
    if pieces[0] == "player"
        && pieces[1] == "levelup"
        && pieces[1].to_upper() == "LEVELUP"
    {
        return pieces[1];
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap string utility method source should compile");
        let mut budget = ExecutionBudget::unbounded();

        let result = Vm::new()
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap string utility methods should run");
        assert_eq!(result, Value::String("levelup".to_owned()));
        assert_eq!(budget.memory_bytes_allocated(), 0);
    }

    #[test]
    fn string_utility_methods_reject_non_string_receivers() {
        let source = r#"
fn main() {
    return 42.trim();
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("string utility type error source should compile");

        let error = Vm::new()
            .run(&code)
            .expect_err("string utility should reject non-string receiver");
        assert_eq!(
            error.kind,
            crate::VmErrorKind::TypeMismatch {
                operation: "method trim"
            }
        );
    }
}

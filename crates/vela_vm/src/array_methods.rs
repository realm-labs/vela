use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::heap::HeapValue;
use crate::method_runtime::{MethodRuntime, call_callback};
use crate::script_object::ScriptFields;
use crate::{
    HeapExecution, Value, VmError, VmErrorKind, VmResult, value_from_heap_slot, values_equal,
};

pub(crate) fn map(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("map", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method map")?;
    let mut mapped = Vec::with_capacity(values.len());
    for value in values {
        mapped.push(call_unary_callback(
            &mut runtime,
            "method map",
            &args[0],
            value,
            &mapped,
        )?);
    }
    Ok(Value::Array(mapped))
}

pub(crate) fn filter(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("filter", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method filter")?;
    let mut filtered = Vec::new();
    for value in values {
        let predicate = call_unary_callback(
            &mut runtime,
            "method filter",
            &args[0],
            value.clone(),
            &filtered,
        )?;
        if is_truthy(&predicate) {
            filtered.push(value);
        }
    }
    Ok(Value::Array(filtered))
}

pub(crate) fn find(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("find", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method find")?;
    for value in values {
        let predicate =
            call_unary_callback(&mut runtime, "method find", &args[0], value.clone(), &[])?;
        if is_truthy(&predicate) {
            return Ok(option_value("Some", Some(value)));
        }
    }
    Ok(option_value("None", None))
}

pub(crate) fn first(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("first", args, 0)?;
    let values = array_values(receiver, heap, "method first")?;
    Ok(values.first().cloned().map_or_else(
        || option_value("None", None),
        |value| option_value("Some", Some(value)),
    ))
}

pub(crate) fn last(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("last", args, 0)?;
    let values = array_values(receiver, heap, "method last")?;
    Ok(values.last().cloned().map_or_else(
        || option_value("None", None),
        |value| option_value("Some", Some(value)),
    ))
}

pub(crate) fn remove_at(
    receiver: &mut Value,
    args: &[Value],
    heap: Option<&mut HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("remove_at", args, 1)?;
    let index = index_value(&args[0], "method remove_at")?;
    match receiver {
        Value::Array(values) => {
            if index >= values.len() {
                return Ok(option_value("None", None));
            }
            Ok(option_value("Some", Some(values.remove(index))))
        }
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("method remove_at");
            };
            let Some(HeapValue::Array(values)) = heap.heap.get_mut(*reference).ok() else {
                return type_error("method remove_at");
            };
            if index >= values.len() {
                return Ok(option_value("None", None));
            }
            let value = value_from_heap_slot(&values.remove(index));
            Ok(option_value("Some", Some(value)))
        }
        _ => type_error("method remove_at"),
    }
}

pub(crate) fn join(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("join", args, 1)?;
    let values = array_values(receiver, heap, "method join")?;
    let separator = string_value(&args[0], heap, "method join")?;
    let mut parts = Vec::with_capacity(values.len());
    for value in values {
        parts.push(string_value(&value, heap, "method join")?.to_owned());
    }
    Ok(Value::String(parts.join(separator)))
}

pub(crate) fn contains(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    expect_arity("contains", args, 1)?;
    let values = array_values(receiver, heap, "method contains")?;
    for value in values {
        if values_equal(&value, &args[0], heap)? {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn index_of(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("index_of", args, 1)?;
    let values = array_values(receiver, heap, "method index_of")?;
    for (index, value) in values.into_iter().enumerate() {
        if values_equal(&value, &args[0], heap)? {
            let index = i64::try_from(index).map_err(|_| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "method index_of",
                })
            })?;
            return Ok(option_value("Some", Some(Value::Int(index))));
        }
    }
    Ok(option_value("None", None))
}

pub(crate) fn distinct(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("distinct", args, 0)?;
    let values = array_values(receiver, heap, "method distinct")?;
    let mut distinct = Vec::new();
    'values: for value in values {
        for existing in &distinct {
            if values_equal(existing, &value, heap)? {
                continue 'values;
            }
        }
        distinct.push(value);
    }
    Ok(Value::Array(distinct))
}

pub(crate) fn reverse(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("reverse", args, 0)?;
    let mut values = array_values(receiver, heap, "method reverse")?;
    values.reverse();
    Ok(Value::Array(values))
}

pub(crate) fn slice(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("slice", args, 2)?;
    let values = array_values(receiver, heap, "method slice")?;
    let start = index_value(&args[0], "method slice")?;
    let end = index_value(&args[1], "method slice")?;
    if start > end {
        return type_error("method slice");
    }
    if start > values.len() {
        return Err(index_out_of_bounds(start, values.len()));
    }
    if end > values.len() {
        return Err(index_out_of_bounds(end, values.len()));
    }
    Ok(Value::Array(values[start..end].to_vec()))
}

pub(crate) fn any(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<bool> {
    expect_arity("any", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method any")?;
    for value in values {
        let predicate = call_unary_callback(&mut runtime, "method any", &args[0], value, &[])?;
        if is_truthy(&predicate) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn all(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<bool> {
    expect_arity("all", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method all")?;
    for value in values {
        let predicate = call_unary_callback(&mut runtime, "method all", &args[0], value, &[])?;
        if !is_truthy(&predicate) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) fn count(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<i64> {
    expect_arity("count", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method count")?;
    let mut count = 0_i64;
    for value in values {
        let predicate = call_unary_callback(&mut runtime, "method count", &args[0], value, &[])?;
        if is_truthy(&predicate) {
            count = count.checked_add(1).ok_or_else(|| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "method count",
                })
            })?;
        }
    }
    Ok(count)
}

pub(crate) fn sum(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    if args.len() > 1 {
        return Err(VmError::new(VmErrorKind::ArityMismatch {
            name: "sum".to_owned(),
            expected: 1,
            actual: args.len(),
        }));
    }
    let values = array_values(receiver, runtime.heap.as_deref(), "method sum")?;
    let mut total = NumericTotal::default();
    if let Some(callback) = args.first() {
        for value in values {
            let mapped = call_unary_callback(&mut runtime, "method sum", callback, value, &[])?;
            total.add_value(&mapped, "method sum")?;
        }
    } else {
        for value in values {
            total.add_value(&value, "method sum")?;
        }
    }
    Ok(total.into_value())
}

pub(crate) fn group_by(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("group_by", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method group_by")?;
    let mut groups = BTreeMap::<String, Value>::new();
    for value in values {
        let protected = groups.values().cloned().collect::<Vec<_>>();
        let key_value = call_unary_callback(
            &mut runtime,
            "method group_by",
            &args[0],
            value.clone(),
            &protected,
        )?;
        let key = group_key(&key_value, runtime.heap.as_deref())?;
        match groups.entry(key) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(Value::Array(vec![value]));
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let Value::Array(values) = entry.get_mut() else {
                    unreachable!("group_by only stores array group values");
                };
                values.push(value);
            }
        }
    }
    Ok(Value::Map(groups))
}

pub(crate) fn sort_by(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("sort_by", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method sort_by")?;
    let mut entries = Vec::<SortEntry>::with_capacity(values.len());
    let mut key_kind = None;
    for value in values {
        let protected = entries
            .iter()
            .map(|entry| entry.value.clone())
            .collect::<Vec<_>>();
        let key_value = call_unary_callback(
            &mut runtime,
            "method sort_by",
            &args[0],
            value.clone(),
            &protected,
        )?;
        let key = sort_key(&key_value, runtime.heap.as_deref())?;
        if let Some(expected) = key_kind {
            if key.kind() != expected {
                return type_error("method sort_by");
            }
        } else {
            key_kind = Some(key.kind());
        }
        entries.push(SortEntry {
            key,
            value,
            index: entries.len(),
        });
    }
    entries.sort_by(|left, right| {
        left.key
            .compare(&right.key)
            .then_with(|| left.index.cmp(&right.index))
    });
    Ok(Value::Array(
        entries.into_iter().map(|entry| entry.value).collect(),
    ))
}

enum NumericTotal {
    Int(i64),
    Float(f64),
}

impl Default for NumericTotal {
    fn default() -> Self {
        Self::Int(0)
    }
}

impl NumericTotal {
    fn add_value(&mut self, value: &Value, operation: &'static str) -> VmResult<()> {
        match (&mut *self, value) {
            (NumericTotal::Int(total), Value::Int(value)) => {
                *total = total
                    .checked_add(*value)
                    .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
            }
            (NumericTotal::Int(total), Value::Float(value)) => {
                *self = NumericTotal::Float(*total as f64 + *value);
            }
            (NumericTotal::Float(total), Value::Int(value)) => {
                *total += *value as f64;
            }
            (NumericTotal::Float(total), Value::Float(value)) => {
                *total += *value;
            }
            _ => return type_error(operation),
        }
        Ok(())
    }

    fn into_value(self) -> Value {
        match self {
            NumericTotal::Int(value) => Value::Int(value),
            NumericTotal::Float(value) => Value::Float(value),
        }
    }
}

fn group_key(value: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<String> {
    string_value(value, heap, "method group_by").map(str::to_owned)
}

struct SortEntry {
    key: SortKey,
    value: Value,
    index: usize,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SortKeyKind {
    Numeric,
    String,
}

enum SortKey {
    Int(i64),
    Float(f64),
    String(String),
}

impl SortKey {
    fn kind(&self) -> SortKeyKind {
        match self {
            Self::Int(_) | Self::Float(_) => SortKeyKind::Numeric,
            Self::String(_) => SortKeyKind::String,
        }
    }

    fn compare(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Int(left), Self::Int(right)) => left.cmp(right),
            (Self::Int(left), Self::Float(right)) => {
                (*left as f64).partial_cmp(right).unwrap_or(Ordering::Equal)
            }
            (Self::Float(left), Self::Int(right)) => left
                .partial_cmp(&(*right as f64))
                .unwrap_or(Ordering::Equal),
            (Self::Float(left), Self::Float(right)) => {
                left.partial_cmp(right).unwrap_or(Ordering::Equal)
            }
            (Self::String(left), Self::String(right)) => left.cmp(right),
            (Self::Int(_) | Self::Float(_), Self::String(_))
            | (Self::String(_), Self::Int(_) | Self::Float(_)) => Ordering::Equal,
        }
    }
}

fn sort_key(value: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<SortKey> {
    match value {
        Value::Int(value) => Ok(SortKey::Int(*value)),
        Value::Float(value) if value.is_finite() => Ok(SortKey::Float(*value)),
        Value::String(value) => Ok(SortKey::String(value.clone())),
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(value)) => Ok(SortKey::String(value.clone())),
            _ => type_error("method sort_by"),
        },
        _ => type_error("method sort_by"),
    }
}

fn string_value<'a>(
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

fn array_values(
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<Vec<Value>> {
    match receiver {
        Value::Array(values) => Ok(values.clone()),
        Value::HeapRef(reference) => {
            let Some(HeapValue::Array(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error(operation);
            };
            Ok(values.iter().map(value_from_heap_slot).collect())
        }
        _ => type_error(operation),
    }
}

fn index_value(value: &Value, operation: &'static str) -> VmResult<usize> {
    match value {
        Value::Int(value) if *value >= 0 => Ok(*value as usize),
        _ => type_error(operation),
    }
}

fn call_unary_callback(
    runtime: &mut MethodRuntime<'_, '_, '_>,
    operation: &'static str,
    callback: &Value,
    value: Value,
    protected_values: &[Value],
) -> VmResult<Value> {
    call_callback(runtime, operation, callback, &[value], protected_values)
}

fn expect_arity(name: &str, args: &[Value], expected: usize) -> VmResult<()> {
    if args.len() == expected {
        return Ok(());
    }
    Err(VmError::new(VmErrorKind::ArityMismatch {
        name: name.to_owned(),
        expected,
        actual: args.len(),
    }))
}

fn is_truthy(value: &Value) -> bool {
    !matches!(value, Value::Missing | Value::Null | Value::Bool(false))
}

fn option_value(variant: &str, payload: Option<Value>) -> Value {
    let fields = payload
        .map(|payload| vec![("0".to_owned(), payload)])
        .unwrap_or_default();
    Value::Enum {
        enum_name: "Option".to_owned(),
        variant: variant.to_owned(),
        fields: ScriptFields::from_pairs(&format!("Option.{variant}"), fields),
    }
}

fn index_out_of_bounds(index: usize, len: usize) -> VmError {
    VmError::new(VmErrorKind::IndexOutOfBounds {
        index: i64::try_from(index).unwrap_or(i64::MAX),
        len,
    })
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}

#[cfg(test)]
mod tests {
    use vela_bytecode::compiler::compile_function_source;
    use vela_common::SourceId;

    use crate::{ExecutionBudget, Value, Vm, VmErrorKind};

    #[test]
    fn runs_compiled_array_higher_order_methods() {
        let source = r#"
fn main() {
    let values = [1, 2, 3, 4];
    let doubled = values.map(|value| value * 2);
    let evens = values.filter(|value| value % 2 == 0);
    let first_large = values.find(|value| value > 2);
    let missing = values.find(|value| value > 10);
    let count = values.count(|value| value > 1);
    if doubled[2] == 6 && evens[0] == 2 && evens[1] == 4
        && option.unwrap_or(first_large, 0) == 3
        && option.unwrap_or(missing, 9) == 9
        && values.any(|value| value == 4)
        && values.all(|value| value > 0)
    {
        return count;
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("array higher-order methods should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm
            .run(&code)
            .expect("array higher-order methods should run");
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn managed_heap_execution_runs_array_higher_order_methods() {
        let source = r#"
fn main() {
    let names = ["boar", "wolf", "wyrm"];
    let lengths = names.map(|name| name.len());
    let matches = names.filter(|name| name.starts_with("w"));
    let found = names.find(|name| name.contains("yr"));
    let missing = names.find(|name| name == "dragon");
    if lengths[0] == 4 && lengths[2] == 4
        && matches.len() == 2 && matches[1] == "wyrm"
        && option.unwrap_or(found, "missing") == "wyrm"
        && option.unwrap_or(missing, "missing") == "missing"
        && names.any(|name| name.ends_with("f"))
        && names.all(|name| name.len() == 4)
    {
        return names.count(|name| name.contains("o"));
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap array higher-order methods should compile");
        let mut budget = ExecutionBudget::unbounded();
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap array higher-order methods should run");
        assert_eq!(result, Value::Int(2));
    }

    #[test]
    fn runs_compiled_array_endpoint_methods() {
        let source = r#"
fn main() {
    let values = [10, 20, 30];
    let empty = [];
    if option.unwrap_or(values.first(), 0) == 10
        && option.unwrap_or(values.last(), 0) == 30
        && option.unwrap_or(empty.first(), 7) == 7
        && option.unwrap_or(empty.last(), 9) == 9
    {
        return 1;
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("array endpoint methods should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("array endpoint methods should run");
        assert_eq!(result, Value::Int(1));
    }

    #[test]
    fn managed_heap_execution_runs_array_endpoint_methods() {
        let source = r#"
fn main() {
    let names = ["boar", "wolf", "wyrm"];
    let empty = [];
    if option.unwrap_or(names.first(), "missing") == "boar"
        && option.unwrap_or(names.last(), "missing") == "wyrm"
        && option.unwrap_or(empty.first(), "empty") == "empty"
        && option.unwrap_or(empty.last(), "empty") == "empty"
    {
        return option.unwrap_or(names.last(), "missing");
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap array endpoint methods should compile");
        let mut budget = ExecutionBudget::unbounded();
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap array endpoint methods should run");
        assert_eq!(result, Value::String("wyrm".to_owned()));
    }

    #[test]
    fn runs_compiled_array_remove_at_method() {
        let source = r#"
fn main() {
    let values = [10, 20, 30];
    let removed = values.remove_at(1);
    let missing = values.remove_at(5);
    if option.unwrap_or(removed, 0) == 20
        && option.unwrap_or(missing, 99) == 99
        && values.len() == 2
        && values[0] == 10
        && values[1] == 30
    {
        return values[1];
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("array remove_at method should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("array remove_at method should run");
        assert_eq!(result, Value::Int(30));
    }

    #[test]
    fn managed_heap_execution_runs_array_remove_at_method() {
        let source = r#"
fn main() {
    let tags = ["daily", "quest", "raid"];
    let removed = tags.remove_at(0);
    let missing = tags.remove_at(9);
    if option.unwrap_or(removed, "missing") == "daily"
        && option.unwrap_or(missing, "none") == "none"
        && tags.join("|") == "quest|raid"
    {
        return tags[0];
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap array remove_at method should compile");
        let mut budget = ExecutionBudget::unbounded();
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap array remove_at method should run");
        assert_eq!(result, Value::String("quest".to_owned()));
    }

    #[test]
    fn runs_compiled_array_contains_method() {
        let source = r#"
fn main() {
    let values = [10, 20, 30];
    let rewards = [Reward { item_id: "gold", count: 2 }];
    let expected = Reward { item_id: "gold", count: 2 };
    if values.contains(20)
        && !values.contains(99)
        && rewards.contains(expected)
        && ![].contains("missing")
    {
        return 1;
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("array contains method should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("array contains method should run");
        assert_eq!(result, Value::Int(1));
    }

    #[test]
    fn managed_heap_execution_runs_array_contains_method() {
        let source = r#"
fn main() {
    let tags = ["daily", "quest", "raid"];
    let nested = [["daily", "quest"], ["raid"]];
    if tags.contains("quest")
        && !tags.contains("bonus")
        && nested.contains(["daily", "quest"])
    {
        return tags.join(",");
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap array contains method should compile");
        let mut budget = ExecutionBudget::unbounded();
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap array contains method should run");
        assert_eq!(result, Value::String("daily,quest,raid".to_owned()));
    }

    #[test]
    fn runs_compiled_array_index_of_method() {
        let source = r#"
fn main() {
    let values = [10, 20, 30, 20];
    let rewards = [Reward { item_id: "gold", count: 2 }];
    let expected = Reward { item_id: "gold", count: 2 };
    if option.unwrap_or(values.index_of(20), -1) == 1
        && option.unwrap_or(values.index_of(99), -1) == -1
        && option.unwrap_or(rewards.index_of(expected), -1) == 0
        && option.unwrap_or([].index_of("missing"), -1) == -1
    {
        return 1;
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("array index_of method should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("array index_of method should run");
        assert_eq!(result, Value::Int(1));
    }

    #[test]
    fn managed_heap_execution_runs_array_index_of_method() {
        let source = r#"
fn main() {
    let tags = ["daily", "quest", "raid"];
    let nested = [["daily", "quest"], ["raid"]];
    if option.unwrap_or(tags.index_of("quest"), -1) == 1
        && option.unwrap_or(tags.index_of("bonus"), -1) == -1
        && option.unwrap_or(nested.index_of(["raid"]), -1) == 1
    {
        return tags[option.unwrap_or(tags.index_of("raid"), 0)];
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap array index_of method should compile");
        let mut budget = ExecutionBudget::unbounded();
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap array index_of method should run");
        assert_eq!(result, Value::String("raid".to_owned()));
    }

    #[test]
    fn runs_compiled_array_distinct_method() {
        let source = r#"
fn main() {
    let rewards = [
        Reward { item_id: "gold", count: 2 },
        Reward { item_id: "xp", count: 1 },
        Reward { item_id: "gold", count: 2 },
    ];
    let unique = [3, 1, 3, 2, 1].distinct();
    let unique_rewards = rewards.distinct();
    if unique.len() == 3
        && unique[0] == 3
        && unique[1] == 1
        && unique[2] == 2
        && rewards.len() == 3
        && unique_rewards.len() == 2
    {
        return unique_rewards[0].item_id;
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("array distinct source should compile");

        let result = Vm::new().run(&code).expect("array distinct should run");
        assert_eq!(result, Value::String("gold".to_owned()));
    }

    #[test]
    fn managed_heap_execution_runs_array_distinct_method() {
        let source = r#"
fn main() {
    let tags = ["raid", "quest", "raid", "daily", "quest"];
    let nested = [["daily", "quest"], ["daily", "quest"], ["raid"]];
    let unique_tags = tags.distinct();
    let unique_nested = nested.distinct();
    if tags.len() == 5
        && unique_tags.join(",") == "raid,quest,daily"
        && unique_nested.len() == 2
    {
        return unique_nested[0].join("|");
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap array distinct source should compile");
        let mut budget = ExecutionBudget::unbounded();

        let result = Vm::new()
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap array distinct should run");
        assert_eq!(result, Value::String("daily|quest".to_owned()));
    }

    #[test]
    fn runs_compiled_array_reverse_method() {
        let source = r#"
fn main() {
    let rewards = [
        Reward { item_id: "gold", count: 2 },
        Reward { item_id: "xp", count: 1 },
    ];
    let reversed = [1, 2, 3].reverse();
    let reversed_rewards = rewards.reverse();
    if reversed[0] == 3
        && reversed[2] == 1
        && rewards[0].item_id == "gold"
        && reversed_rewards[0].item_id == "xp"
    {
        return reversed_rewards[1].count;
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("array reverse source should compile");

        let result = Vm::new().run(&code).expect("array reverse should run");
        assert_eq!(result, Value::Int(2));
    }

    #[test]
    fn managed_heap_execution_runs_array_reverse_method() {
        let source = r#"
fn main() {
    let tags = ["daily", "quest", "raid"];
    let nested = [["daily", "quest"], ["raid"]];
    let reversed_tags = tags.reverse();
    let reversed_nested = nested.reverse();
    if tags.join(",") == "daily,quest,raid"
        && reversed_tags.join(",") == "raid,quest,daily"
        && reversed_nested[0].join("|") == "raid"
    {
        return reversed_nested[1].join("|");
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap array reverse source should compile");
        let mut budget = ExecutionBudget::unbounded();

        let result = Vm::new()
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap array reverse should run");
        assert_eq!(result, Value::String("daily|quest".to_owned()));
    }

    #[test]
    fn runs_compiled_array_slice_method() {
        let source = r#"
fn main() {
    let rewards = [
        Reward { item_id: "gold", count: 2 },
        Reward { item_id: "xp", count: 1 },
        Reward { item_id: "gem", count: 3 },
    ];
    let middle = [10, 20, 30, 40].slice(1, 3);
    let reward_slice = rewards.slice(0, 2);
    let empty = rewards.slice(2, 2);
    if middle[0] == 20
        && middle[1] == 30
        && reward_slice[1].item_id == "xp"
        && rewards.len() == 3
        && empty.is_empty()
    {
        return reward_slice[0].count;
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("array slice source should compile");

        let result = Vm::new().run(&code).expect("array slice should run");
        assert_eq!(result, Value::Int(2));
    }

    #[test]
    fn managed_heap_execution_runs_array_slice_method() {
        let source = r#"
fn main() {
    let tags = ["daily", "quest", "raid", "bonus"];
    let nested = [["daily", "quest"], ["raid"], ["bonus"]];
    let tag_slice = tags.slice(1, 3);
    let nested_slice = nested.slice(0, 2);
    if tags.join(",") == "daily,quest,raid,bonus"
        && tag_slice.join("|") == "quest|raid"
        && nested_slice[0].join("|") == "daily|quest"
    {
        return nested_slice[1].join("|");
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap array slice source should compile");
        let mut budget = ExecutionBudget::unbounded();

        let result = Vm::new()
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap array slice should run");
        assert_eq!(result, Value::String("raid".to_owned()));
    }

    #[test]
    fn array_slice_rejects_out_of_bounds_ranges() {
        let source = r#"
fn main() {
    return [1, 2].slice(0, 3);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("array slice bounds source should compile");

        let error = Vm::new()
            .run(&code)
            .expect_err("array slice should reject out of bounds index");
        assert_eq!(
            error.kind,
            VmErrorKind::IndexOutOfBounds { index: 3, len: 2 }
        );
    }

    #[test]
    fn runs_compiled_array_join_method() {
        let source = r#"
fn main() {
    let path = ["quest", "stage", "done"].join(".");
    if path == "quest.stage.done" && [].join(",") == "" {
        return path;
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("array join method should compile");

        let result = Vm::new().run(&code).expect("array join method should run");
        assert_eq!(result, Value::String("quest.stage.done".to_owned()));
    }

    #[test]
    fn managed_heap_execution_runs_array_join_method() {
        let source = r#"
fn main() {
    let tags = ["boar", "wolf", "wyrm"];
    return tags.join("|");
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap array join method should compile");
        let mut budget = ExecutionBudget::unbounded();

        let result = Vm::new()
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap array join method should run");
        assert_eq!(result, Value::String("boar|wolf|wyrm".to_owned()));
    }

    #[test]
    fn array_join_rejects_non_string_values() {
        let source = r#"
fn main() {
    return ["boar", 1].join(",");
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("array join type error source should compile");

        let error = Vm::new()
            .run(&code)
            .expect_err("array join should reject non-string values");
        assert_eq!(
            error.kind,
            VmErrorKind::TypeMismatch {
                operation: "method join"
            }
        );
    }

    #[test]
    fn runs_compiled_array_sum_methods() {
        let source = r#"
fn main() {
    let values = [1, 2, 3, 4];
    let empty = [];
    let direct = values.sum();
    let weighted = values.sum(|value| value * 2);
    if direct == 10 && weighted == 20 && empty.sum() == 0 {
        return 1;
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("array sum methods should compile");

        let result = Vm::new().run(&code).expect("array sum methods should run");
        assert_eq!(result, Value::Int(1));
    }

    #[test]
    fn managed_heap_execution_runs_array_sum_methods() {
        let source = r#"
fn main() {
    let values = [1, 2, 3, 4];
    let direct = values.sum();
    let incremented = values.sum(|value| value + 1);
    if direct == 10 && incremented == 14 {
        return values.sum(|value| value * 3);
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap array sum methods should compile");
        let mut budget = ExecutionBudget::unbounded();

        let result = Vm::new()
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap array sum methods should run");
        assert_eq!(result, Value::Int(30));
    }

    #[test]
    fn array_sum_rejects_non_numeric_values() {
        let source = r#"
fn main() {
    return ["boar"].sum();
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("array sum type error source should compile");

        let error = Vm::new()
            .run(&code)
            .expect_err("array sum should reject non-numeric values");
        assert_eq!(
            error.kind,
            VmErrorKind::TypeMismatch {
                operation: "method sum"
            }
        );
    }

    #[test]
    fn runs_compiled_array_group_by_method() {
        let source = r#"
fn main() {
    let values = [1, 2, 3, 4, 5];
    let groups = values.group_by(|value| if value % 2 == 0 { "even" } else { "odd" });
    if groups.len() == 2
        && groups["odd"].len() == 3
        && groups["odd"][1] == 3
        && groups["even"].sum() == 6
    {
        return groups["odd"][2];
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("array group_by method should compile");

        let result = Vm::new()
            .run(&code)
            .expect("array group_by method should run");
        assert_eq!(result, Value::Int(5));
    }

    #[test]
    fn managed_heap_execution_runs_array_group_by_method() {
        let source = r#"
fn main() {
    let names = ["boar", "bat", "wolf", "wyrm"];
    let groups = names.group_by(|name| if name.starts_with("w") { "w" } else { "b" });
    if groups.len() == 2
        && groups["b"].len() == 2
        && groups["w"][0] == "wolf"
        && groups["w"][1] == "wyrm"
    {
        return groups["b"][1];
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap array group_by method should compile");
        let mut budget = ExecutionBudget::unbounded();

        let result = Vm::new()
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap array group_by method should run");
        assert_eq!(result, Value::String("bat".to_owned()));
    }

    #[test]
    fn array_group_by_rejects_non_string_keys() {
        let source = r#"
fn main() {
    return [1].group_by(|value| value);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("array group_by type error source should compile");

        let error = Vm::new()
            .run(&code)
            .expect_err("array group_by should reject non-string keys");
        assert_eq!(
            error.kind,
            VmErrorKind::TypeMismatch {
                operation: "method group_by"
            }
        );
    }

    #[test]
    fn runs_compiled_array_sort_by_method() {
        let source = r#"
fn main() {
    let values = [21, 11, 10, 12];
    let sorted = values.sort_by(|value| value % 10);
    if sorted[0] == 10
        && sorted[1] == 21
        && sorted[2] == 11
        && sorted[3] == 12
        && values[0] == 21
    {
        return sorted[2];
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("array sort_by method should compile");

        let result = Vm::new()
            .run(&code)
            .expect("array sort_by method should run");
        assert_eq!(result, Value::Int(11));
    }

    #[test]
    fn managed_heap_execution_runs_array_sort_by_method() {
        let source = r#"
fn main() {
    let names = ["wyrm", "boar", "bat", "wolf"];
    let sorted = names.sort_by(|name| name);
    if sorted[0] == "bat"
        && sorted[1] == "boar"
        && sorted[2] == "wolf"
        && sorted[3] == "wyrm"
    {
        return sorted[1];
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap array sort_by method should compile");
        let mut budget = ExecutionBudget::unbounded();

        let result = Vm::new()
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap array sort_by method should run");
        assert_eq!(result, Value::String("boar".to_owned()));
    }

    #[test]
    fn array_sort_by_rejects_mixed_key_domains() {
        let source = r#"
fn main() {
    return [1, "two"].sort_by(|value| value);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("array sort_by type error source should compile");

        let error = Vm::new()
            .run(&code)
            .expect_err("array sort_by should reject mixed key domains");
        assert_eq!(
            error.kind,
            VmErrorKind::TypeMismatch {
                operation: "method sort_by"
            }
        );
    }
}

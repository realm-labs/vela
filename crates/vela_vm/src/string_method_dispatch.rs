use crate::{ExecutionBudget, HeapExecution, Value, VmResult, iteration, string_methods};

pub(crate) fn call(
    method: &str,
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> Option<VmResult<Value>> {
    match method {
        "contains" if string_methods::is_string(receiver, heap.as_deref()) => {
            Some(string_methods::contains(receiver, args, heap.as_deref()).map(Value::Bool))
        }
        "find" if string_methods::is_string(receiver, heap.as_deref()) => {
            Some(string_methods::find(receiver, args, heap, budget))
        }
        "starts_with" => {
            Some(string_methods::starts_with(receiver, args, heap.as_deref()).map(Value::Bool))
        }
        "ends_with" => {
            Some(string_methods::ends_with(receiver, args, heap.as_deref()).map(Value::Bool))
        }
        "strip_prefix" => Some(string_methods::strip_prefix(receiver, args, heap, budget)),
        "strip_suffix" => Some(string_methods::strip_suffix(receiver, args, heap, budget)),
        "to_upper" => Some(string_methods::to_upper(receiver, args, heap, budget)),
        "to_lower" => Some(string_methods::to_lower(receiver, args, heap, budget)),
        "trim" => Some(string_methods::trim(receiver, args, heap, budget)),
        "trim_start" => Some(string_methods::trim_start(receiver, args, heap, budget)),
        "trim_end" => Some(string_methods::trim_end(receiver, args, heap, budget)),
        "replace" => Some(string_methods::replace(receiver, args, heap, budget)),
        "repeat" => Some(string_methods::repeat(receiver, args, heap, budget)),
        "slice" if string_methods::is_string(receiver, heap.as_deref()) => {
            Some(string_methods::slice(receiver, args, heap, budget))
        }
        "chars" => Some(iteration::chars_method(receiver, args, heap, budget)),
        "bytes" if string_methods::is_string(receiver, heap.as_deref()) => {
            Some(iteration::string_bytes_method(receiver, args, heap, budget))
        }
        "split" => Some(string_methods::split(receiver, args, heap, budget)),
        "split_once" => Some(string_methods::split_once(receiver, args, heap, budget)),
        "split_lines" => Some(string_methods::split_lines(receiver, args, heap, budget)),
        "split_whitespace" => Some(string_methods::split_whitespace(
            receiver, args, heap, budget,
        )),
        "parse_i8" => Some(string_methods::parse_i8(receiver, args, heap, budget)),
        "parse_i16" => Some(string_methods::parse_i16(receiver, args, heap, budget)),
        "parse_i32" => Some(string_methods::parse_i32(receiver, args, heap, budget)),
        "parse_i64" => Some(string_methods::parse_i64(receiver, args, heap, budget)),
        "parse_u8" => Some(string_methods::parse_u8(receiver, args, heap, budget)),
        "parse_u16" => Some(string_methods::parse_u16(receiver, args, heap, budget)),
        "parse_u32" => Some(string_methods::parse_u32(receiver, args, heap, budget)),
        "parse_u64" => Some(string_methods::parse_u64(receiver, args, heap, budget)),
        "parse_f32" => Some(string_methods::parse_f32(receiver, args, heap, budget)),
        "parse_f64" => Some(string_methods::parse_f64(receiver, args, heap, budget)),
        "parse_bool" => Some(string_methods::parse_bool(receiver, args, heap, budget)),
        "parse_char" => Some(string_methods::parse_char(receiver, args, heap, budget)),
        _ => None,
    }
}

pub(crate) fn call_readonly(
    method: &str,
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> Option<VmResult<Value>> {
    match method {
        "contains" if string_methods::is_string(receiver, heap) => {
            Some(string_methods::contains(receiver, args, heap).map(Value::Bool))
        }
        "starts_with" => Some(string_methods::starts_with(receiver, args, heap).map(Value::Bool)),
        "ends_with" => Some(string_methods::ends_with(receiver, args, heap).map(Value::Bool)),
        _ => None,
    }
}

use crate::{HeapExecution, Value, VmResult, string_methods};

pub(crate) fn call(
    method: &str,
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> Option<VmResult<Value>> {
    match method {
        "contains" if string_methods::is_string(receiver, heap) => {
            Some(string_methods::contains(receiver, args, heap).map(Value::Bool))
        }
        "find" if string_methods::is_string(receiver, heap) => {
            Some(string_methods::find(receiver, args, heap))
        }
        "starts_with" => Some(string_methods::starts_with(receiver, args, heap).map(Value::Bool)),
        "ends_with" => Some(string_methods::ends_with(receiver, args, heap).map(Value::Bool)),
        "strip_prefix" => Some(string_methods::strip_prefix(receiver, args, heap)),
        "strip_suffix" => Some(string_methods::strip_suffix(receiver, args, heap)),
        "to_upper" => Some(string_methods::to_upper(receiver, args, heap)),
        "to_lower" => Some(string_methods::to_lower(receiver, args, heap)),
        "trim" => Some(string_methods::trim(receiver, args, heap)),
        "trim_start" => Some(string_methods::trim_start(receiver, args, heap)),
        "trim_end" => Some(string_methods::trim_end(receiver, args, heap)),
        "replace" => Some(string_methods::replace(receiver, args, heap)),
        "repeat" => Some(string_methods::repeat(receiver, args, heap)),
        "slice" if string_methods::is_string(receiver, heap) => {
            Some(string_methods::slice(receiver, args, heap))
        }
        "split" => Some(string_methods::split(receiver, args, heap)),
        "split_once" => Some(string_methods::split_once(receiver, args, heap)),
        "split_lines" => Some(string_methods::split_lines(receiver, args, heap)),
        "split_whitespace" => Some(string_methods::split_whitespace(receiver, args, heap)),
        "char_at" => Some(string_methods::char_at(receiver, args, heap)),
        "parse_int" => Some(string_methods::parse_int(receiver, args, heap)),
        "parse_float" => Some(string_methods::parse_float(receiver, args, heap)),
        "parse_bool" => Some(string_methods::parse_bool(receiver, args, heap)),
        _ => None,
    }
}

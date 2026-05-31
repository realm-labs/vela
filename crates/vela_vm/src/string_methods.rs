use crate::heap::HeapValue;
use crate::option_result::option_value;
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult};

mod parsing;

pub(crate) use parsing::{parse_bool, parse_float, parse_int};

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

pub(crate) fn find(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("find", args, 1)?;
    let value = string_value(receiver, heap, "method find")?;
    let needle = string_value(&args[0], heap, "method find")?;
    let Some(byte_index) = value.find(needle) else {
        return Ok(option_value(None));
    };
    let char_index = value[..byte_index].chars().count();
    Ok(option_value(Some(Value::Int(
        i64::try_from(char_index).unwrap_or(i64::MAX),
    ))))
}

pub(crate) fn char_at(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("char_at", args, 1)?;
    let value = string_value(receiver, heap, "method char_at")?;
    let index = index_value(&args[0], "method char_at")?;
    Ok(option_value(
        value
            .chars()
            .nth(index)
            .map(|ch| Value::String(ch.to_string())),
    ))
}

pub(crate) fn strip_prefix(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    strip_affix(
        receiver,
        args,
        heap,
        "strip_prefix",
        "method strip_prefix",
        str::strip_prefix,
    )
}

pub(crate) fn strip_suffix(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    strip_affix(
        receiver,
        args,
        heap,
        "strip_suffix",
        "method strip_suffix",
        str::strip_suffix,
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
    trim_with(receiver, args, heap, "trim", "method trim", str::trim)
}

pub(crate) fn trim_start(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    trim_with(
        receiver,
        args,
        heap,
        "trim_start",
        "method trim_start",
        str::trim_start,
    )
}

pub(crate) fn trim_end(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    trim_with(
        receiver,
        args,
        heap,
        "trim_end",
        "method trim_end",
        str::trim_end,
    )
}

pub(crate) fn replace(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("replace", args, 2)?;
    let value = string_value(receiver, heap, "method replace")?;
    let from = string_value(&args[0], heap, "method replace")?;
    let to = string_value(&args[1], heap, "method replace")?;
    Ok(Value::String(value.replace(from, to)))
}

pub(crate) fn repeat(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("repeat", args, 1)?;
    let value = string_value(receiver, heap, "method repeat")?;
    let count = index_value(&args[0], "method repeat")?;
    value.len().checked_mul(count).ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "method repeat",
        })
    })?;
    Ok(Value::String(value.repeat(count)))
}

pub(crate) fn slice(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("slice", args, 2)?;
    let value = string_value(receiver, heap, "method slice")?;
    let start = index_value(&args[0], "method slice")?;
    let end = index_value(&args[1], "method slice")?;
    let char_len = value.chars().count();
    if start > end {
        return type_error("method slice range");
    }
    if start > char_len {
        return Err(index_out_of_bounds(start, char_len));
    }
    if end > char_len {
        return Err(index_out_of_bounds(end, char_len));
    }

    let start_byte = char_byte_index(value, start);
    let end_byte = char_byte_index(value, end);
    Ok(Value::String(value[start_byte..end_byte].to_owned()))
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

pub(crate) fn split_lines(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_no_args("split_lines", args)?;
    let value = string_value(receiver, heap, "method split_lines")?;
    Ok(Value::Array(
        value
            .lines()
            .map(|line| Value::String(line.to_owned()))
            .collect(),
    ))
}

pub(crate) fn split_whitespace(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_no_args("split_whitespace", args)?;
    let value = string_value(receiver, heap, "method split_whitespace")?;
    Ok(Value::Array(
        value
            .split_whitespace()
            .map(|word| Value::String(word.to_owned()))
            .collect(),
    ))
}

pub(crate) fn is_string(value: &Value, heap: Option<&HeapExecution<'_>>) -> bool {
    match value {
        Value::String(_) => true,
        Value::HeapRef(reference) => matches!(
            heap.and_then(|heap| heap.heap.get(*reference)),
            Some(HeapValue::String(_))
        ),
        _ => false,
    }
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

fn strip_affix<'a>(
    receiver: &'a Value,
    args: &'a [Value],
    heap: Option<&'a HeapExecution<'_>>,
    method: &str,
    operation: &'static str,
    strip: impl FnOnce(&'a str, &'a str) -> Option<&'a str>,
) -> VmResult<Value> {
    expect_arity(method, args, 1)?;
    let value = string_value(receiver, heap, operation)?;
    let affix = string_value(&args[0], heap, operation)?;
    Ok(option_value(
        strip(value, affix).map(|stripped| Value::String(stripped.to_owned())),
    ))
}

fn trim_with<'a>(
    receiver: &'a Value,
    args: &[Value],
    heap: Option<&'a HeapExecution<'_>>,
    method: &str,
    operation: &'static str,
    trim: impl FnOnce(&'a str) -> &'a str,
) -> VmResult<Value> {
    expect_no_args(method, args)?;
    string_value(receiver, heap, operation)
        .map(trim)
        .map(str::to_owned)
        .map(Value::String)
}

pub(super) fn expect_no_args(method: &str, args: &[Value]) -> VmResult<()> {
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

fn index_value(value: &Value, operation: &'static str) -> VmResult<usize> {
    match value {
        Value::Int(value) if *value >= 0 => Ok(*value as usize),
        _ => type_error(operation),
    }
}

fn char_byte_index(value: &str, index: usize) -> usize {
    if index == 0 {
        return 0;
    }
    value
        .char_indices()
        .nth(index)
        .map_or(value.len(), |(byte, _)| byte)
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

    use crate::{ExecutionBudget, Value, Vm};

    #[test]
    fn runs_compiled_string_utility_methods() {
        let source = r#"
fn main() {
    let label = "  Quest.Log ";
    let padded = label.trim_start().trim_end();
    let parts = padded.replace(".", "_").to_lower().slice(0, 9).split("_");
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
    let pieces = event.trim_start().trim_end().replace(".", "_").to_lower().slice(0, 14).split("_");
    let marker = "!".repeat(3);
    if pieces[0] == "player"
        && pieces[1] == "levelup"
        && pieces[1].to_upper() == "LEVELUP"
        && marker == "!!!"
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

    #[test]
    fn string_repeat_builds_deterministic_strings() {
        let source = r#"
fn main() {
    let tag = "xp".repeat(3);
    let empty = "quest".repeat(0);
    if tag == "xpxpxp" && empty == "" {
        return "-".repeat(2);
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("string repeat source should compile");

        let result = Vm::new().run(&code).expect("string repeat should run");
        assert_eq!(result, Value::String("--".to_owned()));
    }

    #[test]
    fn string_repeat_rejects_negative_counts() {
        let source = r#"
fn main() {
    return "quest".repeat(-1);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("string repeat type error source should compile");

        let error = Vm::new()
            .run(&code)
            .expect_err("string repeat should reject negative counts");
        assert_eq!(
            error.kind,
            crate::VmErrorKind::TypeMismatch {
                operation: "method repeat"
            }
        );
    }

    #[test]
    fn string_slice_uses_character_indexes() {
        let source = r#"
fn main() {
    return "xp奖励".slice(2, 4);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("unicode string slice source should compile");

        let result = Vm::new()
            .run(&code)
            .expect("unicode string slice should run");
        assert_eq!(result, Value::String("奖励".to_owned()));
    }

    #[test]
    fn string_slice_rejects_out_of_bounds_ranges() {
        let source = r#"
fn main() {
    return "quest".slice(0, 10);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("out of bounds string slice source should compile");

        let error = Vm::new()
            .run(&code)
            .expect_err("string slice should reject out of bounds index");
        assert_eq!(
            error.kind,
            crate::VmErrorKind::IndexOutOfBounds { index: 10, len: 5 }
        );
    }

    #[test]
    fn string_find_returns_character_indexes_as_options() {
        let source = r#"
fn main() {
    let event = "xp奖励.done";
    let reward = event.find("奖励");
    let missing = event.find("missing");
    if option.unwrap_or(reward, -1) == 2
        && option.unwrap_or(missing, 99) == 99
    {
        return option.unwrap_or(event.find(".done"), -1);
    }
    return -1;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("string find source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("string find should run");
        assert_eq!(result, Value::Int(4));
    }

    #[test]
    fn managed_heap_execution_runs_string_find() {
        let source = r#"
fn main() {
    let name = "monster.wolf.alpha";
    return option.unwrap_or(name.find("wolf"), -1);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap string find source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap string find should run");
        assert_eq!(result, Value::Int(8));
    }

    #[test]
    fn string_find_rejects_non_string_needles() {
        let source = r#"
fn main() {
    return "quest".find(1);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("string find type error source should compile");

        let error = Vm::new()
            .run(&code)
            .expect_err("string find should reject non-string needles");
        assert_eq!(
            error.kind,
            crate::VmErrorKind::TypeMismatch {
                operation: "method find"
            }
        );
    }

    #[test]
    fn string_char_at_returns_character_options() {
        let source = r#"
fn main() {
    let label = "xp奖励";
    let first = label.char_at(0);
    let reward = label.char_at(2);
    let missing = label.char_at(99);
    if option.unwrap_or(first, "") == "x"
        && option.unwrap_or(reward, "") == "奖"
        && option.is_none(missing)
    {
        return option.unwrap_or(label.char_at(3), "");
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("string char_at source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("string char_at should run");
        assert_eq!(result, Value::String("励".to_owned()));
    }

    #[test]
    fn managed_heap_execution_runs_string_char_at() {
        let source = r#"
fn main() {
    let event = "level.up";
    return event.char_at(5).unwrap_or("");
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap string char_at source should compile");
        let mut budget = ExecutionBudget::unbounded();

        let result = Vm::new()
            .with_standard_natives()
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap string char_at should run");
        assert_eq!(result, Value::String(".".to_owned()));
    }

    #[test]
    fn string_char_at_rejects_negative_indexes() {
        let source = r#"
fn main() {
    return "quest".char_at(-1);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("string char_at type error source should compile");

        let error = Vm::new()
            .run(&code)
            .expect_err("string char_at should reject negative indexes");
        assert_eq!(
            error.kind,
            crate::VmErrorKind::TypeMismatch {
                operation: "method char_at"
            }
        );
    }

    #[test]
    fn string_strip_affixes_return_options() {
        let source = r#"
fn main() {
    let event = "quest.reward.done";
    let body = event.strip_prefix("quest.");
    let reward = option.unwrap_or(body, "").strip_suffix(".done");
    let missing = event.strip_prefix("player.");
    if option.unwrap_or(reward, "") == "reward"
        && option.is_none(missing)
    {
        return option.unwrap_or("奖励.done".strip_suffix(".done"), "");
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("string strip affix source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("string strip affix should run");
        assert_eq!(result, Value::String("奖励".to_owned()));
    }

    #[test]
    fn managed_heap_execution_runs_string_strip_affixes() {
        let source = r#"
fn main() {
    let event = "monster.wolf.alpha";
    let body = event.strip_prefix("monster.");
    return option.unwrap_or(option.unwrap_or(body, "").strip_suffix(".alpha"), "missing");
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap string strip affix source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap string strip affix should run");
        assert_eq!(result, Value::String("wolf".to_owned()));
    }

    #[test]
    fn string_strip_affixes_reject_non_string_affixes() {
        let source = r#"
fn main() {
    return "quest.reward".strip_prefix(1);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("string strip affix type error source should compile");

        let error = Vm::new()
            .run(&code)
            .expect_err("string strip affix should reject non-string affixes");
        assert_eq!(
            error.kind,
            crate::VmErrorKind::TypeMismatch {
                operation: "method strip_prefix"
            }
        );
    }

    #[test]
    fn string_split_lines_returns_line_array() {
        let source = r#"
fn main() {
    let lines = "quest\n\nreward\r\ndone\n".split_lines();
    if lines.len() == 4
        && lines[0] == "quest"
        && lines[1] == ""
        && lines[2] == "reward"
        && lines[3] == "done"
    {
        return lines[2];
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("string split_lines source should compile");

        let result = Vm::new().run(&code).expect("string split_lines should run");
        assert_eq!(result, Value::String("reward".to_owned()));
    }

    #[test]
    fn managed_heap_execution_runs_string_split_lines() {
        let source = r#"
fn main() {
    let log = "start\napply\ncommit";
    let lines = log.split_lines();
    if lines.len() == 3 && lines[1] == "apply" {
        return lines[2];
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap string split_lines source should compile");
        let mut budget = ExecutionBudget::unbounded();

        let result = Vm::new()
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap string split_lines should run");
        assert_eq!(result, Value::String("commit".to_owned()));
        assert_eq!(budget.memory_bytes_allocated(), 0);
    }

    #[test]
    fn string_split_whitespace_returns_words() {
        let source = r#"
fn main() {
    let words = "  player\tlevel_up\nreward  ".split_whitespace();
    if words.len() == 3
        && words[0] == "player"
        && words[1] == "level_up"
        && words[2] == "reward"
    {
        return words.join(".");
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("string split_whitespace source should compile");

        let mut vm = Vm::new();
        vm.register_standard_natives();
        let result = vm.run(&code).expect("string split_whitespace should run");
        assert_eq!(result, Value::String("player.level_up.reward".to_owned()));
    }

    #[test]
    fn managed_heap_execution_runs_string_split_whitespace() {
        let source = r#"
fn main() {
    let command = " grant   xp\t42 ";
    let words = command.split_whitespace();
    if words.len() == 3 && words[0] == "grant" {
        return [words[1], words[2]].join(":");
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap string split_whitespace source should compile");
        let mut budget = ExecutionBudget::unbounded();

        let result = Vm::new()
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap string split_whitespace should run");
        assert_eq!(result, Value::String("xp:42".to_owned()));
        assert_eq!(budget.memory_bytes_allocated(), 0);
    }

    #[test]
    fn string_parse_int_returns_options() {
        let source = r#"
fn main() {
    let level = "42".parse_int();
    let negative = "-7".parse_int();
    let invalid = "level-42".parse_int();
    let overflow = "9223372036854775808".parse_int();
    if option.unwrap_or(level, 0) == 42
        && option.unwrap_or(negative, 0) == -7
        && option.is_none(invalid)
        && option.is_none(overflow)
    {
        return option.unwrap_or("0".parse_int(), -1);
    }
    return -1;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("string parse_int source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("string parse_int should run");
        assert_eq!(result, Value::Int(0));
    }

    #[test]
    fn managed_heap_execution_runs_string_parse_int() {
        let source = r#"
fn main() {
    let raw = " 12 ";
    let parsed = raw.trim().parse_int();
    return option.unwrap_or(parsed, -1);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap string parse_int source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap string parse_int should run");
        assert_eq!(result, Value::Int(12));
    }

    #[test]
    fn string_parse_float_returns_finite_options() {
        let source = r#"
fn main() {
    let rate = "1.25".parse_float();
    let exponent = "2.5e1".parse_float();
    let invalid = "rate:1.25".parse_float();
    let infinite = "1e309".parse_float();
    if option.unwrap_or(rate, 0.0) == 1.25
        && option.unwrap_or(exponent, 0.0) == 25.0
        && option.is_none(invalid)
        && option.is_none(infinite)
    {
        return option.unwrap_or("-0.5".parse_float(), 1.0);
    }
    return 1.0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("string parse_float source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("string parse_float should run");
        assert_eq!(result, Value::Float(-0.5));
    }

    #[test]
    fn managed_heap_execution_runs_string_parse_float() {
        let source = r#"
fn main() {
    let raw = " 3.5 ";
    let parsed = raw.trim().parse_float();
    return math.floor(option.unwrap_or(parsed, -1.0));
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap string parse_float source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap string parse_float should run");
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn string_parse_bool_returns_options() {
        let source = r#"
fn main() {
    let enabled = "true".parse_bool();
    let disabled = "false".parse_bool();
    let uppercase = "TRUE".parse_bool();
    let yes = "yes".parse_bool();
    if option.unwrap_or(enabled, false)
        && !option.unwrap_or(disabled, true)
        && option.is_none(uppercase)
        && option.is_none(yes)
    {
        return option.unwrap_or("false".parse_bool(), true);
    }
    return true;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("string parse_bool source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("string parse_bool should run");
        assert_eq!(result, Value::Bool(false));
    }

    #[test]
    fn managed_heap_execution_runs_string_parse_bool() {
        let source = r#"
fn main() {
    let raw = " true ";
    let parsed = raw.trim().parse_bool();
    return option.unwrap_or(parsed, false);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap string parse_bool source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap string parse_bool should run");
        assert_eq!(result, Value::Bool(true));
    }
}

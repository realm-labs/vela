use crate::heap::HeapValue;
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult};

mod affix;
mod parsing;
mod search;
mod slicing;
mod splitting;
mod transform;

pub(crate) use affix::{strip_prefix, strip_suffix};
pub(crate) use parsing::{parse_bool, parse_float, parse_int};
pub(crate) use search::{char_at, contains, ends_with, find, starts_with};
pub(crate) use slicing::slice;
pub(crate) use splitting::{split, split_lines, split_once, split_whitespace};
pub(crate) use transform::{repeat, replace, to_lower, to_upper, trim, trim_end, trim_start};

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

pub(super) fn expect_no_args(method: &str, args: &[Value]) -> VmResult<()> {
    expect_arity(method, args, 0)
}

pub(super) fn expect_arity(method: &str, args: &[Value], expected: usize) -> VmResult<()> {
    if args.len() == expected {
        return Ok(());
    }
    Err(VmError::new(VmErrorKind::ArityMismatch {
        name: method.to_owned(),
        expected,
        actual: args.len(),
    }))
}

pub(super) fn index_value(value: &Value, operation: &'static str) -> VmResult<usize> {
    match value {
        Value::Int(value) if *value >= 0 => Ok(*value as usize),
        _ => type_error(operation),
    }
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
    fn string_split_once_returns_pair_options() {
        let source = r#"
fn main() {
    let pair = "count=3".split_once("=").unwrap_or(["", "0"]);
    let missing = "count".split_once("=").unwrap_or(["missing", "none"]);
    if pair[0] == "count" && pair[1] == "3" && missing[1] == "none" {
        return pair[1].parse_int().unwrap_or(0);
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("string split_once source should compile");

        let result = Vm::new().run(&code).expect("string split_once should run");
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn managed_heap_execution_runs_string_split_once() {
        let source = r#"
fn main() {
    let pair = "item:gold".split_once(":").unwrap_or(["item", "none"]);
    return pair[0] == "item" && pair[1] == "gold";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap string split_once source should compile");
        let mut budget = ExecutionBudget::unbounded();

        let result = Vm::new()
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap string split_once should run");
        assert_eq!(result, Value::Bool(true));
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

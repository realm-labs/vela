use super::*;

use vela_bytecode::UnlinkedProgram;
use vela_vm::error::{VmErrorKind, VmResult};

fn run_linked_program(
    engine: &Engine,
    program: &UnlinkedProgram,
    entry: &str,
    args: &[OwnedValue],
) -> VmResult<OwnedValue> {
    let linked = engine
        .link_program(program)
        .expect("engine compiler options test program should link");
    engine
        .into_vm_for_program(program)
        .run_linked_program(&linked, entry, args)
}

fn std_method_id(owner: &str, name: &str) -> vela_def::MethodId {
    let Some(id) = vela_stdlib::std_method_id(owner, name) else {
        panic!("missing standard method identity for {owner}::{name}");
    };
    id
}

#[test]
fn engine_installs_registered_native_functions_into_vm() {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::add", NativeFunctionId::new(1))
                .param("lhs", TypeHint::i64())
                .param("rhs", TypeHint::i64())
                .returns(TypeHint::i64())
                .effects(EffectSet::pure())
                .access(FunctionAccess::public())
                .docs("Adds two integers."),
            |args| {
                let [
                    OwnedValue::Scalar(vela_common::ScalarValue::I64(lhs)),
                    OwnedValue::Scalar(vela_common::ScalarValue::I64(rhs)),
                ] = args
                else {
                    return Ok(OwnedValue::Null);
                };
                Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(lhs + rhs)))
            },
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    return game::add(2, 3);
}
"#,
        )
        .expect("program should compile");

    assert_eq!(
        run_linked_program(&engine, &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(5)))
    );
}

#[test]
fn engine_compiler_options_lower_named_registered_native_arguments() {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::subtract", NativeFunctionId::new(27))
                .param("lhs", TypeHint::i64())
                .param("rhs", TypeHint::i64())
                .returns(TypeHint::i64())
                .effects(EffectSet::pure())
                .access(FunctionAccess::public()),
            |args| {
                let [
                    OwnedValue::Scalar(vela_common::ScalarValue::I64(lhs)),
                    OwnedValue::Scalar(vela_common::ScalarValue::I64(rhs)),
                ] = args
                else {
                    return Ok(OwnedValue::Null);
                };
                Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(lhs - rhs)))
            },
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    return game::subtract(rhs = 3, lhs = 10);
}
"#,
        )
        .expect("named registered native arguments should compile");

    assert_eq!(
        run_linked_program(&engine, &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(7)))
    );
}

#[test]
fn engine_compiler_options_lower_named_standard_native_arguments() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    return math::clamp(max = 10, value = 15, min = 1);
}
"#,
        )
        .expect("named stdlib native arguments should compile");

    assert_eq!(
        run_linked_program(&engine, &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(10)))
    );
}

#[test]
fn engine_compiler_options_emit_standard_native_ids() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    return math::clamp(max = 10, value = 15, min = 1);
}
"#,
        )
        .expect("standard native should compile");
    let main = program.function("main").expect("main should compile");

    let native = main
        .instructions
        .iter()
        .find_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::CallNative { name, native, .. } if name == "math::clamp" => {
                Some(*native)
            }
            _ => None,
        });

    let expected = vela_stdlib::STD_FUNCTIONS
        .iter()
        .find(|spec| spec.module == "math" && spec.name == "clamp")
        .map(|spec| spec.id());

    assert_eq!(native, expected);
}

#[test]
fn engine_compile_source_rejects_unregistered_native_function() {
    let engine = Engine::builder()
        .build()
        .expect("engine should build without native functions");
    let error = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    return game::missing(1);
}
"#,
        )
        .expect_err("unregistered native should fail during engine compilation");
    let crate::source::EngineSourceErrorKind::Compile(error) = error.kind else {
        panic!("expected compile error");
    };
    let vela_bytecode::compiler::error::CompileErrorKind::SemanticDiagnostics(diagnostics) =
        error.kind
    else {
        panic!("expected semantic diagnostics");
    };
    let codes = diagnostics
        .into_iter()
        .filter_map(|diagnostic| diagnostic.code)
        .collect::<Vec<_>>();

    assert_eq!(codes, ["compiler::unresolved_native_function"]);
}

#[test]
fn engine_compiler_options_emit_standard_value_method_ids() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    return "gold".len();
}
"#,
        )
        .expect("standard value method should compile");
    let main = program.function("main").expect("main should compile");

    let value_method = main
        .instructions
        .iter()
        .find_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::CallMethodId {
                method, method_id, ..
            } if method == "len" => Some(*method_id),
            _ => None,
        });

    assert_eq!(value_method, Some(std_method_id("String", "len")));
}

#[test]
fn engine_compiler_options_emit_standard_string_predicate_method_ids() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    return "reward:gold".contains(":")
        && "reward:gold".starts_with("reward")
        && "reward:gold".ends_with("gold");
}
"#,
        )
        .expect("standard string predicate methods should compile");
    let main = program.function("main").expect("main should compile");

    let value_methods = main
        .instructions
        .iter()
        .filter_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::CallMethodId {
                method, method_id, ..
            } => Some((method.as_str(), Some(*method_id))),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(value_methods.contains(&("contains", Some(std_method_id("String", "contains")))));
    assert!(value_methods.contains(&("starts_with", Some(std_method_id("String", "starts_with")))));
    assert!(value_methods.contains(&("ends_with", Some(std_method_id("String", "ends_with")))));
}

#[test]
fn engine_compiler_options_emit_standard_string_transform_method_ids() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    let label = " Reward ";
    return label.to_upper() == " REWARD "
        && label.to_lower() == " reward "
        && label.trim() == "Reward"
        && label.trim_start() == "Reward "
        && label.trim_end() == " Reward";
}
"#,
        )
        .expect("standard string transform methods should compile");
    let main = program.function("main").expect("main should compile");

    let value_methods = main
        .instructions
        .iter()
        .filter_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::CallMethodId {
                method, method_id, ..
            } => Some((method.as_str(), Some(*method_id))),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(value_methods.contains(&("to_upper", Some(std_method_id("String", "to_upper")))));
    assert!(value_methods.contains(&("to_lower", Some(std_method_id("String", "to_lower")))));
    assert!(value_methods.contains(&("trim", Some(std_method_id("String", "trim")))));
    assert!(value_methods.contains(&("trim_start", Some(std_method_id("String", "trim_start")))));
    assert!(value_methods.contains(&("trim_end", Some(std_method_id("String", "trim_end")))));
}

#[test]
fn engine_compiler_options_emit_standard_string_argument_transform_method_ids() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    let label = "reward.gold";
    return label.replace(".", ":") == "reward:gold"
        && "xp".repeat(3) == "xpxpxp"
        && label.slice(0, 6) == "reward";
}
"#,
        )
        .expect("standard string argument transform methods should compile");
    let main = program.function("main").expect("main should compile");

    let value_methods = main
        .instructions
        .iter()
        .filter_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::CallMethodId {
                method, method_id, ..
            } => Some((method.as_str(), Some(*method_id))),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(value_methods.contains(&("replace", Some(std_method_id("String", "replace")))));
    assert!(value_methods.contains(&("repeat", Some(std_method_id("String", "repeat")))));
    assert!(value_methods.contains(&("slice", Some(std_method_id("String", "slice")))));
}

#[test]
fn engine_compiler_options_emit_standard_string_option_method_ids() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    let label = "reward:gold";
    return label.find(":").unwrap_or(-1) == 6
        && label.strip_prefix("reward:").unwrap_or("") == "gold"
        && label.strip_suffix(":gold").unwrap_or("") == "reward"
        && label.char_at(6).unwrap_or("") == ":";
}
"#,
        )
        .expect("standard string option methods should compile");
    let main = program.function("main").expect("main should compile");

    let value_methods = main
        .instructions
        .iter()
        .filter_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::CallMethodId {
                method, method_id, ..
            } => Some((method.as_str(), Some(*method_id))),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(value_methods.contains(&("find", Some(std_method_id("String", "find")))));
    assert!(value_methods.contains(&(
        "strip_prefix",
        Some(std_method_id("String", "strip_prefix"))
    )));
    assert!(value_methods.contains(&(
        "strip_suffix",
        Some(std_method_id("String", "strip_suffix"))
    )));
    assert!(value_methods.contains(&("char_at", Some(std_method_id("String", "char_at")))));
}

#[test]
fn engine_compiler_options_emit_standard_string_split_method_ids() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    let pair = "reward:gold".split_once(":").unwrap_or(["", ""]);
    return "reward:gold".split(":").len() == 2
        && pair[1] == "gold"
        && "reward\ngold".split_lines().len() == 2
        && "reward gold".split_whitespace().len() == 2;
}
"#,
        )
        .expect("standard string split methods should compile");
    let main = program.function("main").expect("main should compile");

    let value_methods = main
        .instructions
        .iter()
        .filter_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::CallMethodId {
                method, method_id, ..
            } => Some((method.as_str(), Some(*method_id))),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(value_methods.contains(&("split", Some(std_method_id("String", "split")))));
    assert!(value_methods.contains(&("split_once", Some(std_method_id("String", "split_once")))));
    assert!(value_methods.contains(&("split_lines", Some(std_method_id("String", "split_lines")))));
    assert!(value_methods.contains(&(
        "split_whitespace",
        Some(std_method_id("String", "split_whitespace"))
    )));
}

#[test]
fn engine_links_standard_methods_after_indexed_collection_shapes() {
    let engine = Engine::builder()
        .with_standard_natives()
        .reflection_policy(vela_reflect::permissions::ReflectPolicy::all())
        .build()
        .expect("engine should build with standard natives");
    for (source, text) in [
        (
            SourceId::new(1),
            include_str!("../../../../../examples/src/bin/gameplay_helpers/gameplay_helpers.vela"),
        ),
        (
            SourceId::new(2),
            include_str!(
                "../../../../../examples/src/bin/random_reflect_allowed/random_reflect_allowed.vela"
            ),
        ),
        (
            SourceId::new(3),
            include_str!("../../../../../examples/src/bin/reflect_debug/reflect_debug.vela"),
        ),
    ] {
        let program = engine
            .compile_source(source, text)
            .expect("example stdlib method chain should compile");

        engine
            .link_program(&program)
            .expect("example stdlib method chain should link");
    }
}

#[test]
fn engine_compiler_options_emit_standard_string_parse_method_ids() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    return "42".parse_int().unwrap_or(0) == 42
        && "1.5".parse_float().unwrap_or(0.0) == 1.5
        && "true".parse_bool().unwrap_or(false);
}
"#,
        )
        .expect("standard string parse methods should compile");
    let main = program.function("main").expect("main should compile");

    let value_methods = main
        .instructions
        .iter()
        .filter_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::CallMethodId {
                method, method_id, ..
            } => Some((method.as_str(), Some(*method_id))),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(value_methods.contains(&("parse_int", Some(std_method_id("String", "parse_int")))));
    assert!(value_methods.contains(&("parse_float", Some(std_method_id("String", "parse_float")))));
    assert!(value_methods.contains(&("parse_bool", Some(std_method_id("String", "parse_bool")))));
}

#[test]
fn engine_compiler_options_emit_standard_range_method_ids() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    return (1..4).len();
}
"#,
        )
        .expect("standard range method should compile");
    let main = program.function("main").expect("main should compile");

    let value_method = main
        .instructions
        .iter()
        .find_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::CallMethodId {
                method, method_id, ..
            } if method == "len" => Some(*method_id),
            _ => None,
        });

    assert_eq!(value_method, Some(std_method_id("Range", "len")));
}

#[test]
fn engine_compiler_options_emit_standard_option_result_method_ids() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    let some: Option = option::some(1);
    let err: Result = result::err("bad");
    return some.is_some() && err.is_err();
}
"#,
        )
        .expect("standard option/result methods should compile");
    let main = program.function("main").expect("main should compile");

    let value_methods = main
        .instructions
        .iter()
        .filter_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::CallMethodId {
                method, method_id, ..
            } => Some((method.as_str(), Some(*method_id))),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(value_methods.contains(&("is_some", Some(std_method_id("Option", "is_some")))));
    assert!(value_methods.contains(&("is_err", Some(std_method_id("Result", "is_err")))));
}

#[test]
fn engine_compile_source_emits_standard_value_method_ids_from_registry() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    let names: array = ["gold", "xp"];
    let rewards: map = {"gold": 4};
    let tags: set = set::from_array(["daily"]);
    let some: Option = option::some(1);
    let err: Result = result::err("bad");
    if some.is_some() && err.is_err() {
        return "gold".len()
            + names.len()
            + rewards.len()
            + tags.len()
            + (1..4).len();
    }
    return 0;
}
"#,
        )
        .expect("standard value methods should compile from registry");
    let main = program.function("main").expect("main should compile");

    let value_methods = main
        .instructions
        .iter()
        .filter_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::CallMethodId {
                method, method_id, ..
            } => Some((method.as_str(), Some(*method_id))),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(value_methods.contains(&("len", Some(std_method_id("String", "len")))));
    assert!(value_methods.contains(&("len", Some(std_method_id("Array", "len")))));
    assert!(value_methods.contains(&("len", Some(std_method_id("Map", "len")))));
    assert!(value_methods.contains(&("len", Some(std_method_id("Set", "len")))));
    assert!(value_methods.contains(&("len", Some(std_method_id("Range", "len")))));
    assert!(value_methods.contains(&("is_some", Some(std_method_id("Option", "is_some")))));
    assert!(value_methods.contains(&("is_err", Some(std_method_id("Result", "is_err")))));
}

#[test]
fn engine_compiler_options_emit_standard_collection_method_ids() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    let names: array = ["gold", "xp"];
    let rewards: map = {"gold": 4};
    let tags: set = set::from_array(["daily"]);
    let other: set = set::from_array(["raid"]);
    names.push("bonus");
    names.pop();
    rewards.set("xp", 6);
    rewards.remove("xp");
    tags.add("bonus");
    tags.remove("bonus");
    if names.contains("gold")
        && rewards.has("gold")
        && tags.has("daily")
        && tags.is_subset(tags)
        && tags.is_superset(tags)
        && tags.is_disjoint(other)
    {
        names.clear();
        rewards.clear();
        tags.clear();
        return names.len() + rewards.len() + tags.len();
    }
    return 0;
}
"#,
        )
        .expect("standard collection methods should compile");
    let main = program.function("main").expect("main should compile");

    let value_methods = main
        .instructions
        .iter()
        .filter_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::CallMethodId {
                method, method_id, ..
            } => Some((method.as_str(), Some(*method_id))),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(value_methods.contains(&("len", Some(std_method_id("Array", "len")))));
    assert!(value_methods.contains(&("len", Some(std_method_id("Map", "len")))));
    assert!(value_methods.contains(&("len", Some(std_method_id("Set", "len")))));
    assert!(value_methods.contains(&("contains", Some(std_method_id("Array", "contains")))));
    assert!(value_methods.contains(&("push", Some(std_method_id("Array", "push")))));
    assert!(value_methods.contains(&("pop", Some(std_method_id("Array", "pop")))));
    assert!(value_methods.contains(&("clear", Some(std_method_id("Array", "clear")))));
    assert!(value_methods.contains(&("has", Some(std_method_id("Map", "has")))));
    assert!(value_methods.contains(&("set", Some(std_method_id("Map", "set")))));
    assert!(value_methods.contains(&("remove", Some(std_method_id("Map", "remove")))));
    assert!(value_methods.contains(&("clear", Some(std_method_id("Map", "clear")))));
    assert!(value_methods.contains(&("has", Some(std_method_id("Set", "has")))));
    assert!(value_methods.contains(&("add", Some(std_method_id("Set", "add")))));
    assert!(value_methods.contains(&("remove", Some(std_method_id("Set", "remove")))));
    assert!(value_methods.contains(&("clear", Some(std_method_id("Set", "clear")))));
    assert!(value_methods.contains(&("is_subset", Some(std_method_id("Set", "is_subset")))));
    assert!(value_methods.contains(&("is_superset", Some(std_method_id("Set", "is_superset")))));
    assert!(value_methods.contains(&("is_disjoint", Some(std_method_id("Set", "is_disjoint")))));
}

#[test]
fn engine_compiler_options_emit_standard_array_lookup_method_ids() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    let names: array = ["gold", "xp"];
    return names.first().unwrap_or("") == "gold"
        && names.last().unwrap_or("") == "xp"
        && names.index_of("xp").unwrap_or(-1) == 1;
}
"#,
        )
        .expect("standard array lookup methods should compile");
    let main = program.function("main").expect("main should compile");

    let value_methods = main
        .instructions
        .iter()
        .filter_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::CallMethodId {
                method, method_id, ..
            } => Some((method.as_str(), Some(*method_id))),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(value_methods.contains(&("first", Some(std_method_id("Array", "first")))));
    assert!(value_methods.contains(&("last", Some(std_method_id("Array", "last")))));
    assert!(value_methods.contains(&("index_of", Some(std_method_id("Array", "index_of")))));
}

#[test]
fn engine_compiler_options_emit_standard_array_transform_method_ids() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    let names: array = ["gold", "xp", "gold"];
    return names.join(":") == "gold:xp:gold"
        && names.distinct().len() == 2
        && names.reverse()[0] == "gold"
        && names.slice(1, 3).len() == 2;
}
"#,
        )
        .expect("standard array transform methods should compile");
    let main = program.function("main").expect("main should compile");

    let value_methods = main
        .instructions
        .iter()
        .filter_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::CallMethodId {
                method, method_id, ..
            } => Some((method.as_str(), Some(*method_id))),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(value_methods.contains(&("join", Some(std_method_id("Array", "join")))));
    assert!(value_methods.contains(&("distinct", Some(std_method_id("Array", "distinct")))));
    assert!(value_methods.contains(&("reverse", Some(std_method_id("Array", "reverse")))));
    assert!(value_methods.contains(&("slice", Some(std_method_id("Array", "slice")))));
}

#[test]
fn engine_compiler_options_lower_named_standard_value_method_arguments() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    let pair = "reward:gold".split_once(separator = ":").unwrap_or(["", ""]);
    return {"gold": 4}.get_or(default = 0, key = pair[1]);
}
"#,
        )
        .expect("named stdlib value method arguments should compile");

    assert_eq!(
        run_linked_program(&engine, &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(4)))
    );
}

#[test]
fn engine_compiler_options_lower_receiver_specific_named_standard_value_method_arguments() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    return "reward:gold".contains(needle = ":") && ["gold"].contains(value = "gold");
}
"#,
        )
        .expect("receiver-specific named stdlib value method arguments should compile");

    assert_eq!(
        run_linked_program(&engine, &program, "main", &[]),
        Ok(OwnedValue::Bool(true))
    );
}

#[test]
fn engine_compiler_options_lower_local_receiver_named_standard_value_method_arguments() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(text: string) {
    let parts = ["gold"];
    let reward = "reward:gold";
    return text.contains(needle = ":")
        && reward.contains(needle = ":")
        && parts.contains(value = "gold");
}
"#,
        )
        .expect("local receiver named stdlib value method arguments should compile");

    assert_eq!(
        run_linked_program(
            &engine,
            &program,
            "main",
            &[OwnedValue::String("loot:xp".to_owned())]
        ),
        Ok(OwnedValue::Bool(true))
    );
}

#[test]
fn engine_compiler_options_preserve_unknown_receiver_named_method_arguments() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(value) {
    return value.contains(needle = ":");
}
"#,
        )
        .expect("unknown receiver named method arguments should compile dynamically");

    let error = run_linked_program(
        &engine,
        &program,
        "main",
        &[OwnedValue::String("loot:xp".to_owned())],
    )
    .expect_err("standard dynamic named args should fail at runtime until resolved metadata lands");
    assert!(matches!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "dynamic method named arguments"
        }
    ));
    assert!(error.source_span.is_some());
}

#[test]
fn engine_builder_installs_standard_natives_into_runtime() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = engine
        .compile_source(
        SourceId::new(1),
        r#"
fn main() {
    set::from_array(["fire", "ice", "fire"]);
    let midpoint = math::floor(math::lerp(10, 20, 0.5));
    let range = math::round(math::distance3d(0, 0, 0, 2, 3, 6));
    let score = math::pow(2, 3);
    let root = math::round(math::sqrt(81));
    let direction = math::sign(-3);
    let approach = math::move_towards(0, 10, 4);
    return option::unwrap_or(option::some(midpoint), 0) + math::round(1.5) + range + score + root + direction + approach;
}
"#,
        )
    .expect("program should compile");
    engine
        .link_program(&program)
        .expect("engine-compiled standard native program should link");
    let mut runtime = Runtime::new(engine, program);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    let result = runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx);
    assert_eq!(
        result,
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(44))),
    );
}

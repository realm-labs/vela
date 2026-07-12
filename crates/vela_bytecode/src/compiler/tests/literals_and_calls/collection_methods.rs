use crate::compiler::{compile_test_function, compile_test_program_with_registry};
use crate::{DynamicCallArgument, UnlinkedCodeObject, UnlinkedInstructionKind};
use vela_common::SourceId;

use super::value_method_registry;
use crate::compiler::tests::semantic_diagnostic_codes;

#[test]
fn compiler_rejects_static_record_array_sort_without_ord() {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let error = compile_test_program_with_registry(
        SourceId::new(1),
        r#"
struct Score { value: i64 }

fn main() {
    let values = [Score { value: 2 }, Score { value: 1 }];
    return values.sort();
}
"#,
        registry.compile_view(),
    )
    .expect_err("known record array sort without Ord should be a compile error");

    assert_eq!(
        semantic_diagnostic_codes(error),
        ["compiler::missing_ord_for_array_ordering"]
    );
}

#[test]
fn compiler_rejects_static_record_array_extrema_without_ord() {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let error = compile_test_program_with_registry(
        SourceId::new(1),
        r#"
struct Score { value: i64 }

fn main() {
    let values = [Score { value: 2 }, Score { value: 1 }];
    return values.min();
}
"#,
        registry.compile_view(),
    )
    .expect_err("known record array extrema without Ord should be a compile error");

    assert_eq!(
        semantic_diagnostic_codes(error),
        ["compiler::missing_ord_for_array_ordering"]
    );
}

#[test]
fn compiler_accepts_static_record_array_sort_with_derived_ord() {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    compile_test_program_with_registry(
        SourceId::new(1),
        r#"
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Score { value: i64 }

fn main() {
    let values = [Score { value: 2 }, Score { value: 1 }];
    return values.sort();
}
"#,
        registry.compile_view(),
    )
    .expect("known record array sort with derived Ord should compile");
}

#[test]
fn compiler_rejects_static_float_array_sort_without_ord() {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let error = compile_test_program_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    let values: Array<f64> = [2.0, 1.0];
    return values.sort();
}
"#,
        registry.compile_view(),
    )
    .expect_err("known float array sort without Ord should be a compile error");

    assert_eq!(
        semantic_diagnostic_codes(error),
        ["compiler::missing_ord_for_array_ordering"]
    );
}

#[test]
fn compiler_rejects_static_float_array_sort_by_key_without_ord() {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let error = compile_test_program_with_registry(
        SourceId::new(1),
        r#"
struct Score { value: f64 }

fn main() {
    let values = [Score { value: 2.0 }, Score { value: 1.0 }];
    return values.sort_by(|score| score.value);
}
"#,
        registry.compile_view(),
    )
    .expect_err("known float array sort_by key without Ord should be a compile error");

    assert_eq!(
        semantic_diagnostic_codes(error),
        ["compiler::missing_ord_for_array_ordering"]
    );
}

#[test]
fn compiler_rejects_static_record_array_sort_by_key_without_ord() {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let error = compile_test_program_with_registry(
        SourceId::new(1),
        r#"
struct Rank { value: i64 }
struct Score { rank: Rank }

fn main() {
    let values = [Score { rank: Rank { value: 2 } }, Score { rank: Rank { value: 1 } }];
    return values.sort_by(|score| score.rank);
}
"#,
        registry.compile_view(),
    )
    .expect_err("known record array sort_by key without Ord should be a compile error");

    assert_eq!(
        semantic_diagnostic_codes(error),
        ["compiler::missing_ord_for_array_ordering"]
    );
}

#[test]
fn compiler_lowers_value_method_ids_after_set_values_method() {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let program = compile_test_program_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    let numbers = set::from_array([1, 2, 3]);
    let tags = set::from_array(["raid", "daily"]);
    return numbers.values().collect_array().sum()
        + tags.values().collect_array().sort_by(|tag| tag).join(",").len();
}
"#,
        registry.compile_view(),
    )
    .expect("set values array methods should compile");
    let main = program.function("main").expect("main function");
    let methods = nested_method_id_names(main);

    assert!(main.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        UnlinkedInstructionKind::MakeSetFromArray { .. }
    )));
    assert!(methods.iter().any(|method| method == "values"));
    assert!(methods.iter().any(|method| method == "sum"));
    assert!(methods.iter().any(|method| method == "sort_by"));
    assert!(methods.iter().any(|method| method == "join"));
    assert!(methods.iter().any(|method| method == "len"));
}

#[test]
fn compiler_lowers_set_method_ids_after_mixed_string_shapes() {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let program = compile_test_program_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    let (_, item) = "reward:gold".split_once(":").unwrap_or(("reward", ""));
    let tags = set::from_array(["reward", item, "daily", item]);
    return tags.has("gold");
}
"#,
        registry.compile_view(),
    )
    .expect("mixed literal and indexed string set methods should compile");
    let main = program.function("main").expect("main function");
    let methods = nested_method_id_names(main);

    assert!(methods.iter().any(|method| method == "split_once"));
    assert!(methods.iter().any(|method| method == "unwrap_or"));
    assert!(methods.iter().any(|method| method == "has"));
}

#[test]
fn compiler_lowers_value_method_ids_after_string_find_method() {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let program = compile_test_program_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    return "level.up".find(".").unwrap_or(-1);
}
"#,
        registry.compile_view(),
    )
    .expect("string find option method should compile");
    let main = program.function("main").expect("main function");
    let methods = nested_method_id_names(main);

    assert!(methods.iter().any(|method| method == "find"));
    assert!(methods.iter().any(|method| method == "unwrap_or"));
}

#[test]
fn compiler_lowers_value_method_ids_for_string_sequence_methods() {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let program = compile_test_program_with_registry(
        SourceId::new(1),
        r##"
fn main() {
    let chars = "a奖励".chars();
    let bytes = "AZ".bytes();
    return chars.count() + bytes.count();
}
"##,
        registry.compile_view(),
    )
    .expect("string sequence methods should compile");
    let main = program.function("main").expect("main function");
    let methods = nested_method_id_names(main);

    assert!(methods.iter().any(|method| method == "chars"));
    assert!(methods.iter().any(|method| method == "bytes"));
}

#[test]
fn compiler_uses_stable_std_value_method_targets_without_registry() {
    let code = compile_test_function(
        SourceId::new(1),
        r##"
fn main() {
    let chars = "a奖励".chars();
    let bytes = "AZ".bytes();
    return chars.count() + bytes.count();
}
"##,
        "main",
    )
    .expect("string sequence methods should compile without registry");
    let methods = nested_method_id_names(&code);

    assert!(methods.iter().any(|method| method == "chars"));
    assert!(methods.iter().any(|method| method == "bytes"));
    assert_eq!(
        methods
            .iter()
            .filter(|method| method.as_str() == "count")
            .count(),
        2
    );
}

#[test]
fn compiler_lowers_value_method_ids_after_reflection_metadata_collections() {
    let mut registry = vela_stdlib::standard_registry().expect("standard registry should build");
    for (name, params) in [
        ("type_info", &["name"][..]),
        ("function", &["name"]),
        ("functions", &[]),
        ("effects", &["target"]),
        ("fields", &["target"]),
        ("params", &["target"]),
        ("methods", &["target"]),
        ("method", &["target", "name"]),
        ("variants", &["target"]),
    ] {
        registry
            .register_function(vela_registry::FunctionDef::new(
                vela_def::DefPath::function("host", ["reflect"], name),
                vela_registry::FunctionSignature::new(
                    params
                        .iter()
                        .map(|param| vela_registry::ParamDef::new(*param, None::<String>)),
                    None::<String>,
                ),
            ))
            .expect("test reflection native should register");
    }
    let program = compile_test_program_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    let target = reflect::type_info("Context");
    let option_type = reflect::type_info("Option");
    let fields = reflect::fields(target);
    let methods = reflect::methods(target);
    let functions = reflect::functions();
    let variants = reflect::variants(option_type);
    let emit = reflect::method(target, "emit");
    let random = reflect::function("math::random");
    let random_params = reflect::params(random);
    let effects = reflect::effects(random);
    return fields.len() > 0
        && methods.len() > 0
        && fields[0].name.len() > 0
        && fields[0].access.reflect_readable
        && functions[0].name.len() > 0
        && random.public
        && random.access.reflect_visible
        && random.access.required_permissions.len() == 0
        && random_params.len() == 0
        && emit.owner.len() > 0
        && emit.access.reflect_callable
        && emit.params[0].name.len() > 0
        && emit.params[0].defaulted == false
        && variants[0].name.len() > 0
        && variants[0].fields[0].name.len() > 0
        && effects.uses_random
        && !effects.reads_host;
}
"#,
        registry.compile_view(),
    )
    .expect("reflection metadata collection value methods should compile");
    let main = program.function("main").expect("main function");
    let methods = nested_method_id_names(main);
    let record_fields = main
        .instructions
        .iter()
        .filter_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::GetRecordSlot { field, .. } => Some(field.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(methods.iter().any(|method| method == "len"));
    assert!(record_fields.contains(&"name"));
    assert!(record_fields.contains(&"owner"));
    assert!(record_fields.contains(&"public"));
    assert!(record_fields.contains(&"reflect_callable"));
    assert!(record_fields.contains(&"reflect_readable"));
    assert!(record_fields.contains(&"reflect_visible"));
    assert!(record_fields.contains(&"required_permissions"));
    assert!(record_fields.contains(&"defaulted"));
    assert!(record_fields.contains(&"uses_random"));
    assert!(record_fields.contains(&"reads_host"));
}

#[test]
fn compiler_lowers_value_method_ids_in_option_result_callback_params() {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let program = compile_test_program_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    let option_chain = option::some("quest")
        .map(|value| value.to_upper())
        .filter(|value| value.starts_with("Q"));
    let result_chain = result::ok(["gold", "xp"])
        .map(|values| values.join("+"))
        .and_then(|text| result::ok(text.replace("+", ".")));
    let mapped_err = result::err(["bad", "level"]).map_err(|errors| errors.join("."));
    return option_chain.unwrap_or("")
        + result_chain.unwrap_or("")
        + mapped_err.to_error_option().unwrap_or("");
}
"#,
        registry.compile_view(),
    )
    .expect("Option/Result callback parameter value methods should compile");
    let main = program.function("main").expect("main function");
    let methods = nested_method_id_names(main);

    assert!(methods.iter().any(|method| method == "to_upper"));
    assert!(methods.iter().any(|method| method == "starts_with"));
    assert!(methods.iter().any(|method| method == "join"));
    assert!(methods.iter().any(|method| method == "replace"));
}

#[test]
fn compiler_preserves_named_dynamic_method_args_after_for_body_receiver_fact_expires() {
    let registry = value_method_registry(&[
        ("String", "contains", &["needle"]),
        ("Array", "contains", &["value"]),
    ]);
    let program = compile_test_program_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    let value = 1;
    for item in [] {
        value = "reward:gold";
    }
    return value.contains(needle = ":");
}
"#,
        registry.compile_view(),
    )
    .expect("expired receiver facts should compile to dynamic method dispatch");
    let main = program.function("main").expect("main function");
    let args = dynamic_method_args(main, "contains").expect("dynamic contains call");
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].name.as_deref(), Some("needle"));
}

pub(super) fn nested_method_id_names(code: &UnlinkedCodeObject) -> Vec<String> {
    let mut methods = Vec::new();
    collect_nested_method_id_names(code, &mut methods);
    methods
}

fn collect_nested_method_id_names(code: &UnlinkedCodeObject, methods: &mut Vec<String>) {
    for instruction in &code.instructions {
        if let UnlinkedInstructionKind::CallMethodId { method, .. } = &instruction.kind {
            methods.push(method.clone());
        }
    }
    for nested in &code.nested_functions {
        collect_nested_method_id_names(nested, methods);
    }
}

#[test]
fn compiler_preserves_named_dynamic_method_args_after_match_arm_receiver_fact_expires() {
    let registry = value_method_registry(&[
        ("String", "contains", &["needle"]),
        ("Array", "contains", &["value"]),
    ]);
    let program = compile_test_program_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    let value = 1;
    match value {
        1 => {
            value = "reward:gold";
        }
        _ => {}
    }
    return value.contains(needle = ":");
}
"#,
        registry.compile_view(),
    )
    .expect("expired receiver facts should compile to dynamic method dispatch");
    let main = program.function("main").expect("main function");
    let args = dynamic_method_args(main, "contains").expect("dynamic contains call");
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].name.as_deref(), Some("needle"));
}

#[test]
fn compiler_preserves_named_dynamic_method_args_without_receiver_type() {
    let registry = value_method_registry(&[
        ("String", "contains", &["needle"]),
        ("Array", "contains", &["value"]),
    ]);
    let program = compile_test_program_with_registry(
        SourceId::new(1),
        r#"
fn main(value) {
    return value.contains(needle = ":");
}
"#,
        registry.compile_view(),
    )
    .expect("unknown receiver named method args should compile dynamically");
    let main = program.function("main").expect("main function");
    let args = dynamic_method_args(main, "contains").expect("dynamic contains call");
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].name.as_deref(), Some("needle"));
}

pub(super) fn dynamic_method_args<'a>(
    code: &'a UnlinkedCodeObject,
    name: &str,
) -> Option<&'a [DynamicCallArgument]> {
    for instruction in &code.instructions {
        if let UnlinkedInstructionKind::CallDynamicMethod { method, args, .. } = &instruction.kind
            && method == name
        {
            return Some(args);
        }
    }
    None
}

use super::*;

#[test]
fn compiler_lowers_value_method_ids_after_array_endpoint_methods() {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let program = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    let values = [4, 1, 3, 1];
    return values.first().unwrap_or(0) + values.last().unwrap_or(0);
}
"#,
        registry.compile_view(),
    )
    .expect("array endpoint option methods should compile");
    let main = program.function("main").expect("main function");
    let methods = nested_method_id_names(main);

    assert!(methods.iter().any(|method| method == "first"));
    assert!(methods.iter().any(|method| method == "last"));
    assert_eq!(
        methods
            .iter()
            .filter(|method| method.as_str() == "unwrap_or")
            .count(),
        2
    );
}

#[test]
fn compiler_lowers_value_method_ids_after_array_slice_method() {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let program = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    let values = [1, 2, 3, 4];
    return values.slice(1, 3).sum();
}
"#,
        registry.compile_view(),
    )
    .expect("array slice value methods should compile");
    let main = program.function("main").expect("main function");
    let unresolved = nested_method_names(main);
    let methods = nested_method_id_names(main);

    assert!(
        unresolved.is_empty(),
        "expected slice chain to lower method IDs, unresolved: {unresolved:?}"
    );
    assert!(methods.iter().any(|method| method == "slice"));
    assert!(methods.iter().any(|method| method == "sum"));
}

#[test]
fn compiler_lowers_value_method_ids_after_computed_array_slice_method() {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let program = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    let total = 0;
    for tick in 0..64 {
        let values = [
            tick, tick + 1, tick + 2, tick + 3,
            tick + 4, tick + 5, tick + 6, tick + 7,
            tick + 8, tick + 9, tick + 10, tick + 11,
        ];
        let middle = values.slice(3, 7);
        let tail = values.slice(8, 12);
        total += middle.sum() + tail.sum();
    }
    return total;
}
"#,
        registry.compile_view(),
    )
    .expect("computed array slice value methods should compile");
    let main = program.function("main").expect("main function");
    let unresolved = nested_method_names(main);
    let methods = nested_method_id_names(main);

    assert!(
        unresolved.is_empty(),
        "expected computed slice chain to lower method IDs, unresolved: {unresolved:?}"
    );
    assert_eq!(
        methods
            .iter()
            .filter(|method| method.as_str() == "sum")
            .count(),
        2
    );
}

#[test]
fn compiler_lowers_value_method_ids_after_map_get_or_method() {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let program = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    let groups = {"w": ["wolf", "wisp"], "b": ["bat"]};
    return groups.get_or("w", []).len() + groups.get("b").unwrap_or([]).len();
}
"#,
        registry.compile_view(),
    )
    .expect("map lookup value methods should compile");
    let main = program.function("main").expect("main function");
    let methods = nested_method_id_names(main);

    assert!(methods.iter().any(|method| method == "get_or"));
    assert!(methods.iter().any(|method| method == "get"));
    assert!(methods.iter().any(|method| method == "unwrap_or"));
    assert_eq!(
        methods
            .iter()
            .filter(|method| method.as_str() == "len")
            .count(),
        2
    );
}

#[test]
fn compiler_lowers_value_method_ids_after_result_to_option_method() {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let program = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    let converted = option::some(["quest", "done"]).ok_or(["missing"]);
    return converted.to_option().unwrap_or([]).join(".");
}
"#,
        registry.compile_view(),
    )
    .expect("result to_option value methods should compile");
    let main = program.function("main").expect("main function");
    let methods = nested_method_id_names(main);

    assert!(methods.iter().any(|method| method == "ok_or"));
    assert!(methods.iter().any(|method| method == "to_option"));
    assert!(methods.iter().any(|method| method == "unwrap_or"));
    assert!(methods.iter().any(|method| method == "join"));
}

#[test]
fn compiler_lowers_value_method_ids_in_option_result_helper_chains() {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let program = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    let some = option::some(["quest", "done"]);
    let none = option::none();
    let ok = result::ok(["done"]);
    let err = result::err(["blocked"]);
    let converted_ok = some.ok_or(["missing"]);
    let converted_err = none.ok_or(["missing"]);
    let flattened_some = option::some(option::some(["quest", "done"])).flatten();
    let flattened_ok = result::ok(result::ok(["done"])).flatten();
    return some.unwrap_or([]).join(".")
        + none.unwrap_or(["fallback"]).join(".")
        + ok.unwrap_or([]).join(".")
        + err.unwrap_or(["fallback"]).join(".")
        + converted_ok.to_option().unwrap_or([]).join(".")
        + converted_err.to_option().unwrap_or(["fallback"]).join(".")
        + converted_err.to_error_option().unwrap_or(["fallback"]).join(".")
        + flattened_some.unwrap_or([]).join(".")
        + flattened_ok.unwrap_or([]).join(".");
}
"#,
        registry.compile_view(),
    )
    .expect("Option/Result helper chains should compile");
    let main = program.function("main").expect("main function");
    let unresolved = nested_method_names(main);

    assert!(
        unresolved.is_empty(),
        "expected helper chains to lower method IDs, unresolved: {unresolved:?}"
    );
}

fn nested_method_id_names(code: &UnlinkedCodeObject) -> Vec<String> {
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

fn nested_method_names(code: &UnlinkedCodeObject) -> Vec<String> {
    let mut methods = Vec::new();
    collect_nested_method_names(code, &mut methods);
    methods
}

fn collect_nested_method_names(code: &UnlinkedCodeObject, methods: &mut Vec<String>) {
    for instruction in &code.instructions {
        if let UnlinkedInstructionKind::CallMethod { method, .. } = &instruction.kind {
            methods.push(method.clone());
        }
    }
    for nested in &code.nested_functions {
        collect_nested_method_names(nested, methods);
    }
}

use super::standard_id_dispatch::std_method_id;
use super::*;

#[test]
fn call_method_uses_standard_map_transform_ids_before_name_fallback() {
    let mut program = compile_standard_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let base = {"gold": 2, "xp": 6};
    let patch = {"bonus": 3, "xp": 9};
    base.extend({"rank": 1});
    let merged = base.merge(patch);
    let keys = merged.keys().collect_array().join(",");
    let total = merged.values().collect_array().sum();
    let entries = merged.entries().collect_array();
    if keys == "bonus,gold,rank,xp" && total == 15 && entries.len() == 4 {
        return merged.get_or("xp", 0);
    }
    return 0;
}
"#,
    )
    .expect("standard map transform source should compile");
    replace_call_method_debug_names(
        &mut program,
        &[
            (std_method_id("Map", "extend"), "missing_map_extend"),
            (std_method_id("Map", "merge"), "missing_map_merge"),
            (std_method_id("Map", "keys"), "missing_map_keys"),
            (std_method_id("Map", "values"), "missing_map_values"),
            (std_method_id("Map", "entries"), "missing_map_entries"),
        ],
    );

    let mut budget = ExecutionBudget::unbounded();
    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(9)))
    );
}

#[test]
fn call_method_uses_standard_set_transform_ids_before_name_fallback() {
    let mut program = compile_standard_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let base = set::from_array(["daily", "quest"]);
    let patch = set::from_array(["quest", "raid"]);
    base.extend(set::from_array(["bonus"]));
    let unioned = base.union(patch).values().sort().join(",");
    let shared = base.intersection(patch).values().sort().join(",");
    let only_base = base.difference(patch).values().sort().join(",");
    let changed = base.symmetric_difference(patch).values().sort().join(",");
    if unioned == "bonus,daily,quest,raid"
        && shared == "quest"
        && only_base == "bonus,daily"
        && changed == "bonus,daily,raid"
    {
        return unioned;
    }
    return "";
}
"#,
    )
    .expect("standard set transform source should compile");
    replace_call_method_debug_names(
        &mut program,
        &[
            (std_method_id("Set", "extend"), "missing_set_extend"),
            (std_method_id("Set", "union"), "missing_set_union"),
            (std_method_id("Set", "values"), "missing_set_values"),
            (
                std_method_id("Set", "intersection"),
                "missing_set_intersection",
            ),
            (std_method_id("Set", "difference"), "missing_set_difference"),
            (
                std_method_id("Set", "symmetric_difference"),
                "missing_set_symmetric_difference",
            ),
        ],
    );

    let mut vm = Vm::new();
    vm.register_standard_natives();
    let mut budget = ExecutionBudget::unbounded();
    assert_eq!(
        run_linked_test_program_with_budget(&vm, &program, "main", &[], &mut budget),
        Ok(OwnedValue::String("bonus,daily,quest,raid".to_owned()))
    );
}

fn replace_call_method_debug_names(
    program: &mut UnlinkedProgram,
    replacements: &[(MethodId, &str)],
) {
    let code = program
        .function_mut("main")
        .expect("test function should exist");
    for instruction in &mut code.instructions {
        if let UnlinkedInstructionKind::CallMethodId {
            method, method_id, ..
        } = &mut instruction.kind
            && let Some((_, replacement)) = replacements
                .iter()
                .find(|(expected_method, _)| *expected_method == *method_id)
        {
            *method = (*replacement).to_owned();
        }
    }
}

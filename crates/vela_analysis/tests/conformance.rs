use vela_analysis::facts::AnalysisFacts;
use vela_analysis::type_fact::TypeFact;
use vela_common::SourceId;
use vela_hir::binding::LocalBindingKind;
use vela_hir::module_graph::{Declaration, ModuleGraph, ModulePath, ModuleSource};

const CORE_LANGUAGE: &str = include_str!("../../../tests/fixtures/conformance/core_language.vela");
const REWARD_MODULE: &str = include_str!("../../../tests/fixtures/conformance/reward_module.vela");

fn conformance_graph() -> ModuleGraph {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(1),
        ModulePath::from_qualified("conformance::core"),
        CORE_LANGUAGE,
    ));
    graph.add_source(ModuleSource::new(
        SourceId::new(2),
        ModulePath::from_qualified("conformance::reward"),
        REWARD_MODULE,
    ));
    graph.resolve_imports();
    graph
}

fn declaration<'graph>(graph: &'graph ModuleGraph, name: &str) -> &'graph Declaration {
    graph
        .declarations()
        .find(|declaration| declaration.name == name)
        .unwrap_or_else(|| panic!("missing declaration `{name}`"))
}

#[test]
fn core_language_fixture_analyzes_schema_and_local_hints() {
    let graph = conformance_graph();
    assert_eq!(graph.diagnostics(), &[]);
    let facts = AnalysisFacts::from_module_graph(&graph);

    assert_eq!(
        facts.declaration(declaration(&graph, "Reward").id),
        Some(&TypeFact::record("conformance::core::Reward"))
    );
    assert_eq!(
        facts.declaration(declaration(&graph, "QuestState").id),
        Some(&TypeFact::enum_type(
            "conformance::core::QuestState",
            None::<String>
        ))
    );
    assert_eq!(
        facts.declaration(declaration(&graph, "Scored").id),
        Some(&TypeFact::trait_type("conformance::core::Scored"))
    );
    assert_eq!(
        facts.declaration(declaration(&graph, "RewardConfig").id),
        Some(&TypeFact::record("conformance::reward::RewardConfig"))
    );
    assert_eq!(
        facts.declaration(declaration(&graph, "RewardOutcome").id),
        Some(&TypeFact::enum_type(
            "conformance::reward::RewardOutcome",
            None::<String>
        ))
    );

    let main = declaration(&graph, "main");
    let bindings = graph.bindings(main.id).expect("main bindings should exist");
    assert_eq!(
        local_fact(bindings, &facts, "reward"),
        Some(TypeFact::record("conformance::core::Reward"))
    );
    assert_eq!(
        local_fact(bindings, &facts, "quest"),
        Some(TypeFact::enum_type(
            "conformance::core::QuestState",
            None::<String>
        ))
    );
    assert_eq!(
        local_fact(bindings, &facts, "streak"),
        Some(TypeFact::enum_type(
            "conformance::core::QuestState",
            None::<String>
        ))
    );
    assert_eq!(
        local_fact(bindings, &facts, "streak_score"),
        Some(TypeFact::I64)
    );
    assert_eq!(local_fact(bindings, &facts, "total"), Some(TypeFact::I64));
    assert_eq!(
        local_fact(bindings, &facts, "compound_score"),
        Some(TypeFact::I64)
    );
    assert_eq!(
        local_fact(bindings, &facts, "logical_score"),
        Some(TypeFact::I64)
    );
    assert_eq!(
        local_fact(bindings, &facts, "array_helper_score"),
        Some(TypeFact::I64)
    );
    assert_eq!(
        local_fact(bindings, &facts, "array_mutation_score"),
        Some(TypeFact::I64)
    );
    assert_eq!(
        local_fact(bindings, &facts, "mapped"),
        Some(TypeFact::map(TypeFact::Unknown, TypeFact::Unknown))
    );
    assert_eq!(
        local_fact(bindings, &facts, "map_score"),
        Some(TypeFact::I64)
    );
    assert_eq!(
        local_fact(bindings, &facts, "map_helper_score"),
        Some(TypeFact::I64)
    );
    assert_eq!(
        local_fact(bindings, &facts, "set_score"),
        Some(TypeFact::I64)
    );
    assert_eq!(
        local_fact(bindings, &facts, "set_helper_score"),
        Some(TypeFact::I64)
    );
    assert_eq!(
        local_fact(bindings, &facts, "no_else_score"),
        Some(TypeFact::I64)
    );
    assert_eq!(
        local_fact(bindings, &facts, "named_method_score"),
        Some(TypeFact::I64)
    );
    assert_eq!(
        local_fact(bindings, &facts, "nested_lambda_score"),
        Some(TypeFact::I64)
    );
    assert_eq!(
        local_fact(bindings, &facts, "zero_arg_lambda_score"),
        Some(TypeFact::I64)
    );
    assert_eq!(
        local_fact(bindings, &facts, "string_helper_score"),
        Some(TypeFact::I64)
    );
    assert_eq!(
        local_fact(bindings, &facts, "string_utility_score"),
        Some(TypeFact::I64)
    );
    assert_eq!(
        local_fact(bindings, &facts, "math_helper_score"),
        Some(TypeFact::I64)
    );
    assert_eq!(
        local_fact(bindings, &facts, "option_result_helper_score"),
        Some(TypeFact::I64)
    );
    assert_eq!(
        local_fact(bindings, &facts, "imported_reward"),
        Some(TypeFact::record("conformance::reward::RewardConfig"))
    );
    assert_eq!(
        local_fact(bindings, &facts, "imported_bonus"),
        Some(TypeFact::I64)
    );
    assert_eq!(
        local_fact(bindings, &facts, "outcome"),
        Some(TypeFact::enum_type(
            "conformance::reward::RewardOutcome",
            None::<String>
        ))
    );
    assert_eq!(
        local_fact(bindings, &facts, "imported_match"),
        Some(TypeFact::I64)
    );
}

fn local_fact(
    bindings: &vela_hir::binding::BindingMap,
    facts: &AnalysisFacts,
    name: &str,
) -> Option<TypeFact> {
    bindings
        .locals_named(name)
        .iter()
        .copied()
        .find(|local| {
            bindings
                .local(*local)
                .is_some_and(|binding| binding.kind == LocalBindingKind::Let)
        })
        .and_then(|local| facts.local(local).cloned())
}

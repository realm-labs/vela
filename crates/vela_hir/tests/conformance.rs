use vela_common::SourceId;
use vela_hir::{
    Declaration, DeclarationKind, EnumVariantFieldsHint, ModuleGraph, ModulePath, ModuleSource,
};

const CORE_LANGUAGE: &str = include_str!("../../../tests/fixtures/conformance/core_language.lang");

fn conformance_graph() -> ModuleGraph {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(1),
        ModulePath::from_dotted("conformance.core"),
        CORE_LANGUAGE,
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
fn core_language_fixture_resolves() {
    let graph = conformance_graph();
    assert_eq!(graph.diagnostics(), &[]);

    let reward = declaration(&graph, "Reward");
    assert_eq!(reward.kind, DeclarationKind::Struct);
    let reward_shape = graph
        .struct_shape(reward.id)
        .expect("Reward should have a struct shape");
    assert_eq!(reward_shape.fields[0].name, "item");
    assert_eq!(
        reward_shape.fields[0]
            .type_hint
            .as_ref()
            .map(|hint| hint.display()),
        Some("string".to_owned())
    );
    assert_eq!(reward_shape.fields[1].name, "count");
    assert_eq!(
        reward_shape.fields[1]
            .type_hint
            .as_ref()
            .map(|hint| hint.display()),
        Some("int".to_owned())
    );

    let quest = declaration(&graph, "QuestState");
    assert_eq!(quest.kind, DeclarationKind::Enum);
    let quest_shape = graph
        .enum_shape(quest.id)
        .expect("QuestState should have an enum shape");
    assert_eq!(quest_shape.variants[0].name, "Active");
    assert!(matches!(
        quest_shape.variants[0].fields,
        EnumVariantFieldsHint::Record(_)
    ));
    assert_eq!(quest_shape.variants[1].name, "Done");

    let scored = declaration(&graph, "Scored");
    assert_eq!(scored.kind, DeclarationKind::Trait);
    let scored_shape = graph
        .trait_shape(scored.id)
        .expect("Scored should have a trait shape");
    assert_eq!(scored_shape.methods[0].name, "score");
    assert_eq!(
        scored_shape.methods[0]
            .signature
            .return_type
            .as_ref()
            .map(|hint| hint.display()),
        Some("int".to_owned())
    );

    let impl_decl = graph
        .declarations()
        .find(|declaration| declaration.kind == DeclarationKind::Impl)
        .expect("Scored impl should resolve");
    let impl_metadata = graph
        .impl_metadata(impl_decl.id)
        .expect("impl metadata should exist");
    assert_eq!(impl_metadata.trait_path, ["Scored"]);
    assert_eq!(impl_metadata.target_path, ["Reward"]);
    assert_eq!(impl_metadata.methods[0].name, "score");

    let main = declaration(&graph, "main");
    let main_bindings = graph.bindings(main.id).expect("main bindings should exist");
    assert!(
        main_bindings.expression_count() > 40,
        "fixture should resolve a meaningful expression surface"
    );
}

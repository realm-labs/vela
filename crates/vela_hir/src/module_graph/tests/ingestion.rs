use vela_syntax::parse::parse_source_with_id;

use super::*;

#[test]
fn parsed_source_ingestion_preserves_the_complete_hir_graph_and_ids() {
    let module_source = source(
        17,
        "game::rules",
        r#"
struct Reward {
    amount: i64 = 1,
}

fn grant(multiplier = 2) {
    let adjust = |value| value + multiplier;
    return Reward { amount: adjust(multiplier) };
}
"#,
    );
    let parsed = parse_source_with_id(module_source.id, &module_source.text);

    let mut source_graph = ModuleGraph::new();
    let source_module = source_graph.add_source(module_source.clone());
    source_graph.resolve_imports();

    let mut parsed_graph = ModuleGraph::new();
    let parsed_module = parsed_graph.add_parsed_source(module_source, &parsed);
    parsed_graph.resolve_imports();

    assert_eq!(parsed_module, source_module);
    assert_eq!(parsed_graph, source_graph);

    let source_declaration = source_graph
        .module(source_module)
        .and_then(|declarations| declarations.get("grant"))
        .expect("source graph should contain grant");
    let parsed_declaration = parsed_graph
        .module(parsed_module)
        .and_then(|declarations| declarations.get("grant"))
        .expect("parsed graph should contain grant");
    assert_eq!(parsed_declaration, source_declaration);
    assert_eq!(
        parsed_graph
            .function_body(parsed_declaration)
            .expect("parsed function body")
            .id,
        source_graph
            .function_body(source_declaration)
            .expect("source function body")
            .id,
    );
}

#[test]
fn declaration_names_have_one_canonical_module_qualified_query() {
    let mut graph = ModuleGraph::new();
    let qualified_module =
        graph.add_source(source(18, "game::reward", "struct Reward { amount: i64 }"));
    let root_module = graph.add_source(source(19, "", "struct Local { value: i64 }"));

    let reward = graph
        .module(qualified_module)
        .and_then(|declarations| declarations.get("Reward"))
        .expect("qualified Reward declaration");
    let local = graph
        .module(root_module)
        .and_then(|declarations| declarations.get("Local"))
        .expect("root Local declaration");

    assert_eq!(
        graph.qualified_declaration_name(reward).as_deref(),
        Some("game::reward::Reward")
    );
    assert_eq!(
        graph.qualified_declaration_name(local).as_deref(),
        Some("Local")
    );
}

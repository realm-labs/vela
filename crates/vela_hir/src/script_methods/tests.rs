use vela_common::SourceId;
use vela_def::{script_inherent_method_id, script_trait_method_id};

use super::*;
use crate::module_graph::ModuleSource;

#[test]
fn single_source_mode_keeps_main_identity_separate_from_root_symbols() {
    let (graph, module) = graph(
        301,
        ModulePath::root(),
        r#"
struct Player { level: i64 }
impl Player {
    fn bonus(self, amount: i64 = 2) -> i64 { return self.level + amount; }
}
"#,
    );
    let catalog = ScriptMethodCatalog::from_graph(
        &graph,
        ScriptMethodCatalogMode::single_source(module, "main"),
    )
    .expect("single-source method catalog");
    let method = only_method(&catalog);

    assert_eq!(method.owner().target_type(), "Player");
    assert!(method.owner().actual_module().segments().is_empty());
    assert_eq!(method.owner().identity().canonical_owner(), "main::Player");
    assert_eq!(
        method.method_id(),
        script_inherent_method_id("main::Player", "bonus")
    );
    assert_eq!(method.symbol_seed(), "__impl.Player.bonus");
    assert_eq!(method.module(), module);
    assert_eq!(method.signature_module(), module);
    assert_eq!(method.signature().params.len(), 2);
    assert_eq!(method.parameter_default_bodies().len(), 2);
    assert_eq!(method.parameter_default_bodies()[0], None);
    assert!(method.parameter_default_bodies()[1].is_some());
    assert_eq!(method.origin().span.source, SourceId::new(301));
    assert_eq!(method.name_origin().span.source, SourceId::new(301));
    assert_eq!(method.owner_origin().span.source, SourceId::new(301));
}

#[test]
fn module_mode_qualifies_identity_target_and_symbol_seed_from_actual_path() {
    let (graph, module) = graph(
        302,
        ModulePath::from_qualified("game::combat"),
        r#"
struct Player { level: i64 }
impl Player {
    fn bonus(self) -> i64 { return self.level; }
}
"#,
    );
    let catalog = ScriptMethodCatalog::from_graph(&graph, ScriptMethodCatalogMode::ModuleGraph)
        .expect("module method catalog");
    let method = only_method(&catalog);

    assert_eq!(method.owner().target_type(), "game::combat::Player");
    assert_eq!(method.owner().actual_module().join(), "game::combat");
    assert_eq!(
        method.owner().identity().canonical_owner(),
        "game::combat::Player"
    );
    assert_eq!(
        method.method_id(),
        script_inherent_method_id("game::combat::Player", "bonus")
    );
    assert_eq!(
        method.symbol_seed(),
        "game::combat.__impl.game::combat::Player.bonus"
    );
    assert_eq!(method.module(), module);
}

#[test]
fn trait_defaults_expand_per_owner_share_body_and_yield_to_explicit_methods() {
    let (graph, module) = graph(
        303,
        ModulePath::from_qualified("game"),
        r#"
trait BonusSource {
    fn bonus(self) -> i64 { return self.value; }
}
struct Player { value: i64 }
struct Monster { value: i64 }
struct Boss { value: i64 }
impl BonusSource for Player {}
impl BonusSource for Monster {}
impl BonusSource for Boss {
    fn bonus(self) -> i64 { return self.value + 10; }
}
"#,
    );
    let catalog = ScriptMethodCatalog::from_graph(&graph, ScriptMethodCatalogMode::ModuleGraph)
        .expect("trait-default method catalog");
    assert_eq!(catalog.len(), 3);
    let player = method_for(&catalog, "game::Player");
    let monster = method_for(&catalog, "game::Monster");
    let boss = method_for(&catalog, "game::Boss");

    assert_eq!(player.body(), monster.body());
    assert_eq!(player.node(), monster.node());
    assert_ne!(boss.body(), player.body());
    assert_ne!(boss.node(), player.node());
    assert_eq!(player.method_id(), monster.method_id());
    assert_eq!(player.method_id(), boss.method_id());
    assert_eq!(
        player.method_id(),
        script_trait_method_id("game::BonusSource", "bonus")
    );
    assert_eq!(player.signature_module(), module);
    assert_eq!(monster.signature_module(), module);
    assert_eq!(boss.signature_module(), module);
    assert_eq!(
        player.symbol_seed(),
        "game.__impl.BonusSource.for.game::Player.bonus"
    );
    assert_eq!(
        monster.symbol_seed(),
        "game.__impl.BonusSource.for.game::Monster.bonus"
    );
    assert_eq!(
        boss.symbol_seed(),
        "game.__impl.BonusSource.for.game::Boss.bonus"
    );
}

#[test]
fn inconsistent_impl_metadata_returns_a_source_keyed_catalog_error() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(ModuleSource::new(
        SourceId::new(304),
        ModulePath::root(),
        "struct Player {} impl MissingTrait for Player {}",
    ));
    graph.resolve_imports();

    let error = ScriptMethodCatalog::from_graph(
        &graph,
        ScriptMethodCatalogMode::single_source(module, "main"),
    )
    .expect_err("unresolved trait must not silently omit its impl");

    assert_eq!(error.origin().span.source, SourceId::new(304));
    assert!(error.node().is_none());
    assert!(error.message().contains("MissingTrait"));
    assert!(
        error
            .to_string()
            .contains("script method catalog inconsistency")
    );
}

fn graph(source: u32, path: ModulePath, text: &str) -> (ModuleGraph, ModuleId) {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(ModuleSource::new(SourceId::new(source), path, text));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);
    (graph, module)
}

fn only_method(catalog: &ScriptMethodCatalog) -> &ScriptMethod {
    assert_eq!(catalog.len(), 1);
    catalog.methods().next().expect("script method")
}

fn method_for<'a>(catalog: &'a ScriptMethodCatalog, owner: &str) -> &'a ScriptMethod {
    catalog
        .methods()
        .find(|method| method.owner().target_type() == owner)
        .unwrap_or_else(|| panic!("{owner} script method"))
}

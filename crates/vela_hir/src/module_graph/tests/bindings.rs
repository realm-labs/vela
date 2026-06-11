use super::*;

#[test]
fn function_bindings_resolve_params_and_locals_with_expression_ids() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::player",
        r#"
fn main(player) {
    let next = player.level;
    return next;
}
"#,
    ));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    let [player] = bindings.locals_named("player") else {
        panic!("expected one player binding");
    };
    let [next] = bindings.locals_named("next") else {
        panic!("expected one next binding");
    };
    assert_eq!(
        bindings.local(*player).map(|local| local.kind),
        Some(LocalBindingKind::Parameter)
    );
    assert_eq!(
        bindings.local(*next).map(|local| local.kind),
        Some(LocalBindingKind::Let)
    );
    assert!(bindings.expression_count() >= 2);
    assert!(
        bindings
            .resolutions()
            .any(|(_, resolution)| resolution == &BindingResolution::Local(*player))
    );
    assert!(
        bindings
            .resolutions()
            .any(|(_, resolution)| resolution == &BindingResolution::Local(*next))
    );
}
#[test]
fn binding_unresolved_names_report_candidate_hints() {
    let mut graph = ModuleGraph::new();
    graph.add_source(source(
        1,
        "game::player",
        r#"
fn main(player) {
    return plaeyr;
}
"#,
    ));
    let unresolved = graph
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::unresolved_name"))
        .expect("unresolved name diagnostic");
    assert_eq!(unresolved.labels.len(), 2);
    assert_eq!(unresolved.labels[0].message, "did you mean `player`?");
    assert_eq!(
        unresolved.labels[1].message,
        "candidate `player` is declared here"
    );
    assert_ne!(unresolved.labels[0].span, unresolved.labels[1].span);
}
#[test]
fn binding_tracks_nested_for_and_lambda_scopes() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::reward",
        r#"
fn main(rewards) {
    for reward in rewards {
        let mapper = |reward| reward.count;
    }
    return rewards;
}
"#,
    ));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    let reward_bindings = bindings.locals_named("reward");
    assert_eq!(reward_bindings.len(), 2);
    assert_eq!(
        bindings.local(reward_bindings[0]).map(|local| local.kind),
        Some(LocalBindingKind::For)
    );
    assert_eq!(
        bindings.local(reward_bindings[1]).map(|local| local.kind),
        Some(LocalBindingKind::LambdaParameter)
    );
}
#[test]
fn binding_tracks_for_pattern_locals() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::reward",
        r#"
enum Reward {
    Grant { amount },
    Skip { amount },
}
fn main(rewards) {
    let total = 0;
    for Reward::Grant { amount } in rewards {
        total += amount;
    }
    return total;
}
"#,
    ));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    let amount_bindings = bindings.locals_named("amount");
    assert_eq!(amount_bindings.len(), 1);
    assert_eq!(
        bindings.local(amount_bindings[0]).map(|local| local.kind),
        Some(LocalBindingKind::For)
    );
}
#[test]
fn duplicate_lambda_parameters_report_both_spans() {
    let mut graph = ModuleGraph::new();
    graph.add_source(source(
        1,
        "game::reward",
        r#"
fn main(reward) {
    let mapper = |count, count| count;
    return mapper(reward);
}
"#,
    ));
    let duplicate = graph
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::duplicate_parameter"))
        .expect("duplicate lambda parameter diagnostic");
    assert_eq!(duplicate.labels.len(), 2);
    assert!(duplicate.labels[0].message.contains("previous"));
    assert!(duplicate.labels[1].message.contains("duplicate"));
    assert_ne!(duplicate.labels[0].span, duplicate.labels[1].span);
}
#[test]
fn function_bindings_resolve_imported_names() {
    let mut graph = ModuleGraph::new();
    let reward = graph.add_source(source(1, "game::reward", "pub fn grant() { return 1; }"));
    let module = graph.add_source(source(
        2,
        "game::main",
        r#"
use game::reward::grant
fn main() { return grant; }
"#,
    ));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    let grant = graph
        .module(reward)
        .and_then(|module| module.get("grant"))
        .expect("grant declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    assert!(
        bindings
            .resolutions()
            .any(|(_, resolution)| { resolution == &BindingResolution::Declaration(grant) })
    );
}
#[test]
fn function_bindings_resolve_import_aliases() {
    let mut graph = ModuleGraph::new();
    let reward = graph.add_source(source(1, "game::reward", "pub fn grant() { return 1; }"));
    let module = graph.add_source(source(
        2,
        "game::main",
        r#"
use game::reward::grant as give_reward
fn main() { return give_reward; }
"#,
    ));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    let grant = graph
        .module(reward)
        .and_then(|module| module.get("grant"))
        .expect("grant declaration");
    let imports = graph.imports(module).expect("module imports");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    assert_eq!(imports[0].alias.as_deref(), Some("give_reward"));
    let bindings = graph.bindings(main).expect("main bindings");
    assert!(
        bindings
            .resolutions()
            .any(|(_, resolution)| { resolution == &BindingResolution::Declaration(grant) })
    );
}
#[test]
fn function_bindings_resolve_record_constructor_import_aliases() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::main",
        r#"
use game::reward::Reward as Prize
fn main() {
    return Prize { count: 2 };
}
"#,
    ));
    let reward = graph.add_source(source(
        2,
        "game::reward",
        r#"
pub struct Reward { count: i64 }
"#,
    ));
    graph.resolve_imports();
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    let reward = graph
        .module(reward)
        .and_then(|module| module.get("Reward"))
        .expect("reward declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    assert!(
        bindings
            .resolutions()
            .any(|(_, resolution)| { resolution == &BindingResolution::Declaration(reward) })
    );
}
#[test]
fn function_bindings_resolve_match_pattern_import_aliases() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::main",
        r#"
use game::damage::Damage as Hit
fn main(damage) {
    match damage {
        Hit::Physical { amount } => { return amount; },
        _ => { return 0; },
    }
}
"#,
    ));
    let damage = graph.add_source(source(
        2,
        "game::damage",
        r#"
pub enum Damage { Physical }
"#,
    ));
    graph.resolve_imports();
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    let damage = graph
        .module(damage)
        .and_then(|module| module.get("Damage"))
        .expect("damage declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    assert!(bindings.pattern_resolutions().any(|(path, resolution)| {
        path == ["Hit".to_owned(), "Physical".to_owned()]
            && resolution == &BindingResolution::Declaration(damage)
    }));
}
#[test]
fn function_bindings_resolve_tuple_constructor_call_aliases() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::main",
        r#"
use game::damage::Damage as Hit
fn main() {
    return Hit::Physical(7);
}
"#,
    ));
    let damage = graph.add_source(source(
        2,
        "game::damage",
        r#"
pub enum Damage { Physical(amount) }
"#,
    ));
    graph.resolve_imports();
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    let damage = graph
        .module(damage)
        .and_then(|module| module.get("Damage"))
        .expect("damage declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    assert!(
        bindings
            .resolutions()
            .any(|(_, resolution)| { resolution == &BindingResolution::Declaration(damage) })
    );
}
#[test]
fn resolved_imports_refresh_existing_binding_maps() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::main",
        r#"
use game::reward::grant
fn main() { return grant; }
"#,
    ));
    let reward = graph.add_source(source(2, "game::reward", "pub fn grant() { return 1; }"));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    let grant = graph
        .module(reward)
        .and_then(|module| module.get("grant"))
        .expect("grant declaration");
    assert!(
        graph
            .bindings(main)
            .expect("main bindings")
            .resolutions()
            .any(|(_, resolution)| {
                resolution == &BindingResolution::Import("grant".to_owned())
            })
    );
    graph.resolve_imports();
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    assert!(
        graph
            .bindings(main)
            .expect("main bindings")
            .resolutions()
            .any(|(_, resolution)| { resolution == &BindingResolution::Declaration(grant) })
    );
    assert!(
        !graph
            .bindings(main)
            .expect("main bindings")
            .resolutions()
            .any(|(_, resolution)| {
                resolution == &BindingResolution::Import("grant".to_owned())
            })
    );
}
#[test]
fn resolved_modules_refresh_qualified_path_binding_maps() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::main",
        r#"
fn main() {
    return game::reward::grant() + game::config::BONUS;
}
"#,
    ));
    let reward = graph.add_source(source(
        2,
        "game::reward",
        r#"
pub fn grant() { return 4; }
"#,
    ));
    let config = graph.add_source(source(
        3,
        "game::config",
        r#"
pub const BONUS: i64 = 5;
"#,
    ));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    let grant = graph
        .module(reward)
        .and_then(|module| module.get("grant"))
        .expect("grant declaration");
    let bonus = graph
        .module(config)
        .and_then(|module| module.get("BONUS"))
        .expect("bonus declaration");
    assert!(
        graph
            .bindings(main)
            .expect("main bindings")
            .resolutions()
            .any(|(_, resolution)| {
                resolution
                    == &BindingResolution::QualifiedPath(vec![
                        "game".to_owned(),
                        "reward".to_owned(),
                        "grant".to_owned(),
                    ])
            })
    );
    graph.resolve_imports();
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    assert!(
        bindings
            .resolutions()
            .any(|(_, resolution)| resolution == &BindingResolution::Declaration(grant))
    );
    assert!(
        bindings
            .resolutions()
            .any(|(_, resolution)| resolution == &BindingResolution::Declaration(bonus))
    );
}
#[test]
fn qualified_private_paths_do_not_resolve_across_modules() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::main",
        r#"
fn main() {
    return game::reward::secret();
}
"#,
    ));
    graph.add_source(source(
        2,
        "game::reward",
        r#"
fn secret() { return 1; }
"#,
    ));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    graph.resolve_imports();
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    assert!(bindings.resolutions().any(|(_, resolution)| {
        resolution
            == &BindingResolution::QualifiedPath(vec![
                "game".to_owned(),
                "reward".to_owned(),
                "secret".to_owned(),
            ])
    }));
}
#[test]
fn binding_treats_bare_map_keys_as_keys_not_name_reads() {
    let mut graph = ModuleGraph::new();
    graph.add_source(source(
        1,
        "game::reward",
        r#"
fn main() {
    return { exp: 15 };
}
"#,
    ));
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
}
#[test]
fn binding_resolves_record_shorthand_fields() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::reward",
        r#"
fn main() {
    let count = 2;
    return Reward { count };
}
"#,
    ));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    let [count] = bindings.locals_named("count") else {
        panic!("expected count binding");
    };
    assert!(
        bindings
            .resolutions()
            .any(|(_, resolution)| { resolution == &BindingResolution::Local(*count) })
    );
}

use super::*;
use crate::{BindingResolution, LocalBindingKind};
fn source(id: u32, module: &str, text: &str) -> ModuleSource {
    ModuleSource::new(SourceId::new(id), ModulePath::from_dotted(module), text)
}
#[test]
fn indexes_top_level_declarations_with_stable_ids() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game.reward",
        r#"
pub fn grant(player) { return player; }
pub const START_LEVEL: int = 1 + 2;
struct Reward { item_id, count }
enum QuestProgress { None, Active }
trait Damageable { fn damage(self, amount); }
"#,
    ));
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let declarations = graph.module(module).expect("module declarations");
    let grant = declarations.get("grant").expect("grant declaration");
    let start_level = declarations.get("START_LEVEL").expect("const declaration");
    let reward = declarations.get("Reward").expect("Reward declaration");
    assert_ne!(grant, reward);
    assert_eq!(grant.get(), 0);
    assert_eq!(start_level.get(), 1);
    assert_eq!(reward.get(), 2);
    assert_eq!(
        graph.declaration(grant).map(|decl| decl.kind),
        Some(DeclarationKind::Function)
    );
    assert_eq!(
        graph.declaration(start_level).map(|decl| decl.kind),
        Some(DeclarationKind::Const)
    );
    assert_eq!(
        graph
            .const_metadata(start_level)
            .and_then(|metadata| metadata.type_hint.as_ref())
            .map(HirTypeHint::display)
            .as_deref(),
        Some("int")
    );
    assert_eq!(
        graph.declaration(reward).map(|decl| decl.kind),
        Some(DeclarationKind::Struct)
    );
}
#[test]
fn resolves_imports_across_modules() {
    let mut graph = ModuleGraph::new();
    let _reward = graph.add_source(source(1, "game.reward", "pub fn grant() { return 1; }"));
    let main = graph.add_source(source(
        2,
        "game.main",
        r#"
use game.reward.grant
fn main() { return grant(); }
"#,
    ));
    graph.resolve_imports();
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let imports = graph.imports(main).expect("imports");
    let Some(ImportResolution::Declaration(declaration)) =
        imports.first().and_then(|import| import.resolution)
    else {
        panic!("expected resolved declaration import");
    };
    assert_eq!(
        graph
            .declaration(declaration)
            .map(|decl| decl.name.as_str()),
        Some("grant")
    );
}
#[test]
fn duplicate_declarations_report_both_spans() {
    let mut graph = ModuleGraph::new();
    graph.add_source(source(
        1,
        "game.player",
        r#"
fn level() { return 1; }
struct level { value }
"#,
    ));
    let duplicate = graph
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::duplicate_declaration"))
        .expect("duplicate declaration diagnostic");
    assert_eq!(duplicate.labels.len(), 2);
    assert!(duplicate.labels[0].message.contains("previous"));
    assert!(duplicate.labels[1].message.contains("duplicate"));
}
#[test]
fn duplicate_function_parameters_report_both_spans() {
    let mut graph = ModuleGraph::new();
    graph.add_source(source(
        1,
        "game.player",
        r#"
fn grant(amount, amount) {
    return amount;
}
"#,
    ));
    let duplicate = graph
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::duplicate_parameter"))
        .expect("duplicate parameter diagnostic");
    assert_eq!(duplicate.labels.len(), 2);
    assert!(duplicate.labels[0].message.contains("previous"));
    assert!(duplicate.labels[1].message.contains("duplicate"));
    assert_ne!(duplicate.labels[0].span, duplicate.labels[1].span);
}
#[test]
fn duplicate_import_aliases_report_both_spans() {
    let mut graph = ModuleGraph::new();
    graph.add_source(source(1, "game.reward", "pub fn grant() { return 1; }"));
    graph.add_source(source(2, "game.config", "pub const BONUS = 2"));
    graph.add_source(source(
        3,
        "game.main",
        r#"
use game.reward.grant as reward
use game.config.BONUS as reward
fn main() { return reward; }
"#,
    ));
    let duplicate = graph
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::duplicate_import"))
        .expect("duplicate import diagnostic");
    assert_eq!(duplicate.labels.len(), 2);
    assert!(duplicate.labels[0].message.contains("previous"));
    assert!(duplicate.labels[1].message.contains("duplicate"));
    assert_ne!(duplicate.labels[0].span, duplicate.labels[1].span);
}
#[test]
fn imports_conflicting_with_declarations_report_both_spans() {
    let mut graph = ModuleGraph::new();
    graph.add_source(source(1, "game.reward", "pub fn grant() { return 1; }"));
    graph.add_source(source(
        2,
        "game.main",
        r#"
use game.reward.grant
fn grant() { return 2; }
"#,
    ));
    let conflict = graph
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::import_conflict"))
        .expect("import conflict diagnostic");
    assert_eq!(conflict.labels.len(), 2);
    assert!(conflict.labels[0].message.contains("local declaration"));
    assert!(conflict.labels[1].message.contains("conflicting import"));
    assert_ne!(conflict.labels[0].span, conflict.labels[1].span);
}
#[test]
fn duplicate_struct_fields_report_both_spans() {
    let mut graph = ModuleGraph::new();
    graph.add_source(source(
        1,
        "game.reward",
        r#"
struct Reward {
    count: int,
    count: string
}
"#,
    ));
    let duplicate = graph
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::duplicate_field"))
        .expect("duplicate field diagnostic");
    assert_eq!(duplicate.labels.len(), 2);
    assert!(duplicate.labels[0].message.contains("previous"));
    assert!(duplicate.labels[1].message.contains("duplicate"));
    assert_ne!(duplicate.labels[0].span, duplicate.labels[1].span);
}
#[test]
fn duplicate_enum_variants_and_fields_report_both_spans() {
    let mut graph = ModuleGraph::new();
    graph.add_source(source(
        1,
        "game.quest",
        r#"
enum QuestProgress {
    Active { count: int, count: string },
    Active
}
"#,
    ));
    let duplicate_variant = graph
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::duplicate_variant"))
        .expect("duplicate variant diagnostic");
    let duplicate_field = graph
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::duplicate_variant_field"))
        .expect("duplicate variant field diagnostic");
    assert_eq!(duplicate_variant.labels.len(), 2);
    assert_eq!(duplicate_field.labels.len(), 2);
    assert_ne!(
        duplicate_variant.labels[0].span,
        duplicate_variant.labels[1].span
    );
    assert_ne!(
        duplicate_field.labels[0].span,
        duplicate_field.labels[1].span
    );
}
#[test]
fn duplicate_trait_and_impl_methods_report_both_spans() {
    let mut graph = ModuleGraph::new();
    graph.add_source(source(
        1,
        "game.player",
        r#"
struct Player { level: int }
trait Rewardable {
    fn reward(self, amount);
    fn reward(self, bonus);
}
impl Rewardable for Player {
    fn reward(self, amount) { return amount; }
    fn reward(self, bonus) { return bonus; }
}
"#,
    ));
    let duplicate_trait = graph
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::duplicate_trait_method"))
        .expect("duplicate trait method diagnostic");
    let duplicate_impl = graph
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::duplicate_impl_method"))
        .expect("duplicate impl method diagnostic");
    assert_eq!(duplicate_trait.labels.len(), 2);
    assert_eq!(duplicate_impl.labels.len(), 2);
    assert_ne!(
        duplicate_trait.labels[0].span,
        duplicate_trait.labels[1].span
    );
    assert_ne!(duplicate_impl.labels[0].span, duplicate_impl.labels[1].span);
}
#[test]
fn unresolved_imports_include_candidate_hints() {
    let mut graph = ModuleGraph::new();
    graph.add_source(source(1, "game.reward", "pub fn grant() { return 1; }"));
    graph.add_source(source(2, "game.main", "use game.reward.grant_reward"));
    graph.resolve_imports();
    let unresolved = graph
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::unresolved_import"))
        .expect("unresolved import diagnostic");
    assert_eq!(unresolved.labels.len(), 1);
    assert!(unresolved.labels[0].message.contains("grant"));
}
#[test]
fn private_imports_are_rejected_across_modules() {
    let mut graph = ModuleGraph::new();
    let reward = graph.add_source(source(1, "game.reward", "fn secret() { return 1; }"));
    let main = graph.add_source(source(2, "game.main", "use game.reward.secret"));
    graph.resolve_imports();
    let private = graph
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::private_import"))
        .expect("private import diagnostic");
    let imports = graph.imports(main).expect("main imports");
    let secret = graph
        .module(reward)
        .and_then(|module| module.get("secret"))
        .expect("secret declaration");
    assert_eq!(imports[0].resolution, None);
    assert_eq!(private.labels.len(), 2);
    assert_eq!(
        graph.declaration(secret).map(|decl| &decl.visibility),
        Some(&Visibility::Private)
    );
}
#[test]
fn function_bindings_resolve_params_and_locals_with_expression_ids() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game.player",
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
        "game.player",
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
        "game.reward",
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
        "game.reward",
        r#"
enum Reward {
    Grant { amount },
    Skip { amount },
}
fn main(rewards) {
    let total = 0;
    for Reward.Grant { amount } in rewards {
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
        "game.reward",
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
    let reward = graph.add_source(source(1, "game.reward", "pub fn grant() { return 1; }"));
    let module = graph.add_source(source(
        2,
        "game.main",
        r#"
use game.reward.grant
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
    let reward = graph.add_source(source(1, "game.reward", "pub fn grant() { return 1; }"));
    let module = graph.add_source(source(
        2,
        "game.main",
        r#"
use game.reward.grant as give_reward
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
        "game.main",
        r#"
use game.reward.Reward as Prize
fn main() {
    return Prize { count: 2 };
}
"#,
    ));
    let reward = graph.add_source(source(
        2,
        "game.reward",
        r#"
pub struct Reward { count: int }
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
        "game.main",
        r#"
use game.damage.Damage as Hit
fn main(damage) {
    match damage {
        Hit.Physical { amount } => { return amount; },
        _ => { return 0; },
    }
}
"#,
    ));
    let damage = graph.add_source(source(
        2,
        "game.damage",
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
        "game.main",
        r#"
use game.damage.Damage as Hit
fn main() {
    return Hit.Physical(7);
}
"#,
    ));
    let damage = graph.add_source(source(
        2,
        "game.damage",
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
        "game.main",
        r#"
use game.reward.grant
fn main() { return grant; }
"#,
    ));
    let reward = graph.add_source(source(2, "game.reward", "pub fn grant() { return 1; }"));
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
        "game.main",
        r#"
fn main() {
    return game.reward.grant() + game.config.BONUS;
}
"#,
    ));
    let reward = graph.add_source(source(
        2,
        "game.reward",
        r#"
pub fn grant() { return 4; }
"#,
    ));
    let config = graph.add_source(source(
        3,
        "game.config",
        r#"
pub const BONUS: int = 5;
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
        "game.main",
        r#"
fn main() {
    return game.reward.secret();
}
"#,
    ));
    graph.add_source(source(
        2,
        "game.reward",
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
        "game.reward",
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
        "game.reward",
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
#[test]
fn lowers_type_hint_metadata_for_signatures_structs_and_locals() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game.reward",
        r#"
fn grant(player: game.Player, amount: int) -> Result {
    let reward: Reward = Reward { count: amount };
    let mapper = |entry: Reward| entry.count;
    return reward;
}
struct Reward {
    count: int,
}
"#,
    ));
    let declarations = graph.module(module).expect("module declarations");
    let grant = declarations.get("grant").expect("grant declaration");
    let reward = declarations.get("Reward").expect("Reward declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let signature = graph.function_signature(grant).expect("function signature");
    assert_eq!(signature.params[0].name, "player");
    assert_eq!(
        signature.params[0]
            .type_hint
            .as_ref()
            .map(HirTypeHint::display)
            .as_deref(),
        Some("game.Player")
    );
    assert_eq!(
        signature
            .return_type
            .as_ref()
            .map(HirTypeHint::display)
            .as_deref(),
        Some("Result")
    );
    let shape = graph.struct_shape(reward).expect("struct shape");
    assert_eq!(shape.fields[0].name, "count");
    assert_eq!(
        shape.fields[0]
            .type_hint
            .as_ref()
            .map(HirTypeHint::display)
            .as_deref(),
        Some("int")
    );
    let bindings = graph.bindings(grant).expect("grant bindings");
    let [reward_local] = bindings.locals_named("reward") else {
        panic!("expected reward local");
    };
    assert_eq!(
        bindings
            .local(*reward_local)
            .and_then(|local| local.type_hint.as_ref())
            .map(HirTypeHint::display)
            .as_deref(),
        Some("Reward")
    );
    let entry_bindings = bindings.locals_named("entry");
    assert_eq!(
        bindings
            .local(entry_bindings[0])
            .and_then(|local| local.type_hint.as_ref())
            .map(HirTypeHint::display)
            .as_deref(),
        Some("Reward")
    );
}
#[test]
fn unknown_schema_type_hints_report_ranked_related_candidates() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game.combat",
        r#"
struct Player { hp: int }
fn grant(player: Plyer) {
    return null;
}
"#,
    ));
    graph.resolve_imports();
    let player = graph
        .module(module)
        .and_then(|module| module.get("Player"))
        .and_then(|declaration| graph.declaration(declaration))
        .expect("Player declaration");
    let diagnostics = graph.diagnostics();
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic.code.as_deref(), Some("hir::unknown_schema"));
    assert_eq!(diagnostic.message, "unknown schema `Plyer`");
    assert_eq!(diagnostic.labels.len(), 2);
    assert_eq!(
        diagnostic.labels[0].message,
        "`Plyer` does not resolve to a known schema"
    );
    assert_eq!(diagnostic.labels[1].span, player.span);
    assert_eq!(
        diagnostic.labels[1].message,
        "candidate `Player` is declared here"
    );
}
#[test]
fn unknown_impl_schema_names_report_trait_and_target_candidates() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game.combat",
        r#"
trait Damageable {
    fn damage(self);
}
struct Player { hp: int }
impl Damageabl for Playr {}
"#,
    ));
    graph.resolve_imports();
    let declarations = graph.module(module).expect("module declarations");
    let damageable = declarations
        .get("Damageable")
        .and_then(|declaration| graph.declaration(declaration))
        .expect("Damageable declaration");
    let player = declarations
        .get("Player")
        .and_then(|declaration| graph.declaration(declaration))
        .expect("Player declaration");
    let diagnostics = graph.diagnostics();
    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>(),
        ["unknown trait `Damageabl`", "unknown schema `Playr`"]
    );
    assert!(
        diagnostics[0]
            .labels
            .iter()
            .any(|label| label.span == damageable.span
                && label.message == "candidate `Damageable` is declared here")
    );
    assert!(diagnostics[1].labels.iter().any(|label| {
        label.span == player.span && label.message == "candidate `Player` is declared here"
    }));
}
#[test]
fn lowers_parameter_default_metadata_and_bindings() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game.rewards",
        r#"
const BASE = 10
fn grant(amount = BASE, bonus = amount + 1) {
    return amount + bonus;
}
"#,
    ));
    let declarations = graph.module(module).expect("module declarations");
    let grant = declarations.get("grant").expect("grant declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let signature = graph.function_signature(grant).expect("function signature");
    assert!(signature.params[0].default_value_span.is_some());
    assert!(signature.params[1].default_value_span.is_some());
    let bindings = graph.bindings(grant).expect("function bindings");
    assert!(bindings.resolutions().any(|(_, resolution)| {
        resolution
            == &BindingResolution::Declaration(declarations.get("BASE").expect("BASE declaration"))
    }));
    assert!(bindings.resolutions().any(|(_, resolution)| {
        matches!(resolution, BindingResolution::Local(local) if bindings
                .local(*local)
                .is_some_and(|binding| binding.name == "amount"))
    }));
}
#[test]
fn rejects_side_effecting_const_initializers() {
    let mut graph = ModuleGraph::new();
    graph.add_source(source(
        1,
        "game.config",
        r#"
const SAFE_LIMIT: int = 10 + 5;
const BAD_CALL = register_event("monster.kill");
const BAD_ASSIGN = { global_counter += 1; 0 };
fn main() { return SAFE_LIMIT; }
"#,
    ));
    let diagnostics = graph
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code.as_deref() == Some("hir::top_level_side_effect"))
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 2, "{:?}", graph.diagnostics());
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("BAD_CALL"))
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("BAD_ASSIGN"))
    );
}
#[test]
fn lowers_attribute_metadata_for_declarations_and_members() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game.reward",
        r#"
#[event("monster.kill")]
pub fn grant(player: Player) {
    return null;
}
#[doc("Reward metadata")]
#[domain("gameplay")]
#[policy(level = 3, tags = ["reward", game.reward.Event])]
struct Reward {
    #[doc("Reward item id")]
    item_id: string,
}
enum QuestProgress {
    #[terminal]
    Finished { #[doc("Quest id")] quest_id: string },
}
trait Damageable {
    #[doc("Apply damage")]
    fn damage(self, amount: int) -> int;
}
"#,
    ));
    let declarations = graph.module(module).expect("module declarations");
    let grant = declarations.get("grant").expect("grant declaration");
    let reward = declarations.get("Reward").expect("Reward declaration");
    let progress = declarations
        .get("QuestProgress")
        .expect("QuestProgress declaration");
    let damageable = declarations
        .get("Damageable")
        .expect("Damageable declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let grant_attrs = graph.declaration_attrs(grant);
    assert_eq!(grant_attrs[0].name, "event");
    assert_eq!(grant_attrs[0].value.as_deref(), Some("monster.kill"));
    let reward_attrs = graph.declaration_attrs(reward);
    assert_eq!(reward_attrs[0].name, "doc");
    assert_eq!(reward_attrs[0].value.as_deref(), Some("Reward metadata"));
    assert_eq!(reward_attrs[1].name, "domain");
    assert_eq!(reward_attrs[2].name, "policy");
    assert_eq!(
        reward_attrs[2].value.as_deref(),
        Some("level=3,tags=[\"reward\",game.reward.Event]")
    );
    let reward_shape = graph.struct_shape(reward).expect("Reward shape");
    assert_eq!(reward_shape.fields[0].attrs[0].name, "doc");
    assert_eq!(
        reward_shape.fields[0].attrs[0].value.as_deref(),
        Some("Reward item id")
    );
    let progress_shape = graph.enum_shape(progress).expect("Progress shape");
    assert_eq!(progress_shape.variants[0].attrs[0].name, "terminal");
    let crate::EnumVariantFieldsHint::Record(fields) = &progress_shape.variants[0].fields else {
        panic!("expected record variant fields");
    };
    assert_eq!(fields[0].attrs[0].name, "doc");
    assert_eq!(fields[0].attrs[0].value.as_deref(), Some("Quest id"));
    let trait_shape = graph.trait_shape(damageable).expect("Damageable shape");
    assert_eq!(trait_shape.methods[0].attrs[0].name, "doc");
    assert_eq!(
        trait_shape.methods[0].attrs[0].value.as_deref(),
        Some("Apply damage")
    );
}
#[test]
fn lowers_enum_shape_metadata() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game.quest",
        r#"
enum QuestProgress {
    None,
    Active { quest_id: string, count: int },
    Finished(quest_id: string),
}
"#,
    ));
    let declarations = graph.module(module).expect("module declarations");
    let progress = declarations
        .get("QuestProgress")
        .expect("QuestProgress declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let shape = graph.enum_shape(progress).expect("enum shape");
    let variants = shape
        .variants
        .iter()
        .map(|variant| variant.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(variants, ["None", "Active", "Finished"]);
    let active = shape
        .variants
        .iter()
        .find(|variant| variant.name == "Active")
        .expect("Active variant");
    let crate::EnumVariantFieldsHint::Record(fields) = &active.fields else {
        panic!("expected record fields");
    };
    assert_eq!(
        fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        ["quest_id", "count"]
    );
    let finished = shape
        .variants
        .iter()
        .find(|variant| variant.name == "Finished")
        .expect("Finished variant");
    let crate::EnumVariantFieldsHint::Tuple(fields) = &finished.fields else {
        panic!("expected tuple fields");
    };
    assert_eq!(
        fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        ["quest_id"]
    );
}
#[test]
fn lowers_schema_field_default_metadata() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game.quest",
        r#"
struct Reward {
    item_id: string = "gold",
    count: int = 1,
}
enum QuestProgress {
    Active { quest_id: string, count: int = 0 },
}
"#,
    ));
    let declarations = graph.module(module).expect("module declarations");
    let reward = declarations.get("Reward").expect("Reward declaration");
    let progress = declarations
        .get("QuestProgress")
        .expect("QuestProgress declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let reward_shape = graph.struct_shape(reward).expect("Reward shape");
    assert!(reward_shape.fields[0].default_value_span.is_some());
    assert!(reward_shape.fields[1].default_value_span.is_some());
    let progress_shape = graph.enum_shape(progress).expect("Progress shape");
    let crate::EnumVariantFieldsHint::Record(fields) = &progress_shape.variants[0].fields else {
        panic!("expected record fields");
    };
    assert!(fields[0].default_value_span.is_none());
    assert!(fields[1].default_value_span.is_some());
}
#[test]
fn lowers_impl_metadata_and_method_bindings() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game.combat",
        r#"
trait Damageable {
    fn damage(self, amount: int) -> int;
    fn alive(self) -> bool { return true; }
}
struct Player { hp: int }
impl Damageable for Player {
    fn damage(self, amount: int) -> int {
        let remaining: int = self.hp - amount;
        return remaining;
    }
}
"#,
    ));
    let declarations = graph.module(module).expect("module declarations");
    let trait_decl = declarations
        .get("Damageable")
        .expect("Damageable declaration");
    let impl_decl = declarations
        .get("impl Damageable for Player")
        .expect("impl declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let trait_shape = graph.trait_shape(trait_decl).expect("trait shape");
    assert_eq!(trait_shape.methods.len(), 2);
    assert_eq!(trait_shape.methods[0].name, "damage");
    assert!(!trait_shape.methods[0].has_default);
    assert_eq!(trait_shape.methods[1].name, "alive");
    assert!(trait_shape.methods[1].has_default);
    let default_node = trait_shape.methods[1]
        .default_body_node
        .expect("alive default body node");
    assert!(trait_shape.methods[1].default_body_span.is_some());
    let default_bindings = graph
        .trait_default_method_bindings(default_node)
        .expect("trait default method bindings");
    assert_eq!(default_bindings.locals_named("self").len(), 1);
    assert_eq!(
        graph.declaration(impl_decl).map(|decl| decl.kind),
        Some(DeclarationKind::Impl)
    );
    let metadata = graph.impl_metadata(impl_decl).expect("impl metadata");
    assert_eq!(metadata.trait_path, ["Damageable"]);
    assert_eq!(metadata.target_path, ["Player"]);
    assert_eq!(metadata.methods.len(), 1);
    let method = &metadata.methods[0];
    assert_eq!(method.name, "damage");
    assert_eq!(
        method.signature.params[1]
            .type_hint
            .as_ref()
            .map(HirTypeHint::display)
            .as_deref(),
        Some("int")
    );
    assert_eq!(
        method
            .signature
            .return_type
            .as_ref()
            .map(HirTypeHint::display)
            .as_deref(),
        Some("int")
    );
    let bindings = graph
        .impl_method_bindings(method.node)
        .expect("impl method bindings");
    let [remaining] = bindings.locals_named("remaining") else {
        panic!("expected remaining binding");
    };
    assert_eq!(
        bindings
            .local(*remaining)
            .and_then(|local| local.type_hint.as_ref())
            .map(HirTypeHint::display)
            .as_deref(),
        Some("int")
    );
    assert!(bindings.expression_count() >= 3);
}

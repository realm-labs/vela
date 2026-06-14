use super::*;

#[test]
fn indexes_top_level_declarations_with_stable_ids() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::reward",
        r#"
pub fn grant(player) { return player; }
pub const START_LEVEL: i64 = 1 + 2;
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
        Some("i64")
    );
    assert_eq!(
        graph.declaration(reward).map(|decl| decl.kind),
        Some(DeclarationKind::Struct)
    );
}
#[test]
fn resolves_imports_across_modules() {
    let mut graph = ModuleGraph::new();
    let _reward = graph.add_source(source(1, "game::reward", "pub fn grant() { return 1; }"));
    let main = graph.add_source(source(
        2,
        "game::main",
        r#"
use game::reward::grant
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
        "game::player",
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
        "game::player",
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
    graph.add_source(source(1, "game::reward", "pub fn grant() { return 1; }"));
    graph.add_source(source(2, "game::config", "pub const BONUS = 2"));
    graph.add_source(source(
        3,
        "game::main",
        r#"
use game::reward::grant as reward
use game::config::BONUS as reward
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
    graph.add_source(source(1, "game::reward", "pub fn grant() { return 1; }"));
    graph.add_source(source(
        2,
        "game::main",
        r#"
use game::reward::grant
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
        "game::reward",
        r#"
struct Reward {
    count: i64,
    count: String
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
        "game::quest",
        r#"
enum QuestProgress {
    Active { count: i64, count: String },
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
        "game::player",
        r#"
struct Player { level: i64 }
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
    graph.add_source(source(1, "game::reward", "pub fn grant() { return 1; }"));
    graph.add_source(source(2, "game::main", "use game::reward::grant_reward"));
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
    let reward = graph.add_source(source(1, "game::reward", "fn secret() { return 1; }"));
    let main = graph.add_source(source(2, "game::main", "use game::reward::secret"));
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

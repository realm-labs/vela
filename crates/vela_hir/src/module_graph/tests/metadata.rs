use super::*;
use crate::attributes::HirAttributeValue;
use crate::binding::LocalBindingKind;
use crate::body::{HirBodyOwner, HirBodyRoot};

#[test]
fn lowers_type_hint_metadata_for_signatures_structs_and_locals() {
    let mut graph = ModuleGraph::new();
    let source_text = r#"
fn grant(player: game::Player, amount: i64, bonuses: Array<Option<i64>>) -> Result<Map<String, i64>, String> {
    let reward: Reward = Reward { count: amount };
    let names: Set<String> = [];
    let mapper = |entry: Reward| entry.count;
    return reward;
}
struct Reward {
    count: i64,
}
"#;
    let module = graph.add_source(source(1, "game::reward", source_text));
    let declarations = graph.module(module).expect("module declarations");
    let grant = declarations.get("grant").expect("grant declaration");
    let reward = declarations.get("Reward").expect("Reward declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    assert_eq!(
        graph
            .declaration(grant)
            .map(|decl| span_text(source_text, decl.name_span)),
        Some("grant")
    );
    assert_eq!(
        graph
            .declaration(reward)
            .map(|decl| span_text(source_text, decl.name_span)),
        Some("Reward")
    );
    let signature = graph.function_signature(grant).expect("function signature");
    assert_eq!(signature.params[0].name, "player");
    assert_eq!(
        signature.params[0]
            .type_hint
            .as_ref()
            .map(HirTypeHint::display)
            .as_deref(),
        Some("game::Player")
    );
    assert_eq!(
        signature.params[2]
            .type_hint
            .as_ref()
            .map(HirTypeHint::display)
            .as_deref(),
        Some("Array<Option<i64>>")
    );
    assert_eq!(
        signature
            .return_type
            .as_ref()
            .map(HirTypeHint::display)
            .as_deref(),
        Some("Result<Map<String, i64>, String>")
    );
    let shape = graph.struct_shape(reward).expect("struct shape");
    assert_eq!(shape.fields[0].name, "count");
    assert_eq!(
        shape.fields[0]
            .type_hint
            .as_ref()
            .map(HirTypeHint::display)
            .as_deref(),
        Some("i64")
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
    let [names_local] = bindings.locals_named("names") else {
        panic!("expected names local");
    };
    assert_eq!(
        bindings
            .local(*names_local)
            .and_then(|local| local.type_hint.as_ref())
            .map(HirTypeHint::display)
            .as_deref(),
        Some("Set<String>")
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
        "game::combat",
        r#"
struct Player { hp: i64 }
fn grant(player: Plyer) {
    return;
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
fn lowers_unit_and_tuple_type_hint_metadata() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::tuples",
        r#"
fn split(value: String, fallback: ()) -> Option<(String, i64)> {
    let pair: (String, i64) = ("level", 3);
    return Option::Some(pair);
}
"#,
    ));
    let declarations = graph.module(module).expect("module declarations");
    let split = declarations.get("split").expect("split declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());

    let split_signature = graph.function_signature(split).expect("split signature");
    assert_eq!(
        split_signature.params[1]
            .type_hint
            .as_ref()
            .map(HirTypeHint::display)
            .as_deref(),
        Some("()")
    );
    assert_eq!(
        split_signature
            .return_type
            .as_ref()
            .map(HirTypeHint::display)
            .as_deref(),
        Some("Option<(String, i64)>")
    );
    let bindings = graph.bindings(split).expect("split bindings");
    let [pair] = bindings.locals_named("pair") else {
        panic!("expected pair local");
    };
    assert_eq!(
        bindings
            .local(*pair)
            .and_then(|local| local.type_hint.as_ref())
            .map(HirTypeHint::display)
            .as_deref(),
        Some("(String, i64)")
    );
}

#[test]
fn unknown_impl_schema_names_report_trait_and_target_candidates() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::combat",
        r#"
trait Damageable {
    fn damage(self);
}
struct Player { hp: i64 }
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
fn builtin_operator_trait_prerequisites_are_validated() {
    let mut graph = ModuleGraph::new();
    graph.add_source(source(
        1,
        "game::scores",
        r#"
struct Score { value: i64 }
impl Eq for Score {}
impl Ord for Score {}
"#,
    ));
    graph.resolve_imports();

    let diagnostics = graph.diagnostics();
    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_deref())
            .collect::<Vec<_>>(),
        [
            Some("hir::missing_comparison_trait_prerequisite"),
            Some("hir::missing_comparison_trait_prerequisite")
        ]
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("`Eq` without required `PartialEq`")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("`Ord` without required `PartialOrd`")
    }));
}

#[test]
fn builtin_operator_trait_prerequisites_accept_complete_chain() {
    let mut graph = ModuleGraph::new();
    graph.add_source(source(
        1,
        "game::scores",
        r#"
struct Score { value: i64 }
impl PartialEq for Score { fn eq(self, other: Score) -> bool { return true; } }
impl Eq for Score {}
impl PartialOrd for Score { fn partial_cmp(self, other: Score) { return option::some(0); } }
impl Ord for Score { fn cmp(self, other: Score) -> i64 { return 0; } }
"#,
    ));
    graph.resolve_imports();

    assert_eq!(graph.diagnostics(), &[]);
}

#[test]
fn builtin_operator_trait_prerequisites_accept_derived_prerequisites() {
    let mut graph = ModuleGraph::new();
    graph.add_source(source(
        1,
        "game::scores",
        r#"
#[derive(PartialEq)]
struct Score { value: i64 }
impl Eq for Score {}

#[derive(PartialEq, Eq, PartialOrd)]
struct Rank { value: i64 }
impl Ord for Rank { fn cmp(self, other: Rank) -> i64 { return self.value - other.value; } }
"#,
    ));
    graph.resolve_imports();

    assert_eq!(graph.diagnostics(), &[]);
}

#[test]
fn builtin_operator_derive_prerequisites_are_validated() {
    let mut graph = ModuleGraph::new();
    graph.add_source(source(
        1,
        "game::scores",
        r#"
#[derive(Eq)]
struct Score { value: i64 }
#[derive(Ord)]
struct Rank { value: i64 }
"#,
    ));
    graph.resolve_imports();

    let diagnostics = graph.diagnostics();
    assert_eq!(diagnostics.len(), 3, "{diagnostics:?}");
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code.as_deref()
                == Some("hir::missing_comparison_derive_prerequisite"))
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("`Eq` without required `PartialEq`")
    }));
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("`Ord` without required `Eq`"))
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("`Ord` without required `PartialOrd`")
    }));
}

#[test]
fn builtin_operator_derive_rejects_unsupported_fields() {
    let mut graph = ModuleGraph::new();
    graph.add_source(source(
        1,
        "game::scores",
        r#"
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Score { value: f64 }
#[derive(PartialEq)]
struct Dynamic { value }
"#,
    ));
    graph.resolve_imports();

    let diagnostics = graph.diagnostics();
    assert_eq!(diagnostics.len(), 3, "{diagnostics:?}");
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code.as_deref()
                == Some("hir::unsupported_comparison_derive_field"))
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("derive `Eq`"))
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("derive `Ord`"))
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("field `value`"))
    );
}

#[test]
fn builtin_operator_derive_accepts_nested_script_field_traits() {
    let mut graph = ModuleGraph::new();
    graph.add_source(source(
        1,
        "game::scores",
        r#"
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct ScoreId { value: i64 }
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Score { id: ScoreId, label: String }
"#,
    ));
    graph.resolve_imports();

    assert_eq!(graph.diagnostics(), &[]);
}

#[test]
fn lowers_parameter_default_metadata_and_bindings() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::rewards",
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
    assert!(
        signature
            .params
            .iter()
            .all(|param| param.default_body.is_none())
    );
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
fn lowers_const_initializer_bodies_and_refreshes_imports() {
    let mut graph = ModuleGraph::new();
    let main_text = r#"
use game::config::BASE
const BONUS = BASE + 1
"#;
    let main = graph.add_source(source(1, "game::main", main_text));
    let config = graph.add_source(source(2, "game::config", "pub const BASE = 10"));

    let bonus = graph
        .module(main)
        .and_then(|module| module.get("BONUS"))
        .expect("BONUS declaration");
    let base = graph
        .module(config)
        .and_then(|module| module.get("BASE"))
        .expect("BASE declaration");

    let body = graph
        .const_initializer_body(bonus)
        .expect("const initializer body");
    assert_eq!(body.owner, HirBodyOwner::ConstInitializer(bonus));
    assert!(matches!(body.root, HirBodyRoot::Expr(_)));
    assert!(body.scopes.contains_key(&body.root_scope));
    assert!(body.params.is_empty());
    assert!(body.expressions.len() >= 3);

    let initializer_bindings = graph
        .const_initializer_bindings(bonus)
        .expect("const initializer bindings");
    assert_eq!(initializer_bindings.body(), body.id);
    assert!(
        initializer_bindings
            .resolutions()
            .any(|(_, resolution)| resolution == &BindingResolution::Import("BASE".to_owned()))
    );
    let imports = graph.imports(main).expect("main imports");
    assert_eq!(imports.len(), 1);
    assert_eq!(
        imports[0]
            .path_spans
            .iter()
            .map(|span| span_text(main_text, *span))
            .collect::<Vec<_>>(),
        ["game", "config", "BASE"]
    );
    graph.resolve_imports();
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    assert!(
        graph
            .const_initializer_bindings(bonus)
            .expect("const initializer bindings")
            .resolutions()
            .any(|(_, resolution)| resolution == &BindingResolution::Declaration(base))
    );
}

#[test]
fn malformed_items_do_not_shift_following_metadata() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::rewards",
        r#"
fn () {}
fn grant(amount: i64) -> i64 {
    return amount;
}
"#,
    ));

    let declarations = graph.module(module).expect("module declarations");
    let grant = declarations.get("grant").expect("grant declaration");
    let signature = graph.function_signature(grant).expect("grant signature");

    assert!(
        graph
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code.as_deref() == Some("E_PARSE")),
        "{:?}",
        graph.diagnostics()
    );
    assert_eq!(signature.params.len(), 1);
    assert_eq!(signature.params[0].name, "amount");
    assert_eq!(
        signature.params[0]
            .type_hint
            .as_ref()
            .map(HirTypeHint::display)
            .as_deref(),
        Some("i64")
    );
    assert_eq!(
        signature
            .return_type
            .as_ref()
            .map(HirTypeHint::display)
            .as_deref(),
        Some("i64")
    );
}

#[test]
fn rejects_side_effecting_const_initializers() {
    let mut graph = ModuleGraph::new();
    graph.add_source(source(
        1,
        "game::config",
        r#"
const SAFE_LIMIT: i64 = 10 + 5;
const BAD_CALL = register_event("monster.kill");
const BAD_ASSIGN = { global_counter += 1; 0 };
const BAD_INTERP = f"{register_event("monster.spawn")}";
fn main() { return SAFE_LIMIT; }
"#,
    ));
    let diagnostics = graph
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code.as_deref() == Some("hir::top_level_side_effect"))
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 3, "{:?}", graph.diagnostics());
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
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("BAD_INTERP"))
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.labels.iter().any(|label| label
                .message
                .contains("move this work into a runtime function")))
    );
}
#[test]
fn lowers_attribute_metadata_for_declarations_and_members() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::reward",
        r#"
#[event("monster.kill")]
pub fn grant(player: Player) {
    return;
}
#[doc("Reward metadata")]
#[domain("gameplay")]
#[policy(level = 3, tags = ["reward", game::reward::Event])]
struct Reward {
    #[doc("Reward item id")]
    item_id: String,
}
enum QuestProgress {
    #[terminal]
    Finished { #[doc("Quest id")] quest_id: String },
}
trait Damageable {
    #[doc("Apply damage")]
    fn damage(self, amount: i64) -> i64;
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
    assert_eq!(grant_attrs[0].string_value(), "monster.kill");
    let reward_attrs = graph.declaration_attrs(reward);
    assert_eq!(reward_attrs[0].name, "doc");
    assert_eq!(reward_attrs[0].string_value(), "Reward metadata");
    assert_eq!(reward_attrs[1].name, "domain");
    assert_eq!(reward_attrs[2].name, "policy");
    assert_eq!(
        reward_attrs[2].string_value(),
        "level=3,tags=[\"reward\",game::reward::Event]"
    );
    assert_eq!(reward_attrs[2].arguments.len(), 2);
    assert_eq!(reward_attrs[2].arguments[0].name.as_deref(), Some("level"));
    assert!(matches!(
        &reward_attrs[2].arguments[0].value,
        HirAttributeValue::Integer(value) if value == "3"
    ));
    assert_eq!(reward_attrs[2].arguments[1].name.as_deref(), Some("tags"));
    assert!(matches!(
        &reward_attrs[2].arguments[1].value,
        HirAttributeValue::Array(values)
            if matches!(values.as_slice(), [
                HirAttributeValue::String(value),
                HirAttributeValue::Path(path),
            ] if value == "reward" && path == &["game", "reward", "Event"])
    ));
    assert!(reward_attrs[2].arguments.iter().all(|argument| {
        argument.span.source == SourceId::new(1)
            && argument.value_span.start >= reward_attrs[2].span.start
            && argument.value_span.end <= reward_attrs[2].span.end
    }));
    let reward_shape = graph.struct_shape(reward).expect("Reward shape");
    assert_eq!(reward_shape.fields[0].attrs[0].name, "doc");
    assert_eq!(
        reward_shape.fields[0].attrs[0].string_value(),
        "Reward item id"
    );
    let progress_shape = graph.enum_shape(progress).expect("Progress shape");
    assert_eq!(progress_shape.variants[0].attrs[0].name, "terminal");
    let EnumVariantFieldsHint::Record(fields) = &progress_shape.variants[0].fields else {
        panic!("expected record variant fields");
    };
    assert_eq!(fields[0].attrs[0].name, "doc");
    assert_eq!(fields[0].attrs[0].string_value(), "Quest id");
    let trait_shape = graph.trait_shape(damageable).expect("Damageable shape");
    assert_eq!(trait_shape.methods[0].attrs[0].name, "doc");
    assert_eq!(
        trait_shape.methods[0].attrs[0].string_value(),
        "Apply damage"
    );
}
#[test]
fn lowers_enum_shape_metadata() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::quest",
        r#"
enum QuestProgress {
    None,
    Active { quest_id: String, count: i64 },
    Finished(quest_id: String),
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
    let EnumVariantFieldsHint::Record(fields) = &active.fields else {
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
    let EnumVariantFieldsHint::Tuple(fields) = &finished.fields else {
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
        "game::quest",
        r#"
struct Reward {
    item_id: String = "gold",
    count: i64 = 1,
}
enum QuestProgress {
    Active { quest_id: String, count: i64 = 0 },
    Finished(label: String = "done"),
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
    assert!(reward_shape.fields[0].default_body.is_some());
    assert!(reward_shape.fields[1].default_body.is_some());
    assert_ne!(
        reward_shape.fields[0].default_body,
        reward_shape.fields[1].default_body
    );
    let progress_shape = graph.enum_shape(progress).expect("Progress shape");
    let EnumVariantFieldsHint::Record(fields) = &progress_shape.variants[0].fields else {
        panic!("expected record fields");
    };
    assert!(fields[0].default_value_span.is_none());
    assert!(fields[0].default_body.is_none());
    assert!(fields[1].default_value_span.is_some());
    assert!(fields[1].default_body.is_some());
    let EnumVariantFieldsHint::Tuple(fields) = &progress_shape.variants[1].fields else {
        panic!("expected tuple fields");
    };
    assert!(fields[0].default_value_span.is_some());
    assert!(fields[0].default_body.is_some());
}

#[test]
fn lowers_schema_field_default_bindings() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::quest",
        r#"
struct Reward {
    count: i64 = { let base = 1; base + 1 },
}
"#,
    ));
    let declarations = graph.module(module).expect("module declarations");
    let reward = declarations.get("Reward").expect("Reward declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let reward_shape = graph.struct_shape(reward).expect("Reward shape");
    let field = &reward_shape.fields[0];
    let span = field.default_value_span.expect("field default span");
    let body_id = field.default_body.expect("field default body ID");
    let body = graph.body(body_id).expect("schema default body");
    assert_eq!(body.owner, HirBodyOwner::SchemaFieldDefault(reward));
    assert_eq!(body.origin.span, span);
    let bindings = graph
        .schema_field_default_bindings(body.id)
        .expect("schema default bindings");
    let [base] = bindings.locals_named("base") else {
        panic!("base local should exist");
    };
    assert_eq!(
        bindings.local(*base).map(|local| local.kind),
        Some(LocalBindingKind::Let)
    );
    assert_eq!(bindings.body(), body.id);
}

#[test]
fn lowers_impl_metadata_and_method_bindings() {
    let mut graph = ModuleGraph::new();
    let source_text = r#"
trait Damageable {
    fn damage(self, amount: i64) -> i64;
    fn alive(self) -> bool { return true; }
}
struct Player { hp: i64 }
impl Damageable for Player {
    fn damage(self, amount: i64) -> i64 {
        let remaining: i64 = self.hp - amount;
        let adjust = |bonus| self.hp + bonus;
        return remaining;
    }
}
"#;
    let module = graph.add_source(source(1, "game::combat", source_text));
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
    assert_eq!(
        span_text(source_text, trait_shape.methods[0].name_span),
        "damage"
    );
    assert!(!trait_shape.methods[0].has_default);
    assert_eq!(trait_shape.methods[1].name, "alive");
    assert_eq!(
        span_text(source_text, trait_shape.methods[1].name_span),
        "alive"
    );
    assert!(trait_shape.methods[1].has_default);
    let default_node = trait_shape.methods[1]
        .default_body_node
        .expect("alive default body node");
    assert!(trait_shape.methods[1].default_body_span.is_some());
    let default_bindings = graph
        .trait_default_method_bindings(default_node)
        .expect("trait default method bindings");
    assert_eq!(default_bindings.locals_named("self").len(), 1);
    let default_body = graph
        .trait_default_method_body(default_node)
        .expect("trait default method body");
    assert_eq!(default_bindings.body(), default_body.id);
    assert_eq!(
        default_body.owner,
        HirBodyOwner::TraitDefaultMethod(default_node)
    );
    assert!(matches!(default_body.root, HirBodyRoot::Block(_)));
    assert_eq!(default_body.params.len(), 1);
    assert_eq!(
        graph.declaration(impl_decl).map(|decl| decl.kind),
        Some(DeclarationKind::Impl)
    );
    let metadata = graph.impl_metadata(impl_decl).expect("impl metadata");
    assert_eq!(
        metadata.kind,
        ImplMetadataKind::Trait {
            trait_path: vec!["Damageable".to_owned()]
        }
    );
    assert_eq!(metadata.target_path, ["Player"]);
    assert_eq!(metadata.methods.len(), 1);
    let method = &metadata.methods[0];
    assert_eq!(method.name, "damage");
    assert_eq!(span_text(source_text, method.name_span), "damage");
    assert_eq!(method.span, method.body_span);
    assert!(span_text(source_text, method.body_span).starts_with("{\n        let"));
    assert_eq!(
        method.signature.params[1]
            .type_hint
            .as_ref()
            .map(HirTypeHint::display)
            .as_deref(),
        Some("i64")
    );
    assert_eq!(
        method
            .signature
            .return_type
            .as_ref()
            .map(HirTypeHint::display)
            .as_deref(),
        Some("i64")
    );
    let bindings = graph
        .impl_method_bindings(method.node)
        .expect("impl method bindings");
    let body = graph
        .impl_method_body(method.node)
        .expect("impl method body");
    assert_eq!(bindings.body(), body.id);
    assert_eq!(body.owner, HirBodyOwner::ImplMethod(method.node));
    assert!(matches!(body.root, HirBodyRoot::Block(_)));
    assert_eq!(body.params.len(), 2);
    assert_eq!(body.statements.len(), 3);
    let [self_local] = bindings.locals_named("self") else {
        panic!("expected self binding");
    };
    assert_eq!(body.self_binding, Some(*self_local));
    assert!(!body.self_uses.is_empty());
    assert!(
        body.self_uses
            .iter()
            .all(|expression| body.expressions.contains_key(expression))
    );
    let method_body_id = body.id;
    let lambda_body = graph
        .bodies()
        .find(|body| {
            matches!(body.owner, HirBodyOwner::Lambda { parent, .. } if parent == method_body_id)
        })
        .expect("lambda body");
    assert!(
        lambda_body
            .captures
            .iter()
            .any(|capture| capture.local == *self_local),
        "lambda should capture method self"
    );
    let [remaining] = bindings.locals_named("remaining") else {
        panic!("expected remaining binding");
    };
    assert_eq!(
        bindings
            .local(*remaining)
            .and_then(|local| local.type_hint.as_ref())
            .map(HirTypeHint::display)
            .as_deref(),
        Some("i64")
    );
    assert!(
        graph
            .body(bindings.body())
            .is_some_and(|body| body.expressions.len() >= 3)
    );
}

fn span_text(source: &str, span: Span) -> &str {
    &source[span.start as usize..span.end as usize]
}

#[test]
fn lowers_inherent_impl_metadata_and_method_bindings() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::combat",
        r#"
struct Player { level: i64 }
impl Player {
    fn bonus(self, amount: i64) -> i64 {
        let total: i64 = self.level + amount;
        return total;
    }
}
"#,
    ));
    let declarations = graph.module(module).expect("module declarations");
    let impl_decl = declarations.get("impl Player").expect("impl declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let metadata = graph.impl_metadata(impl_decl).expect("impl metadata");
    assert_eq!(metadata.kind, ImplMetadataKind::Inherent);
    assert_eq!(metadata.target_path, ["Player"]);
    assert_eq!(metadata.methods.len(), 1);
    let method = &metadata.methods[0];
    assert_eq!(method.name, "bonus");
    assert_eq!(
        method.signature.params[1]
            .type_hint
            .as_ref()
            .map(HirTypeHint::display)
            .as_deref(),
        Some("i64")
    );
    let bindings = graph
        .impl_method_bindings(method.node)
        .expect("impl method bindings");
    assert_eq!(bindings.locals_named("total").len(), 1);
}

#[test]
fn graph_tracks_source_hashes_and_dependent_modules() {
    let mut graph = ModuleGraph::new();
    let main = graph.add_source(source(
        1,
        "game::main",
        r#"
use game::reward::grant

fn main() {
    return grant();
}
"#,
    ));
    let reward = graph.add_source(source(
        2,
        "game::reward",
        r#"
pub fn grant() {
    return 4;
}
"#,
    ));
    graph.resolve_imports();

    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    assert!(graph.module_source_hash(main).is_some());
    assert!(graph.module_source_hash(reward).is_some());

    let impacted = graph.dependent_modules([reward]);
    let mut impacted_names = impacted
        .iter()
        .filter_map(|module| graph.module_path(*module))
        .map(ModulePath::join)
        .collect::<Vec<_>>();
    impacted_names.sort();

    assert_eq!(impacted_names, ["game::main", "game::reward"]);
}

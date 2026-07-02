use crate::SyntaxKind;
use crate::ast::{AstNode, SyntaxSourceFile};
use crate::parse::{Parse, parse_source_with_id};

use super::source_id;

fn parse_cst(text: &str) -> Parse<SyntaxSourceFile> {
    parse_source_with_id(source_id(), text)
}

#[test]
fn parses_core_module_items() {
    let parsed = parse_cst(
        r#"
use game::player::Player;

pub const START_LEVEL: i64 = 1 + 2;

pub global state: GameState;

#[event("monster.kill")]
pub fn on_kill(ctx, player, monster) {
    player.exp += monster.exp
}

struct KillReward {
    #[doc("Reward item")]
    item_id,
    count,
}

enum QuestProgress {
    #[empty]
    None,
    Active { quest_id, count },
}

trait Damageable {
    #[doc("Apply damage")]
    fn damage(self, amount);
    fn alive(self) { return true; }
}

impl Damageable for Player {
    fn damage(self, amount) {
        return amount;
    }
}
"#,
    );

    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let tree = parsed.tree();
    let items = tree.items().collect::<Vec<_>>();
    assert_eq!(items.len(), 8);
    assert_eq!(items[0].syntax().kind(), SyntaxKind::UseItem);
    assert_eq!(items[1].syntax().kind(), SyntaxKind::ConstItem);
    assert_eq!(items[2].syntax().kind(), SyntaxKind::GlobalItem);
    assert_eq!(items[3].syntax().kind(), SyntaxKind::FunctionItem);
    assert_eq!(items[4].syntax().kind(), SyntaxKind::StructItem);
    assert_eq!(items[5].syntax().kind(), SyntaxKind::EnumItem);
    assert_eq!(items[6].syntax().kind(), SyntaxKind::TraitItem);
    assert_eq!(items[7].syntax().kind(), SyntaxKind::ImplItem);

    let import = tree.uses().next().expect("use item");
    assert_eq!(
        import.path().expect("use path").path_segments(),
        ["game", "player", "Player"]
    );
    assert_eq!(import.alias_text(), None);

    let constant = tree.consts().next().expect("const item");
    assert!(constant.is_public());
    assert_eq!(constant.name_text().as_deref(), Some("START_LEVEL"));
    assert_eq!(
        constant
            .type_hint()
            .expect("const type hint")
            .path_segments(),
        ["i64"]
    );

    let global = tree.globals().next().expect("global item");
    assert!(global.is_public());
    assert_eq!(global.name_text().as_deref(), Some("state"));
    assert_eq!(
        global
            .type_hint()
            .expect("global type hint")
            .path_segments(),
        ["GameState"]
    );

    let function = tree.functions().next().expect("function item");
    assert!(function.is_public());
    assert_eq!(function.name_text().as_deref(), Some("on_kill"));
    assert_eq!(
        param_names(&function.param_list().expect("function params")),
        ["ctx", "player", "monster"]
    );
    assert_eq!(
        function.body().expect("function body").statements().count(),
        1
    );
    let event = function.attributes().next().expect("function attribute");
    assert_eq!(event.path_segments(), ["event"]);
    assert_eq!(
        event
            .arguments()
            .next()
            .expect("event argument")
            .value_text()
            .as_deref(),
        Some("\"monster.kill\"")
    );

    let record = tree.structs().next().expect("struct item");
    let fields = record.field_list().expect("struct fields");
    assert_eq!(struct_field_names(&fields), ["item_id", "count"]);
    let item_id = fields.fields().next().expect("item_id field");
    let doc = item_id.attributes().next().expect("field doc attribute");
    assert_eq!(doc.path_segments(), ["doc"]);
    assert_eq!(
        doc.arguments()
            .next()
            .expect("doc argument")
            .value_text()
            .as_deref(),
        Some("\"Reward item\"")
    );

    let enumeration = tree.enums().next().expect("enum item");
    let variants = enumeration.variant_list().expect("enum variants");
    let variants_vec = variants.variants().collect::<Vec<_>>();
    assert_eq!(
        variants_vec[0]
            .attributes()
            .next()
            .expect("variant attribute")
            .path_segments(),
        ["empty"]
    );
    assert_eq!(enum_variant_names(&variants), ["None", "Active"]);

    let trait_item = tree.traits().next().expect("trait item");
    let methods = trait_item.methods().collect::<Vec<_>>();
    assert_eq!(trait_method_names(&trait_item), ["damage", "alive"]);
    let doc = methods[0].attributes().next().expect("trait method doc");
    assert_eq!(doc.path_segments(), ["doc"]);
    assert_eq!(
        doc.arguments()
            .next()
            .expect("trait method doc argument")
            .value_text()
            .as_deref(),
        Some("\"Apply damage\"")
    );
    assert!(methods[0].body().is_none());
    assert!(methods[1].body().is_some());

    let impl_item = tree.impls().next().expect("impl item");
    assert_eq!(impl_item.trait_path_segments(), ["Damageable"]);
    assert_eq!(impl_item.target_path_segments(), ["Player"]);
    let impl_methods = impl_item.methods().collect::<Vec<_>>();
    assert_eq!(impl_methods.len(), 1);
    assert_eq!(impl_methods[0].name_text().as_deref(), Some("damage"));
}

#[test]
fn parses_inherent_impl_methods() {
    let source = r#"
struct Player { level }
impl Player {
    fn bonus(self, amount) {
        return self.level + amount;
    }
}
"#;
    let parsed = parse_cst(source);

    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let tree = parsed.tree();
    let impl_item = tree.impls().next().expect("impl item");
    assert!(impl_item.for_token().is_none());
    assert_eq!(impl_item.target_path_segments(), ["Player"]);
    let methods = impl_item.methods().collect::<Vec<_>>();
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].name_text().as_deref(), Some("bonus"));
    let method_start = source.find("fn bonus").expect("method start") as u32;
    let method_end = (source.find("\n    }\n}").expect("method end") + "\n    }".len()) as u32;
    let range = methods[0].syntax().text_range();
    assert_eq!(u32::from(range.start()), method_start);
    assert_eq!(u32::from(range.end()), method_end);
}

#[test]
fn parses_structured_attribute_arguments() {
    let parsed = parse_cst(
        r#"
#[rule(kind = game::reward::Rule, tags = ["daily", "quest"], config = { enabled: true, limit: 10 })]
fn main() {
    return null;
}
"#,
    );

    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let function = parsed.tree().functions().next().expect("function item");
    assert_eq!(function.name_text().as_deref(), Some("main"));
    let attribute = function.attributes().next().expect("rule attribute");
    assert_eq!(attribute.path_segments(), ["rule"]);
    let args = attribute.arguments().collect::<Vec<_>>();
    assert_eq!(args.len(), 3);
    assert_eq!(args[0].name_text().as_deref(), Some("kind"));
    assert_eq!(args[0].value_path_segments(), ["game", "reward", "Rule"]);
    assert_eq!(args[1].name_text().as_deref(), Some("tags"));
    assert!(args[1].value_array().is_some());
    assert_eq!(args[2].name_text().as_deref(), Some("config"));
    assert!(args[2].value_map().is_some());
}

#[test]
fn parses_use_alias_metadata() {
    let parsed = parse_cst("use game::reward::grant as give_reward;");

    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let import = parsed.tree().uses().next().expect("use item");
    assert_eq!(
        import.path().expect("use path").path_segments(),
        ["game", "reward", "grant"]
    );
    assert_eq!(import.alias_text().as_deref(), Some("give_reward"));
}

#[test]
fn diagnoses_dotted_static_paths() {
    let parsed = parse_cst("use game.reward.grant;");

    assert!(parsed.diagnostics().iter().any(|diagnostic| {
        diagnostic.message == "use `::` for module/type paths; `.` is value access"
    }));
}

fn param_names(params: &crate::ast::SyntaxParamList) -> Vec<String> {
    params
        .params()
        .map(|param| param.name_text().expect("parameter name"))
        .collect()
}

fn struct_field_names(fields: &crate::ast::SyntaxStructFieldList) -> Vec<String> {
    fields
        .fields()
        .map(|field| field.name_text().expect("field name"))
        .collect()
}

fn enum_variant_names(variants: &crate::ast::SyntaxEnumVariantList) -> Vec<String> {
    variants
        .variants()
        .map(|variant| variant.name_text().expect("variant name"))
        .collect()
}

fn trait_method_names(trait_item: &crate::ast::SyntaxTraitItem) -> Vec<String> {
    trait_item
        .methods()
        .map(|method| method.name_text().expect("trait method name"))
        .collect()
}

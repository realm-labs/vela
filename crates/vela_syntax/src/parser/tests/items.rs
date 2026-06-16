use super::*;

#[test]
fn parses_core_module_items() {
    let parsed = parse_source(
        source_id(),
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

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    assert_eq!(parsed.items.len(), 8);
    let ItemKind::Use(import) = &parsed.items[0].kind else {
        panic!("expected use item");
    };
    assert_eq!(import.path, ["game", "player", "Player"]);
    assert_eq!(import.alias, None);

    let ItemKind::Const(constant) = &parsed.items[1].kind else {
        panic!("expected const item");
    };
    assert_eq!(parsed.items[1].visibility, Visibility::Public);
    assert_eq!(constant.name, "START_LEVEL");
    assert_eq!(
        constant.type_hint.as_ref().expect("const type hint").path,
        ["i64"]
    );

    let ItemKind::Global(global) = &parsed.items[2].kind else {
        panic!("expected global item");
    };
    assert_eq!(parsed.items[2].visibility, Visibility::Public);
    assert_eq!(global.name, "state");
    assert_eq!(global.type_hint.path, ["GameState"]);

    let ItemKind::Function(function) = &parsed.items[3].kind else {
        panic!("expected function item");
    };
    assert_eq!(parsed.items[3].visibility, Visibility::Public);
    assert_eq!(function.name, "on_kill");
    assert_eq!(param_names(&function.params), ["ctx", "player", "monster"]);
    assert_eq!(function.body.statements.len(), 1);
    assert_eq!(parsed.items[3].attrs[0].path, ["event"]);
    assert_eq!(
        parsed.items[3].attrs[0].value.as_deref(),
        Some("monster.kill")
    );

    let ItemKind::Struct(record) = &parsed.items[4].kind else {
        panic!("expected struct item");
    };
    assert_eq!(struct_field_names(&record.fields), ["item_id", "count"]);
    assert_eq!(record.fields[0].attrs[0].path, ["doc"]);
    assert_eq!(
        record.fields[0].attrs[0].value.as_deref(),
        Some("Reward item")
    );

    let ItemKind::Enum(enumeration) = &parsed.items[5].kind else {
        panic!("expected enum item");
    };
    assert_eq!(enumeration.variants[0].attrs[0].path, ["empty"]);
    assert_eq!(
        enum_variant_names(&enumeration.variants),
        ["None", "Active"]
    );

    let ItemKind::Trait(trait_item) = &parsed.items[6].kind else {
        panic!("expected trait item");
    };
    assert_eq!(trait_method_names(&trait_item.methods), ["damage", "alive"]);
    assert_eq!(trait_item.methods[0].attrs[0].path, ["doc"]);
    assert_eq!(
        trait_item.methods[0].attrs[0].value.as_deref(),
        Some("Apply damage")
    );
    assert!(!trait_item.methods[0].has_default);
    assert!(trait_item.methods[0].default_body.is_none());
    assert!(trait_item.methods[1].has_default);
    assert!(trait_item.methods[1].default_body.is_some());

    let ItemKind::Impl(impl_item) = &parsed.items[7].kind else {
        panic!("expected impl item");
    };
    assert_eq!(
        impl_item.kind,
        ImplKind::Trait {
            trait_path: vec!["Damageable".to_owned()]
        }
    );
    assert_eq!(impl_item.target_path, ["Player"]);
    assert_eq!(impl_item.methods.len(), 1);
    assert_eq!(impl_item.methods[0].function.name, "damage");
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
    let parsed = parse_source(source_id(), source);

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let ItemKind::Impl(impl_item) = &parsed.items[1].kind else {
        panic!("expected impl item");
    };
    assert_eq!(impl_item.kind, ImplKind::Inherent);
    assert_eq!(impl_item.target_path, ["Player"]);
    assert_eq!(impl_item.methods.len(), 1);
    assert_eq!(impl_item.methods[0].function.name, "bonus");
    let method_start = source.find("fn bonus").expect("method start");
    let method_end = source.find("\n    }\n}").expect("method end") + "\n    }".len();
    assert_eq!(
        impl_item.methods[0].span,
        Span::new(source_id(), method_start as u32, method_end as u32)
    );
}

#[test]
fn parses_structured_attribute_arguments() {
    let parsed = parse_source(
        source_id(),
        r#"
#[rule(kind = game::reward::Rule, tags = ["daily", "quest"], config = { enabled: true, limit: 10 })]
fn main() {
    return null;
}
"#,
    );

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let ItemKind::Function(function) = &parsed.items[0].kind else {
        panic!("expected function item");
    };
    assert_eq!(function.name, "main");
    assert_eq!(parsed.items[0].attrs[0].path, ["rule"]);
    assert_eq!(
        parsed.items[0].attrs[0].value.as_deref(),
        Some("kind=game::reward::Rule,tags=[\"daily\",\"quest\"],config={enabled:true,limit:10}")
    );
}

#[test]
fn parses_use_alias_metadata() {
    let parsed = parse_source(source_id(), "use game::reward::grant as give_reward;");

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let ItemKind::Use(import) = &parsed.items[0].kind else {
        panic!("expected use item");
    };
    assert_eq!(import.path, ["game", "reward", "grant"]);
    assert_eq!(import.alias.as_deref(), Some("give_reward"));
}

#[test]
fn diagnoses_dotted_static_paths() {
    let parsed = parse_source(source_id(), "use game.reward.grant;");

    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic.message == "use `::` for module/type paths; `.` is value access"
    }));
}

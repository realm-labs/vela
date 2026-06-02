use super::*;

#[test]
fn parses_type_hint_metadata_and_rejects_generics() {
    let parsed = parse_source(
        source_id(),
        r#"
fn level_up(player: game::Player, amount: int) -> Result {
    let next: int = player.level + amount;
    let mapper = |reward: Reward| reward.count;
    return next;
}

struct Reward {
    item_id: string,
    count: int,
}
"#,
    );

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let ItemKind::Function(function) = &parsed.items[0].kind else {
        panic!("expected function item");
    };
    assert_eq!(
        function.params[0]
            .type_hint
            .as_ref()
            .expect("player type hint")
            .path,
        ["game", "Player"]
    );
    assert_eq!(
        function.params[1]
            .type_hint
            .as_ref()
            .expect("amount type hint")
            .path,
        ["int"]
    );
    assert_eq!(
        function
            .return_type
            .as_ref()
            .expect("function return type hint")
            .path,
        ["Result"]
    );

    let StmtKind::Let {
        type_hint: Some(next_hint),
        ..
    } = &function.body.statements[0].kind
    else {
        panic!("expected typed let");
    };
    assert_eq!(next_hint.path, ["int"]);

    let StmtKind::Let {
        value: Some(lambda),
        ..
    } = &function.body.statements[1].kind
    else {
        panic!("expected lambda let");
    };
    let ExprKind::Lambda { params, .. } = &lambda.kind else {
        panic!("expected lambda");
    };
    assert_eq!(
        params[0]
            .type_hint
            .as_ref()
            .expect("lambda param type hint")
            .path,
        ["Reward"]
    );

    let ItemKind::Struct(record) = &parsed.items[1].kind else {
        panic!("expected struct item");
    };
    assert_eq!(
        record.fields[0]
            .type_hint
            .as_ref()
            .expect("item_id field type hint")
            .path,
        ["string"]
    );
    assert_eq!(
        record.fields[1]
            .type_hint
            .as_ref()
            .expect("count field type hint")
            .path,
        ["int"]
    );

    let generic = parse_source(source_id(), "fn bad(xs: Array<int>) { return xs; }");
    assert!(
        generic
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code.as_deref() == Some("syntax::generic_type_hint") })
    );
}

#[test]
fn parses_enum_variant_payload_metadata() {
    let parsed = parse_source(
        source_id(),
        r#"
enum QuestProgress {
    None,
    Active { quest_id: string, count: int },
    Finished(quest_id: string),
}
"#,
    );

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let ItemKind::Enum(enumeration) = &parsed.items[0].kind else {
        panic!("expected enum item");
    };
    assert_eq!(
        enum_variant_names(&enumeration.variants),
        ["None", "Active", "Finished"]
    );
    let EnumVariantFields::Record(fields) = &enumeration.variants[1].fields else {
        panic!("expected record variant fields");
    };
    assert_eq!(struct_field_names(fields), ["quest_id", "count"]);
    let EnumVariantFields::Tuple(fields) = &enumeration.variants[2].fields else {
        panic!("expected tuple variant fields");
    };
    assert_eq!(param_names(fields), ["quest_id"]);
}

#[test]
fn parses_struct_and_record_variant_field_defaults() {
    let parsed = parse_source(
        source_id(),
        r#"
struct Reward {
    item_id: string = "gold",
    count: int = 1,
}

enum QuestProgress {
    Active { quest_id: string, count: int = 0 },
}
"#,
    );

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let ItemKind::Struct(record) = &parsed.items[0].kind else {
        panic!("expected struct item");
    };
    assert!(matches!(
        record.fields[0]
            .default_value
            .as_ref()
            .map(|expr| &expr.kind),
        Some(ExprKind::Literal(Literal::String(value))) if value == "gold"
    ));
    assert!(matches!(
        record.fields[1]
            .default_value
            .as_ref()
            .map(|expr| &expr.kind),
        Some(ExprKind::Literal(Literal::Int(value))) if value == "1"
    ));

    let ItemKind::Enum(enumeration) = &parsed.items[1].kind else {
        panic!("expected enum item");
    };
    let EnumVariantFields::Record(fields) = &enumeration.variants[0].fields else {
        panic!("expected record variant fields");
    };
    assert!(matches!(
        fields[1].default_value.as_ref().map(|expr| &expr.kind),
        Some(ExprKind::Literal(Literal::Int(value))) if value == "0"
    ));
}

#[test]
fn parses_schema_members_separated_by_newlines() {
    let parsed = parse_source(
        source_id(),
        r#"
struct Reward {
    item_id
    count
}

enum QuestProgress {
    None
    Active {
        quest_id
        count
    }
    Finished(quest_id)
}
"#,
    );

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let ItemKind::Struct(record) = &parsed.items[0].kind else {
        panic!("expected struct item");
    };
    assert_eq!(struct_field_names(&record.fields), ["item_id", "count"]);

    let ItemKind::Enum(enumeration) = &parsed.items[1].kind else {
        panic!("expected enum item");
    };
    assert_eq!(
        enum_variant_names(&enumeration.variants),
        ["None", "Active", "Finished"]
    );
    let EnumVariantFields::Record(fields) = &enumeration.variants[1].fields else {
        panic!("expected record variant fields");
    };
    assert_eq!(struct_field_names(fields), ["quest_id", "count"]);
    let EnumVariantFields::Tuple(fields) = &enumeration.variants[2].fields else {
        panic!("expected tuple variant fields");
    };
    assert_eq!(param_names(fields), ["quest_id"]);
}

#[test]
fn parses_parameter_defaults_and_named_arguments() {
    let parsed = parse_source(
        source_id(),
        r#"
fn grant(player, amount = 10, reason: string = "quest") {
    return apply(amount = amount, reason = reason);
}
"#,
    );

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let ItemKind::Function(function) = &parsed.items[0].kind else {
        panic!("expected function item");
    };
    assert!(function.params[0].default_value.is_none());
    assert!(matches!(
        function.params[1]
            .default_value
            .as_ref()
            .map(|expr| &expr.kind),
        Some(ExprKind::Literal(Literal::Int(value))) if value == "10"
    ));
    assert!(matches!(
        function.params[2]
            .default_value
            .as_ref()
            .map(|expr| &expr.kind),
        Some(ExprKind::Literal(Literal::String(value))) if value == "quest"
    ));
    let StmtKind::Return(Some(Expr {
        kind: ExprKind::Call { args, .. },
        ..
    })) = &function.body.statements[0].kind
    else {
        panic!("expected call return");
    };
    assert_eq!(args[0].name.as_deref(), Some("amount"));
    assert_eq!(args[1].name.as_deref(), Some("reason"));
}

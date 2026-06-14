use super::*;

#[test]
fn parses_type_hint_metadata_and_restricted_type_arguments() {
    let parsed = parse_source(
        source_id(),
        r#"
fn level_up(player: game::Player, amount: i64) -> Result<i64, String> {
    let next: i64 = player.level + amount;
    let mapper = |reward: Reward| reward.count;
    return next;
}

struct Reward {
    item_id: String,
    count: i64,
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
        ["i64"]
    );
    let return_type = function
        .return_type
        .as_ref()
        .expect("function return type hint");
    assert_eq!(return_type.path, ["Result"]);
    assert_eq!(return_type.args.len(), 2);
    assert_eq!(return_type.args[0].path, ["i64"]);
    assert_eq!(return_type.args[1].path, ["String"]);

    let StmtKind::Let {
        type_hint: Some(next_hint),
        ..
    } = &function.body.statements[0].kind
    else {
        panic!("expected typed let");
    };
    assert_eq!(next_hint.path, ["i64"]);

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
        ["String"]
    );
    assert_eq!(
        record.fields[1]
            .type_hint
            .as_ref()
            .expect("count field type hint")
            .path,
        ["i64"]
    );

    let option = parse_source(source_id(), "fn ok(value: Option<i64>) { return value; }");
    assert!(option.diagnostics.is_empty(), "{:?}", option.diagnostics);
}

#[test]
fn parses_builtin_parameterized_container_type_hints() {
    let parsed = parse_source(
        source_id(),
        r#"
fn ok(
    ids: Array<i64>,
    names: Set<String>,
    scores: Map<String, i64>,
    players: Iterator<Player>,
    optional: Option<Array<i64>>,
    result: Result<Map<String, i64>, String>,
) -> Result<Array<Option<i64>>, String> {
    return result;
}
"#,
    );

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let ItemKind::Function(function) = &parsed.items[0].kind else {
        panic!("expected function item");
    };
    let hint = function.params[0].type_hint.as_ref().expect("Array hint");
    assert_eq!(hint.path, ["Array"]);
    assert_eq!(hint.args[0].path, ["i64"]);
    let hint = function.params[2].type_hint.as_ref().expect("Map hint");
    assert_eq!(hint.path, ["Map"]);
    assert_eq!(hint.args[0].path, ["String"]);
    assert_eq!(hint.args[1].path, ["i64"]);
    let hint = function.params[4].type_hint.as_ref().expect("Option hint");
    assert_eq!(hint.path, ["Option"]);
    assert_eq!(hint.args[0].path, ["Array"]);
    assert_eq!(hint.args[0].args[0].path, ["i64"]);
    let hint = function.params[5].type_hint.as_ref().expect("Result hint");
    assert_eq!(hint.path, ["Result"]);
    assert_eq!(hint.args[0].path, ["Map"]);
    assert_eq!(hint.args[0].args[1].path, ["i64"]);
    let return_type = function.return_type.as_ref().expect("return hint");
    assert_eq!(return_type.path, ["Result"]);
    assert_eq!(return_type.args[0].path, ["Array"]);
    assert_eq!(return_type.args[0].args[0].path, ["Option"]);
    assert_eq!(return_type.args[0].args[0].args[0].path, ["i64"]);
}

#[test]
fn rejects_unsupported_parameterized_type_hints() {
    for (source, code) in [
        (
            "fn bad(xs: Array<i64, String>) { return xs; }",
            "syntax::type_argument_arity",
        ),
        (
            "fn bad(xs: Map<String>) { return xs; }",
            "syntax::type_argument_arity",
        ),
        (
            "fn bad(xs: Map<PathProxy, String>) { return xs; }",
            "syntax::map_key_type_argument",
        ),
        (
            "fn bad(xs: Map<Range, String>) { return xs; }",
            "syntax::map_key_type_argument",
        ),
        (
            "fn bad(xs: Set<Function>) { return xs; }",
            "syntax::set_element_type_argument",
        ),
        (
            "fn bad(xs: Player<i64>) { return xs; }",
            "syntax::generic_type_hint",
        ),
        (
            "fn bad(xs: Function<i64>) { return xs; }",
            "syntax::generic_type_hint",
        ),
        (
            "fn bad(xs: Range<i64>) { return xs; }",
            "syntax::generic_type_hint",
        ),
        (
            "fn bad(xs: Option<i64, String>) { return xs; }",
            "syntax::type_argument_arity",
        ),
    ] {
        let parsed = parse_source(source_id(), source);
        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.as_deref() == Some(code)),
            "{source}: {:?}",
            parsed.diagnostics
        );
    }
}

#[test]
fn parses_value_keyed_map_and_set_type_hints() {
    let parsed = parse_source(
        source_id(),
        r#"
fn accepts(
    scores: Map<i64, String>,
    by_player: Map<Player, i64>,
    dynamic_keys: Map<Any, String>,
    players: Set<Player>,
    dynamic_values: Set<Any>,
) {
    return scores;
}
"#,
    );

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let ItemKind::Function(function) = &parsed.items[0].kind else {
        panic!("expected function item");
    };
    let map_i64 = function.params[0].type_hint.as_ref().expect("map hint");
    assert_eq!(map_i64.path, ["Map"]);
    assert_eq!(map_i64.args[0].path, ["i64"]);
    let map_player = function.params[1].type_hint.as_ref().expect("map hint");
    assert_eq!(map_player.args[0].path, ["Player"]);
    let set_player = function.params[3].type_hint.as_ref().expect("set hint");
    assert_eq!(set_player.path, ["Set"]);
    assert_eq!(set_player.args[0].path, ["Player"]);
}

#[test]
fn parses_enum_variant_payload_metadata() {
    let parsed = parse_source(
        source_id(),
        r#"
enum QuestProgress {
    None,
    Active { quest_id: String, count: i64 },
    Finished(quest_id: String),
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
    item_id: String = "gold",
    count: i64 = 1,
}

enum QuestProgress {
    Active { quest_id: String, count: i64 = 0 },
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
        Some(ExprKind::Literal(Literal::Integer(value))) if value.source_text() == "1"
    ));

    let ItemKind::Enum(enumeration) = &parsed.items[1].kind else {
        panic!("expected enum item");
    };
    let EnumVariantFields::Record(fields) = &enumeration.variants[0].fields else {
        panic!("expected record variant fields");
    };
    assert!(matches!(
        fields[1].default_value.as_ref().map(|expr| &expr.kind),
        Some(ExprKind::Literal(Literal::Integer(value))) if value.source_text() == "0"
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
fn grant(player, amount = 10, reason: String = "quest") {
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
        Some(ExprKind::Literal(Literal::Integer(value))) if value.source_text() == "10"
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

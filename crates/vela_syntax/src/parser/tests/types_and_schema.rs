use crate::ast::{
    AstNode, Literal, SyntaxExpression, SyntaxExpressionKind, SyntaxFunctionItem, SyntaxLiteral,
    SyntaxParam, SyntaxRecordFieldList, SyntaxSourceFile, SyntaxStructField, SyntaxStructFieldList,
    SyntaxTypeHint,
};
use crate::parse::{Parse, parse_source_with_id};

use super::source_id;

fn parse_cst(text: &str) -> Parse<SyntaxSourceFile> {
    parse_source_with_id(source_id(), text)
}

#[test]
fn parses_type_hint_metadata_and_restricted_type_arguments() {
    let parsed = parse_cst(
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

    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let tree = parsed.tree();
    let function = tree.functions().next().expect("function item");
    let params = params(&function);
    assert_eq!(
        params[0]
            .type_hint()
            .expect("player type hint")
            .path_segments(),
        ["game", "Player"]
    );
    assert_eq!(
        params[1]
            .type_hint()
            .expect("amount type hint")
            .path_segments(),
        ["i64"]
    );

    let return_type = function.return_type().expect("function return type hint");
    assert_eq!(return_type.path_segments(), ["Result"]);
    let return_args = type_args(&return_type);
    assert_eq!(return_args.len(), 2);
    assert_eq!(return_args[0].path_segments(), ["i64"]);
    assert_eq!(return_args[1].path_segments(), ["String"]);

    let body = function.body().expect("function body");
    let statements = body.statements().collect::<Vec<_>>();
    let next = statements[0].as_let().expect("typed let");
    assert_eq!(
        next.type_hint().expect("next type hint").path_segments(),
        ["i64"]
    );

    let mapper = statements[1].as_let().expect("lambda let");
    let lambda = mapper
        .initializer()
        .expect("lambda initializer")
        .as_lambda()
        .expect("lambda expression");
    let lambda_params = lambda
        .param_list()
        .expect("lambda param list")
        .params()
        .collect::<Vec<_>>();
    assert_eq!(
        lambda_params[0]
            .type_hint()
            .expect("lambda param type hint")
            .path_segments(),
        ["Reward"]
    );

    let record = tree.structs().next().expect("struct item");
    let fields = struct_fields(record.field_list().expect("struct fields"));
    assert_eq!(
        fields[0]
            .type_hint()
            .expect("item_id field type hint")
            .path_segments(),
        ["String"]
    );
    assert_eq!(
        fields[1]
            .type_hint()
            .expect("count field type hint")
            .path_segments(),
        ["i64"]
    );

    let option = parse_cst("fn ok(value: Option<i64>) { return value; }");
    assert!(
        option.diagnostics().is_empty(),
        "{:?}",
        option.diagnostics()
    );
}

#[test]
fn parses_builtin_parameterized_container_type_hints() {
    let parsed = parse_cst(
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

    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let function = parsed.tree().functions().next().expect("function item");
    let params = params(&function);

    let hint = params[0].type_hint().expect("Array hint");
    assert_eq!(hint.path_segments(), ["Array"]);
    assert_eq!(type_args(&hint)[0].path_segments(), ["i64"]);

    let hint = params[2].type_hint().expect("Map hint");
    let args = type_args(&hint);
    assert_eq!(hint.path_segments(), ["Map"]);
    assert_eq!(args[0].path_segments(), ["String"]);
    assert_eq!(args[1].path_segments(), ["i64"]);

    let hint = params[4].type_hint().expect("Option hint");
    let option_args = type_args(&hint);
    let array_args = type_args(&option_args[0]);
    assert_eq!(hint.path_segments(), ["Option"]);
    assert_eq!(option_args[0].path_segments(), ["Array"]);
    assert_eq!(array_args[0].path_segments(), ["i64"]);

    let hint = params[5].type_hint().expect("Result hint");
    let result_args = type_args(&hint);
    let map_args = type_args(&result_args[0]);
    assert_eq!(hint.path_segments(), ["Result"]);
    assert_eq!(result_args[0].path_segments(), ["Map"]);
    assert_eq!(map_args[1].path_segments(), ["i64"]);

    let return_type = function.return_type().expect("return hint");
    let return_args = type_args(&return_type);
    let array_args = type_args(&return_args[0]);
    let option_args = type_args(&array_args[0]);
    assert_eq!(return_type.path_segments(), ["Result"]);
    assert_eq!(return_args[0].path_segments(), ["Array"]);
    assert_eq!(array_args[0].path_segments(), ["Option"]);
    assert_eq!(option_args[0].path_segments(), ["i64"]);
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
            "fn bad(xs: Map<Function, String>) { return xs; }",
            "syntax::map_key_type_argument",
        ),
        (
            "fn bad(xs: Set<PathProxy>) { return xs; }",
            "syntax::set_element_type_argument",
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
        let parsed = parse_cst(source);
        assert!(
            parsed
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code.as_deref() == Some(code)),
            "{source}: {:?}",
            parsed.diagnostics()
        );
    }
}

#[test]
fn parses_value_keyed_map_and_set_type_hints() {
    let parsed = parse_cst(
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

    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let function = parsed.tree().functions().next().expect("function item");
    let params = params(&function);

    let map_i64 = params[0].type_hint().expect("map hint");
    assert_eq!(map_i64.path_segments(), ["Map"]);
    assert_eq!(type_args(&map_i64)[0].path_segments(), ["i64"]);

    let map_player = params[1].type_hint().expect("map hint");
    assert_eq!(type_args(&map_player)[0].path_segments(), ["Player"]);

    let set_player = params[3].type_hint().expect("set hint");
    assert_eq!(set_player.path_segments(), ["Set"]);
    assert_eq!(type_args(&set_player)[0].path_segments(), ["Player"]);
}

#[test]
fn parses_enum_variant_payload_metadata() {
    let parsed = parse_cst(
        r#"
enum QuestProgress {
    None,
    Active { quest_id: String, count: i64 },
    Finished(quest_id: String),
}
"#,
    );

    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let enumeration = parsed.tree().enums().next().expect("enum item");
    let variants = enumeration
        .variant_list()
        .expect("variant list")
        .variants()
        .collect::<Vec<_>>();

    assert_eq!(variant_names(&variants), ["None", "Active", "Finished"]);
    let fields = variants[1]
        .record_field_list()
        .expect("record variant fields");
    assert_eq!(field_names(record_fields(fields)), ["quest_id", "count"]);
    let fields = variants[2]
        .tuple_field_list()
        .expect("tuple variant fields")
        .params()
        .collect::<Vec<_>>();
    assert_eq!(param_names(&fields), ["quest_id"]);
}

#[test]
fn parses_struct_and_record_variant_field_defaults() {
    let parsed = parse_cst(
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

    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let record = parsed.tree().structs().next().expect("struct item");
    let fields = struct_fields(record.field_list().expect("struct fields"));
    assert!(matches!(
        literal_value(fields[0].default_value().as_ref().expect("item default")),
        Some(Literal::String(value)) if value == "gold"
    ));
    assert!(matches!(
        literal_value(fields[1].default_value().as_ref().expect("count default")),
        Some(Literal::Integer(value)) if value.source_text() == "1"
    ));

    let enumeration = parsed.tree().enums().next().expect("enum item");
    let variants = enumeration
        .variant_list()
        .expect("variant list")
        .variants()
        .collect::<Vec<_>>();
    let fields = variants[0]
        .record_field_list()
        .expect("record variant fields");
    let fields = record_fields(fields);
    assert!(matches!(
        literal_value(fields[1].default_value().as_ref().expect("count default")),
        Some(Literal::Integer(value)) if value.source_text() == "0"
    ));
}

#[test]
fn parses_schema_members_separated_by_newlines() {
    let parsed = parse_cst(
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

    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let record = parsed.tree().structs().next().expect("struct item");
    let fields = struct_fields(record.field_list().expect("struct fields"));
    assert_eq!(field_names(fields), ["item_id", "count"]);

    let enumeration = parsed.tree().enums().next().expect("enum item");
    let variants = enumeration
        .variant_list()
        .expect("variant list")
        .variants()
        .collect::<Vec<_>>();
    assert_eq!(variant_names(&variants), ["None", "Active", "Finished"]);
    let fields = variants[1]
        .record_field_list()
        .expect("record variant fields");
    assert_eq!(field_names(record_fields(fields)), ["quest_id", "count"]);
    let fields = variants[2]
        .tuple_field_list()
        .expect("tuple variant fields")
        .params()
        .collect::<Vec<_>>();
    assert_eq!(param_names(&fields), ["quest_id"]);
}

#[test]
fn parses_parameter_defaults_and_named_arguments() {
    let parsed = parse_cst(
        r#"
fn grant(player, amount = 10, reason: String = "quest") {
    return apply(amount = amount, reason = reason);
}
"#,
    );

    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let function = parsed.tree().functions().next().expect("function item");
    let params = params(&function);
    assert!(params[0].default_value().is_none());
    assert!(matches!(
        literal_value(params[1].default_value().as_ref().expect("amount default")),
        Some(Literal::Integer(value)) if value.source_text() == "10"
    ));
    assert!(matches!(
        literal_value(params[2].default_value().as_ref().expect("reason default")),
        Some(Literal::String(value)) if value == "quest"
    ));

    let body = function.body().expect("function body");
    let return_stmt = body
        .statements()
        .next()
        .expect("return statement")
        .as_return()
        .expect("return statement");
    let call = return_stmt
        .expression()
        .expect("return expression")
        .as_call()
        .expect("call return");
    let args = call.arguments();
    assert_eq!(args[0].name_text().as_deref(), Some("amount"));
    assert_eq!(args[1].name_text().as_deref(), Some("reason"));
}

fn params(function: &SyntaxFunctionItem) -> Vec<SyntaxParam> {
    function
        .param_list()
        .expect("parameter list")
        .params()
        .collect()
}

fn type_args(hint: &SyntaxTypeHint) -> Vec<SyntaxTypeHint> {
    hint.type_arg_list()
        .expect("type argument list")
        .type_hints()
        .collect()
}

fn struct_fields(fields: SyntaxStructFieldList) -> Vec<SyntaxStructField> {
    fields.fields().collect()
}

fn record_fields(fields: SyntaxRecordFieldList) -> Vec<SyntaxStructField> {
    fields.fields().collect()
}

fn field_names(fields: Vec<SyntaxStructField>) -> Vec<String> {
    fields
        .iter()
        .map(|field| field.name_text().expect("field name"))
        .collect()
}

fn param_names(params: &[SyntaxParam]) -> Vec<String> {
    params
        .iter()
        .map(|param| param.name_text().expect("param name"))
        .collect()
}

fn variant_names(variants: &[crate::ast::SyntaxEnumVariant]) -> Vec<String> {
    variants
        .iter()
        .map(|variant| variant.name_text().expect("variant name"))
        .collect()
}

fn literal_value(expression: &SyntaxExpression) -> Option<Literal> {
    assert_eq!(
        expression.expression_kind(),
        SyntaxExpressionKind::Literal,
        "expected literal expression, got {:?}",
        expression.syntax().kind()
    );
    SyntaxLiteral::cast(expression.syntax().clone())
        .expect("literal expression")
        .literal()
}

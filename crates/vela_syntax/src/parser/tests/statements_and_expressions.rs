use crate::ast::{
    AstNode, BinaryOp, FloatSuffix, IntRadix, IntegerSuffix, Literal, SyntaxAttribute, SyntaxBlock,
    SyntaxExpression, SyntaxExpressionKind, SyntaxFunctionItem, SyntaxLiteral, SyntaxPatternKind,
    SyntaxSourceFile, SyntaxStatement, SyntaxStatementKind,
};
use crate::parse::{Parse, parse_source_with_id};

use super::source_id;

fn parse_cst(text: &str) -> Parse<SyntaxSourceFile> {
    parse_source_with_id(source_id(), text)
}

#[test]
fn parses_function_body_statements_and_expressions() {
    let parsed = parse_cst(
        r#"
fn on_kill(ctx, player, monster) {
    let rewards = [monster.exp, 2 + 3 * 4];
    player.exp += monster.exp;
    if player.exp >= ctx.config.exp_to_next_level(player.level) {
        player.level += 1;
        player.exp = 0;
    } else {
        return null;
    }
    for reward in rewards {
        player.inventory.add(reward.item_id, reward.count);
    }
}

"#,
    );

    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let body = first_function(&parsed).body().expect("function body");
    let statements = statements(&body);
    assert_eq!(statements.len(), 4);
    assert_eq!(statements[0].statement_kind(), SyntaxStatementKind::Let);
    assert_eq!(statements[2].statement_kind(), SyntaxStatementKind::If);
    assert_eq!(statements[3].statement_kind(), SyntaxStatementKind::For);

    let value = statements[0]
        .as_let()
        .expect("let statement")
        .initializer()
        .expect("let initializer");
    let array = value.as_array().expect("array literal");
    let items = array.expressions().collect::<Vec<_>>();
    assert_eq!(items.len(), 2);
    assert_eq!(items[1].expression_kind(), SyntaxExpressionKind::Binary);
    assert_eq!(
        items[1].as_binary().expect("binary expression").operator(),
        Some(BinaryOp::Add)
    );
}

#[test]
fn parses_function_body_blocks_at_exact_cst_spans_after_recovery() {
    let text = r#"
fn first() {
    return 1;
}

bogus @@@

fn second() {
    let value = 2;
    return value;
}
"#;
    let first_start = text.find("{\n    return 1;").expect("first body") as u32;
    let first_end = first_start + "{\n    return 1;\n}".len() as u32;
    let second_start = text.find("{\n    let value = 2;").expect("second body") as u32;
    let second_end = second_start + "{\n    let value = 2;\n    return value;\n}".len() as u32;

    let parsed = parse_cst(text);

    assert!(
        !parsed.diagnostics().is_empty(),
        "expected recovery diagnostics"
    );
    let functions = parsed.tree().functions().collect::<Vec<_>>();
    assert_eq!(functions.len(), 2);
    assert_body_range(&functions[0], first_start, first_end);
    assert_body_range(&functions[1], second_start, second_end);
    assert_eq!(
        functions[0]
            .body()
            .expect("first body")
            .statements()
            .count(),
        1
    );
    assert_eq!(
        functions[1]
            .body()
            .expect("second body")
            .statements()
            .count(),
        2
    );
}

#[test]
fn parses_identity_comparison_expressions() {
    let parsed = parse_cst(
        r#"
fn main(a, b, c) {
    return a === b || a !== c;
}
"#,
    );

    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let expr = first_return_expression(&parsed);
    let binary = expr.as_binary().expect("logical or expression");
    assert_eq!(binary.operator(), Some(BinaryOp::Or));
    assert_eq!(
        binary
            .lhs()
            .expect("left comparison")
            .as_binary()
            .expect("identity comparison")
            .operator(),
        Some(BinaryOp::IdentityEqual)
    );
    assert_eq!(
        binary
            .rhs()
            .expect("right comparison")
            .as_binary()
            .expect("identity comparison")
            .operator(),
        Some(BinaryOp::IdentityNotEqual)
    );
}

#[test]
fn parses_for_in_patterns() {
    let parsed = parse_cst(
        r#"
fn main(rewards) {
    for Reward::Grant { amount } in rewards {
        total += amount;
    }
}
"#,
    );

    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let body = first_function(&parsed).body().expect("function body");
    let for_stmt = statements(&body)[0].as_for().expect("for statement");
    let pattern = for_stmt.value_pattern().expect("for value pattern");
    let record = pattern.record_pattern().expect("record variant pattern");
    let fields = record.fields().collect::<Vec<_>>();
    assert_eq!(record.path_segments(), ["Reward", "Grant"]);
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].label_text().as_deref(), Some("amount"));
    assert!(fields[0].pattern().is_none());
}

#[test]
fn parses_indexed_for_in_patterns() {
    let parsed = parse_cst(
        r#"
fn main(rewards) {
    for index, Reward::Grant { amount } in rewards {
        total += index + amount;
    }
}
"#,
    );

    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let body = first_function(&parsed).body().expect("function body");
    let for_stmt = statements(&body)[0].as_for().expect("for statement");
    let index = for_stmt.index_pattern().expect("index pattern");
    assert_eq!(index.binding_name().as_deref(), Some("index"));

    let pattern = for_stmt.value_pattern().expect("for value pattern");
    let record = pattern.record_pattern().expect("record variant pattern");
    assert_eq!(record.path_segments(), ["Reward", "Grant"]);
    assert_eq!(record.fields().count(), 1);
}

#[test]
fn parses_statement_attributes() {
    let parsed = parse_cst(
        r#"
fn main(rewards) {
    #[trace("reward")]
    let total = 0;
    #[audit]
    for reward in rewards {
        total += reward;
    }
}
"#,
    );

    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let body = first_function(&parsed).body().expect("function body");
    let statements = statements(&body);

    let let_attrs = statement_attrs(&statements[0]);
    assert_eq!(let_attrs.len(), 1);
    assert_eq!(let_attrs[0].path_segments(), ["trace"]);
    assert_eq!(
        first_attr_value(&let_attrs[0]).as_deref(),
        Some("\"reward\"")
    );
    assert_eq!(statements[0].statement_kind(), SyntaxStatementKind::Let);

    let for_attrs = statement_attrs(&statements[1]);
    assert_eq!(for_attrs.len(), 1);
    assert_eq!(for_attrs[0].path_segments(), ["audit"]);
    assert!(for_attrs[0].arguments().next().is_none());
    assert_eq!(statements[1].statement_kind(), SyntaxStatementKind::For);
}

#[test]
fn parses_match_lambda_record_and_map_expressions() {
    let parsed = parse_cst(
        r#"
fn update(player) {
    let values = {"level": player.level, count: 1};
    let reward = KillReward { item_id: "gold", count };
    let mapped = values.map(|entry| entry.value + 1);
    match player.quest_progress {
        QuestProgress::Active { quest_id, count } => {
            player.quest_progress = QuestProgress::Active { quest_id, count: count + 1 };
        },
        _ => reward,
    }
}
"#,
    );

    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let body = first_function(&parsed).body().expect("function body");
    let statements = statements(&body);
    assert_eq!(statements.len(), 4);

    let map = statements[0]
        .as_let()
        .expect("map let")
        .initializer()
        .expect("map initializer");
    assert_eq!(map.expression_kind(), SyntaxExpressionKind::Map);

    let record = statements[1]
        .as_let()
        .expect("record let")
        .initializer()
        .expect("record initializer");
    assert_eq!(record.expression_kind(), SyntaxExpressionKind::Record);

    let match_expr = statements[3].as_match().expect("match statement");
    let arms = match_expr.arms();
    assert_eq!(arms.len(), 2);
    assert_eq!(
        arms[1]
            .pattern()
            .expect("wildcard arm pattern")
            .pattern_kind(),
        Some(SyntaxPatternKind::Wildcard)
    );
}

#[test]
fn parses_zero_arg_lambda_expression() {
    let parsed = parse_cst(
        r#"
fn main() {
    let predicate = || true;
}
"#,
    );

    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let body = first_function(&parsed).body().expect("function body");
    let lambda = statements(&body)[0]
        .as_let()
        .expect("lambda let")
        .initializer()
        .expect("lambda initializer")
        .as_lambda()
        .expect("lambda expression");
    assert_eq!(
        lambda
            .param_list()
            .expect("lambda param list")
            .params()
            .count(),
        0
    );
}

#[test]
fn parser_recovers_after_bad_item() {
    let parsed = parse_cst("bogus @@@\nfn next() {}");

    assert!(!parsed.diagnostics().is_empty());
    assert_eq!(parsed.tree().items().count(), 1);
    assert_eq!(
        parsed
            .tree()
            .items()
            .next()
            .expect("recovered item")
            .syntax()
            .kind(),
        crate::SyntaxKind::FunctionItem
    );
}

#[test]
fn parses_literal_return() {
    let parsed = parse_cst("fn answer() { return 42; }");

    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    assert_eq!(
        literal_value(&first_return_expression(&parsed)),
        Some(Literal::integer("42"))
    );
}

#[test]
fn parses_char_literal_return() {
    let parsed = parse_cst("fn marker() { return '奖'; }");

    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    assert_eq!(
        literal_value(&first_return_expression(&parsed)),
        Some(Literal::Char('奖'))
    );
}

#[test]
fn parses_interpolated_string_return() {
    let parsed = parse_cst(r#"fn label(player) { return f"player {player.name}"; }"#);

    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let value = first_return_expression(&parsed);
    let literal = value.as_literal().expect("interpolated literal");
    assert_eq!(
        literal.token_text().as_deref(),
        Some(r#"f"player {player.name}""#)
    );
    let parts = literal.interpolations().collect::<Vec<_>>();
    assert_eq!(parts.len(), 1);
    assert_eq!(
        parts[0]
            .expression()
            .expect("interpolation expression")
            .expression_kind(),
        SyntaxExpressionKind::Field
    );
}

#[test]
fn parses_integer_literal_radix_metadata() {
    let parsed = parse_cst("fn numbers() { let hex = 0x2a; let binary = 0b1010; }");

    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let body = first_function(&parsed).body().expect("function body");
    let statements = statements(&body);

    assert!(matches!(
        literal_value(&let_initializer(&statements[0])),
        Some(Literal::Integer(value))
            if value.source_text() == "0x2a" && value.radix == IntRadix::Hex
    ));
    assert!(matches!(
        literal_value(&let_initializer(&statements[1])),
        Some(Literal::Integer(value))
            if value.source_text() == "0b1010" && value.radix == IntRadix::Binary
    ));
}

#[test]
fn parses_numeric_literal_suffix_metadata() {
    let parsed = parse_cst("fn numbers() { let int = 12i8; let float = 12.0f32; }");

    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let body = first_function(&parsed).body().expect("function body");
    let statements = statements(&body);

    assert!(matches!(
        literal_value(&let_initializer(&statements[0])),
        Some(Literal::Integer(value))
            if value.source_text() == "12" && value.suffix == Some(IntegerSuffix::I8)
    ));
    assert!(matches!(
        literal_value(&let_initializer(&statements[1])),
        Some(Literal::Float(value))
            if value.source_text() == "12.0" && value.suffix == Some(FloatSuffix::F32)
    ));
}

#[test]
fn parses_byte_literal_metadata() {
    let parsed = parse_cst(r#"fn bytes() { return b"\x00\xff"; }"#);

    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    assert_eq!(
        literal_value(&first_return_expression(&parsed)),
        Some(Literal::Bytes(vec![0, 255]))
    );
}

#[test]
fn parses_range_expressions() {
    let parsed = parse_cst(
        r#"
fn main() {
    let exclusive = 1..4;
    let inclusive = 1..=4;
    return inclusive;
}
"#,
    );

    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let body = first_function(&parsed).body().expect("function body");
    let statements = statements(&body);
    assert_eq!(
        let_initializer(&statements[0])
            .as_binary()
            .expect("exclusive range")
            .operator(),
        Some(BinaryOp::Range)
    );
    assert_eq!(
        let_initializer(&statements[1])
            .as_binary()
            .expect("inclusive range")
            .operator(),
        Some(BinaryOp::RangeInclusive)
    );
}

fn first_function(parsed: &Parse<SyntaxSourceFile>) -> SyntaxFunctionItem {
    parsed.tree().functions().next().expect("function item")
}

fn statements(body: &SyntaxBlock) -> Vec<SyntaxStatement> {
    body.statements().collect()
}

fn first_return_expression(parsed: &Parse<SyntaxSourceFile>) -> SyntaxExpression {
    let body = first_function(parsed).body().expect("function body");
    statements(&body)[0]
        .as_return()
        .expect("return statement")
        .expression()
        .expect("return expression")
}

fn let_initializer(statement: &SyntaxStatement) -> SyntaxExpression {
    statement
        .as_let()
        .expect("let statement")
        .initializer()
        .expect("let initializer")
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

fn statement_attrs(statement: &SyntaxStatement) -> Vec<SyntaxAttribute> {
    statement.attributes().collect()
}

fn first_attr_value(attribute: &SyntaxAttribute) -> Option<String> {
    attribute
        .arguments()
        .next()
        .and_then(|argument| argument.value_text())
}

fn assert_body_range(function: &SyntaxFunctionItem, start: u32, end: u32) {
    let range = function
        .body()
        .expect("function body")
        .syntax()
        .text_range();
    assert_eq!(u32::from(range.start()), start);
    assert_eq!(u32::from(range.end()), end);
}

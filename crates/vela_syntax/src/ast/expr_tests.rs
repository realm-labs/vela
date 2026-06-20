use crate::SyntaxKind;
use crate::ast::{
    AstNode, SyntaxArrayExpr, SyntaxAssignExpr, SyntaxBinaryExpr, SyntaxBlock, SyntaxCallExpr,
    SyntaxExprStmt, SyntaxFieldExpr, SyntaxIndexExpr, SyntaxLambdaExpr, SyntaxLiteral,
    SyntaxMapExpr, SyntaxMatchExpr, SyntaxPathExpr, SyntaxRecordExpr, SyntaxTryExpr,
    SyntaxUnaryExpr,
};
use crate::parse::parse_source;

#[test]
fn ast_block_expression_exposes_statement_children() {
    let source = r#"fn update(score) {
    let value = {
        return score;
    };
    let table = { score: score };
}
"#;
    let parse = parse_source(source);
    let body = parse
        .tree()
        .functions()
        .next()
        .expect("function item")
        .body()
        .expect("function body");
    let lets = body.let_statements().collect::<Vec<_>>();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(lets.len(), 2);

    let block_initializer = lets[0].initializer().expect("block initializer");
    assert_eq!(block_initializer.syntax().kind(), SyntaxKind::Block);
    let block =
        SyntaxBlock::cast(block_initializer.syntax().clone()).expect("typed block expression");
    assert_eq!(
        block
            .statements()
            .map(|statement| statement.syntax().kind())
            .collect::<Vec<_>>(),
        vec![SyntaxKind::ReturnStmt]
    );

    let map_initializer = lets[1].initializer().expect("map initializer");
    assert_eq!(map_initializer.syntax().kind(), SyntaxKind::MapExpr);
    assert_eq!(
        SyntaxMapExpr::cast(map_initializer.syntax().clone())
            .expect("typed map expression")
            .entries()
            .count(),
        1
    );
}

#[test]
fn ast_binary_expression_exposes_operator_and_operands() {
    let source = r#"fn update(start, end) {
    let exclusive = start..end;
    let inclusive = start..=end;
    let sum = start + end;
}
"#;
    let parse = parse_source(source);
    let body = parse
        .tree()
        .functions()
        .next()
        .expect("function item")
        .body()
        .expect("function body");
    let binary_expressions = body
        .let_statements()
        .map(|statement| {
            let initializer = statement.initializer().expect("initializer");
            SyntaxBinaryExpr::cast(initializer.syntax().clone()).expect("binary expr")
        })
        .collect::<Vec<_>>();
    let operators = binary_expressions
        .iter()
        .map(SyntaxBinaryExpr::operator_kind)
        .collect::<Vec<_>>();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(
        operators,
        vec![
            Some(SyntaxKind::DotDot),
            Some(SyntaxKind::DotDotEqual),
            Some(SyntaxKind::Plus),
        ]
    );
    for expression in &binary_expressions {
        assert_eq!(
            expression.lhs().expect("lhs").syntax().kind(),
            SyntaxKind::PathExpr
        );
        assert_eq!(
            expression.rhs().expect("rhs").syntax().kind(),
            SyntaxKind::PathExpr
        );
        assert_eq!(expression.expressions().count(), 2);
    }
}

#[test]
fn ast_assignment_expression_exposes_operator_target_and_value() {
    let source = r#"fn update(score) {
    score = 1;
    score += 2;
    score -= 3;
    score *= 4;
    score /= 5;
    score %= 6;
}
"#;
    let parse = parse_source(source);
    let body = parse
        .tree()
        .functions()
        .next()
        .expect("function item")
        .body()
        .expect("function body");
    let assignments = body
        .statements()
        .map(|statement| {
            let expr_statement =
                SyntaxExprStmt::cast(statement.syntax().clone()).expect("expression statement");
            let expression = expr_statement.expression().expect("assignment expression");
            SyntaxAssignExpr::cast(expression.syntax().clone()).expect("assign expr")
        })
        .collect::<Vec<_>>();
    let operators = assignments
        .iter()
        .map(SyntaxAssignExpr::operator_kind)
        .collect::<Vec<_>>();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(
        operators,
        vec![
            Some(SyntaxKind::Equal),
            Some(SyntaxKind::PlusEqual),
            Some(SyntaxKind::MinusEqual),
            Some(SyntaxKind::StarEqual),
            Some(SyntaxKind::SlashEqual),
            Some(SyntaxKind::PercentEqual),
        ]
    );
    for assignment in &assignments {
        assert_eq!(
            assignment.target().expect("target").syntax().kind(),
            SyntaxKind::PathExpr
        );
        assert_eq!(
            assignment.value().expect("value").syntax().kind(),
            SyntaxKind::Literal
        );
        assert_eq!(assignment.expressions().count(), 2);
    }
}

#[test]
fn ast_unary_expression_exposes_operator_tokens() {
    let source = r#"fn update(score, active) {
    let negative = -score;
    let inverted = !active;
}
"#;
    let parse = parse_source(source);
    let body = parse
        .tree()
        .functions()
        .next()
        .expect("function item")
        .body()
        .expect("function body");
    let operators = body
        .let_statements()
        .map(|statement| {
            let initializer = statement.initializer().expect("initializer");
            let unary = SyntaxUnaryExpr::cast(initializer.syntax().clone()).expect("unary expr");
            unary.operator_kind()
        })
        .collect::<Vec<_>>();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(
        operators,
        vec![Some(SyntaxKind::Minus), Some(SyntaxKind::Bang)]
    );
}

#[test]
fn ast_literal_expression_exposes_token_text_and_kind() {
    let source = r#"fn literals(name) {
    let truthy = true;
    let falsey = false;
    let empty = null;
    let count = 42;
    let ratio = 3.5;
    let label = "gold";
    let marker = 'x';
    let packet = b"\x00\xff";
    let message = f"hello {name}";
}
"#;
    let parse = parse_source(source);
    let body = parse
        .tree()
        .functions()
        .next()
        .expect("function item")
        .body()
        .expect("function body");
    let literals = body
        .let_statements()
        .map(|statement| {
            let initializer = statement.initializer().expect("initializer");
            let literal = SyntaxLiteral::cast(initializer.syntax().clone()).expect("literal expr");
            (literal.token_kind(), literal.token_text())
        })
        .collect::<Vec<_>>();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(
        literals,
        vec![
            (Some(SyntaxKind::TrueKw), Some("true".to_owned())),
            (Some(SyntaxKind::FalseKw), Some("false".to_owned())),
            (Some(SyntaxKind::NullKw), Some("null".to_owned())),
            (Some(SyntaxKind::Int), Some("42".to_owned())),
            (Some(SyntaxKind::Float), Some("3.5".to_owned())),
            (Some(SyntaxKind::String), Some(r#""gold""#.to_owned())),
            (Some(SyntaxKind::Char), Some("'x'".to_owned())),
            (Some(SyntaxKind::Bytes), Some(r#"b"\x00\xff""#.to_owned())),
            (
                Some(SyntaxKind::InterpolatedString),
                Some(r#"f"hello {name}""#.to_owned()),
            ),
        ]
    );
}

#[test]
fn ast_call_arguments_expose_names_and_values() {
    let source = r#"fn build(count, reason) {
    reward(count, amount = count + 1, reason = reason);
}
"#;
    let parse = parse_source(source);
    let body = parse
        .tree()
        .functions()
        .next()
        .expect("function item")
        .body()
        .expect("function body");
    let statement = body.statements().next().expect("call statement");
    let expression = SyntaxExprStmt::cast(statement.syntax().clone())
        .expect("expression statement")
        .expression()
        .expect("call expression");
    let call = SyntaxCallExpr::cast(expression.syntax().clone()).expect("call expr");
    let arguments = call
        .arg_list()
        .expect("argument list")
        .arguments()
        .collect::<Vec<_>>();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(arguments.len(), 3);
    assert!(arguments[0].name_token().is_none());
    assert!(arguments[0].name_text().is_none());
    assert!(arguments[0].equal_token().is_none());
    assert_eq!(
        arguments[0]
            .expression()
            .expect("positional argument value")
            .syntax()
            .kind(),
        SyntaxKind::PathExpr
    );

    assert_eq!(arguments[1].name_text().as_deref(), Some("amount"));
    assert_eq!(
        arguments[1].name_token().expect("named argument").text(),
        "amount"
    );
    assert_eq!(
        arguments[1].equal_token().expect("argument equal").kind(),
        SyntaxKind::Equal
    );
    assert_eq!(
        arguments[1]
            .expression()
            .expect("named argument value")
            .syntax()
            .kind(),
        SyntaxKind::BinaryExpr
    );

    assert_eq!(arguments[2].name_text().as_deref(), Some("reason"));
    assert_eq!(
        arguments[2]
            .expression()
            .expect("second named argument value")
            .syntax()
            .kind(),
        SyntaxKind::PathExpr
    );
}

#[test]
fn ast_path_and_delimited_expressions_expose_source_tokens() {
    let source = r#"fn build(items, index, count) {
    let path = game::reward;
    let call = grant(count, reason = "xp");
    let indexed = items[index + 1];
    let attempted = grant()?;
    let array = [count, index + 1];
    let map = { "count": count };
    let record = Reward { amount: count };
    let lambda = |item: i64| item + 1;
}
"#;
    let parse = parse_source(source);
    let body = parse
        .tree()
        .functions()
        .next()
        .expect("function item")
        .body()
        .expect("function body");
    let initializers = body
        .let_statements()
        .map(|statement| statement.initializer().expect("initializer"))
        .collect::<Vec<_>>();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());

    let path = SyntaxPathExpr::cast(initializers[0].syntax().clone()).expect("path expr");
    assert_eq!(path.path_text().as_deref(), Some("game::reward"));
    assert_eq!(
        path.path_tokens()
            .iter()
            .map(|token| (token.kind(), token.text().to_owned()))
            .collect::<Vec<_>>(),
        vec![
            (SyntaxKind::Ident, "game".to_owned()),
            (SyntaxKind::ColonColon, "::".to_owned()),
            (SyntaxKind::Ident, "reward".to_owned()),
        ]
    );

    let call = SyntaxCallExpr::cast(initializers[1].syntax().clone()).expect("call expr");
    let arg_list = call.arg_list().expect("argument list");
    assert_eq!(arg_list.l_paren_token().expect("call open").text(), "(");
    assert_eq!(arg_list.r_paren_token().expect("call close").text(), ")");

    let index = SyntaxIndexExpr::cast(initializers[2].syntax().clone()).expect("index expr");
    assert_eq!(
        index.l_bracket_token().expect("index open").kind(),
        SyntaxKind::LBracket
    );
    assert_eq!(
        index.r_bracket_token().expect("index close").kind(),
        SyntaxKind::RBracket
    );

    let tried = SyntaxTryExpr::cast(initializers[3].syntax().clone()).expect("try expr");
    assert_eq!(
        tried.question_token().expect("question token").kind(),
        SyntaxKind::Question
    );

    let array = SyntaxArrayExpr::cast(initializers[4].syntax().clone()).expect("array expr");
    assert_eq!(
        array.l_bracket_token().expect("array open").kind(),
        SyntaxKind::LBracket
    );
    assert_eq!(
        array.r_bracket_token().expect("array close").kind(),
        SyntaxKind::RBracket
    );

    let map = SyntaxMapExpr::cast(initializers[5].syntax().clone()).expect("map expr");
    assert_eq!(
        map.l_brace_token().expect("map open").kind(),
        SyntaxKind::LBrace
    );
    assert_eq!(
        map.r_brace_token().expect("map close").kind(),
        SyntaxKind::RBrace
    );

    let record = SyntaxRecordExpr::cast(initializers[6].syntax().clone()).expect("record expr");
    let record_fields = record.field_list().expect("record field list");
    assert_eq!(
        record_fields
            .l_brace_token()
            .expect("record field open")
            .kind(),
        SyntaxKind::LBrace
    );
    assert_eq!(
        record_fields
            .r_brace_token()
            .expect("record field close")
            .kind(),
        SyntaxKind::RBrace
    );

    let lambda = SyntaxLambdaExpr::cast(initializers[7].syntax().clone()).expect("lambda expr");
    let lambda_params = lambda.param_list().expect("lambda param list");
    assert_eq!(
        lambda_params
            .opening_pipe_token()
            .expect("lambda open")
            .kind(),
        SyntaxKind::Pipe
    );
    assert_eq!(
        lambda_params
            .closing_pipe_token()
            .expect("lambda close")
            .kind(),
        SyntaxKind::Pipe
    );
    assert_eq!(lambda_params.pipe_tokens().len(), 2);
    assert!(lambda_params.l_paren_token().is_none());
    assert!(lambda_params.r_paren_token().is_none());
}

#[test]
fn ast_field_expression_exposes_receiver_and_member_name() {
    let source = r#"fn update(account) {
    let balance = account.balance;
}
"#;
    let parse = parse_source(source);
    let body = parse
        .tree()
        .functions()
        .next()
        .expect("function item")
        .body()
        .expect("function body");
    let initializer = body
        .let_statements()
        .next()
        .expect("field binding")
        .initializer()
        .expect("field initializer");
    let field = SyntaxFieldExpr::cast(initializer.syntax().clone()).expect("field expr");

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(
        field.receiver().expect("receiver").syntax().kind(),
        SyntaxKind::PathExpr
    );
    assert_eq!(field.dot_token().expect("dot").kind(), SyntaxKind::Dot);
    assert_eq!(field.name_token().expect("field name").text(), "balance");
    assert_eq!(field.name_text().as_deref(), Some("balance"));
}

#[test]
fn ast_index_expression_exposes_receiver_and_index() {
    let source = r#"fn update(items, index) {
    let item = items[index + 1];
}
"#;
    let parse = parse_source(source);
    let body = parse
        .tree()
        .functions()
        .next()
        .expect("function item")
        .body()
        .expect("function body");
    let initializer = body
        .let_statements()
        .next()
        .expect("index binding")
        .initializer()
        .expect("index initializer");
    let index = SyntaxIndexExpr::cast(initializer.syntax().clone()).expect("index expr");

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(
        index.receiver().expect("receiver").syntax().kind(),
        SyntaxKind::PathExpr
    );
    assert_eq!(
        index.index().expect("index expression").syntax().kind(),
        SyntaxKind::BinaryExpr
    );
    assert_eq!(index.expressions().count(), 2);
}

#[test]
fn ast_map_entries_expose_key_colon_and_value() {
    let source = r#"fn build(amount, item) {
    let reward = {
        "amount": amount + 1,
        item: item,
    };
}
"#;
    let parse = parse_source(source);
    let body = parse
        .tree()
        .functions()
        .next()
        .expect("function item")
        .body()
        .expect("function body");
    let initializer = body
        .let_statements()
        .next()
        .expect("map binding")
        .initializer()
        .expect("map initializer");
    let map = SyntaxMapExpr::cast(initializer.syntax().clone()).expect("map expr");
    let entries = map.entries().collect::<Vec<_>>();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries[0].key().expect("string key").syntax().kind(),
        SyntaxKind::Literal
    );
    assert_eq!(
        entries[0].colon_token().expect("first colon").kind(),
        SyntaxKind::Colon
    );
    assert_eq!(
        entries[0].value().expect("first value").syntax().kind(),
        SyntaxKind::BinaryExpr
    );
    assert_eq!(entries[0].expressions().count(), 2);

    assert_eq!(
        entries[1].key().expect("identifier key").syntax().kind(),
        SyntaxKind::PathExpr
    );
    assert_eq!(
        entries[1].colon_token().expect("second colon").kind(),
        SyntaxKind::Colon
    );
    assert_eq!(
        entries[1].value().expect("second value").syntax().kind(),
        SyntaxKind::PathExpr
    );
}

#[test]
fn ast_record_expression_fields_expose_labels_and_shorthand() {
    let source = r#"fn build(amount, item) {
    let reward = Reward {
        amount: amount + 1,
        item,
    };
}

"#;
    let parse = parse_source(source);
    let body = parse
        .tree()
        .functions()
        .next()
        .expect("function item")
        .body()
        .expect("function body");
    let initializer = body
        .let_statements()
        .next()
        .expect("reward binding")
        .initializer()
        .expect("record initializer");
    let record = SyntaxRecordExpr::cast(initializer.syntax().clone()).expect("record expr");
    let fields = record
        .field_list()
        .expect("record field list")
        .fields()
        .collect::<Vec<_>>();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].label_kind(), Some(SyntaxKind::Ident));
    assert_eq!(fields[0].label_text().as_deref(), Some("amount"));
    assert_eq!(
        fields[0]
            .label_token()
            .expect("explicit field label")
            .text(),
        "amount"
    );
    assert_eq!(
        fields[0]
            .colon_token()
            .expect("explicit field colon")
            .kind(),
        SyntaxKind::Colon
    );
    assert_eq!(
        fields[0]
            .expression()
            .expect("explicit field value")
            .syntax()
            .kind(),
        SyntaxKind::BinaryExpr
    );
    assert!(!fields[0].is_shorthand());

    assert_eq!(fields[1].label_text().as_deref(), Some("item"));
    assert!(fields[1].colon_token().is_none());
    assert!(fields[1].expression().is_none());
    assert!(fields[1].is_shorthand());
}

#[test]
fn ast_match_expression_exposes_control_tokens() {
    let source = r#"fn reward(status) {
    match status {
        Ready if status.enabled => grant(),
        Pending => { return wait(); },
    };
}
"#;
    let parse = parse_source(source);
    let body = parse
        .tree()
        .functions()
        .next()
        .expect("function item")
        .body()
        .expect("function body");
    let statement = body.statements().next().expect("match statement");
    let match_expr =
        SyntaxMatchExpr::cast(statement.syntax().clone()).expect("typed match expression");
    let arm_list = match_expr.arm_list().expect("match arms");
    let arms = arm_list.arms().collect::<Vec<_>>();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(
        match_expr.match_token().expect("match token").text(),
        "match"
    );
    assert_eq!(arm_list.l_brace_token().expect("left brace").text(), "{");
    assert_eq!(arm_list.r_brace_token().expect("right brace").text(), "}");
    assert_eq!(arms.len(), 2);
    assert_eq!(
        arms[0].guard_if_token().expect("guard if token").text(),
        "if"
    );
    assert_eq!(arms[0].fat_arrow_token().expect("arrow").text(), "=>");
    assert!(arms[1].guard_if_token().is_none());
    assert_eq!(arms[1].fat_arrow_token().expect("arrow").text(), "=>");
}

use crate::SyntaxKind;
use crate::ast::{
    AstNode, SyntaxArrayExpr, SyntaxAssignExpr, SyntaxBinaryExpr, SyntaxBlock, SyntaxCallExpr,
    SyntaxExprStmt, SyntaxExpressionKind, SyntaxFieldExpr, SyntaxIndexExpr, SyntaxLambdaBody,
    SyntaxLambdaExpr, SyntaxLiteral, SyntaxMapExpr, SyntaxMatchArmBody, SyntaxMatchExpr,
    SyntaxPathExpr, SyntaxRecordExpr, SyntaxTryExpr, SyntaxUnaryExpr,
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
fn ast_expression_exposes_typed_variant_helpers() {
    let source = r#"fn variants(value, account, items, ready, state) {
    let literal = 1;
    let path = value;
    let unary = -value;
    let binary = value + 1;
    let assign = value = 1;
    let field = account.balance;
    let call = grant();
    let index = items[0];
    let tried = grant()?;
    let array = [value];
    let map = { key: value };
    let record = Reward { amount: value };
    let lambda = |item| item;
    let block = { value; };
    let branch = if ready { 1 } else { 0 };
    let matched = match state { Ready => 1, _ => 0 };
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
    let expressions = body
        .let_statements()
        .map(|statement| statement.initializer().expect("initializer"))
        .collect::<Vec<_>>();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(
        expressions
            .iter()
            .map(|expression| expression.expression_kind())
            .collect::<Vec<_>>(),
        vec![
            SyntaxExpressionKind::Literal,
            SyntaxExpressionKind::Path,
            SyntaxExpressionKind::Unary,
            SyntaxExpressionKind::Binary,
            SyntaxExpressionKind::Assign,
            SyntaxExpressionKind::Field,
            SyntaxExpressionKind::Call,
            SyntaxExpressionKind::Index,
            SyntaxExpressionKind::Try,
            SyntaxExpressionKind::Array,
            SyntaxExpressionKind::Map,
            SyntaxExpressionKind::Record,
            SyntaxExpressionKind::Lambda,
            SyntaxExpressionKind::Block,
            SyntaxExpressionKind::If,
            SyntaxExpressionKind::Match,
        ]
    );
    assert!(expressions[0].as_literal().is_some());
    assert!(expressions[1].as_path().is_some());
    assert!(expressions[2].as_unary().is_some());
    assert!(expressions[3].as_binary().is_some());
    assert!(expressions[4].as_assign().is_some());
    assert!(expressions[5].as_field().is_some());
    assert!(expressions[6].as_call().is_some());
    assert!(expressions[7].as_index().is_some());
    assert!(expressions[8].as_try().is_some());
    assert!(expressions[9].as_array().is_some());
    assert!(expressions[10].as_map().is_some());
    assert!(expressions[11].as_record().is_some());
    assert!(expressions[12].as_lambda().is_some());
    assert!(expressions[13].as_block().is_some());
    assert!(expressions[14].as_if().is_some());
    assert!(expressions[15].as_match().is_some());
    assert!(expressions[0].as_match().is_none());
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
    let arguments = call.arguments();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(call.l_paren_token().expect("call open").text(), "(");
    assert_eq!(call.r_paren_token().expect("call close").text(), ")");
    assert_eq!(
        call.separator_tokens()
            .iter()
            .map(|token| token.text().to_owned())
            .collect::<Vec<_>>(),
        vec![",", ","]
    );
    assert_eq!(
        call.callee().expect("callee").syntax().kind(),
        SyntaxKind::PathExpr
    );
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
    let map = { "count": count, index: index };
    let record = Reward { amount: count, bonus };
    let lambda = |item: i64, extra| item + extra;
    let block_lambda = |item| { return item; };
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
    assert_eq!(call.l_paren_token().expect("call open").text(), "(");
    assert_eq!(call.r_paren_token().expect("call close").text(), ")");
    assert_eq!(
        call.separator_tokens()
            .iter()
            .map(|token| token.text().to_owned())
            .collect::<Vec<_>>(),
        vec![","]
    );
    assert_eq!(call.arguments().len(), 2);
    assert_eq!(arg_list.l_paren_token().expect("call open").text(), "(");
    assert_eq!(arg_list.r_paren_token().expect("call close").text(), ")");
    assert_eq!(
        arg_list
            .separator_tokens()
            .iter()
            .map(|token| token.text().to_owned())
            .collect::<Vec<_>>(),
        vec![","]
    );

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
    assert_eq!(
        array
            .separator_tokens()
            .iter()
            .map(|token| token.text().to_owned())
            .collect::<Vec<_>>(),
        vec![","]
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
    assert_eq!(
        map.separator_tokens()
            .iter()
            .map(|token| token.text().to_owned())
            .collect::<Vec<_>>(),
        vec![","]
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
    assert_eq!(
        record_fields
            .separator_tokens()
            .iter()
            .map(|token| token.text().to_owned())
            .collect::<Vec<_>>(),
        vec![","]
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
    assert_eq!(
        lambda_params
            .separator_tokens()
            .iter()
            .map(|token| token.text().to_owned())
            .collect::<Vec<_>>(),
        vec![","]
    );
    assert!(lambda_params.l_paren_token().is_none());
    assert!(lambda_params.r_paren_token().is_none());
    assert!(matches!(
        lambda.body().expect("lambda body"),
        SyntaxLambdaBody::Expression(_)
    ));
    assert_eq!(
        lambda
            .body_expression()
            .expect("lambda expression body")
            .syntax()
            .kind(),
        SyntaxKind::BinaryExpr
    );
    assert!(lambda.body_block().is_none());

    let block_lambda =
        SyntaxLambdaExpr::cast(initializers[8].syntax().clone()).expect("block lambda expr");
    assert!(matches!(
        block_lambda.body().expect("block lambda body"),
        SyntaxLambdaBody::Block(_)
    ));
    assert!(block_lambda.body_expression().is_none());
    assert_eq!(
        block_lambda
            .body_block()
            .expect("block lambda body")
            .statements()
            .map(|statement| statement.syntax().kind())
            .collect::<Vec<_>>(),
        vec![SyntaxKind::ReturnStmt]
    );
}

#[test]
fn ast_self_path_expression_exposes_self_token() {
    let source = r#"fn build() {
    let receiver = self;
    let member = self.score;
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

    let receiver = initializers[0].as_path().expect("self path expression");
    assert!(receiver.is_self());
    assert_eq!(receiver.self_token().expect("self token").text(), "self");
    assert_eq!(receiver.path_text().as_deref(), Some("self"));

    let field = initializers[1].as_field().expect("self field expression");
    let field_receiver = field
        .receiver()
        .and_then(|expression| expression.as_path())
        .expect("field receiver path");
    assert!(field_receiver.is_self());
    assert_eq!(
        field_receiver
            .self_token()
            .expect("field self token")
            .kind(),
        SyntaxKind::SelfKw
    );
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
    let field_list = record.field_list().expect("record field list");
    let fields = record.fields();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(record.path_text().as_deref(), Some("Reward"));
    assert_eq!(
        record
            .path_tokens()
            .iter()
            .map(|token| (token.kind(), token.text().to_owned()))
            .collect::<Vec<_>>(),
        vec![(SyntaxKind::Ident, "Reward".to_owned())]
    );
    assert_eq!(
        record
            .l_brace_token()
            .expect("record expression open")
            .kind(),
        SyntaxKind::LBrace
    );
    assert_eq!(
        record
            .r_brace_token()
            .expect("record expression close")
            .kind(),
        SyntaxKind::RBrace
    );
    assert_eq!(
        record
            .separator_tokens()
            .iter()
            .map(|token| token.text().to_owned())
            .collect::<Vec<_>>(),
        vec![",", ","]
    );
    assert_eq!(
        field_list
            .separator_tokens()
            .iter()
            .map(|token| token.text().to_owned())
            .collect::<Vec<_>>(),
        vec![",", ","]
    );
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
        Pending => { return wait(); };
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
    let arms = match_expr.arms();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(
        match_expr.match_token().expect("match token").text(),
        "match"
    );
    assert_eq!(
        match_expr.l_brace_token().expect("match left brace").text(),
        "{"
    );
    assert_eq!(
        match_expr
            .r_brace_token()
            .expect("match right brace")
            .text(),
        "}"
    );
    assert_eq!(arm_list.l_brace_token().expect("left brace").text(), "{");
    assert_eq!(arm_list.r_brace_token().expect("right brace").text(), "}");
    assert_eq!(
        match_expr
            .separator_tokens()
            .iter()
            .map(|token| token.text())
            .collect::<Vec<_>>(),
        vec![",", ";"]
    );
    assert_eq!(
        arm_list
            .separator_tokens()
            .iter()
            .map(|token| token.text())
            .collect::<Vec<_>>(),
        vec![",", ";"]
    );
    assert_eq!(arms.len(), 2);
    assert_eq!(
        arms[0].guard_if_token().expect("guard if token").text(),
        "if"
    );
    assert_eq!(arms[0].fat_arrow_token().expect("arrow").text(), "=>");
    assert!(matches!(
        arms[0].body().expect("expression body"),
        SyntaxMatchArmBody::Expression(_)
    ));
    assert!(arms[0].body_expression().is_some());
    assert!(arms[0].body_block().is_none());
    assert!(arms[1].guard_if_token().is_none());
    assert_eq!(arms[1].fat_arrow_token().expect("arrow").text(), "=>");
    assert!(matches!(
        arms[1].body().expect("block body"),
        SyntaxMatchArmBody::Block(_)
    ));
    assert!(arms[1].body_expression().is_none());
    assert!(arms[1].body_block().is_some());
}

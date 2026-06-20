use super::{AstNode, SyntaxSourceFile};
use crate::ast::{
    SyntaxBreakStmt, SyntaxContinueStmt, SyntaxElseBranch, SyntaxExprStmt, SyntaxForStmt,
    SyntaxIfExpr, SyntaxPatternKind, SyntaxReturnStmt, SyntaxStatementKind,
};
use crate::parse::parse_source;
use crate::{SyntaxKind, SyntaxTreeBuilder};

#[test]
fn ast_block_exposes_statement_children() {
    let mut builder = SyntaxTreeBuilder::default();
    builder.start_node(SyntaxKind::SourceFile);
    builder.start_node(SyntaxKind::FunctionItem);
    builder.start_node(SyntaxKind::Block);
    builder.token(SyntaxKind::LBrace, "{");
    builder.start_node(SyntaxKind::LetStmt);
    builder.token(SyntaxKind::LetKw, "let");
    builder.token(SyntaxKind::Ident, "score");
    builder.token(SyntaxKind::Colon, ":");
    builder.start_node(SyntaxKind::TypeHint);
    builder.token(SyntaxKind::Ident, "i64");
    builder.finish_node();
    builder.token(SyntaxKind::Equal, "=");
    builder.token(SyntaxKind::Int, "1");
    builder.token(SyntaxKind::Semicolon, ";");
    builder.finish_node();
    builder.start_node(SyntaxKind::ForStmt);
    builder.token(SyntaxKind::ForKw, "for");
    builder.start_node(SyntaxKind::Block);
    builder.token(SyntaxKind::LBrace, "{");
    builder.start_node(SyntaxKind::ContinueStmt);
    builder.token(SyntaxKind::ContinueKw, "continue");
    builder.token(SyntaxKind::Semicolon, ";");
    builder.finish_node();
    builder.token(SyntaxKind::RBrace, "}");
    builder.finish_node();
    builder.finish_node();
    builder.token(SyntaxKind::RBrace, "}");
    builder.finish_node();
    builder.finish_node();
    builder.finish_node();

    let parse: crate::Parse<SyntaxSourceFile> = builder.finish();
    let source = SyntaxSourceFile::cast(parse.syntax_node()).expect("source file root");
    let body = source
        .functions()
        .next()
        .expect("function item")
        .body()
        .expect("body");

    assert_eq!(body.l_brace_token().expect("block open").text(), "{");
    assert_eq!(body.r_brace_token().expect("block close").text(), "}");
    assert_eq!(
        body.statements()
            .map(|statement| statement.syntax().kind())
            .collect::<Vec<_>>(),
        vec![SyntaxKind::LetStmt, SyntaxKind::ForStmt]
    );
    assert_eq!(
        body.let_statements()
            .next()
            .expect("let statement")
            .type_hint()
            .expect("let type")
            .syntax()
            .text()
            .to_string(),
        "i64"
    );
    let for_stmt = body
        .syntax()
        .children()
        .find_map(SyntaxForStmt::cast)
        .expect("for statement");
    assert_eq!(
        for_stmt.body_l_brace_token().expect("for body open").text(),
        "{"
    );
    assert_eq!(
        for_stmt
            .body_r_brace_token()
            .expect("for body close")
            .text(),
        "}"
    );
    let for_body = for_stmt.body().expect("for body");
    assert_eq!(for_body.l_brace_token().expect("for body open").text(), "{");
    assert_eq!(
        for_body.r_brace_token().expect("for body close").text(),
        "}"
    );
    assert_eq!(
        for_body
            .statements()
            .map(|statement| statement.syntax().kind())
            .collect::<Vec<_>>(),
        vec![SyntaxKind::ContinueStmt]
    );
}

#[test]
fn ast_statement_exposes_typed_variant_helpers() {
    let parse = parse_source(
        r#"fn variants(items, ready, state) {
    let value = 1;
    return value;
    break;
    continue;
    for item in items {
        item;
    }
    if ready {
        value;
    }
    #[audit]
    match state {
        Ready => value,
    }
    {
        value;
    }
    value;
}
"#,
    );
    let body = parse
        .tree()
        .functions()
        .next()
        .expect("function item")
        .body()
        .expect("function body");
    let statements = body.statements().collect::<Vec<_>>();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(
        statements
            .iter()
            .map(|statement| statement.statement_kind())
            .collect::<Vec<_>>(),
        vec![
            SyntaxStatementKind::Let,
            SyntaxStatementKind::Return,
            SyntaxStatementKind::Break,
            SyntaxStatementKind::Continue,
            SyntaxStatementKind::For,
            SyntaxStatementKind::If,
            SyntaxStatementKind::Match,
            SyntaxStatementKind::Block,
            SyntaxStatementKind::Expr,
        ]
    );
    assert!(statements[0].as_let().is_some());
    assert!(statements[1].as_return().is_some());
    assert!(statements[2].as_break().is_some());
    assert!(statements[3].as_continue().is_some());
    assert!(statements[4].as_for().is_some());
    assert!(statements[5].as_if().is_some());
    assert!(statements[6].as_match().is_some());
    assert_eq!(
        statements[6]
            .as_match()
            .and_then(|match_expr| match_expr.attributes().next())
            .and_then(|attribute| attribute.path_text())
            .as_deref(),
        Some("audit")
    );
    assert!(statements[7].as_block().is_some());
    assert!(statements[8].as_expr().is_some());
    assert!(statements[0].as_match().is_none());
}

#[test]
fn ast_for_statement_exposes_index_and_value_patterns() {
    let parse = parse_source(
        r#"fn collect(rewards) {
    #[audit]
    for reward in rewards {
        continue;
    }
    for index, Reward::Item { item_id } in rewards {
        continue;
    }
}
"#,
    );
    let body = parse
        .tree()
        .functions()
        .next()
        .expect("function item")
        .body()
        .expect("function body");
    let for_statements = body
        .syntax()
        .children()
        .filter_map(SyntaxForStmt::cast)
        .collect::<Vec<_>>();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(for_statements.len(), 2);

    let ordinary = &for_statements[0];
    assert!(ordinary.binding_separator_token().is_none());
    assert!(ordinary.index_pattern().is_none());
    assert_eq!(
        ordinary
            .attributes()
            .next()
            .expect("for attribute")
            .path_text()
            .as_deref(),
        Some("audit")
    );
    assert_eq!(
        ordinary
            .iterable()
            .expect("ordinary iterable")
            .syntax()
            .kind(),
        SyntaxKind::PathExpr
    );
    assert_eq!(
        ordinary
            .value_pattern()
            .expect("ordinary value pattern")
            .binding_name()
            .as_deref(),
        Some("reward")
    );
    assert_eq!(ordinary.patterns().count(), 1);

    let indexed = &for_statements[1];
    assert_eq!(
        indexed
            .binding_separator_token()
            .expect("binding separator")
            .text(),
        ","
    );
    assert_eq!(
        indexed
            .index_pattern()
            .expect("index pattern")
            .binding_name()
            .as_deref(),
        Some("index")
    );
    let value_pattern = indexed.value_pattern().expect("indexed value pattern");
    assert_eq!(
        value_pattern.pattern_kind(),
        Some(SyntaxPatternKind::RecordVariant)
    );
    assert_eq!(
        value_pattern
            .record_pattern()
            .expect("record value pattern")
            .path_text()
            .as_deref(),
        Some("Reward::Item")
    );
    assert_eq!(indexed.patterns().count(), 2);
}

#[test]
fn ast_control_flow_accessors_do_not_confuse_missing_operands_with_body_blocks() {
    let parse = parse_source(
        r#"fn run(items) {
    if {
        return;
    }
    for item in {
        continue;
    }
    for item in { items } {
        continue;
    }
}
"#,
    );
    let body = parse
        .tree()
        .functions()
        .next()
        .expect("function item")
        .body()
        .expect("function body");
    let if_expr = body
        .syntax()
        .children()
        .find_map(SyntaxIfExpr::cast)
        .expect("if expression");
    let for_statements = body
        .syntax()
        .children()
        .filter_map(SyntaxForStmt::cast)
        .collect::<Vec<_>>();

    assert!(if_expr.condition().is_none());
    assert_eq!(
        if_expr.then_block().expect("if body").syntax().kind(),
        SyntaxKind::Block
    );

    assert_eq!(for_statements.len(), 2);
    assert!(for_statements[0].iterable().is_none());
    assert_eq!(
        for_statements[0]
            .body()
            .expect("missing iterable body")
            .syntax()
            .kind(),
        SyntaxKind::Block
    );
    assert_eq!(
        for_statements[1]
            .iterable()
            .expect("block expression iterable")
            .syntax()
            .kind(),
        SyntaxKind::Block
    );
    assert_eq!(
        for_statements[1]
            .body()
            .expect("for body after block iterable")
            .syntax()
            .children()
            .find_map(SyntaxContinueStmt::cast)
            .expect("continue in for body")
            .continue_token()
            .expect("continue token")
            .text(),
        "continue"
    );
}

#[test]
fn ast_if_condition_preserves_multiline_logical_chain() {
    let parse = parse_source(
        r#"fn main() {
    let score = if expect_i64(default_integer) == 12
        && expect_i8(contextual) == 7i8
    {
        19
    } else {
        0
    };
}
"#,
    );
    let body = parse
        .tree()
        .functions()
        .next()
        .expect("function")
        .body()
        .expect("body");
    let if_expr = body
        .let_statements()
        .next()
        .and_then(|stmt| stmt.initializer())
        .and_then(|expr| expr.as_if())
        .expect("if initializer");

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    let condition = if_expr.condition().expect("if condition");
    assert_eq!(condition.syntax().kind(), SyntaxKind::BinaryExpr);
    let condition_text = condition.syntax().text().to_string();
    assert!(condition_text.contains("expect_i64"));
    assert!(condition_text.contains("expect_i8"));
}

#[test]
fn ast_statements_expose_keyword_and_binding_tokens() {
    let parse = parse_source(
        r#"fn update(items) {
    let total: i64 = 0;
    total;
    for index, item in items {
        if item.ready {
            return item;
        } else if item.skipped {
            continue;
        } else {
            break;
        }
    }
}
"#,
    );
    let body = parse
        .tree()
        .functions()
        .next()
        .expect("function item")
        .body()
        .expect("function body");

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());

    let let_stmt = body.let_statements().next().expect("let statement");
    assert_eq!(let_stmt.let_token().expect("let token").text(), "let");
    assert_eq!(let_stmt.name_text().as_deref(), Some("total"));
    assert_eq!(
        let_stmt.name_token().expect("let name").kind(),
        SyntaxKind::Ident
    );
    assert_eq!(
        let_stmt
            .semicolon_token()
            .expect("let statement terminator")
            .text(),
        ";"
    );

    let expr_stmt = body
        .syntax()
        .children()
        .find_map(SyntaxExprStmt::cast)
        .expect("expression statement");
    assert_eq!(
        expr_stmt
            .semicolon_token()
            .expect("expression statement terminator")
            .text(),
        ";"
    );

    let for_stmt = body
        .syntax()
        .children()
        .find_map(SyntaxForStmt::cast)
        .expect("for statement");
    assert_eq!(for_stmt.for_token().expect("for token").text(), "for");
    assert_eq!(for_stmt.in_token().expect("in token").text(), "in");
    assert_eq!(
        for_stmt.iterable().expect("for iterable").syntax().kind(),
        SyntaxKind::PathExpr
    );

    let if_expr = for_stmt
        .body()
        .expect("for body")
        .syntax()
        .children()
        .find_map(SyntaxIfExpr::cast)
        .expect("if expression");
    assert_eq!(if_expr.if_token().expect("if token").text(), "if");
    assert_eq!(
        if_expr.condition().expect("if condition").syntax().kind(),
        SyntaxKind::FieldExpr
    );
    assert_eq!(
        if_expr
            .then_as_expression()
            .expect("then as expression")
            .syntax()
            .kind(),
        SyntaxKind::Block
    );
    assert_eq!(if_expr.else_token().expect("else token").text(), "else");
    assert_eq!(
        if_expr
            .else_if_else_token()
            .expect("else-if else token")
            .text(),
        "else"
    );
    assert!(if_expr.else_block_else_token().is_none());
    assert_eq!(
        if_expr
            .else_as_expression()
            .expect("else-if as expression")
            .syntax()
            .kind(),
        SyntaxKind::IfExpr
    );
    match if_expr.else_branch().expect("else-if branch") {
        SyntaxElseBranch::If(branch) => {
            assert_eq!(
                branch
                    .condition()
                    .expect("else-if condition")
                    .syntax()
                    .kind(),
                SyntaxKind::FieldExpr
            );
        }
        SyntaxElseBranch::Block(_) => panic!("expected else-if branch"),
    }
    assert_eq!(
        if_expr.then_l_brace_token().expect("then open").kind(),
        SyntaxKind::LBrace
    );
    assert_eq!(
        if_expr.then_r_brace_token().expect("then close").kind(),
        SyntaxKind::RBrace
    );
    assert!(if_expr.else_l_brace_token().is_none());
    assert!(if_expr.else_r_brace_token().is_none());
    let else_if = if_expr.else_if().expect("else-if");
    assert_eq!(else_if.if_token().expect("else-if token").text(), "if");
    assert!(else_if.else_if_else_token().is_none());
    assert_eq!(
        else_if
            .then_as_expression()
            .expect("else-if then as expression")
            .syntax()
            .kind(),
        SyntaxKind::Block
    );
    assert_eq!(
        else_if.else_token().expect("else-if else token").text(),
        "else"
    );
    assert_eq!(
        else_if
            .else_block_else_token()
            .expect("else-block else token")
            .text(),
        "else"
    );
    assert_eq!(
        else_if
            .else_as_expression()
            .expect("else block as expression")
            .syntax()
            .kind(),
        SyntaxKind::Block
    );
    match else_if.else_branch().expect("else block branch") {
        SyntaxElseBranch::Block(block) => {
            assert_eq!(block.syntax().kind(), SyntaxKind::Block);
        }
        SyntaxElseBranch::If(_) => panic!("expected else block branch"),
    }
    assert_eq!(
        else_if
            .else_l_brace_token()
            .expect("else block open")
            .kind(),
        SyntaxKind::LBrace
    );
    assert_eq!(
        else_if
            .else_r_brace_token()
            .expect("else block close")
            .kind(),
        SyntaxKind::RBrace
    );

    let return_stmt = if_expr
        .then_block()
        .expect("then block")
        .syntax()
        .children()
        .find_map(SyntaxReturnStmt::cast)
        .expect("return statement");
    assert_eq!(
        return_stmt.return_token().expect("return token").text(),
        "return"
    );
    assert_eq!(
        return_stmt
            .semicolon_token()
            .expect("return terminator")
            .text(),
        ";"
    );

    let continue_stmt = else_if
        .then_block()
        .expect("else-if then")
        .syntax()
        .children()
        .find_map(SyntaxContinueStmt::cast)
        .expect("continue statement");
    assert_eq!(
        continue_stmt
            .continue_token()
            .expect("continue token")
            .text(),
        "continue"
    );
    assert_eq!(
        continue_stmt
            .semicolon_token()
            .expect("continue terminator")
            .text(),
        ";"
    );

    let break_stmt = else_if
        .else_block()
        .expect("else block")
        .syntax()
        .children()
        .find_map(SyntaxBreakStmt::cast)
        .expect("break statement");
    assert_eq!(
        break_stmt.break_token().expect("break token").text(),
        "break"
    );
    assert_eq!(
        break_stmt
            .semicolon_token()
            .expect("break terminator")
            .text(),
        ";"
    );
}

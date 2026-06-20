use super::{AstNode, SyntaxSourceFile};
use crate::ast::{SyntaxForStmt, SyntaxPatternKind};
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
        for_stmt
            .body()
            .expect("for body")
            .statements()
            .map(|statement| statement.syntax().kind())
            .collect::<Vec<_>>(),
        vec![SyntaxKind::ContinueStmt]
    );
}

#[test]
fn ast_for_statement_exposes_index_and_value_patterns() {
    let parse = parse_source(
        r#"fn collect(rewards) {
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
    assert!(ordinary.index_pattern().is_none());
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

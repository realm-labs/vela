use super::{AstNode, SyntaxSourceFile};
use crate::ast::SyntaxForStmt;
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

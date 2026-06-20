use crate::SyntaxKind;
use crate::ast::{AstNode, SyntaxPathExpr};
use crate::parse::parse_source;

#[test]
fn ast_path_expression_exposes_segments_and_separators() {
    let parse = parse_source(
        r#"fn main() {
    let reward = game::reward::grant;
    let current = self;
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
    let paths = body
        .let_statements()
        .map(|statement| {
            let initializer = statement.initializer().expect("initializer");
            SyntaxPathExpr::cast(initializer.syntax().clone()).expect("path expression")
        })
        .collect::<Vec<_>>();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(paths[0].path_text().as_deref(), Some("game::reward::grant"));
    assert_eq!(paths[0].path_segments(), ["game", "reward", "grant"]);
    assert_eq!(
        paths[0]
            .path_separator_tokens()
            .iter()
            .map(|token| token.kind())
            .collect::<Vec<_>>(),
        vec![SyntaxKind::ColonColon, SyntaxKind::ColonColon]
    );

    assert_eq!(paths[1].path_text().as_deref(), Some("self"));
    assert!(paths[1].path_segments().is_empty());
    assert!(paths[1].path_separator_tokens().is_empty());
    assert_eq!(
        paths[1].self_token().expect("self token").kind(),
        SyntaxKind::SelfKw
    );
}

use crate::ast::AstNode;
use crate::{SyntaxKind, parse::parse_source};

#[test]
fn unclosed_method_owners_preserve_methods_calls_and_lossless_source() {
    for (source, kind) in [
        (
            "/* 中😀 */ impl Patch { fn run(value: i64) { call(value); }",
            SyntaxKind::ImplMethod,
        ),
        (
            "/* 中😀 */ impl Patch { async fn run(value: i64) { call(value",
            SyntaxKind::ImplMethod,
        ),
        (
            "/* 中😀 */ trait Work { fn run(value: i64) { call(value",
            SyntaxKind::TraitMethod,
        ),
    ] {
        let parsed = parse_source(source);
        assert!(
            !parsed.diagnostics().is_empty(),
            "missing braces must remain diagnosed"
        );
        assert_eq!(parsed.tree().syntax().to_string(), source);
        let methods = parsed
            .tree()
            .syntax()
            .descendants()
            .filter(|node| node.kind() == kind)
            .collect::<Vec<_>>();
        assert_eq!(methods.len(), 1, "{source}");
        assert!(
            methods[0]
                .descendants()
                .any(|node| node.kind() == SyntaxKind::CallExpr),
            "call retained: {source}"
        );
        assert!(
            methods[0]
                .descendants()
                .any(|node| node.kind() == SyntaxKind::ParamList),
            "parameters retained: {source}"
        );
    }
}

use crate::ast::{AstNode, SyntaxExpressionKind, SyntaxParam};
use crate::parse::parse_source;

#[test]
fn typed_lambda_defaults_keep_their_own_parameter_type_annotation() {
    for source in [
        "fn run(value = |inner: bool| { inner }) {}",
        "trait Reader { fn run(self, value = |inner: bool| { inner }); }",
        "trait Reader { fn run(self, value = |inner: bool| { inner }) {} }",
        "struct Record {} impl Record { fn run(self, value = |inner: bool| { inner }) {} }",
        "fn run(value = |inner: bool, other: i64| { inner }, after: i64 = 1) {}",
        "trait Reader { fn run(self, value = |inner: bool, other: i64| { inner }); }",
    ] {
        let parsed = parse_source(source);
        assert!(
            parsed.diagnostics().is_empty(),
            "{source}: {:?}",
            parsed.diagnostics()
        );
        assert_eq!(parsed.tree().syntax().to_string(), source);
        let params = parsed
            .tree()
            .syntax()
            .descendants()
            .filter_map(SyntaxParam::cast)
            .collect::<Vec<_>>();
        let outer = params
            .iter()
            .find(|param| param.name_text().as_deref() == Some("value"))
            .expect("outer parameter");
        assert!(
            outer.type_hint().is_none(),
            "the nested colon must not type the outer parameter"
        );
        assert_eq!(
            outer.default_value().map(|value| value.expression_kind()),
            Some(SyntaxExpressionKind::Lambda)
        );
        let inner = params
            .iter()
            .find(|param| param.name_text().as_deref() == Some("inner"))
            .expect("lambda parameter");
        assert_eq!(
            inner
                .type_hint()
                .and_then(|hint| hint.path_text())
                .as_deref(),
            Some("bool")
        );
        if source.contains("other:") {
            let other = params
                .iter()
                .find(|param| param.name_text().as_deref() == Some("other"))
                .expect("second lambda parameter must not split the outer list");
            assert_eq!(
                other
                    .type_hint()
                    .and_then(|hint| hint.path_text())
                    .as_deref(),
                Some("i64")
            );
            assert_eq!(other.syntax().parent(), inner.syntax().parent());
            assert_ne!(other.syntax().parent(), outer.syntax().parent());
        }
    }
}

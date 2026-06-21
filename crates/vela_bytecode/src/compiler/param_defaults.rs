use vela_common::{SourceId, Span};
use vela_syntax::ast::{AstNode, Expr, Param, SyntaxExpression, SyntaxParamList};

use crate::compiler::body_payloads::CompilerExpressionPayload;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ParamDefaultValue<'ast> {
    pub(super) source: SourceId,
    pub(super) expression: SyntaxExpression,
    pub(super) fallback: &'ast Expr,
}

impl<'ast> ParamDefaultValue<'ast> {
    #[must_use]
    pub(super) fn fallback(&self) -> &'ast Expr {
        self.fallback
    }

    #[must_use]
    pub(super) fn compiler_payload(&self) -> CompilerExpressionPayload<'ast> {
        CompilerExpressionPayload::syntax(self.source, self.expression.clone(), self.fallback)
    }
}

pub(super) fn syntax_param_default_values<'ast>(
    source: SourceId,
    params: Option<SyntaxParamList>,
    legacy_params: &'ast [Param],
    param_count: usize,
) -> Vec<Option<ParamDefaultValue<'ast>>> {
    let syntax_params = params
        .map(|params| params.params().collect::<Vec<_>>())
        .unwrap_or_default();
    (0..param_count)
        .map(|index| {
            let legacy = legacy_params
                .get(index)
                .and_then(|param| param.default_value.as_ref())?;
            let expression = syntax_default_expression_for_fallback(&syntax_params, legacy)?;
            Some(ParamDefaultValue {
                source,
                expression,
                fallback: legacy,
            })
        })
        .collect()
}

fn syntax_default_expression_for_fallback(
    params: &[vela_syntax::ast::SyntaxParam],
    fallback: &Expr,
) -> Option<SyntaxExpression> {
    params
        .iter()
        .filter_map(vela_syntax::ast::SyntaxParam::default_value)
        .find(|expression| {
            syntax_range_overlaps_span(expression.syntax().text_range(), fallback.span)
        })
}

fn syntax_range_overlaps_span(range: vela_syntax::TextRange, span: Span) -> bool {
    let start = u32::from(range.start());
    let end = u32::from(range.end());
    start < span.end && span.start < end
}

#[cfg(test)]
mod tests {
    use vela_common::{SourceId, Span};
    use vela_syntax::ast::{Expr, ExprKind, Param};
    use vela_syntax::parse::parse_source_with_id as parse_syntax_source;

    use super::syntax_param_default_values;

    #[test]
    fn mismatched_param_defaults_do_not_pair_by_index() {
        let source = SourceId::new(1);
        let text = r#"
fn cst(first = 1) {
    return first;
}
"#;
        let syntax = parse_syntax_source(source, text);
        let cst_function = syntax
            .tree()
            .functions()
            .find(|function| function.name_text().as_deref() == Some("cst"))
            .expect("CST function");
        let fallback_params = vec![Param {
            name: "first".to_owned(),
            span: Span::new(source, 0, 0),
            type_hint: None,
            default_value: Some(Expr {
                kind: ExprKind::Error,
                span: Span::new(source, 1000, 1001),
            }),
        }];

        let defaults = syntax_param_default_values(
            source,
            cst_function.param_list(),
            &fallback_params,
            fallback_params.len(),
        );

        assert_eq!(defaults.len(), 1);
        assert!(
            defaults[0].is_none(),
            "mismatched default spans must not receive index-based CST params"
        );
    }
}

use vela_common::{SourceId, Span};
use vela_syntax::ast::{AstNode, Expr, SyntaxExpression};

use crate::compiler::body_payloads::CompilerExpressionPayload;
use crate::compiler::syntax_payloads::ParamDefaultExpression;

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

pub(super) fn attach_param_default_fallbacks<'ast>(
    syntax_defaults: &[Option<ParamDefaultExpression>],
    legacy_defaults: &[Option<&'ast Expr>],
) -> Vec<Option<ParamDefaultValue<'ast>>> {
    syntax_defaults
        .iter()
        .enumerate()
        .map(|(index, syntax_default)| {
            let syntax_default = syntax_default.clone()?;
            let legacy = legacy_defaults.get(index).copied().flatten()?;
            if !syntax_range_overlaps_span(
                syntax_default.expression.syntax().text_range(),
                legacy.span,
            ) {
                return None;
            }
            Some(ParamDefaultValue {
                source: syntax_default.source,
                expression: syntax_default.expression,
                fallback: legacy,
            })
        })
        .collect()
}

fn syntax_range_overlaps_span(range: vela_syntax::TextRange, span: Span) -> bool {
    let start = u32::from(range.start());
    let end = u32::from(range.end());
    start < span.end && span.start < end
}

#[cfg(test)]
mod tests {
    use vela_common::{SourceId, Span};
    use vela_syntax::ast::{Expr, ExprKind};
    use vela_syntax::parse::parse_source_with_id as parse_syntax_source;

    use crate::compiler::syntax_payloads::ParamDefaultExpression;

    use super::attach_param_default_fallbacks;

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
        let fallback_expr = Expr {
            kind: ExprKind::Error,
            span: Span::new(source, 1000, 1001),
        };
        let syntax_expression = cst_function
            .param_list()
            .and_then(|params| params.params().next())
            .and_then(|param| param.default_value())
            .expect("CST default expression");
        let syntax_defaults = vec![Some(ParamDefaultExpression {
            source,
            expression: syntax_expression,
        })];
        let fallback_defaults = vec![Some(&fallback_expr)];

        let defaults = attach_param_default_fallbacks(&syntax_defaults, &fallback_defaults);

        assert_eq!(defaults.len(), 1);
        assert!(
            defaults[0].is_none(),
            "mismatched default spans must not receive index-based CST params"
        );
    }
}

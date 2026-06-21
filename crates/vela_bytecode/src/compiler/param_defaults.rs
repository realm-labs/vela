use vela_common::SourceId;
use vela_syntax::ast::{Expr, Param, SyntaxExpression, SyntaxParamList};

use crate::compiler::body_payloads::CompilerExpressionPayload;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ParamDefaultValue {
    pub(super) source: SourceId,
    pub(super) expression: SyntaxExpression,
    pub(super) fallback: Expr,
}

impl ParamDefaultValue {
    #[must_use]
    pub(super) fn fallback(&self) -> &Expr {
        &self.fallback
    }

    #[must_use]
    pub(super) fn compiler_payload(&self) -> CompilerExpressionPayload<'_> {
        CompilerExpressionPayload::syntax(self.source, self.expression.clone(), &self.fallback)
    }
}

pub(super) fn syntax_param_default_values(
    source: SourceId,
    params: Option<SyntaxParamList>,
    legacy_params: &[Param],
    param_count: usize,
) -> Vec<Option<ParamDefaultValue>> {
    let syntax_params = params
        .map(|params| params.params().collect::<Vec<_>>())
        .unwrap_or_default();
    (0..param_count)
        .map(|index| {
            let legacy = legacy_params
                .get(index)
                .and_then(|param| param.default_value.clone())?;
            let expression = syntax_params
                .get(index)
                .and_then(|param| param.default_value())?;
            Some(ParamDefaultValue {
                source,
                expression,
                fallback: legacy,
            })
        })
        .collect()
}

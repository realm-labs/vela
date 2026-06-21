use vela_common::SourceId;
use vela_syntax::ast::{Expr, Param, SyntaxExpression, SyntaxParamList};

use crate::compiler::body_payloads::CompilerExpressionPayload;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum ParamDefaultValue {
    Syntax {
        source: SourceId,
        expression: SyntaxExpression,
        fallback: Expr,
    },
    Legacy(Expr),
}

impl ParamDefaultValue {
    #[must_use]
    pub(super) fn fallback(&self) -> &Expr {
        match self {
            Self::Syntax { fallback, .. } | Self::Legacy(fallback) => fallback,
        }
    }

    #[must_use]
    pub(super) fn compiler_payload(&self) -> Option<CompilerExpressionPayload<'_>> {
        match self {
            Self::Syntax {
                source,
                expression,
                fallback,
            } => Some(CompilerExpressionPayload::syntax(
                *source,
                expression.clone(),
                fallback,
            )),
            Self::Legacy(_) => None,
        }
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
            let Some(expression) = syntax_params
                .get(index)
                .and_then(|param| param.default_value())
            else {
                return Some(ParamDefaultValue::Legacy(legacy));
            };
            Some(ParamDefaultValue::Syntax {
                source,
                expression,
                fallback: legacy,
            })
        })
        .collect()
}

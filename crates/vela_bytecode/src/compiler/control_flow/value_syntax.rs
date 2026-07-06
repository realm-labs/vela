use crate::compiler::body_payloads::{
    CompilerBodyPayload, CompilerExpressionPayload, CompilerIfPayload, CompilerMatchArmPayload,
};
use crate::compiler::value_types::{RuntimeTypeFact, TypeContractContext};
use crate::compiler::{CompileResult, Compiler};
use vela_syntax::ast::{Expr, SyntaxExpressionKind};

use crate::compiler::expression_facts::payload_kind_matches_expression;

#[derive(Clone, Copy)]
pub(super) struct ValueSyntaxPayloads<'payload, 'ast> {
    pub(super) kind: Option<SyntaxExpressionKind>,
    pub(super) expression: Option<&'payload CompilerExpressionPayload<'ast>>,
    pub(super) block_body: Option<&'payload CompilerBodyPayload<'ast>>,
    pub(super) if_expr: Option<&'payload CompilerIfPayload<'ast>>,
    pub(super) match_arms: Option<&'payload [CompilerMatchArmPayload]>,
    pub(super) syntax_value_missing: bool,
}

impl<'payload, 'ast> ValueSyntaxPayloads<'payload, 'ast> {
    pub(super) fn new(
        kind: Option<SyntaxExpressionKind>,
        expression: Option<&'payload CompilerExpressionPayload<'ast>>,
        block_body: Option<&'payload CompilerBodyPayload<'ast>>,
        if_expr: Option<&'payload CompilerIfPayload<'ast>>,
        match_arms: Option<&'payload [CompilerMatchArmPayload]>,
        syntax_value_missing: bool,
    ) -> Self {
        Self {
            kind,
            expression,
            block_body,
            if_expr,
            match_arms,
            syntax_value_missing,
        }
    }

    pub(super) fn has_unclassified_expression_payload(self) -> bool {
        self.expression.is_some() && self.kind.is_none()
    }

    pub(super) fn has_kind_without_expression_payload(self) -> bool {
        self.kind.is_some() && self.expression.is_none()
    }

    pub(super) fn matches_value(self, value: &Expr) -> bool {
        if self.kind.is_none() {
            return false;
        }
        self.expression
            .is_some_and(|payload| value_payload_matches_expr(payload, value))
    }
}

fn value_payload_matches_expr(payload: &CompilerExpressionPayload<'_>, value: &Expr) -> bool {
    payload_kind_matches_expression(payload, value)
}

impl Compiler<'_, '_> {
    pub(super) fn check_value_payload_type(
        &self,
        value: &Expr,
        expected: RuntimeTypeFact,
        context: TypeContractContext,
        syntax_payloads: ValueSyntaxPayloads<'_, '_>,
    ) -> CompileResult<()> {
        self.expected_type_for_expr_with_payload(
            value,
            expected,
            context,
            syntax_payloads.expression,
        )
        .map(|_| ())
    }
}

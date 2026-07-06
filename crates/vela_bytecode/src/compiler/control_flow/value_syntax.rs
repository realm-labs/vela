use crate::compiler::body_payloads::{
    CompilerBodyPayload, CompilerExpressionPayload, CompilerIfPayload, CompilerMatchArmPayload,
};
use crate::compiler::value_types::{RuntimeTypeFact, TypeContractContext};
use crate::compiler::{CompileResult, Compiler};
use vela_syntax::ast::{Expr, ExprKind, SyntaxExpressionKind};

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
    let Some(kind) = payload.stored_syntax_kind() else {
        return false;
    };
    if kind == SyntaxExpressionKind::Paren {
        return true;
    }
    value_expr_matches_syntax_kind(value, kind)
        && (kind != SyntaxExpressionKind::Path
            || value_path_self_shape_matches(value, payload.syntax_is_self()))
}

fn value_expr_matches_syntax_kind(value: &Expr, kind: SyntaxExpressionKind) -> bool {
    matches!(
        (&value.kind, kind),
        (
            ExprKind::Literal(_) | ExprKind::InterpolatedString(_),
            SyntaxExpressionKind::Literal
        ) | (
            ExprKind::Path(_) | ExprKind::SelfValue,
            SyntaxExpressionKind::Path
        ) | (ExprKind::Unary { .. }, SyntaxExpressionKind::Unary)
            | (ExprKind::Binary { .. }, SyntaxExpressionKind::Binary)
            | (ExprKind::Assign { .. }, SyntaxExpressionKind::Assign)
            | (ExprKind::Field { .. }, SyntaxExpressionKind::Field)
            | (ExprKind::Call { .. }, SyntaxExpressionKind::Call)
            | (ExprKind::Index { .. }, SyntaxExpressionKind::Index)
            | (ExprKind::Try(_), SyntaxExpressionKind::Try)
            | (ExprKind::Array(_), SyntaxExpressionKind::Array)
            | (ExprKind::Map(_), SyntaxExpressionKind::Map)
            | (ExprKind::Record { .. }, SyntaxExpressionKind::Record)
            | (ExprKind::Lambda { .. }, SyntaxExpressionKind::Lambda)
            | (ExprKind::Block(_), SyntaxExpressionKind::Block)
            | (ExprKind::If(_), SyntaxExpressionKind::If)
            | (ExprKind::Match(_), SyntaxExpressionKind::Match)
    )
}

fn value_path_self_shape_matches(value: &Expr, syntax_is_self: bool) -> bool {
    matches!(
        (&value.kind, syntax_is_self),
        (ExprKind::Path(_), false) | (ExprKind::SelfValue, true)
    )
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

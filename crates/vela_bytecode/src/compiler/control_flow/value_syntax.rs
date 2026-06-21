use crate::compiler::body_payloads::{
    CompilerBodyPayload, CompilerExpressionPayload, CompilerIfPayload, CompilerMatchArmPayload,
};
use crate::compiler::value_types::{RuntimeTypeFact, TypeContractContext};
use crate::compiler::{CompileResult, Compiler};
use vela_syntax::ast::Expr;

#[derive(Clone, Copy)]
pub(super) struct ValueSyntaxPayloads<'payload, 'ast> {
    pub(super) expression: Option<&'payload CompilerExpressionPayload<'ast>>,
    pub(super) block_body: Option<&'payload CompilerBodyPayload<'ast>>,
    pub(super) if_expr: Option<&'payload CompilerIfPayload<'ast>>,
    pub(super) match_arms: Option<&'payload [CompilerMatchArmPayload<'ast>]>,
}

impl<'payload, 'ast> ValueSyntaxPayloads<'payload, 'ast> {
    pub(super) fn new(
        expression: Option<&'payload CompilerExpressionPayload<'ast>>,
        block_body: Option<&'payload CompilerBodyPayload<'ast>>,
        if_expr: Option<&'payload CompilerIfPayload<'ast>>,
        match_arms: Option<&'payload [CompilerMatchArmPayload<'ast>]>,
    ) -> Self {
        Self {
            expression,
            block_body,
            if_expr,
            match_arms,
        }
    }
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

use crate::compiler::body_payloads::{
    CompilerBodyPayload, CompilerExpressionPayload, CompilerIfPayload, CompilerMatchArmPayload,
};

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

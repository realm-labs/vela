use vela_syntax::ast::Expr;

use crate::compiler::body_payloads::CompilerExpressionPayload;
use crate::compiler::error::{CompileError, CompileErrorKind, CompileResult};
use crate::compiler::expression_payload_kinds::expression_payload_is_aligned;

pub(in crate::compiler) fn validate_assignment_target_payload(
    target: &Expr,
    payload: Option<&CompilerExpressionPayload<'_>>,
) -> CompileResult<()> {
    if payload.is_some_and(|payload| !expression_payload_is_aligned(payload, target)) {
        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "mismatched CST assignment target",
        )));
    }
    Ok(())
}

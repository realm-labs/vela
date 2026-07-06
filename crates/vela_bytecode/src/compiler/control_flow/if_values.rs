use vela_syntax::ast::{ElseBranch, IfExpr};

use crate::compiler::body_payloads::{CompilerBodyPayload, CompilerIfPayload};
use crate::compiler::{CompileError, CompileErrorKind, CompileResult, Compiler};
use crate::{Constant, Register};

impl Compiler<'_, '_> {
    #[cfg(test)]
    pub(in crate::compiler) fn compile_if_value_to(
        &mut self,
        if_expr: &IfExpr,
        dst: Register,
    ) -> CompileResult<bool> {
        self.compile_if_value_with_payloads(if_expr, dst, None)
    }

    pub(in crate::compiler) fn compile_if_value_with_payloads(
        &mut self,
        if_expr: &IfExpr,
        dst: Register,
        payload: Option<&CompilerIfPayload<'_>>,
    ) -> CompileResult<bool> {
        let condition_payload = payload.and_then(CompilerIfPayload::condition_payload);
        let condition_payload = required_if_child_payload(
            payload,
            condition_payload.as_ref(),
            "missing CST if condition payload",
        )?;
        let jump_to_else =
            self.emit_condition_jump_if_false(&if_expr.condition, condition_payload)?;

        let then_body_payload = required_if_child_payload(
            payload,
            payload.and_then(CompilerIfPayload::then_body),
            "missing CST if then body payload",
        )?;
        let then_returned =
            self.compile_if_value_block_to(&if_expr.then_branch, then_body_payload, dst)?;
        let jump_to_end = if then_returned {
            None
        } else {
            Some(self.emit_jump())
        };

        self.patch_jump(jump_to_else, self.current_offset())?;

        let else_returned = match &if_expr.else_branch {
            Some(ElseBranch::Block(block)) => {
                let else_body_payload = required_if_child_payload(
                    payload,
                    payload.and_then(CompilerIfPayload::else_body),
                    "missing CST if else body payload",
                )?;
                self.compile_if_value_block_to(block, else_body_payload, dst)?
            }
            Some(ElseBranch::If(if_expr)) => {
                let else_if_payload = required_if_child_payload(
                    payload,
                    payload.and_then(CompilerIfPayload::else_if),
                    "missing CST else-if payload",
                )?;
                self.compile_if_value_with_payloads(if_expr, dst, else_if_payload)?
            }
            None => {
                self.emit_constant_to(dst, Constant::Null);
                false
            }
        };

        if let Some(jump_to_end) = jump_to_end {
            self.patch_jump(jump_to_end, self.current_offset())?;
        }

        Ok(then_returned && else_returned)
    }

    fn compile_if_value_block_to(
        &mut self,
        block: &vela_syntax::ast::Block,
        payload: Option<&CompilerBodyPayload<'_>>,
        dst: Register,
    ) -> CompileResult<bool> {
        if let Some(payload) = payload {
            self.compile_block_payload_value_to(payload, dst)
        } else {
            let _ = block;
            let _ = dst;
            Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST if block body payload",
            )))
        }
    }
}

fn required_if_child_payload<'payload, T>(
    parent: Option<&CompilerIfPayload<'_>>,
    child: Option<&'payload T>,
    context: &'static str,
) -> CompileResult<Option<&'payload T>> {
    if parent.is_none() {
        return Ok(None);
    }
    child
        .map(Some)
        .ok_or_else(|| CompileError::new(CompileErrorKind::UnsupportedSyntax(context)))
}

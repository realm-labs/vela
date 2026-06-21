use vela_syntax::ast::{ElseBranch, IfExpr};

use crate::compiler::body_payloads::{CompilerBodyPayload, CompilerIfPayload};
use crate::compiler::{CompileResult, Compiler};
use crate::{Constant, Register};

impl Compiler<'_, '_> {
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
        let jump_to_else = self.emit_condition_jump_if_false(
            &if_expr.condition,
            payload.and_then(CompilerIfPayload::condition_operator),
            payload.and_then(CompilerIfPayload::condition_payload),
        )?;

        let then_returned = self.compile_if_value_block_to(
            &if_expr.then_branch,
            payload.and_then(CompilerIfPayload::then_body),
            dst,
        )?;
        let jump_to_end = if then_returned {
            None
        } else {
            Some(self.emit_jump())
        };

        self.patch_jump(jump_to_else, self.current_offset())?;

        let else_returned = match &if_expr.else_branch {
            Some(ElseBranch::Block(block)) => self.compile_if_value_block_to(
                block,
                payload.and_then(CompilerIfPayload::else_body),
                dst,
            )?,
            Some(ElseBranch::If(if_expr)) => self.compile_if_value_with_payloads(
                if_expr,
                dst,
                payload.and_then(CompilerIfPayload::else_if),
            )?,
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
            self.compile_block_value_to(block, dst)
        }
    }
}

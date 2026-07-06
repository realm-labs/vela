use vela_syntax::ast::{ElseBranch, IfExpr};

use crate::compiler::body_payloads::{CompilerBodyPayload, CompilerIfPayload};
use crate::compiler::{CompileError, CompileErrorKind, CompileResult, Compiler};
use crate::{Constant, Register};

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn compile_if_value_with_payloads(
        &mut self,
        if_expr: &IfExpr,
        dst: Register,
        payload: &CompilerIfPayload<'_>,
    ) -> CompileResult<bool> {
        let condition_payload = payload.condition_payload();
        let condition_payload = required_if_child_payload(
            condition_payload.as_ref(),
            "missing CST if condition payload",
        )?;
        let jump_to_else =
            self.emit_condition_jump_if_false(&if_expr.condition, Some(condition_payload))?;

        let then_body_payload =
            required_if_child_payload(payload.then_body(), "missing CST if then body payload")?;
        let then_returned = self.compile_if_value_block_to(then_body_payload, dst)?;
        let jump_to_end = if then_returned {
            None
        } else {
            Some(self.emit_jump())
        };

        self.patch_jump(jump_to_else, self.current_offset())?;

        let else_returned = match &if_expr.else_branch {
            Some(ElseBranch::Block(_)) => {
                let else_body_payload = required_if_child_payload(
                    payload.else_body(),
                    "missing CST if else body payload",
                )?;
                self.compile_if_value_block_to(else_body_payload, dst)?
            }
            Some(ElseBranch::If(if_expr)) => {
                let else_if_payload =
                    required_if_child_payload(payload.else_if(), "missing CST else-if payload")?;
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
        payload: &CompilerBodyPayload<'_>,
        dst: Register,
    ) -> CompileResult<bool> {
        self.compile_block_payload_value_to(payload, dst)
    }
}

fn required_if_child_payload<'payload, T>(
    child: Option<&'payload T>,
    context: &'static str,
) -> CompileResult<&'payload T> {
    child.ok_or_else(|| CompileError::new(CompileErrorKind::UnsupportedSyntax(context)))
}

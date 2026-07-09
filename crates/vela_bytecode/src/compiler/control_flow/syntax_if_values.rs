use vela_common::SourceId;
use vela_syntax::ast::{SyntaxElseBranch, SyntaxExpression, SyntaxIfExpr};

use crate::compiler::body_payloads::CompilerBodyPayload;
use crate::compiler::{CompileResult, Compiler};
use crate::{Constant, Register};

impl Compiler<'_, '_> {
    pub(super) fn compile_syntax_if_value(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        let Some(if_expr) = expression.as_if() else {
            return Ok(None);
        };
        if !syntax_if_value_lowering_covers(&if_expr) {
            return Ok(None);
        }
        let dst = self.alloc_register()?;
        let Some(returned) = self.compile_syntax_if_value_to(source, &if_expr, dst)? else {
            return Ok(None);
        };
        let _ = returned;
        Ok(Some(dst))
    }

    pub(super) fn compile_syntax_if_value_to(
        &mut self,
        source: SourceId,
        if_expr: &SyntaxIfExpr,
        dst: Register,
    ) -> CompileResult<Option<bool>> {
        let Some(condition_expression) = if_expr.condition() else {
            return Ok(None);
        };
        let Some(condition) = self.compile_syntax_expression(source, &condition_expression)? else {
            return Ok(None);
        };
        let Some(then_block) = if_expr.then_block() else {
            return Ok(None);
        };
        let then_body = CompilerBodyPayload::nested_syntax(source, then_block);

        let jump_to_else = self.emit_jump_if_false(condition);
        let then_returned = self.compile_block_payload_value_to(&then_body, dst)?;
        let jump_to_end = if then_returned {
            None
        } else {
            Some(self.emit_jump())
        };

        self.patch_jump(jump_to_else, self.current_offset())?;
        let else_returned = match if_expr.else_branch() {
            Some(SyntaxElseBranch::If(else_if)) => {
                let Some(returned) = self.compile_syntax_if_value_to(source, &else_if, dst)? else {
                    return Ok(None);
                };
                returned
            }
            Some(SyntaxElseBranch::Block(block)) => {
                let else_body = CompilerBodyPayload::nested_syntax(source, block);
                self.compile_block_payload_value_to(&else_body, dst)?
            }
            None => {
                self.emit_constant_to(dst, Constant::Unit);
                false
            }
        };

        if let Some(jump_to_end) = jump_to_end {
            self.patch_jump(jump_to_end, self.current_offset())?;
        }
        Ok(Some(then_returned && else_returned))
    }
}

fn syntax_if_value_lowering_covers(if_expr: &SyntaxIfExpr) -> bool {
    if if_expr.condition().is_none() || if_expr.then_block().is_none() {
        return false;
    }
    match if_expr.else_branch() {
        Some(SyntaxElseBranch::If(else_if)) => syntax_if_value_lowering_covers(&else_if),
        Some(SyntaxElseBranch::Block(_)) => true,
        None => true,
    }
}

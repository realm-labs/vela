use vela_syntax::ast::{BinaryOp, Expr, ExprKind, Literal};

use crate::compiler::body_payloads::CompilerExpressionPayload;
use crate::compiler::const_eval::compile_literal_constant_for_type;
use crate::compiler::control_flow::classification::condition_operator_for_fallback;
use crate::compiler::operators::i64_compare_op;
use crate::compiler::value_types::RuntimeTypeFact;
use crate::compiler::{CompileResult, Compiler};
use crate::{Constant, InstructionOffset, UnlinkedInstructionKind};

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn emit_condition_jump_if_false(
        &mut self,
        condition: &Expr,
        condition_operator: Option<BinaryOp>,
        condition_payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> CompileResult<usize> {
        if let Some(jump) = self.try_emit_i64_immediate_jump_if_false(
            condition,
            condition_operator,
            condition_payload,
        )? {
            return Ok(jump);
        }
        let condition = self.compile_expr_with_payload(condition, condition_payload)?;
        Ok(self.emit_jump_if_false(condition))
    }

    fn try_emit_i64_immediate_jump_if_false(
        &mut self,
        condition: &Expr,
        condition_operator: Option<BinaryOp>,
        condition_payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> CompileResult<Option<usize>> {
        let ExprKind::Binary { left, right, .. } = &condition.kind else {
            return Ok(None);
        };
        let operand_payloads =
            condition_payload.and_then(CompilerExpressionPayload::binary_operand_payloads);
        let left_payload = operand_payloads.as_ref().map(|(left, _)| left);
        let Some(op) =
            condition_operator_for_fallback(condition_operator, condition).and_then(i64_compare_op)
        else {
            return Ok(None);
        };
        if self.value_type_for_expr(left)
            != Some(RuntimeTypeFact::Primitive(vela_common::PrimitiveTag::I64))
        {
            return Ok(None);
        }
        let Some(imm) = self.i64_literal_value(right)? else {
            return Ok(None);
        };
        let lhs = self.compile_expr_with_payload(left, left_payload)?;
        let offset = self.current_offset();
        self.emit(UnlinkedInstructionKind::I64CmpImmJumpIfFalse {
            op,
            lhs,
            imm,
            target: InstructionOffset(usize::MAX),
        });
        Ok(Some(offset))
    }

    fn i64_literal_value(&self, expr: &Expr) -> CompileResult<Option<i64>> {
        let ExprKind::Literal(Literal::Integer(value)) = &expr.kind else {
            return Ok(None);
        };
        let Some(Constant::Scalar(vela_common::ScalarValue::I64(value))) =
            compile_literal_constant_for_type(
                &Literal::Integer(value.clone()),
                vela_common::PrimitiveTag::I64,
            )?
        else {
            return Ok(None);
        };
        Ok(Some(value))
    }
}

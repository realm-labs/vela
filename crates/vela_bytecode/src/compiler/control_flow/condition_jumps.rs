use vela_syntax::ast::{BinaryOp, Expr, ExprKind, Literal};

use crate::compiler::body_payloads::CompilerExpressionPayload;
use crate::compiler::const_eval::compile_literal_constant_for_type;
use crate::compiler::control_flow::classification::{
    condition_operator_for_fallback, control_flow_expression_requires_matching_syntax,
    value_expression_kind_matches,
};
use crate::compiler::operators::i64_compare_op;
use crate::compiler::value_types::RuntimeTypeFact;
use crate::compiler::{CompileError, CompileErrorKind};
use crate::compiler::{CompileResult, Compiler};
use crate::{Constant, InstructionOffset, UnlinkedInstructionKind};

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn emit_condition_jump_if_false(
        &mut self,
        condition: &Expr,
        condition_operator: Option<BinaryOp>,
        condition_payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> CompileResult<usize> {
        if let Some(payload) = condition_payload
            && let Some(kind) = payload.kind()
            && !value_expression_kind_matches(kind, condition)
            && control_flow_expression_requires_matching_syntax(condition)
        {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "mismatched CST if condition payload",
            )));
        }
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
        let right_payload = operand_payloads.as_ref().map(|(_, right)| right);
        let Some(op) = condition_operator_for_fallback(
            condition_operator,
            condition_payload.is_some(),
            condition,
        )
        .and_then(i64_compare_op) else {
            return Ok(None);
        };
        if self.value_type_for_expr_with_payload(left, left_payload)
            != Some(RuntimeTypeFact::Primitive(vela_common::PrimitiveTag::I64))
        {
            return Ok(None);
        }
        let Some(imm) = self.i64_literal_value(right, right_payload)? else {
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

    fn i64_literal_value(
        &self,
        expr: &Expr,
        payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> CompileResult<Option<i64>> {
        let literal = match payload {
            Some(payload) => payload.syntax_literal(),
            None => match &expr.kind {
                ExprKind::Literal(literal) => Some(literal.clone()),
                _ => None,
            },
        };
        let Some(Literal::Integer(value)) = literal else {
            return Ok(None);
        };
        let Some(Constant::Scalar(vela_common::ScalarValue::I64(value))) =
            compile_literal_constant_for_type(
                &Literal::Integer(value),
                vela_common::PrimitiveTag::I64,
            )?
        else {
            return Ok(None);
        };
        Ok(Some(value))
    }
}

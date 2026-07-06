use vela_syntax::ast::{Expr, ExprKind, Literal};

use crate::compiler::body_payloads::CompilerExpressionPayload;
use crate::compiler::const_eval::compile_literal_constant_for_type;
use crate::compiler::control_flow::classification::condition_operator_for_payload;
use crate::compiler::expression_facts::{
    expression_path_is_self, expression_syntax_kind, payload_syntax_kind_matches_expression_facts,
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
        condition_payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> CompileResult<usize> {
        if let Some(payload) = condition_payload
            && !condition_payload_matches_expr(payload, condition)
        {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "mismatched CST if condition payload",
            )));
        }
        if let Some(jump) =
            self.try_emit_i64_immediate_jump_if_false(condition, condition_payload)?
        {
            return Ok(jump);
        }
        let condition = self.compile_expr_with_payload(condition, condition_payload)?;
        Ok(self.emit_jump_if_false(condition))
    }

    fn try_emit_i64_immediate_jump_if_false(
        &mut self,
        condition: &Expr,
        condition_payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> CompileResult<Option<usize>> {
        let ExprKind::Binary { left, right: _, .. } = &condition.kind else {
            return Ok(None);
        };
        let operand_payloads =
            condition_payload.and_then(|payload| payload.binary_operand_payloads());
        let left_payload = operand_payloads.as_ref().map(|(left, _)| left);
        let right_payload = operand_payloads.as_ref().map(|(_, right)| right);
        let Some(op) = condition_operator_for_payload(condition_payload).and_then(i64_compare_op)
        else {
            return Ok(None);
        };
        if self.value_type_for_expr_with_payload(left, left_payload)
            != Some(RuntimeTypeFact::Primitive(vela_common::PrimitiveTag::I64))
        {
            return Ok(None);
        }
        let Some(imm) = self.i64_literal_value(right_payload)? else {
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
        payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> CompileResult<Option<i64>> {
        let Some(payload) = payload else {
            return Ok(None);
        };
        let literal = payload.syntax_literal();
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

fn condition_payload_matches_expr(
    payload: &CompilerExpressionPayload<'_>,
    condition: &Expr,
) -> bool {
    payload_syntax_kind_matches_expression_facts(
        payload,
        expression_syntax_kind(condition),
        expression_path_is_self(condition),
    )
}

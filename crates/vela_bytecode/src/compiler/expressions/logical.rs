use vela_syntax::ast::{BinaryOp, Expr, ExprKind};

use crate::Register;

use crate::compiler::body_payloads::CompilerExpressionPayload;
use crate::compiler::{CompileError, CompileErrorKind, CompileResult, Compiler};

impl Compiler<'_, '_> {
    pub(super) fn compile_logical_chain(
        &mut self,
        op: BinaryOp,
        expr: &Expr,
        payloads: &[CompilerExpressionPayload<'_>],
    ) -> CompileResult<Register> {
        let operands = logical_chain_operands(op, expr);
        reject_mismatched_logical_chain_payloads(&operands, payloads)?;
        match op {
            BinaryOp::And => self.compile_logical_and_chain(&operands, payloads),
            BinaryOp::Or => self.compile_logical_or_chain(&operands, payloads),
            _ => unreachable!("logical chain only supports && and ||"),
        }
    }

    fn compile_logical_and_chain(
        &mut self,
        operands: &[&Expr],
        payloads: &[CompilerExpressionPayload<'_>],
    ) -> CompileResult<Register> {
        reject_mismatched_logical_chain_payloads(operands, payloads)?;
        let dst = self.alloc_register()?;
        let Some((last, prefix)) = operands.split_last() else {
            self.emit_bool_constant_to(dst, true);
            return Ok(dst);
        };

        let mut false_branches = Vec::with_capacity(prefix.len());
        for (index, operand) in prefix.iter().enumerate() {
            let value = self.compile_expr_with_payload(operand, Some(&payloads[index]))?;
            false_branches.push(self.emit_jump_if_false(value));
        }

        let last = self.compile_expr_with_payload(last, Some(&payloads[prefix.len()]))?;
        self.emit_truthy_to_bool(dst, last)?;
        let end = self.emit_jump();

        for false_branch in false_branches {
            self.patch_jump(false_branch, self.current_offset())?;
        }
        self.emit_bool_constant_to(dst, false);
        self.patch_jump(end, self.current_offset())?;

        Ok(dst)
    }

    fn compile_logical_or_chain(
        &mut self,
        operands: &[&Expr],
        payloads: &[CompilerExpressionPayload<'_>],
    ) -> CompileResult<Register> {
        reject_mismatched_logical_chain_payloads(operands, payloads)?;
        let dst = self.alloc_register()?;
        let Some((last, prefix)) = operands.split_last() else {
            self.emit_bool_constant_to(dst, false);
            return Ok(dst);
        };

        let mut end_jumps = Vec::with_capacity(prefix.len());
        for (index, operand) in prefix.iter().enumerate() {
            let value = self.compile_expr_with_payload(operand, Some(&payloads[index]))?;
            let next_operand = self.emit_jump_if_false(value);
            self.emit_bool_constant_to(dst, true);
            end_jumps.push(self.emit_jump());
            self.patch_jump(next_operand, self.current_offset())?;
        }

        let last = self.compile_expr_with_payload(last, Some(&payloads[prefix.len()]))?;
        self.emit_truthy_to_bool(dst, last)?;
        for end in end_jumps {
            self.patch_jump(end, self.current_offset())?;
        }

        Ok(dst)
    }
}

fn logical_chain_operands(op: BinaryOp, expr: &Expr) -> Vec<&Expr> {
    let mut operands = Vec::new();
    let mut stack = vec![expr];
    while let Some(expr) = stack.pop() {
        if let ExprKind::Binary {
            op: expr_op,
            left,
            right,
        } = &expr.kind
            && *expr_op == op
        {
            stack.push(right);
            stack.push(left);
            continue;
        }
        operands.push(expr);
    }
    operands
}

fn reject_mismatched_logical_chain_payloads(
    operands: &[&Expr],
    payloads: &[CompilerExpressionPayload<'_>],
) -> CompileResult<()> {
    if payloads.len() != operands.len() {
        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "mismatched CST logical chain payload",
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_common::{SourceId, Span};

    #[test]
    fn logical_chain_payload_guard_requires_exact_operand_count() {
        let expr = Expr {
            kind: ExprKind::Error,
            span: Span::new(SourceId::new(1), 0, 0),
        };

        let missing = reject_mismatched_logical_chain_payloads(&[&expr], &[])
            .expect_err("missing logical-chain payload should be rejected");
        assert!(
            matches!(
                missing.kind,
                CompileErrorKind::UnsupportedSyntax("mismatched CST logical chain payload")
            ),
            "expected mismatched CST logical chain payload, got {missing:?}"
        );

        let extra_payload = CompilerExpressionPayload::from_syntax(None, None);
        let extra = reject_mismatched_logical_chain_payloads(&[], &[extra_payload])
            .expect_err("extra logical-chain payload should be rejected");
        assert!(
            matches!(
                extra.kind,
                CompileErrorKind::UnsupportedSyntax("mismatched CST logical chain payload")
            ),
            "expected mismatched CST logical chain payload, got {extra:?}"
        );
    }
}

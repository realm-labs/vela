use vela_syntax::ast::{Block, Expr, ExprKind, SyntaxExpressionKind};

use crate::compiler::body_payloads::{CompilerBodyPayload, CompilerStatementPayload};
use crate::compiler::control_flow::classification::value_expression_kind_matches;
use crate::compiler::value_flow::{BlockValue, block_value};
use crate::compiler::{CompileResult, Compiler};
use crate::{Constant, Register, UnlinkedInstructionKind};

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn compile_block_value_to(
        &mut self,
        block: &Block,
        dst: Register,
    ) -> CompileResult<bool> {
        match block_value(block) {
            BlockValue::Empty => {
                self.emit_constant_to(dst, Constant::Null);
                Ok(false)
            }
            BlockValue::TailExpr { prefix, expr } => {
                for stmt in prefix {
                    if self.compile_statement(stmt)? {
                        return Ok(true);
                    }
                }
                let value = self.compile_expr(expr)?;
                self.emit(UnlinkedInstructionKind::Move { dst, src: value });
                Ok(false)
            }
            BlockValue::Statements(statements) => {
                let returned = self.compile_statements(statements)?;
                if !returned {
                    self.emit_constant_to(dst, Constant::Null);
                }
                Ok(returned)
            }
        }
    }

    pub(in crate::compiler) fn compile_block_payload_value_to(
        &mut self,
        body: &CompilerBodyPayload<'_>,
        dst: Register,
    ) -> CompileResult<bool> {
        let statements = body.statement_payloads();
        match block_value(body.fallback()) {
            BlockValue::Empty => {
                self.emit_constant_to(dst, Constant::Null);
                Ok(false)
            }
            BlockValue::TailExpr { prefix, expr } => {
                for stmt in statements.iter().take(prefix.len()) {
                    if self.compile_statement_payload(stmt)? {
                        return Ok(true);
                    }
                }
                self.compile_block_tail_expr_to(expr, statements.get(prefix.len()), dst)
            }
            BlockValue::Statements(_) => {
                let returned = self.compile_statement_payloads(&statements)?;
                if !returned {
                    self.emit_constant_to(dst, Constant::Null);
                }
                Ok(returned)
            }
        }
    }

    fn compile_block_tail_expr_to(
        &mut self,
        expr: &Expr,
        payload: Option<&CompilerStatementPayload<'_>>,
        dst: Register,
    ) -> CompileResult<bool> {
        if let Some(payload) = payload
            && let Some(kind) = payload.value_expression_kind()
            && value_expression_kind_matches(kind, expr)
        {
            return self.compile_cst_block_tail_expr_to(expr, payload, kind, dst);
        }
        self.compile_legacy_block_tail_expr_to(expr, dst)
    }

    fn compile_cst_block_tail_expr_to(
        &mut self,
        expr: &Expr,
        payload: &CompilerStatementPayload<'_>,
        kind: SyntaxExpressionKind,
        dst: Register,
    ) -> CompileResult<bool> {
        match kind {
            SyntaxExpressionKind::Block => {
                let ExprKind::Block(block) = &expr.kind else {
                    unreachable!("validated CST block tail expression kind");
                };
                if let Some(body) = payload.expression_block_body_payload() {
                    self.compile_block_payload_value_to(&body, dst)
                } else {
                    self.compile_block_value_to(block, dst)
                }
            }
            SyntaxExpressionKind::If => {
                let ExprKind::If(if_expr) = &expr.kind else {
                    unreachable!("validated CST if tail expression kind");
                };
                let if_payload = payload.expression_if_payload();
                self.compile_if_value_with_payloads(if_expr, dst, if_payload.as_ref())
            }
            SyntaxExpressionKind::Match => {
                let ExprKind::Match(match_expr) = &expr.kind else {
                    unreachable!("validated CST match tail expression kind");
                };
                let scrutinee_payload = payload.expression_match_scrutinee_payload();
                let arm_payloads = payload.expression_match_arm_payloads();
                self.compile_match_value_with_payloads(
                    match_expr,
                    dst,
                    scrutinee_payload.as_ref(),
                    arm_payloads.as_deref(),
                )
            }
            _ => {
                let expression_payload = payload.expression_payload();
                let value = self.compile_expr_with_payload(expr, expression_payload.as_ref())?;
                self.emit(UnlinkedInstructionKind::Move { dst, src: value });
                Ok(false)
            }
        }
    }

    fn compile_legacy_block_tail_expr_to(
        &mut self,
        expr: &Expr,
        dst: Register,
    ) -> CompileResult<bool> {
        match &expr.kind {
            ExprKind::Block(block) => self.compile_block_value_to(block, dst),
            ExprKind::If(if_expr) => self.compile_if_value_to(if_expr, dst),
            ExprKind::Match(match_expr) => self.compile_match_value_to(match_expr, dst),
            _ => {
                let value = self.compile_expr(expr)?;
                self.emit(UnlinkedInstructionKind::Move { dst, src: value });
                Ok(false)
            }
        }
    }
}

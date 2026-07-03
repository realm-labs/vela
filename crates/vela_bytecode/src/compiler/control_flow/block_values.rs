#[cfg(test)]
use vela_syntax::ast::StmtKind;
#[cfg(test)]
use vela_syntax::ast::SyntaxExpressionKind;
use vela_syntax::ast::{Block, Expr, ExprKind};

#[cfg(test)]
use crate::compiler::body_payloads::CompilerStatementPayload;
use crate::compiler::body_payloads::{CompilerBlockValue, CompilerBodyPayload};
#[cfg(test)]
use crate::compiler::control_flow::classification::aligned_statement;
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
                self.compile_block_tail_expr_without_payload_to(expr, dst)
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
        if body.syntax_statements_are_empty() {
            self.emit_constant_to(dst, Constant::Null);
            return Ok(false);
        }
        self.reject_extra_body_statement_payloads(body)?;
        let statements = body.statement_payloads();
        match body.block_value(&statements) {
            CompilerBlockValue::Empty => {
                self.emit_constant_to(dst, Constant::Null);
                Ok(false)
            }
            CompilerBlockValue::TailExpression { prefix, tail } => {
                for stmt in prefix {
                    if self.compile_statement_payload(stmt)? {
                        return Ok(true);
                    }
                }
                #[cfg(test)]
                let syntax_only = tail.optional_fallback().is_none();
                #[cfg(not(test))]
                let syntax_only = true;
                if syntax_only
                    && let Some((source, expression)) =
                        tail.expression_statement_syntax_expression()
                    && let Some(done) =
                        self.compile_syntax_constant_expr_to(source, &expression, dst)?
                {
                    return Ok(done);
                }
                if syntax_only
                    && let Some((source, expression)) =
                        tail.expression_statement_syntax_expression()
                    && let Some(done) =
                        self.compile_syntax_path_expr_to(source, &expression, dst)?
                {
                    return Ok(done);
                }
                if syntax_only
                    && let Some((source, expression)) =
                        tail.expression_statement_syntax_expression()
                    && let Some(done) =
                        self.compile_syntax_range_expr_to(source, &expression, dst)?
                {
                    return Ok(done);
                }
                if syntax_only && let Some(body) = tail.expression_statement_block_body_payload() {
                    return self.compile_block_payload_value_to(&body, dst);
                }
                if syntax_only
                    && let Some((source, if_expr)) = tail.syntax_if()
                    && let Some(done) = self.compile_syntax_if_value_to(source, &if_expr, dst)?
                {
                    return Ok(done);
                }
                if syntax_only
                    && let Some((source, expression)) =
                        tail.expression_statement_syntax_expression()
                    && let Some(done) =
                        self.compile_syntax_value_expr_to(source, &expression, dst)?
                {
                    return Ok(done);
                }
                #[cfg(not(test))]
                {
                    Err(crate::compiler::CompileError::new(
                        crate::compiler::CompileErrorKind::UnsupportedSyntax(
                            "unsupported CST block tail expression payload",
                        ),
                    ))
                }
                #[cfg(test)]
                let fallback = aligned_statement(tail).ok_or_else(|| {
                    crate::compiler::CompileError::new(
                        crate::compiler::CompileErrorKind::UnsupportedSyntax(
                            "mismatched CST block tail expression",
                        ),
                    )
                })?;
                #[cfg(test)]
                let StmtKind::Expr(expr) = &fallback.kind else {
                    return Err(crate::compiler::CompileError::new(
                        crate::compiler::CompileErrorKind::UnsupportedSyntax(
                            "mismatched CST block tail expression",
                        ),
                    ));
                };
                #[cfg(test)]
                self.compile_block_tail_expr_to(expr, Some(tail), dst)
            }
            CompilerBlockValue::Statements(statements) => {
                let returned = self.compile_statement_payloads(statements)?;
                if !returned {
                    self.emit_constant_to(dst, Constant::Null);
                }
                Ok(returned)
            }
        }
    }

    #[cfg(test)]
    fn compile_block_tail_expr_to(
        &mut self,
        expr: &Expr,
        payload: Option<&CompilerStatementPayload<'_>>,
        dst: Register,
    ) -> CompileResult<bool> {
        if let Some(payload) = payload {
            let Some(kind) = payload.stored_value_expression_kind() else {
                return Err(crate::compiler::CompileError::new(
                    crate::compiler::CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST block tail expression",
                    ),
                ));
            };
            let Some(expression_payload) = payload.expression_payload() else {
                return Err(crate::compiler::CompileError::new(
                    crate::compiler::CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST block tail expression",
                    ),
                ));
            };
            if expression_payload.matches_paired_expr(expr) {
                return self.compile_cst_block_tail_expr_to(expr, payload, kind, dst);
            }
            return Err(crate::compiler::CompileError::new(
                crate::compiler::CompileErrorKind::UnsupportedSyntax(
                    "mismatched CST block tail expression",
                ),
            ));
        }
        self.compile_block_tail_expr_without_payload_to(expr, dst)
    }

    #[cfg(test)]
    fn compile_cst_block_tail_expr_to(
        &mut self,
        expr: &Expr,
        payload: &CompilerStatementPayload<'_>,
        kind: SyntaxExpressionKind,
        dst: Register,
    ) -> CompileResult<bool> {
        match kind {
            SyntaxExpressionKind::Block => {
                if let Some(body) = payload.expression_block_body_payload() {
                    self.compile_block_payload_value_to(&body, dst)
                } else {
                    Err(missing_cst_block_tail_payload(
                        "missing CST block tail body payload",
                    ))
                }
            }
            SyntaxExpressionKind::If => {
                let Some((if_expr, if_payload)) = payload.expression_if_payload_with_fallback()
                else {
                    return Err(missing_cst_block_tail_payload(
                        "missing CST block tail if payload",
                    ));
                };
                self.compile_if_value_with_payloads(if_expr, dst, Some(&if_payload))
            }
            SyntaxExpressionKind::Match => {
                let Some((match_expr, scrutinee_payload, arm_payloads)) =
                    payload.expression_match_payloads_with_fallback()
                else {
                    return Err(missing_cst_block_tail_payload(
                        "missing CST block tail match payload",
                    ));
                };
                self.compile_match_value_with_payloads(
                    match_expr,
                    dst,
                    Some(&scrutinee_payload),
                    Some(&arm_payloads),
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

    fn compile_block_tail_expr_without_payload_to(
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

    #[cfg(test)]
    pub(in crate::compiler) fn compile_block_tail_expr_to_for_test(
        &mut self,
        expr: &Expr,
        payload: Option<&CompilerStatementPayload<'_>>,
        dst: Register,
    ) -> CompileResult<bool> {
        self.compile_block_tail_expr_to(expr, payload, dst)
    }
}

#[cfg(test)]
fn missing_cst_block_tail_payload(message: &'static str) -> crate::compiler::CompileError {
    crate::compiler::CompileError::new(crate::compiler::CompileErrorKind::UnsupportedSyntax(
        message,
    ))
}

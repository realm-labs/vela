use vela_common::SourceId;
use vela_syntax::ast::SyntaxExpression;
#[cfg(test)]
use vela_syntax::ast::SyntaxExpressionKind;
#[cfg(test)]
use vela_syntax::ast::{Expr, ExprKind};

#[cfg(test)]
use crate::UnlinkedInstructionKind;
#[cfg(test)]
use crate::compiler::body_payloads::CompilerStatementPayload;
use crate::compiler::body_payloads::{CompilerBlockValue, CompilerBodyPayload};
use crate::compiler::{CompileResult, Compiler};
use crate::{Constant, Register};

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn compile_syntax_block_expr_to(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        dst: Register,
    ) -> CompileResult<Option<bool>> {
        let Some(block) = expression.as_block() else {
            return Ok(None);
        };
        let body = CompilerBodyPayload::nested_syntax(source, block);
        self.compile_block_payload_value_to(&body, dst).map(Some)
    }

    pub(in crate::compiler::control_flow) fn compile_syntax_block_expr_statement(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<bool>> {
        if expression.as_block().is_none() {
            return Ok(None);
        }
        let dst = self.alloc_register()?;
        self.compile_syntax_block_expr_to(source, expression, dst)
    }

    pub(in crate::compiler) fn compile_block_payload_value_to(
        &mut self,
        body: &CompilerBodyPayload<'_>,
        dst: Register,
    ) -> CompileResult<bool> {
        let statements = body.statement_payloads();
        self.compile_block_payload_value_to_from_statements(body, dst, &statements)
    }

    fn compile_block_payload_value_to_from_statements(
        &mut self,
        body: &CompilerBodyPayload<'_>,
        dst: Register,
        statements: &[crate::compiler::body_payloads::CompilerStatementPayload<'_>],
    ) -> CompileResult<bool> {
        if body.syntax_statements_are_empty() {
            self.emit_constant_to(dst, Constant::Null);
            return Ok(false);
        }
        match body.block_value(statements) {
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
                if let Some((source, expression)) = tail.expression_statement_syntax_expression()
                    && let Some(done) =
                        self.compile_syntax_constant_expr_to(source, &expression, dst)?
                {
                    return Ok(done);
                }
                if let Some((source, expression)) = tail.expression_statement_syntax_expression()
                    && let Some(done) =
                        self.compile_syntax_path_expr_to(source, &expression, dst)?
                {
                    return Ok(done);
                }
                if let Some((source, expression)) = tail.expression_statement_syntax_expression()
                    && let Some(done) =
                        self.compile_syntax_range_expr_to(source, &expression, dst)?
                {
                    return Ok(done);
                }
                if let Some((source, expression)) = tail.expression_statement_syntax_expression()
                    && let Some(done) =
                        self.compile_syntax_block_expr_to(source, &expression, dst)?
                {
                    return Ok(done);
                }
                if let Some((source, if_expr)) = tail.syntax_if()
                    && let Some(done) = self.compile_syntax_if_value_to(source, &if_expr, dst)?
                {
                    return Ok(done);
                }
                if let Some((source, expression)) = tail.expression_statement_syntax_expression()
                    && let Some(done) =
                        self.compile_syntax_value_expr_to(source, &expression, dst)?
                {
                    return Ok(done);
                }
                Err(crate::compiler::CompileError::new(
                    crate::compiler::CompileErrorKind::UnsupportedSyntax(
                        "unsupported CST block tail expression payload",
                    ),
                ))
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
            if payload.expression_payload().is_none() {
                return Err(crate::compiler::CompileError::new(
                    crate::compiler::CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST block tail expression",
                    ),
                ));
            };
            return self.compile_cst_block_tail_expr_to(expr, payload, kind, dst);
        }
        Err(crate::compiler::CompileError::new(
            crate::compiler::CompileErrorKind::UnsupportedSyntax(
                "missing CST block tail expression payload",
            ),
        ))
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
                if let Some(body) = payload
                    .expression_payload()
                    .and_then(|payload| payload.block_body_payload())
                {
                    self.compile_block_payload_value_to(&body, dst)
                } else {
                    Err(missing_cst_block_tail_payload(
                        "missing CST block tail body payload",
                    ))
                }
            }
            SyntaxExpressionKind::If => {
                let ExprKind::If(if_expr) = &expr.kind else {
                    return Err(missing_cst_block_tail_payload(
                        "missing CST block tail if payload",
                    ));
                };
                let Some(if_payload) = payload
                    .expression_payload()
                    .and_then(|payload| payload.if_payload())
                else {
                    return Err(missing_cst_block_tail_payload(
                        "missing CST block tail if payload",
                    ));
                };
                self.compile_if_value_with_payloads(if_expr, dst, &if_payload)
            }
            SyntaxExpressionKind::Match => {
                if let Some(expression_payload) = payload.expression_payload()
                    && let Some(returned) =
                        self.compile_syntax_match_payload_value_to(&expression_payload, dst)?
                {
                    return Ok(returned);
                }
                let ExprKind::Match(match_expr) = &expr.kind else {
                    return Err(missing_cst_block_tail_payload(
                        "missing CST block tail match payload",
                    ));
                };
                let Some(expression_payload) = payload.expression_payload() else {
                    return Err(missing_cst_block_tail_payload(
                        "missing CST block tail match payload",
                    ));
                };
                let Some(scrutinee_payload) = expression_payload.match_scrutinee_payload() else {
                    return Err(missing_cst_block_tail_payload(
                        "missing CST block tail match payload",
                    ));
                };
                let Some(arm_payloads) = expression_payload.match_arm_payloads() else {
                    return Err(missing_cst_block_tail_payload(
                        "missing CST block tail match payload",
                    ));
                };
                self.compile_match_value_with_payloads(
                    match_expr,
                    dst,
                    Some(&scrutinee_payload),
                    &arm_payloads,
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

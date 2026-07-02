use vela_syntax::ast::SyntaxStatementKind;

use crate::compiler::body_payloads::{
    CompilerBodyPayload, CompilerExpressionPayload, CompilerStatementPayload,
};
use crate::compiler::control_flow::loops::reject_missing_for_pattern_payloads;
use crate::compiler::control_flow::value_syntax::ValueSyntaxPayloads;
use crate::compiler::{CompileError, CompileErrorKind, CompileResult, Compiler};

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn compile_statement_payloads(
        &mut self,
        statements: &[CompilerStatementPayload<'_>],
    ) -> CompileResult<bool> {
        for stmt in statements {
            if self.compile_statement_payload(stmt)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(in crate::compiler::control_flow) fn compile_body_payload_statements(
        &mut self,
        body: &CompilerBodyPayload<'_>,
    ) -> CompileResult<bool> {
        self.reject_extra_body_statement_payloads(body)?;
        let statements = body.statement_payloads();
        self.compile_statement_payloads(&statements)
    }

    pub(in crate::compiler::control_flow) fn reject_extra_body_statement_payloads(
        &self,
        body: &CompilerBodyPayload<'_>,
    ) -> CompileResult<()> {
        if body.has_unmatched_extra_statement_payloads() {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "mismatched CST body statements",
            )));
        }
        Ok(())
    }

    pub(in crate::compiler::control_flow) fn compile_statement_payload(
        &mut self,
        stmt: &CompilerStatementPayload<'_>,
    ) -> CompileResult<bool> {
        let Some(syntax_kind) = stmt.syntax_statement_kind() else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST statement payload",
            )));
        };
        match syntax_kind {
            SyntaxStatementKind::Break => return self.compile_break(),
            SyntaxStatementKind::Continue => return self.compile_continue(),
            SyntaxStatementKind::Return if stmt.return_value_missing_in_syntax() => {
                let Some(span) = stmt.syntax_statement_span() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST statement payload",
                    )));
                };
                return self.compile_empty_return(span);
            }
            _ => {}
        }

        let Some(kind) = stmt.statement_kind() else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST statement payload",
            )));
        };
        if kind == SyntaxStatementKind::Let {
            let initializer_body = stmt.let_initializer_block_body_payload();
            let initializer_if = stmt.let_initializer_if_payload();
            let initializer_match_arms = stmt.let_initializer_match_arm_payloads();
            let initializer_expression = stmt.let_initializer_expression_payload();
            self.compile_let_statement(
                stmt.fallback(),
                ValueSyntaxPayloads::new(
                    stmt.syntax_let_initializer_kind(),
                    initializer_expression.as_ref(),
                    initializer_body.as_ref(),
                    initializer_if.as_ref(),
                    initializer_match_arms.as_deref(),
                    stmt.let_initializer_missing_in_syntax(),
                ),
            )
        } else if kind == SyntaxStatementKind::Return {
            let value_body = stmt.return_value_block_body_payload();
            let value_if = stmt.return_value_if_payload();
            let value_match_arms = stmt.return_value_match_arm_payloads();
            let value_expression = stmt.return_value_expression_payload();
            self.compile_return_statement(
                stmt.fallback(),
                ValueSyntaxPayloads::new(
                    stmt.syntax_return_value_kind(),
                    value_expression.as_ref(),
                    value_body.as_ref(),
                    value_if.as_ref(),
                    value_match_arms.as_deref(),
                    stmt.return_value_missing_in_syntax(),
                ),
            )
        } else if kind == SyntaxStatementKind::For {
            let iterable_payload = stmt.for_iterable_expression_payload();
            if iterable_payload
                .as_ref()
                .and_then(CompilerExpressionPayload::syntax_kind)
                .is_none()
            {
                return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "missing CST for iterable payload",
                )));
            }
            let body_payload = stmt.for_body_payload();
            if body_payload.is_none() {
                return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "missing CST for statement body payload",
                )));
            }
            let index_pattern_payload = stmt.for_index_pattern_payload();
            let value_pattern_payload = stmt.for_value_pattern_payload();
            reject_missing_for_pattern_payloads(
                index_pattern_payload.as_ref(),
                value_pattern_payload.as_ref(),
            )?;
            self.compile_for_statement(
                stmt.fallback(),
                iterable_payload,
                body_payload,
                index_pattern_payload,
                value_pattern_payload,
            )
        } else if kind == SyntaxStatementKind::If {
            let if_payload = stmt.if_payload();
            if if_payload.is_none() {
                return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "missing CST if statement payload",
                )));
            }
            self.compile_if_statement(stmt.fallback(), if_payload.as_ref())
        } else if kind == SyntaxStatementKind::Match {
            self.compile_match_statement_payload(stmt)
        } else if kind == SyntaxStatementKind::Block {
            self.compile_block_statement_payload(stmt)
        } else if kind == SyntaxStatementKind::Expr {
            self.compile_expr_statement_payload(stmt)
        } else {
            self.compile_statement_as(kind, stmt.fallback())
        }
    }

    #[cfg(test)]
    pub(in crate::compiler) fn compile_statement_payload_for_test(
        &mut self,
        stmt: &CompilerStatementPayload<'_>,
    ) -> CompileResult<bool> {
        self.compile_statement_payload(stmt)
    }

    #[cfg(test)]
    pub(in crate::compiler) fn compile_body_payload_statements_for_test(
        &mut self,
        body: &CompilerBodyPayload<'_>,
    ) -> CompileResult<bool> {
        self.compile_body_payload_statements(body)
    }
}

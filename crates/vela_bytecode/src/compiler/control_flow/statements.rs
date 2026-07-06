#[cfg(test)]
use vela_syntax::ast::{ExprKind, StmtKind};
use vela_syntax::ast::{SyntaxExpressionKind, SyntaxStatementKind};

#[cfg(test)]
use crate::compiler::body_payloads::CompilerExpressionPayload;
use crate::compiler::body_payloads::{CompilerBodyPayload, CompilerStatementPayload};
#[cfg(test)]
use crate::compiler::control_flow::loops::reject_missing_for_pattern_payloads;
#[cfg(test)]
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
        if body.syntax_statements_are_empty() {
            return Ok(false);
        }
        let statements = body.statement_payloads();
        self.compile_statement_payloads(&statements)
    }

    pub(in crate::compiler::control_flow) fn compile_statement_payload(
        &mut self,
        stmt: &CompilerStatementPayload<'_>,
    ) -> CompileResult<bool> {
        let Some(syntax_kind) = stmt.stored_statement_kind() else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST statement payload",
            )));
        };
        match syntax_kind {
            SyntaxStatementKind::Break => return self.compile_break(),
            SyntaxStatementKind::Continue => return self.compile_continue(),
            SyntaxStatementKind::Let if stmt.let_initializer_missing_in_syntax() => {
                let Some(name) = stmt.let_name_text() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST let binding name",
                    )));
                };
                let Some(span) = stmt.syntax_statement_span() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST statement payload",
                    )));
                };
                return self.compile_let_without_initializer(name, span);
            }
            SyntaxStatementKind::Let => {
                if let Some((literal, literal_span)) =
                    stmt.let_initializer_syntax_literal_and_span()
                {
                    let Some(name) = stmt.let_name_text() else {
                        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "missing CST let binding name",
                        )));
                    };
                    let Some(span) = stmt.syntax_statement_span() else {
                        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "missing CST statement payload",
                        )));
                    };
                    return self.compile_let_literal(name, span, literal, literal_span);
                }
                if let Some((literal, literal_span)) =
                    stmt.let_initializer_syntax_negated_literal_and_span()
                {
                    let Some(name) = stmt.let_name_text() else {
                        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "missing CST let binding name",
                        )));
                    };
                    let Some(span) = stmt.syntax_statement_span() else {
                        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "missing CST statement payload",
                        )));
                    };
                    return self.compile_let_negated_literal(name, span, literal, literal_span);
                }
                if stmt.is_syntax_only()
                    && let Some((source, expression, _)) =
                        stmt.let_initializer_syntax_expression_and_span()
                    && let Some(name) = stmt.let_name_text()
                    && let Some(span) = stmt.syntax_statement_span()
                    && let Some(compiled) =
                        self.compile_let_syntax_range(name.clone(), span, source, &expression)?
                {
                    return Ok(compiled);
                }
                if stmt.is_syntax_only()
                    && let Some((source, expression, _)) =
                        stmt.let_initializer_syntax_expression_and_span()
                    && let Some(name) = stmt.let_name_text()
                    && let Some(span) = stmt.syntax_statement_span()
                    && let Some(compiled) =
                        self.compile_let_syntax_constant(source, name, span, &expression)?
                {
                    return Ok(compiled);
                }
                if let Some((path, path_span)) = stmt.let_initializer_syntax_path_and_span() {
                    let Some(name) = stmt.let_name_text() else {
                        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "missing CST let binding name",
                        )));
                    };
                    let Some(span) = stmt.syntax_statement_span() else {
                        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "missing CST statement payload",
                        )));
                    };
                    return self.compile_let_path(name, span, path, path_span);
                }
                if stmt.stored_let_initializer_kind() == Some(SyntaxExpressionKind::Block)
                    && stmt.syntax_statement_span().is_some()
                {
                    let Some(name) = stmt.let_name_text() else {
                        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "missing CST let binding name",
                        )));
                    };
                    let Some(span) = stmt.syntax_statement_span() else {
                        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "missing CST statement payload",
                        )));
                    };
                    let Some((source, expression, _)) =
                        stmt.let_initializer_syntax_expression_and_span()
                    else {
                        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "missing CST let initializer block body payload",
                        )));
                    };
                    return self.compile_let_syntax_block_value(name, span, source, &expression);
                }
                if stmt.is_syntax_only()
                    && let Some((source, expression, _)) =
                        stmt.let_initializer_syntax_expression_and_span()
                    && let Some(name) = stmt.let_name_text()
                    && let Some(span) = stmt.syntax_statement_span()
                    && let Some(compiled) =
                        self.compile_let_syntax_expression(source, name, span, &expression)?
                {
                    return Ok(compiled);
                }
            }
            SyntaxStatementKind::Return if stmt.return_value_missing_in_syntax() => {
                let Some(span) = stmt.syntax_statement_span() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST statement payload",
                    )));
                };
                return self.compile_empty_return(span);
            }
            SyntaxStatementKind::Return => {
                if let Some((literal, span)) = stmt.return_value_syntax_literal_and_span() {
                    return self.compile_return_literal(literal, span);
                }
                if let Some((literal, span)) = stmt.return_value_syntax_negated_literal_and_span() {
                    return self.compile_return_negated_literal(literal, span);
                }
                if let Some((source, expression, span)) =
                    stmt.return_value_syntax_expression_and_span()
                    && let Some(compiled) =
                        self.compile_return_syntax_range(source, &expression, span)?
                {
                    return Ok(compiled);
                }
                if let Some((source, expression, span)) =
                    stmt.return_value_syntax_expression_and_span()
                    && let Some(compiled) =
                        self.compile_return_syntax_constant(source, &expression, span)?
                {
                    return Ok(compiled);
                }
                if let Some((path, span)) = stmt.return_value_syntax_path_and_span() {
                    return self.compile_return_path(path, span);
                }
                if stmt.stored_return_value_kind() == Some(SyntaxExpressionKind::Block)
                    && stmt.syntax_statement_span().is_some()
                {
                    let Some(span) = stmt.syntax_statement_span() else {
                        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "missing CST statement payload",
                        )));
                    };
                    let Some((source, expression, _)) =
                        stmt.return_value_syntax_expression_and_span()
                    else {
                        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "missing CST return block body payload",
                        )));
                    };
                    return self.compile_return_syntax_block_value(span, source, &expression);
                }
                if stmt.is_syntax_only()
                    && let Some((source, expression, _)) =
                        stmt.return_value_syntax_expression_and_span()
                    && let Some(compiled) =
                        self.compile_return_syntax_expression(source, &expression)?
                {
                    return Ok(compiled);
                }
            }
            SyntaxStatementKind::Block => return self.compile_block_statement_payload(stmt),
            SyntaxStatementKind::If => {
                if let Some((source, if_expr)) = stmt.syntax_if()
                    && let Some(compiled) = self.compile_syntax_if_statement(source, &if_expr)?
                {
                    return Ok(compiled);
                }
            }
            SyntaxStatementKind::For if stmt.is_syntax_only() => {
                if let Some((source, for_stmt)) = stmt.syntax_for()
                    && let Some(compiled) = self.compile_syntax_for_statement(source, &for_stmt)?
                {
                    return Ok(compiled);
                }
            }
            SyntaxStatementKind::Match if stmt.is_syntax_only() => {
                if let Some((source, match_expr)) = stmt.syntax_match()
                    && let Some(compiled) =
                        self.compile_syntax_match_statement(source, &match_expr)?
                {
                    return Ok(compiled);
                }
            }
            _ => {}
        }

        let Some(kind) = stmt.stored_statement_kind() else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST statement payload",
            )));
        };
        if kind == SyntaxStatementKind::Expr {
            return self.compile_expr_statement_payload(stmt);
        }
        #[cfg(test)]
        {
            return self.compile_paired_statement_payload_for_test(kind, stmt);
        }
        #[cfg_attr(test, allow(unreachable_code))]
        let _ = kind;
        let mut error = CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "unsupported CST statement payload",
        ));
        if let Some(span) = stmt.syntax_statement_span() {
            error = error.with_span(span);
        }
        Err(error)
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

    #[cfg(test)]
    fn compile_paired_statement_payload_for_test(
        &mut self,
        kind: SyntaxStatementKind,
        stmt: &CompilerStatementPayload<'_>,
    ) -> CompileResult<bool> {
        if stmt.is_syntax_only() {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "unsupported CST statement payload",
            )));
        }
        match kind {
            SyntaxStatementKind::Let => {
                let initializer_body = stmt.let_initializer_block_body_payload();
                let initializer_expression = stmt.let_initializer_expression_payload();
                let initializer_if = initializer_expression
                    .as_ref()
                    .and_then(CompilerExpressionPayload::if_payload);
                let initializer_match_arms = initializer_expression
                    .as_ref()
                    .and_then(CompilerExpressionPayload::match_arm_payloads);
                if stmt.stored_let_initializer_kind() == Some(SyntaxExpressionKind::Block)
                    && initializer_body.is_none()
                {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST let initializer block body payload",
                    )));
                }
                if stmt.stored_let_initializer_kind() == Some(SyntaxExpressionKind::If)
                    && initializer_if.is_none()
                {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST let initializer if payload",
                    )));
                }
                if stmt.stored_let_initializer_kind().is_some() && initializer_expression.is_none()
                {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST let initializer payload",
                    )));
                }
                let Some(name) = stmt.let_name_text() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST let binding name",
                    )));
                };
                let span = stmt
                    .syntax_statement_span()
                    .unwrap_or_else(|| stmt.fallback().span);
                self.compile_let_binding(
                    name,
                    span,
                    initializer_expression
                        .as_ref()
                        .map(CompilerExpressionPayload::fallback),
                    ValueSyntaxPayloads::new(
                        stmt.stored_let_initializer_kind(),
                        initializer_expression.as_ref(),
                        initializer_body.as_ref(),
                        initializer_if.as_ref(),
                        initializer_match_arms.as_deref(),
                        stmt.let_initializer_missing_in_syntax(),
                    ),
                )
            }
            SyntaxStatementKind::Return => {
                let value_body = stmt.return_value_block_body_payload();
                let value_expression = stmt.return_value_expression_payload();
                let value_if = value_expression
                    .as_ref()
                    .and_then(CompilerExpressionPayload::if_payload);
                let value_match_arms = value_expression
                    .as_ref()
                    .and_then(CompilerExpressionPayload::match_arm_payloads);
                if stmt.stored_return_value_kind().is_some() && value_expression.is_none() {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST return value payload",
                    )));
                }
                let span = stmt
                    .syntax_statement_span()
                    .unwrap_or_else(|| stmt.fallback().span);
                let (register, returned) = self.compile_return_value(
                    span,
                    value_expression
                        .as_ref()
                        .map(CompilerExpressionPayload::fallback),
                    ValueSyntaxPayloads::new(
                        stmt.stored_return_value_kind(),
                        value_expression.as_ref(),
                        value_body.as_ref(),
                        value_if.as_ref(),
                        value_match_arms.as_deref(),
                        stmt.return_value_missing_in_syntax(),
                    ),
                )?;
                if !returned {
                    self.emit(crate::UnlinkedInstructionKind::Return { src: register });
                }
                Ok(true)
            }
            SyntaxStatementKind::For => {
                let StmtKind::For { .. } = &stmt.fallback().kind else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST for statement payload",
                    )));
                };
                let body_payload = for_body_payload(stmt);
                if body_payload.is_none() {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST for statement body payload",
                    )));
                }
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
            }
            SyntaxStatementKind::If => {
                let StmtKind::Expr(expr) = &stmt.fallback().kind else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST if statement payload",
                    )));
                };
                let ExprKind::If(_) = &expr.kind else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST if statement payload",
                    )));
                };
                let if_payload = stmt
                    .expression_payload()
                    .and_then(|payload| payload.if_payload());
                if if_payload.is_none() {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST if statement payload",
                    )));
                }
                self.compile_if_statement(stmt.fallback(), if_payload.as_ref())
            }
            SyntaxStatementKind::Match => self.compile_paired_match_statement_payload(stmt),
            SyntaxStatementKind::Block
            | SyntaxStatementKind::Break
            | SyntaxStatementKind::Continue
            | SyntaxStatementKind::Expr => Err(CompileError::new(
                CompileErrorKind::UnsupportedSyntax("unsupported CST statement payload"),
            )),
        }
    }

    #[cfg(test)]
    fn compile_paired_match_statement_payload(
        &mut self,
        stmt: &CompilerStatementPayload<'_>,
    ) -> CompileResult<bool> {
        let Some(expression_payload) = stmt.expression_payload() else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "mismatched CST match statement payload",
            )));
        };
        let expr = expression_payload.fallback();
        let ExprKind::Match(match_expr) = &expr.kind else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "mismatched CST match statement payload",
            )));
        };
        let Some(scrutinee_payload) = expression_payload.match_scrutinee_payload() else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "mismatched CST match statement payload",
            )));
        };
        let arm_payloads = expression_payload.match_arm_payloads();
        self.compile_match_with_payloads(
            match_expr,
            Some(&scrutinee_payload),
            arm_payloads.as_deref(),
        )
    }
}

#[cfg(test)]
fn for_body_payload<'ast>(
    stmt: &CompilerStatementPayload<'ast>,
) -> Option<CompilerBodyPayload<'ast>> {
    Some(CompilerBodyPayload::nested_syntax(
        stmt.syntax_statement_span()?.source,
        stmt.syntax_statement()?.as_for()?.body()?,
    ))
}

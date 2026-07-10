use vela_hir::body::HirStmtTag;
use vela_syntax::ast::SyntaxExpressionKind;

use crate::compiler::body_payloads::{CompilerBodyPayload, CompilerStatementPayload};
use crate::compiler::{CompileError, CompileErrorKind, CompileResult, Compiler};

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn compile_statement_payloads(
        &mut self,
        statements: &[CompilerStatementPayload],
    ) -> CompileResult<bool> {
        for stmt in statements {
            if self.compile_statement_payload(stmt)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) fn compile_body_payload_statements(
        &mut self,
        body: &CompilerBodyPayload<'_>,
    ) -> CompileResult<bool> {
        if body.syntax_statements_are_empty() {
            return Ok(false);
        }
        let statements = body.statement_payloads()?;
        self.compile_statement_payloads(&statements)
    }

    pub(super) fn compile_statement_payload(
        &mut self,
        stmt: &CompilerStatementPayload,
    ) -> CompileResult<bool> {
        let hir_kind = stmt.hir_statement_kind();
        match hir_kind {
            HirStmtTag::Break => return self.compile_break(),
            HirStmtTag::Continue => return self.compile_continue(),
            HirStmtTag::Let if stmt.let_initializer_missing_in_syntax() => {
                let Some(name) = self.let_binding_name_for_patterns(stmt.hir_patterns()) else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing let binding name",
                    )));
                };
                let span = stmt.statement_span();
                return self.compile_let_without_initializer(name, span, stmt.hir_patterns());
            }
            HirStmtTag::Let => {
                let span = stmt.statement_span();
                if let Some((source, pattern, expression, span)) =
                    stmt.let_pattern_initializer_syntax_expression_and_span()
                {
                    return self.compile_let_syntax_pattern(
                        source,
                        &pattern,
                        span,
                        &expression,
                        stmt.hir_patterns(),
                    );
                }
                if let Some((literal, literal_span)) =
                    stmt.let_initializer_syntax_literal_and_span()
                {
                    let Some(name) = self.let_binding_name_for_patterns(stmt.hir_patterns()) else {
                        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "missing let binding name",
                        )));
                    };
                    return self.compile_let_literal(
                        name,
                        span,
                        literal,
                        literal_span,
                        stmt.hir_patterns(),
                    );
                }
                if let Some((literal, literal_span)) =
                    stmt.let_initializer_syntax_negated_literal_and_span()
                {
                    let Some(name) = self.let_binding_name_for_patterns(stmt.hir_patterns()) else {
                        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "missing let binding name",
                        )));
                    };
                    return self.compile_let_negated_literal(
                        name,
                        span,
                        literal,
                        literal_span,
                        stmt.hir_patterns(),
                    );
                }
                if let Some((source, expression, _)) =
                    stmt.let_initializer_syntax_expression_and_span()
                    && let Some(name) = self.let_binding_name_for_patterns(stmt.hir_patterns())
                    && let Some(compiled) = self.compile_let_syntax_range(
                        name.clone(),
                        span,
                        source,
                        &expression,
                        stmt.hir_patterns(),
                    )?
                {
                    return Ok(compiled);
                }
                if let Some((source, expression, _)) =
                    stmt.let_initializer_syntax_expression_and_span()
                    && let Some(name) = self.let_binding_name_for_patterns(stmt.hir_patterns())
                    && let Some(compiled) = self.compile_let_syntax_constant(
                        source,
                        name,
                        span,
                        &expression,
                        stmt.hir_patterns(),
                    )?
                {
                    return Ok(compiled);
                }
                if let Some((_, _, path_span)) = stmt.let_initializer_syntax_expression_and_span()
                    && let Some(path) = self.hir_value_path_for_span(path_span)
                {
                    let Some(name) = self.let_binding_name_for_patterns(stmt.hir_patterns()) else {
                        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "missing let binding name",
                        )));
                    };
                    return self.compile_let_path(name, span, path, path_span, stmt.hir_patterns());
                }
                if stmt.stored_let_initializer_kind() == Some(SyntaxExpressionKind::Block) {
                    let Some(name) = self.let_binding_name_for_patterns(stmt.hir_patterns()) else {
                        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "missing let binding name",
                        )));
                    };
                    let Some((source, expression, _)) =
                        stmt.let_initializer_syntax_expression_and_span()
                    else {
                        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "missing let initializer block body",
                        )));
                    };
                    return self.compile_let_syntax_block_value(
                        name,
                        span,
                        source,
                        &expression,
                        stmt.hir_patterns(),
                    );
                }
                if let Some((source, expression, _)) =
                    stmt.let_initializer_syntax_expression_and_span()
                    && let Some(name) = self.let_binding_name_for_patterns(stmt.hir_patterns())
                    && let Some(compiled) = self.compile_let_syntax_expression(
                        source,
                        name,
                        span,
                        &expression,
                        stmt.hir_patterns(),
                    )?
                {
                    return Ok(compiled);
                }
            }
            HirStmtTag::Return if stmt.return_value_missing_in_syntax() => {
                let span = stmt.statement_span();
                return self.compile_empty_return(span);
            }
            HirStmtTag::Return => {
                let span = stmt.statement_span();
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
                if let Some((_, _, span)) = stmt.return_value_syntax_expression_and_span()
                    && let Some(path) = self.hir_value_path_for_span(span)
                {
                    return self.compile_return_path(path, span);
                }
                if stmt.stored_return_value_kind() == Some(SyntaxExpressionKind::Block) {
                    let Some((source, expression, _)) =
                        stmt.return_value_syntax_expression_and_span()
                    else {
                        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "missing return block body",
                        )));
                    };
                    return self.compile_return_syntax_block_value(span, source, &expression);
                }
                if let Some((source, expression, _)) =
                    stmt.return_value_syntax_expression_and_span()
                    && let Some(compiled) =
                        self.compile_return_syntax_expression(source, &expression)?
                {
                    return Ok(compiled);
                }
            }
            HirStmtTag::Block => return self.compile_block_statement_payload(stmt),
            HirStmtTag::If => {
                if let Some((source, if_expr)) = stmt.syntax_if()
                    && let Some(compiled) = self.compile_syntax_if_statement(source, &if_expr)?
                {
                    return Ok(compiled);
                }
            }
            HirStmtTag::For => {
                if let Some((source, for_stmt)) = stmt.syntax_for()
                    && let Some(compiled) =
                        self.compile_syntax_for_statement(source, &for_stmt, stmt.hir_patterns())?
                {
                    return Ok(compiled);
                }
            }
            HirStmtTag::Match => {
                if let Some((source, match_expr)) = stmt.syntax_match()
                    && let Some(compiled) =
                        self.compile_syntax_match_statement(source, &match_expr)?
                {
                    return Ok(compiled);
                }
            }
            _ => {}
        }

        if hir_kind == HirStmtTag::Expr {
            return self.compile_expr_statement_payload(stmt);
        }
        let _ = hir_kind;
        let mut error =
            CompileError::new(CompileErrorKind::UnsupportedSyntax("unsupported statement"));
        error = error.with_span(stmt.statement_span());
        Err(error)
    }
}

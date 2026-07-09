use vela_common::SourceId;
use vela_syntax::ast::SyntaxExpression;

use crate::compiler::body_payloads::{CompilerBlockValue, CompilerBodyPayload};
use crate::compiler::{CompileResult, Compiler};
use crate::{Constant, Register};

impl Compiler<'_, '_> {
    fn compile_syntax_block_expr_to(
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

    pub(super) fn compile_syntax_block_expr_statement(
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
        let statements = body.statement_payloads()?;
        self.compile_block_payload_value_to_from_statements(body, dst, &statements)
    }

    fn compile_block_payload_value_to_from_statements(
        &mut self,
        body: &CompilerBodyPayload<'_>,
        dst: Register,
        statements: &[crate::compiler::body_payloads::CompilerStatementPayload<'_>],
    ) -> CompileResult<bool> {
        if body.syntax_statements_are_empty() {
            self.emit_constant_to(dst, Constant::Unit);
            return Ok(false);
        }
        match body.block_value(statements) {
            CompilerBlockValue::Empty => {
                self.emit_constant_to(dst, Constant::Unit);
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
                        "unsupported block tail expression",
                    ),
                ))
            }
            CompilerBlockValue::Statements(statements) => {
                let returned = self.compile_statement_payloads(statements)?;
                if !returned {
                    self.emit_constant_to(dst, Constant::Unit);
                }
                Ok(returned)
            }
        }
    }
}

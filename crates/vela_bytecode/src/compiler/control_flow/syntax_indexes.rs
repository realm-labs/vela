use vela_common::SourceId;
use vela_syntax::ast::{AstNode, Literal, SyntaxExpression};

use crate::compiler::body_payloads::expression_syntax_literal;
use crate::compiler::host_paths::HostIndexAccessKind;
use crate::compiler::{CompileResult, Compiler};
use crate::{Register, UnlinkedInstructionKind};

use super::spans::syntax_expression_span;

impl Compiler<'_, '_> {
    pub(super) fn compile_syntax_index(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        if expression.as_index().is_none() {
            return Ok(None);
        }
        if let Some(register) = self.compile_syntax_host_index(source, expression)? {
            return Ok(Some(register));
        }
        let span = syntax_expression_span(source, expression);
        let Some(index) = self.hir_index_for_span(span) else {
            return Ok(None);
        };
        let Some(receiver_span) = self.expression_span(index.receiver) else {
            return Ok(None);
        };
        let Some(index_span) = self.expression_span(index.index) else {
            return Ok(None);
        };
        let Some(receiver_expression) =
            syntax_index_expression_at_span(source, expression, receiver_span)
        else {
            return Ok(None);
        };
        let Some(index_expression) =
            syntax_index_expression_at_span(source, expression, index_span)
        else {
            return Ok(None);
        };
        self.reject_invalid_syntax_host_index_read(source, expression)?;
        let Some(base) = self.compile_syntax_expression(source, &receiver_expression)? else {
            return Ok(None);
        };
        let dst = self.alloc_register()?;
        if let Some(Literal::String(key)) = expression_syntax_literal(&index_expression) {
            let key = self.code.push_constant(crate::Constant::String(key));
            self.emit(UnlinkedInstructionKind::GetStringKeyIndex { dst, base, key });
        } else {
            let Some(index) = self.compile_syntax_expression(source, &index_expression)? else {
                return Ok(None);
            };
            self.emit(UnlinkedInstructionKind::GetIndex { dst, base, index });
        }
        Ok(Some(dst))
    }

    pub(super) fn reject_invalid_syntax_host_index_read(
        &self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<()> {
        self.reject_invalid_syntax_host_index_access(
            source,
            expression,
            expression,
            HostIndexAccessKind::Read,
        )
    }
}

fn syntax_index_expression_at_span(
    source: SourceId,
    expression: &SyntaxExpression,
    span: vela_common::Span,
) -> Option<SyntaxExpression> {
    if span.source != source {
        return None;
    }
    expression
        .syntax()
        .descendants()
        .filter_map(SyntaxExpression::cast)
        .find(|child| syntax_expression_span(source, child) == span)
}

use vela_common::SourceId;
use vela_syntax::ast::{Literal, SyntaxExpression};

use crate::compiler::body_payloads::expression_syntax_literal;
use crate::compiler::host_paths::HostIndexAccessKind;
use crate::compiler::{CompileResult, Compiler};
use crate::{Register, UnlinkedInstructionKind};

impl Compiler<'_, '_> {
    pub(super) fn compile_syntax_index(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        let Some(index) = expression.as_index() else {
            return Ok(None);
        };
        if let Some(register) = self.compile_syntax_host_index(source, expression)? {
            return Ok(Some(register));
        }
        let Some(receiver_expression) = index.receiver() else {
            return Ok(None);
        };
        let Some(index_expression) = index.index() else {
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

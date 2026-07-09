use vela_common::SourceId;
use vela_syntax::ast::SyntaxExpression;

use crate::compiler::param_defaults::syntax_map_key_name;
use crate::compiler::{CompileResult, Compiler};
use crate::{Register, UnlinkedInstructionKind};

impl Compiler<'_, '_> {
    pub(super) fn compile_syntax_container(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        if let Some(array) = expression.as_array() {
            let elements = array
                .expressions()
                .map(|element| self.compile_syntax_expression(source, &element))
                .collect::<CompileResult<Option<Vec<_>>>>()?;
            let Some(elements) = elements else {
                return Ok(None);
            };
            let dst = self.alloc_register()?;
            self.emit(UnlinkedInstructionKind::MakeArray { dst, elements });
            return Ok(Some(dst));
        }
        if let Some(map) = expression.as_map() {
            let entries = map
                .entries()
                .map(|entry| {
                    let Some(key) = entry.key() else {
                        return Ok(None);
                    };
                    let Some(value) = entry.value() else {
                        return Ok(None);
                    };
                    let key = syntax_map_key_name(source, &key)?;
                    let Some(value) = self.compile_syntax_expression(source, &value)? else {
                        return Ok(None);
                    };
                    Ok(Some((key, value)))
                })
                .collect::<CompileResult<Option<Vec<_>>>>()?;
            let Some(entries) = entries else {
                return Ok(None);
            };
            let dst = self.alloc_register()?;
            self.emit(UnlinkedInstructionKind::MakeMap { dst, entries });
            return Ok(Some(dst));
        }
        if let Some(record) = expression.as_record() {
            return self.compile_syntax_record_literal(source, expression, &record);
        }
        Ok(None)
    }
}

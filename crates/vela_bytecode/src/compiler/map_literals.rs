use vela_syntax::ast::MapEntry;

use crate::Register;

use super::body_payloads::CompilerMapEntryPayload;
use super::{CompileError, CompileErrorKind, CompileResult, Compiler};

impl Compiler<'_, '_> {
    pub(super) fn compile_map_entry(
        &mut self,
        entry: &MapEntry,
        payload: &CompilerMapEntryPayload<'_>,
    ) -> CompileResult<(String, Register)> {
        if !payload.has_key_syntax() {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST map entry key",
            )));
        }
        let key = payload
            .syntax_key_name()
            .ok_or_else(|| CompileError::new(CompileErrorKind::UnsupportedSyntax("map key")))?;
        if !payload.has_value_syntax() {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST map entry value",
            )));
        }
        let value_payload = payload.value_expression_payload(&entry.value);
        let value = self.compile_expr_with_payload(&entry.value, Some(&value_payload))?;
        Ok((key, value))
    }
}

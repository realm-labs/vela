use vela_syntax::ast::{Expr, ExprKind, Literal, MapEntry};

use crate::Register;

use super::{CompileError, CompileErrorKind, CompileResult, Compiler};

impl Compiler<'_> {
    pub(super) fn compile_map_entry(
        &mut self,
        entry: &MapEntry,
    ) -> CompileResult<(String, Register)> {
        let key = map_key_name(&entry.key)?;
        let value = self.compile_expr(&entry.value)?;
        Ok((key, value))
    }
}

fn map_key_name(key: &Expr) -> CompileResult<String> {
    match &key.kind {
        ExprKind::Literal(Literal::String(value))
        | ExprKind::Literal(Literal::Int(value))
        | ExprKind::Literal(Literal::Float(value)) => Ok(value.clone()),
        ExprKind::Path(path) => Ok(path.join(".")),
        _ => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "map key",
        ))),
    }
}

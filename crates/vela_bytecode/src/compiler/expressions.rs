use vela_common::Span;
use vela_hir::body::HirLiteral;

use crate::{Register, UnlinkedInstructionKind};

use super::const_eval::compile_literal_constant;
use super::{CompileResult, Compiler};

impl Compiler<'_, '_> {
    pub(super) fn compile_literal(
        &mut self,
        span: Option<Span>,
        literal: &HirLiteral,
    ) -> CompileResult<Register> {
        let constant = compile_literal_constant(literal).map_err(|error| match span {
            Some(span) => error.with_span(span),
            None => error,
        })?;
        self.emit_constant(constant)
    }

    pub(super) fn emit_truthy_to_bool(
        &mut self,
        dst: Register,
        src: Register,
    ) -> CompileResult<()> {
        self.emit(UnlinkedInstructionKind::Truthy { dst, src });
        Ok(())
    }
}

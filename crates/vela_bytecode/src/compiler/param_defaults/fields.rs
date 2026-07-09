use vela_common::SourceId;
use vela_syntax::ast::{SyntaxExpression, SyntaxFieldExpr};

use crate::{Register, UnlinkedInstructionKind};

use crate::compiler::host_paths::HostPath;
use crate::compiler::{CompileResult, Compiler};

use super::{param_default_expression_supported, param_default_unsupported, span_for};

impl Compiler<'_, '_> {
    pub(super) fn compile_param_default_field(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        field: &SyntaxFieldExpr,
    ) -> CompileResult<Register> {
        if !param_default_field_supported(expression) {
            return Err(param_default_unsupported(source, expression));
        }
        let Some(receiver) = field.receiver() else {
            return Err(param_default_unsupported(source, expression));
        };
        let Some(name) = field.name_text() else {
            return Err(param_default_unsupported(source, expression));
        };

        if let Some(path) = self.param_default_host_field_path(source, expression) {
            let root = self.compile_host_path_root(&path.root)?;
            let dst = self.alloc_register()?;
            self.emit_host_read(dst, root, path, span_for(source, expression))?;
            return Ok(dst);
        }

        let slot = self
            .record_shape_for_syntax_expression(Some(source), &receiver)
            .and_then(|shape| shape.field_slot(&name));
        let record = self.compile_param_default_expression(source, &receiver)?;
        let dst = self.alloc_register()?;
        if let Some(slot) = slot {
            self.emit(UnlinkedInstructionKind::GetRecordSlot {
                dst,
                record,
                field: name,
                slot,
            });
        } else {
            self.emit_spanned(
                UnlinkedInstructionKind::GetRecordField {
                    dst,
                    record,
                    field: name,
                },
                span_for(source, expression),
            );
        }
        Ok(dst)
    }

    fn param_default_host_field_path(
        &self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> Option<HostPath<'static>> {
        let span = span_for(source, expression);
        let path = self.hir_value_path_for_span(span)?;
        if path.len() < 2 {
            return None;
        }
        let root_span = self.hir_value_path_root_span_for_span(span).unwrap_or(span);
        self.owned_host_field_path_parts(root_span, &path)
            .map(|resolved| resolved.path)
    }
}

pub(super) fn param_default_field_supported(expression: &SyntaxExpression) -> bool {
    expression.as_field().is_some_and(|field| {
        field.name_token().is_some()
            && field
                .receiver()
                .is_some_and(|receiver| param_default_expression_supported(&receiver))
    })
}

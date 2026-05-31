use vela_common::Span;
use vela_hir::BindingResolution;

use crate::{Constant, InstructionKind, Register};

use super::host_paths::host_field_path_parts;
use super::{CompileError, CompileErrorKind, CompileResult, Compiler};

impl Compiler<'_> {
    pub(super) fn compile_path_expr(
        &mut self,
        span: Span,
        path: &[String],
    ) -> CompileResult<Register> {
        if let Some(value) = self.const_value_at_span(span) {
            return self.emit_constant(value);
        }
        if path.len() == 1 {
            return self.compile_local_path(span, path);
        }
        self.compile_path_access(span, path)
    }

    pub(super) fn local_register_at_span(
        &mut self,
        span: Span,
        name: &str,
    ) -> CompileResult<Register> {
        if let Some(BindingResolution::Local(local)) = self.bindings.resolution_at_span(span)
            && let Some(register) = self.hir_locals.get(local).copied()
        {
            return Ok(register);
        }
        if let Some(value) = self.const_value_at_span(span) {
            return self.emit_constant(value);
        }
        self.locals
            .get(name)
            .copied()
            .ok_or_else(|| CompileError::new(CompileErrorKind::UnknownLocal(name.to_owned())))
    }

    pub(super) fn const_value_at_span(&self, span: Span) -> Option<Constant> {
        let BindingResolution::Declaration(declaration) = self.bindings.resolution_at_span(span)?
        else {
            return None;
        };
        self.facts.const_values.get(declaration).cloned()
    }

    pub(super) fn script_record_field_slot_for_path_root(
        &self,
        span: Span,
        root: &str,
        field: &str,
    ) -> Option<usize> {
        let type_name = match self.bindings.resolution_at_span(span) {
            Some(BindingResolution::Local(local)) => self.script_types.local(*local),
            _ => self.script_types.name(root),
        }?;
        self.script_record_field_slot_for_type(&type_name, field)
    }

    fn compile_local_path(&mut self, span: Span, path: &[String]) -> CompileResult<Register> {
        let [name] = path else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "path expression",
            )));
        };
        self.local_register_at_span(span, name)
    }

    fn compile_path_access(&mut self, span: Span, path: &[String]) -> CompileResult<Register> {
        if path.len() < 2 {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "path expression",
            )));
        }
        if let Some(host_path) = host_field_path_parts(&self.facts.options, path)
            && host_path.requires_path_instruction()
        {
            let root = self.compile_host_path_root(span, host_path.root)?;
            let segments = self.compile_host_path_segments(host_path.segments)?;
            let dst = self.alloc_register()?;
            self.emit(InstructionKind::GetHostPath {
                dst,
                root,
                segments,
            });
            return Ok(dst);
        }
        let mut current = self.local_register_at_span(span, &path[0])?;
        for (index, segment) in path.iter().enumerate().skip(1) {
            let dst = self.alloc_register()?;
            if index == 1
                && let Some(slot) =
                    self.script_record_field_slot_for_path_root(span, &path[0], segment)
            {
                self.emit(InstructionKind::GetRecordSlot {
                    dst,
                    record: current,
                    field: segment.clone(),
                    slot,
                });
            } else if index == 1
                && let Some(slot) =
                    self.script_enum_field_slot_for_path_root(span, &path[0], segment)
            {
                self.emit(InstructionKind::GetEnumSlot {
                    dst,
                    value: current,
                    field: segment.clone(),
                    slot,
                });
            } else if index == 1
                && let Some(field) = self.facts.options.host_fields.get(segment).copied()
            {
                self.emit(InstructionKind::GetHostField {
                    dst,
                    root: current,
                    field,
                });
            } else {
                self.emit(InstructionKind::GetRecordField {
                    dst,
                    record: current,
                    field: segment.clone(),
                });
            }
            current = dst;
        }
        Ok(current)
    }

    fn script_enum_field_slot_for_path_root(
        &self,
        span: Span,
        root: &str,
        field: &str,
    ) -> Option<usize> {
        let fact = match self.bindings.resolution_at_span(span) {
            Some(BindingResolution::Local(local)) => self.script_types.local_fact(*local),
            _ => self.script_types.name_fact(root),
        }?;
        let variant = fact.enum_variant.as_deref()?;
        self.facts
            .script_field_slots
            .enum_variant(&fact.type_name, variant, field)
    }
}

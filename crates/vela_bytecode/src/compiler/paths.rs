use vela_common::Span;
use vela_hir::binding::BindingResolution;
use vela_hir::ids::HirExprId;

use crate::{Constant, Register, UnlinkedInstructionKind};

use super::host_paths::{HostPath, HostPathPart, HostPathRoot};
use super::{CompileError, CompileErrorKind, CompileResult, Compiler};

impl Compiler<'_, '_> {
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

    pub(super) fn required_local_register_at_hir_expression_span(
        &mut self,
        span: Span,
        name: &str,
    ) -> CompileResult<Register> {
        let expression = self.expression_at_span(span).ok_or_else(|| {
            CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "HIR expression source origin",
            ))
            .with_span(span)
        })?;
        self.local_register_for_hir_expression(expression, name, span)
    }

    fn local_register_for_hir_expression(
        &mut self,
        expression: HirExprId,
        name: &str,
        span: Span,
    ) -> CompileResult<Register> {
        match self.binding_resolution_for_expression(expression) {
            Some(BindingResolution::Local(local)) => {
                if let Some(register) = self.hir_locals.get(local).copied() {
                    Ok(register)
                } else {
                    Err(CompileError::new(CompileErrorKind::UnknownLocal(
                        name.to_owned(),
                    )))
                }
            }
            Some(BindingResolution::Declaration(declaration)) => {
                if let Some(global) = self.facts.global_symbols.get(declaration).cloned() {
                    let dst = self.alloc_register()?;
                    self.emit_load_global(dst, global);
                    Ok(dst)
                } else if let Some(value) = self.facts.const_values.get(declaration).cloned() {
                    self.emit_constant(value)
                } else {
                    Err(CompileError::new(CompileErrorKind::UnknownLocal(
                        name.to_owned(),
                    )))
                }
            }
            Some(BindingResolution::Import(_) | BindingResolution::QualifiedPath(_)) | None => Err(
                CompileError::new(CompileErrorKind::UnknownLocal(name.to_owned())).with_span(span),
            ),
        }
    }

    pub(super) fn const_value_at_span(&self, span: Span) -> Option<Constant> {
        let expression = self.expression_at_span(span)?;
        let BindingResolution::Declaration(declaration) =
            self.binding_resolution_for_expression(expression)?
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
        let type_name = self.script_type_for_path_root(span, root)?;
        self.script_record_field_slot_for_type(&type_name, field)
    }

    pub(super) fn script_type_for_path_root(&self, span: Span, root: &str) -> Option<String> {
        let expression = self.expression_at_span(span);
        match expression.and_then(|expression| self.binding_resolution_for_expression(expression)) {
            Some(BindingResolution::Local(local)) => self.script_types.local(*local),
            Some(BindingResolution::Declaration(declaration)) => {
                self.facts.global_type_symbols.get(declaration).cloned()
            }
            _ => self
                .script_types
                .name(root)
                .or_else(|| self.global_type_named(root)),
        }
    }

    fn compile_local_path(&mut self, span: Span, path: &[String]) -> CompileResult<Register> {
        let [name] = path else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "path expression",
            )));
        };
        self.required_local_register_at_hir_expression_span(span, name)
    }

    fn emit_load_global(&mut self, dst: Register, global: String) {
        let slot = self.facts.global_slots.get(&global).copied();
        self.emit(UnlinkedInstructionKind::LoadGlobal {
            dst,
            global,
            slot,
            cache_site: None,
        });
    }

    fn compile_path_access(&mut self, span: Span, path: &[String]) -> CompileResult<Register> {
        if path.len() < 2 {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "path expression",
            )));
        }
        if let Some(host_path) = self
            .host_field_path_parts(span, path)
            .map(|resolved| resolved.path)
            && host_path.requires_path_instruction()
        {
            let root = self.compile_host_path_root(&host_path.root)?;
            let dst = self.alloc_register()?;
            self.emit_host_read(dst, root, host_path, span)?;
            return Ok(dst);
        }
        let root_span = self.hir_value_path_root_span_for_span(span).unwrap_or(span);
        let mut current =
            self.required_local_register_at_hir_expression_span(root_span, &path[0])?;
        let mut current_record_shape = self.record_shape_for_path_root(root_span, &path[0]);
        for (index, segment) in path.iter().enumerate().skip(1) {
            let dst = self.alloc_register()?;
            let record_slot = (index == 1)
                .then(|| self.script_record_field_slot_for_path_root(root_span, &path[0], segment))
                .flatten()
                .or_else(|| {
                    current_record_shape
                        .as_ref()
                        .and_then(|shape| shape.field_slot(segment))
                });
            if let Some(slot) = record_slot {
                self.emit(UnlinkedInstructionKind::GetRecordSlot {
                    dst,
                    record: current,
                    field: segment.clone(),
                    slot,
                });
            } else if index == 1
                && let Some(slot) =
                    self.script_enum_field_slot_for_path_root(root_span, &path[0], segment)
            {
                self.emit(UnlinkedInstructionKind::GetEnumSlot {
                    dst,
                    value: current,
                    field: segment.clone(),
                    slot,
                });
            } else if index == 1
                && let Some(field) = self
                    .host_field_info(
                        self.host_local_type_name(&path[0], root_span).as_deref(),
                        segment,
                    )
                    .map(|field| field.id)
            {
                self.emit_host_read(
                    dst,
                    current,
                    HostPath {
                        root: HostPathRoot::LocalPath {
                            name: &path[0],
                            span: root_span,
                        },
                        segments: vec![HostPathPart::Field(field)],
                    },
                    span,
                )?;
            } else {
                self.emit(UnlinkedInstructionKind::GetRecordField {
                    dst,
                    record: current,
                    field: segment.clone(),
                });
            }
            current_record_shape = current_record_shape
                .as_ref()
                .and_then(|shape| shape.field_record_shape(segment))
                .cloned();
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
        let expression = self.expression_at_span(span);
        let fact = match expression
            .and_then(|expression| self.binding_resolution_for_expression(expression))
        {
            Some(BindingResolution::Local(local)) => self.script_types.local_fact(*local),
            _ => self.script_types.name_fact(root),
        }?;
        let variant = fact.enum_variant.as_deref()?;
        self.facts
            .script_field_slots
            .enum_variant(&fact.type_name, variant, field)
    }
}

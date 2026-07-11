use vela_common::Span;
use vela_hir::binding::BindingResolution;
use vela_hir::ids::HirExprId;

use crate::{Register, UnlinkedInstructionKind};

use super::constant_encoding::encode_evaluated_constant;
use super::host_paths::{HostPath, HostPathPart, HostPathRoot};
use super::{CompileError, CompileErrorKind, CompileResult, Compiler};

impl Compiler<'_, '_> {
    pub(super) fn compile_path_expr(
        &mut self,
        expression: HirExprId,
        span: Span,
        path: &[String],
    ) -> CompileResult<Register> {
        if let Some(value) = self.const_value_at_expression(expression) {
            return self.emit_constant(encode_evaluated_constant(&value));
        }
        if path.len() == 1 {
            return self.compile_local_path(expression, span, path);
        }
        self.compile_path_access(expression, span, path)
    }

    pub(super) fn required_local_register_for_hir_expression(
        &mut self,
        expression: HirExprId,
        span: Span,
        name: &str,
    ) -> CompileResult<Register> {
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
                } else if let Some(value) = self.facts.evaluated_constants.get(declaration).cloned()
                {
                    self.emit_constant(encode_evaluated_constant(&value))
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

    pub(super) fn const_value_at_expression(
        &self,
        expression: HirExprId,
    ) -> Option<vela_mir::MirEvaluatedConstant> {
        let BindingResolution::Declaration(declaration) =
            self.binding_resolution_for_expression(expression)?
        else {
            return None;
        };
        self.facts.evaluated_constants.get(declaration).cloned()
    }

    pub(super) fn script_record_field_slot_for_path_root(
        &self,
        expression: HirExprId,
        root: &str,
        field: &str,
    ) -> Option<usize> {
        let type_name = self.script_type_for_path_root(expression, root)?;
        self.script_record_field_slot_for_type(&type_name, field)
    }

    pub(super) fn script_type_for_path_root(
        &self,
        expression: HirExprId,
        root: &str,
    ) -> Option<String> {
        match self.binding_resolution_for_expression(expression) {
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

    fn compile_local_path(
        &mut self,
        expression: HirExprId,
        span: Span,
        path: &[String],
    ) -> CompileResult<Register> {
        let [name] = path else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "path expression",
            )));
        };
        self.required_local_register_for_hir_expression(expression, span, name)
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

    fn compile_path_access(
        &mut self,
        expression: HirExprId,
        span: Span,
        path: &[String],
    ) -> CompileResult<Register> {
        if path.len() < 2 {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "path expression",
            )));
        }
        if let Some(host_path) = self
            .host_field_path_parts(expression, span, path)
            .map(|resolved| resolved.path)
            && host_path.requires_path_instruction()
        {
            let root = self.compile_host_path_root(&host_path.root)?;
            let dst = self.alloc_register()?;
            self.emit_host_read(dst, root, host_path, span)?;
            return Ok(dst);
        }
        let root_expression = self
            .hir_value_path_root_expression(expression)
            .unwrap_or(expression);
        let root_span = self.expression_span(root_expression).unwrap_or(span);
        let mut current =
            self.required_local_register_for_hir_expression(root_expression, root_span, &path[0])?;
        let mut current_record_shape = self.record_shape_for_path_root(root_expression, &path[0]);
        for (index, segment) in path.iter().enumerate().skip(1) {
            let dst = self.alloc_register()?;
            let record_slot = (index == 1)
                .then(|| {
                    self.script_record_field_slot_for_path_root(root_expression, &path[0], segment)
                })
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
                    self.script_enum_field_slot_for_path_root(root_expression, &path[0], segment)
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
                        self.host_local_type_name(&path[0], root_expression)
                            .as_deref(),
                        segment,
                    )
                    .map(|field| field.id)
            {
                self.emit_host_read(
                    dst,
                    current,
                    HostPath {
                        root: HostPathRoot::LocalPath {
                            name: path[0].clone(),
                            expression: root_expression,
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
        expression: HirExprId,
        root: &str,
        field: &str,
    ) -> Option<usize> {
        let fact = match self.binding_resolution_for_expression(expression) {
            Some(BindingResolution::Local(local)) => self.script_types.local_fact(*local),
            _ => self.script_types.name_fact(root),
        }?;
        let variant = fact.enum_variant.as_deref()?;
        self.facts
            .script_field_slots
            .enum_variant(&fact.type_name, variant, field)
    }
}

use vela_common::Span;
use vela_hir::binding::{BindingResolution, LocalBindingKind};
use vela_hir::body::HirPatternKind;
use vela_hir::ids::HirPatternId;

use crate::{Register, UnlinkedInstructionKind};

use super::record_shapes::ValueShape;
use super::script_types::ScriptTypeFact;
use super::value_types::RuntimeTypeFact;
use super::{CompileError, CompileErrorKind, CompileResult, Compiler, frame_slot_kind};

pub(crate) fn enum_variant_path(path: &[String]) -> Option<(String, String)> {
    let (variant, enum_path) = path.split_last()?;
    if enum_path.is_empty() {
        return None;
    }
    Some((enum_path.join("::"), variant.clone()))
}

pub(crate) fn tuple_variant_field_name(index: usize) -> String {
    index.to_string()
}

#[derive(Clone, Debug, Default)]
pub(super) struct PatternBindingFacts {
    script: Option<ScriptTypeFact>,
    value_type: Option<RuntimeTypeFact>,
    value_shape: Option<ValueShape>,
}

impl PatternBindingFacts {
    pub(super) fn value(value_type: Option<RuntimeTypeFact>) -> Self {
        Self {
            script: None,
            value_shape: value_type.clone().map(ValueShape::from_runtime_type),
            value_type,
        }
    }

    pub(super) fn value_shape(value_shape: Option<ValueShape>) -> Self {
        Self {
            script: None,
            value_type: value_shape.as_ref().and_then(ValueShape::value_type),
            value_shape,
        }
    }

    pub(super) fn with_script(mut self, script: Option<ScriptTypeFact>) -> Self {
        self.script = script;
        self
    }

    fn tuple_element(&self, index: usize) -> Self {
        let value_shape = match self.value_shape.as_ref() {
            Some(ValueShape::Tuple(elements)) => elements.get(index).cloned(),
            _ => None,
        };
        Self {
            script: None,
            value_type: value_shape.as_ref().and_then(ValueShape::value_type),
            value_shape,
        }
    }
}

struct PatternLocalBinding<'a> {
    binding: &'a str,
    register: Register,
    scope_span: Span,
    facts: PatternBindingFacts,
    kind: LocalBindingKind,
    hir_pattern: HirPatternId,
}

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn bind_hir_pattern_locals(
        &mut self,
        scrutinee: Register,
        pattern: HirPatternId,
        scope_span: Span,
        facts: PatternBindingFacts,
        kind: LocalBindingKind,
    ) -> CompileResult<()> {
        let pattern_kind = self
            .hir_bodies
            .iter()
            .find_map(|body| body.patterns.get(&pattern))
            .map(|pattern| pattern.kind.clone())
            .ok_or_else(|| {
                CompileError::new(CompileErrorKind::UnsupportedSyntax("HIR pattern"))
                    .with_span(scope_span)
            })?;
        match pattern_kind {
            HirPatternKind::Binding { local } => {
                let local = local.and_then(|local| {
                    self.bindings
                        .local(local)
                        .map(|binding| (binding.id, binding.name.clone()))
                });
                let Some((_, name)) = local else {
                    return Ok(());
                };
                self.bind_pattern_local(PatternLocalBinding {
                    binding: &name,
                    register: scrutinee,
                    scope_span,
                    facts,
                    kind,
                    hir_pattern: pattern,
                });
            }
            HirPatternKind::TupleVariant { path, fields } => {
                let path = path.and_then(|path| {
                    self.hir_bodies
                        .iter()
                        .find_map(|body| body.paths.get(&path))
                        .map(|path| path.path.clone())
                });
                if path.is_none() {
                    self.emit(UnlinkedInstructionKind::GuardTupleArity {
                        value: scrutinee,
                        arity: fields.len(),
                    });
                }
                for (index, field) in fields.into_iter().enumerate() {
                    if !self.hir_pattern_declares_local(field) {
                        continue;
                    }
                    let value = if let Some(path) = path.as_ref() {
                        self.emit_enum_pattern_field_read(
                            scrutinee,
                            path,
                            tuple_variant_field_name(index),
                        )?
                    } else {
                        self.emit_tuple_pattern_field_read(scrutinee, index)?
                    };
                    let field_facts = if let Some(path) = path.as_ref() {
                        let field_name = tuple_variant_field_name(index);
                        PatternBindingFacts::value(
                            self.enum_variant_field_value_type(path, &field_name),
                        )
                        .with_script(self.enum_variant_field_fact(path, &field_name))
                    } else {
                        facts.tuple_element(index)
                    };
                    self.bind_hir_pattern_locals(value, field, scope_span, field_facts, kind)?;
                }
            }
            HirPatternKind::RecordVariant { path, fields } => {
                let path = path.and_then(|path| {
                    self.hir_bodies
                        .iter()
                        .find_map(|body| body.paths.get(&path))
                        .map(|path| path.path.clone())
                });
                let Some(path) = path else {
                    return Ok(());
                };
                for field in fields {
                    let Some(pattern) = field.pattern else {
                        continue;
                    };
                    if !self.hir_pattern_declares_local(pattern) {
                        continue;
                    }
                    let value =
                        self.emit_enum_pattern_field_read(scrutinee, &path, field.name.clone())?;
                    let field_facts = PatternBindingFacts::value(
                        self.enum_variant_field_value_type(&path, &field.name),
                    )
                    .with_script(self.enum_variant_field_fact(&path, &field.name));
                    self.bind_hir_pattern_locals(value, pattern, scope_span, field_facts, kind)?;
                }
            }
            HirPatternKind::Path { .. }
            | HirPatternKind::Wildcard
            | HirPatternKind::Literal(_)
            | HirPatternKind::Missing => {}
        }
        Ok(())
    }

    fn hir_pattern_declares_local(&self, pattern: HirPatternId) -> bool {
        let Some(pattern) = self
            .hir_bodies
            .iter()
            .find_map(|body| body.patterns.get(&pattern))
        else {
            return false;
        };
        match &pattern.kind {
            HirPatternKind::Binding { local } => local.is_some(),
            HirPatternKind::TupleVariant { fields, .. } => fields
                .iter()
                .any(|field| self.hir_pattern_declares_local(*field)),
            HirPatternKind::RecordVariant { fields, .. } => fields.iter().any(|field| {
                field
                    .pattern
                    .is_some_and(|pattern| self.hir_pattern_declares_local(pattern))
            }),
            HirPatternKind::Path { .. }
            | HirPatternKind::Wildcard
            | HirPatternKind::Literal(_)
            | HirPatternKind::Missing => false,
        }
    }

    pub(in crate::compiler) fn compile_variant_tag_pattern(
        &mut self,
        scrutinee: Register,
        path: &[String],
    ) -> CompileResult<Vec<usize>> {
        let Some((enum_name, variant)) = enum_variant_path(path) else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "match pattern",
            )));
        };
        let enum_name = self.type_symbol_for_pattern(path).unwrap_or(enum_name);
        let condition = self.alloc_register()?;
        self.emit(UnlinkedInstructionKind::EnumTagEqual {
            dst: condition,
            value: scrutinee,
            enum_name,
            variant,
        });
        Ok(vec![self.emit_jump_if_false(condition)])
    }

    fn bind_pattern_local(&mut self, local: PatternLocalBinding<'_>) {
        self.locals.insert(local.binding.to_owned(), local.register);
        if let Some(hir_local) = self.local_for_pattern(local.hir_pattern, local.kind) {
            self.hir_locals.insert(hir_local, local.register);
            self.record_frame_slot(
                local.binding.to_owned(),
                local.register,
                frame_slot_kind(local.kind),
                Some(hir_local),
                Some(local.scope_span),
            );
            self.script_types
                .set_local_fact(hir_local, local.binding, local.facts.script);
            self.value_types
                .set_local(hir_local, local.binding, local.facts.value_type);
            self.value_shapes
                .set_local(hir_local, local.binding, local.facts.value_shape);
        } else {
            self.record_frame_slot(
                local.binding.to_owned(),
                local.register,
                frame_slot_kind(local.kind),
                None,
                Some(local.scope_span),
            );
            self.value_types
                .set_name(local.binding, local.facts.value_type);
            self.value_shapes
                .set_name(local.binding, local.facts.value_shape);
        }
    }

    pub(in crate::compiler) fn enum_variant_field_fact(
        &self,
        path: &[String],
        field: &str,
    ) -> Option<ScriptTypeFact> {
        let (_, variant) = enum_variant_path(path)?;
        let enum_name = self.type_symbol_for_pattern(path)?;
        self.facts
            .script_field_slots
            .enum_variant_field_fact(&enum_name, &variant, field)
    }

    pub(in crate::compiler) fn enum_variant_field_value_type(
        &self,
        path: &[String],
        field: &str,
    ) -> Option<RuntimeTypeFact> {
        let (_, variant) = enum_variant_path(path)?;
        let enum_name = self.type_symbol_for_pattern(path)?;
        self.facts
            .script_field_slots
            .enum_variant_field_value_type(&enum_name, &variant, field)
    }

    pub(in crate::compiler) fn emit_enum_pattern_field_read(
        &mut self,
        scrutinee: Register,
        path: &[String],
        field: String,
    ) -> CompileResult<Register> {
        let dst = self.alloc_register()?;
        if let Some(slot) = self.enum_variant_field_slot_for_pattern(path, &field) {
            self.emit(UnlinkedInstructionKind::GetEnumSlot {
                dst,
                value: scrutinee,
                field,
                slot,
            });
        } else {
            self.emit(UnlinkedInstructionKind::GetEnumField {
                dst,
                value: scrutinee,
                field,
            });
        }
        Ok(dst)
    }

    pub(in crate::compiler) fn emit_tuple_pattern_field_read(
        &mut self,
        scrutinee: Register,
        index: usize,
    ) -> CompileResult<Register> {
        let dst = self.alloc_register()?;
        self.emit(UnlinkedInstructionKind::GetTupleField {
            dst,
            value: scrutinee,
            index,
        });
        Ok(dst)
    }

    fn enum_variant_field_slot_for_pattern(&self, path: &[String], field: &str) -> Option<usize> {
        let (_, variant) = enum_variant_path(path)?;
        let enum_name = self.type_symbol_for_pattern(path)?;
        self.facts
            .script_field_slots
            .enum_variant(&enum_name, &variant, field)
    }

    fn type_symbol_for_pattern(&self, path: &[String]) -> Option<String> {
        let Some(BindingResolution::Declaration(declaration)) =
            self.bindings.pattern_resolution(path)
        else {
            return None;
        };
        self.facts.type_symbols.get(declaration).cloned()
    }
}

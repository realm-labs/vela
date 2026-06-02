use vela_common::Span;
use vela_hir::binding::{BindingResolution, LocalBindingKind};
use vela_syntax::ast::{Pattern, RecordPatternField};

use crate::{InstructionKind, Register};

use super::script_types::ScriptTypeFact;
use super::{CompileError, CompileErrorKind, CompileResult, Compiler, frame_slot_kind};

pub(crate) fn enum_variant_path(path: &[String]) -> Option<(String, String)> {
    let (variant, enum_path) = path.split_last()?;
    if enum_path.is_empty() {
        return None;
    }
    Some((enum_path.join("::"), variant.clone()))
}

pub(crate) fn record_pattern_field_match(field: &RecordPatternField) -> Option<&Pattern> {
    match field.pattern.as_ref() {
        Some(Pattern::Wildcard | Pattern::Binding(_)) | None => None,
        Some(pattern) => Some(pattern),
    }
}

pub(crate) fn record_pattern_field_declares_locals(field: &RecordPatternField) -> bool {
    field.pattern.as_ref().is_none_or(pattern_declares_locals)
}

pub(crate) fn tuple_variant_field_name(index: usize) -> String {
    index.to_string()
}

pub(crate) fn pattern_declares_locals(pattern: &Pattern) -> bool {
    match pattern {
        Pattern::Binding(_) => true,
        Pattern::TupleVariant { fields, .. } => fields.iter().any(pattern_declares_locals),
        Pattern::RecordVariant { fields, .. } => {
            fields.iter().any(record_pattern_field_declares_locals)
        }
        Pattern::Wildcard | Pattern::Literal(_) | Pattern::Path(_) => false,
    }
}

impl Compiler<'_> {
    pub(super) fn compile_match_pattern(
        &mut self,
        scrutinee: Register,
        pattern: &Pattern,
    ) -> CompileResult<Vec<usize>> {
        match pattern {
            Pattern::Wildcard | Pattern::Binding(_) => Ok(Vec::new()),
            Pattern::Literal(literal) => {
                let pattern = self.compile_literal(literal)?;
                let condition = self.alloc_register()?;
                self.emit(InstructionKind::Equal {
                    dst: condition,
                    lhs: scrutinee,
                    rhs: pattern,
                });
                Ok(vec![self.emit_jump_if_false(condition)])
            }
            Pattern::Path(path) => self.compile_variant_tag_pattern(scrutinee, path),
            Pattern::RecordVariant { path, fields } => {
                let mut jumps = self.compile_variant_tag_pattern(scrutinee, path)?;
                for field in fields {
                    let Some(pattern) = record_pattern_field_match(field) else {
                        continue;
                    };
                    let field_value = self.alloc_register()?;
                    self.emit(InstructionKind::GetEnumField {
                        dst: field_value,
                        value: scrutinee,
                        field: field.name.clone(),
                    });
                    jumps.extend(self.compile_match_pattern(field_value, pattern)?);
                }
                Ok(jumps)
            }
            Pattern::TupleVariant { path, fields } => {
                let mut jumps = self.compile_variant_tag_pattern(scrutinee, path)?;
                for (index, field) in fields.iter().enumerate() {
                    if matches!(field, Pattern::Wildcard | Pattern::Binding(_)) {
                        continue;
                    }
                    let field_value = self.alloc_register()?;
                    self.emit(InstructionKind::GetEnumField {
                        dst: field_value,
                        value: scrutinee,
                        field: tuple_variant_field_name(index),
                    });
                    jumps.extend(self.compile_match_pattern(field_value, field)?);
                }
                Ok(jumps)
            }
        }
    }

    fn compile_variant_tag_pattern(
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
        self.emit(InstructionKind::EnumTagEqual {
            dst: condition,
            value: scrutinee,
            enum_name,
            variant,
        });
        Ok(vec![self.emit_jump_if_false(condition)])
    }

    pub(super) fn bind_pattern_locals(
        &mut self,
        scrutinee: Register,
        pattern: &Pattern,
        body_span: Span,
        script_fact: Option<ScriptTypeFact>,
        kind: LocalBindingKind,
    ) -> CompileResult<()> {
        match pattern {
            Pattern::Binding(binding) => {
                let dst = self.alloc_register()?;
                self.emit(InstructionKind::Move {
                    dst,
                    src: scrutinee,
                });
                self.bind_pattern_local(binding, dst, body_span, script_fact, kind);
                Ok(())
            }
            Pattern::RecordVariant { path, fields } => {
                for field in fields {
                    if !record_pattern_field_declares_locals(field) {
                        continue;
                    }
                    let dst = self.alloc_register()?;
                    self.emit(InstructionKind::GetEnumField {
                        dst,
                        value: scrutinee,
                        field: field.name.clone(),
                    });
                    let field_fact = self.enum_variant_field_fact(path, &field.name);
                    match &field.pattern {
                        Some(pattern) => {
                            self.bind_pattern_locals(dst, pattern, body_span, field_fact, kind)?
                        }
                        None => {
                            self.bind_pattern_local(&field.name, dst, body_span, field_fact, kind)
                        }
                    }
                }
                Ok(())
            }
            Pattern::TupleVariant { path, fields } => {
                for (index, field) in fields.iter().enumerate() {
                    if !pattern_declares_locals(field) {
                        continue;
                    }
                    let field_value = self.alloc_register()?;
                    self.emit(InstructionKind::GetEnumField {
                        dst: field_value,
                        value: scrutinee,
                        field: tuple_variant_field_name(index),
                    });
                    let field_name = tuple_variant_field_name(index);
                    let field_fact = self.enum_variant_field_fact(path, &field_name);
                    self.bind_pattern_locals(field_value, field, body_span, field_fact, kind)?;
                }
                Ok(())
            }
            Pattern::Wildcard | Pattern::Literal(_) | Pattern::Path(_) => Ok(()),
        }
    }

    fn bind_pattern_local(
        &mut self,
        binding: &str,
        register: Register,
        body_span: Span,
        script_fact: Option<ScriptTypeFact>,
        kind: LocalBindingKind,
    ) {
        self.locals.insert(binding.to_owned(), register);
        if let Some(local) = self.bindings.local_named_at(binding, kind, body_span) {
            self.hir_locals.insert(local, register);
            self.record_frame_slot(
                binding.to_owned(),
                register,
                frame_slot_kind(kind),
                Some(local),
                Some(body_span),
            );
            self.script_types
                .set_local_fact(local, binding, script_fact);
        } else {
            self.record_frame_slot(
                binding.to_owned(),
                register,
                frame_slot_kind(kind),
                None,
                Some(body_span),
            );
        }
    }

    fn enum_variant_field_fact(&self, path: &[String], field: &str) -> Option<ScriptTypeFact> {
        let (_, variant) = enum_variant_path(path)?;
        let enum_name = self.type_symbol_for_pattern(path)?;
        self.facts
            .script_field_slots
            .enum_variant_field_fact(&enum_name, &variant, field)
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

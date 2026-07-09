use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingResolution, LocalBindingKind};
use vela_hir::body::{HirPathKind, HirPathOwner};
use vela_hir::ids::HirPatternId;
use vela_syntax::TextRange;
use vela_syntax::ast::{AstNode, SyntaxPattern, SyntaxPatternKind, SyntaxRecordPatternField};

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

fn pattern_kind_declares_locals(kind: SyntaxPatternKind) -> bool {
    matches!(
        kind,
        SyntaxPatternKind::Binding
            | SyntaxPatternKind::TupleVariant
            | SyntaxPatternKind::RecordVariant
    )
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

    pub(in crate::compiler) fn bind_syntax_pattern_locals_from_hir_patterns(
        &mut self,
        scrutinee: Register,
        pattern: &SyntaxPattern,
        body_span: Span,
        facts: PatternBindingFacts,
        kind: LocalBindingKind,
        patterns: &[HirPatternId],
    ) -> CompileResult<()> {
        let mut cursor = HirPatternCursor::new(patterns);
        self.bind_syntax_pattern_locals_from_hir_cursor(
            scrutinee,
            pattern,
            body_span,
            facts,
            kind,
            &mut cursor,
        )
    }

    pub(in crate::compiler) fn bind_syntax_pattern_locals_from_hir_cursor(
        &mut self,
        scrutinee: Register,
        pattern: &SyntaxPattern,
        body_span: Span,
        facts: PatternBindingFacts,
        kind: LocalBindingKind,
        cursor: &mut HirPatternCursor<'_>,
    ) -> CompileResult<()> {
        self.bind_syntax_pattern_locals_inner(scrutinee, pattern, body_span, facts, kind, cursor)
    }

    pub(in crate::compiler) fn hir_pattern_ids_for_syntax_pattern(
        &self,
        source: SourceId,
        pattern: &SyntaxPattern,
    ) -> CompileResult<Vec<HirPatternId>> {
        let mut patterns = Vec::new();
        self.collect_hir_pattern_ids_for_syntax_pattern(source, pattern, &mut patterns)?;
        Ok(patterns)
    }

    fn collect_hir_pattern_ids_for_syntax_pattern(
        &self,
        source: SourceId,
        pattern: &SyntaxPattern,
        patterns: &mut Vec<HirPatternId>,
    ) -> CompileResult<()> {
        patterns.push(
            self.required_hir_pattern_at_source_origin(source, pattern.syntax().text_range())?,
        );
        match pattern.pattern_kind() {
            Some(SyntaxPatternKind::RecordVariant) => {
                let Some(record) = pattern.record_pattern() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "record pattern",
                    )));
                };
                for field in record.fields() {
                    if let Some(pattern) = field.pattern() {
                        self.collect_hir_pattern_ids_for_syntax_pattern(
                            source, &pattern, patterns,
                        )?;
                    } else if let Some(binding) = field.shorthand_binding_name_token() {
                        patterns.push(
                            self.required_hir_pattern_at_source_origin(
                                source,
                                binding.text_range(),
                            )?,
                        );
                    }
                }
            }
            Some(SyntaxPatternKind::TupleVariant) => {
                let Some(tuple) = pattern.tuple_pattern() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "tuple pattern",
                    )));
                };
                for field in tuple.patterns() {
                    self.collect_hir_pattern_ids_for_syntax_pattern(source, &field, patterns)?;
                }
            }
            Some(
                SyntaxPatternKind::Binding
                | SyntaxPatternKind::Path
                | SyntaxPatternKind::Wildcard
                | SyntaxPatternKind::Literal,
            )
            | None => {}
        }
        Ok(())
    }

    fn bind_syntax_pattern_locals_inner(
        &mut self,
        scrutinee: Register,
        pattern: &SyntaxPattern,
        body_span: Span,
        facts: PatternBindingFacts,
        kind: LocalBindingKind,
        cursor: &mut HirPatternCursor<'_>,
    ) -> CompileResult<()> {
        let hir_pattern = cursor.next(body_span)?;
        let pattern_kind = pattern.pattern_kind().ok_or_else(|| {
            CompileError::new(CompileErrorKind::UnsupportedSyntax("match pattern"))
        })?;
        match pattern_kind {
            SyntaxPatternKind::Binding => {
                let binding_token = pattern.binding_name_token().ok_or_else(|| {
                    CompileError::new(CompileErrorKind::UnsupportedSyntax("binding pattern"))
                })?;
                let binding = binding_token.text().to_owned();
                let dst = self.alloc_register()?;
                self.emit(UnlinkedInstructionKind::Move {
                    dst,
                    src: scrutinee,
                });
                self.bind_pattern_local(PatternLocalBinding {
                    binding: &binding,
                    register: dst,
                    scope_span: body_span,
                    facts,
                    kind,
                    hir_pattern,
                });
                Ok(())
            }
            SyntaxPatternKind::RecordVariant => {
                let path = self.required_hir_pattern_path(body_span.source, pattern)?;
                let record = pattern.record_pattern().ok_or_else(|| {
                    CompileError::new(CompileErrorKind::UnsupportedSyntax("match pattern"))
                })?;
                for field in record.fields() {
                    let field_name = syntax_record_pattern_field_name(&field)?;
                    let field_pattern = field.pattern();
                    let field_declares_locals = field_pattern
                        .as_ref()
                        .and_then(SyntaxPattern::pattern_kind)
                        .is_some_and(pattern_kind_declares_locals)
                        || (field_pattern.is_none() && field.is_shorthand());
                    if !field_declares_locals {
                        continue;
                    }
                    let dst =
                        self.emit_enum_pattern_field_read(scrutinee, &path, field_name.clone())?;
                    let field_facts = PatternBindingFacts::value(
                        self.enum_variant_field_value_type(&path, &field_name),
                    )
                    .with_script(self.enum_variant_field_fact(&path, &field_name));
                    if let Some(field_pattern) = field_pattern {
                        self.bind_syntax_pattern_locals_inner(
                            dst,
                            &field_pattern,
                            body_span,
                            field_facts,
                            kind,
                            cursor,
                        )?;
                    } else if field.shorthand_binding_name_token().is_some() {
                        let hir_pattern = cursor.next(body_span)?;
                        self.bind_pattern_local(PatternLocalBinding {
                            binding: &field_name,
                            register: dst,
                            scope_span: body_span,
                            facts: field_facts,
                            kind,
                            hir_pattern,
                        });
                    }
                }
                Ok(())
            }
            SyntaxPatternKind::TupleVariant => {
                let tuple = pattern.tuple_pattern().ok_or_else(|| {
                    CompileError::new(CompileErrorKind::UnsupportedSyntax("match pattern"))
                })?;
                let Some(path) = self.hir_pattern_path(body_span.source, pattern) else {
                    self.emit(UnlinkedInstructionKind::GuardTupleArity {
                        value: scrutinee,
                        arity: tuple.patterns().count(),
                    });
                    for (index, field) in tuple.patterns().enumerate() {
                        let field_kind =
                            required_syntax_pattern_kind(&field, "tuple pattern field")?;
                        if !pattern_kind_declares_locals(field_kind) {
                            continue;
                        }
                        let field_value = self.emit_tuple_pattern_field_read(scrutinee, index)?;
                        self.bind_syntax_pattern_locals_inner(
                            field_value,
                            &field,
                            body_span,
                            PatternBindingFacts::default(),
                            kind,
                            cursor,
                        )?;
                    }
                    return Ok(());
                };
                for (index, field) in tuple.patterns().enumerate() {
                    let field_kind = required_syntax_pattern_kind(&field, "tuple pattern field")?;
                    if !pattern_kind_declares_locals(field_kind) {
                        continue;
                    }
                    let field_name = tuple_variant_field_name(index);
                    let field_value =
                        self.emit_enum_pattern_field_read(scrutinee, &path, field_name.clone())?;
                    let field_facts = PatternBindingFacts::value(
                        self.enum_variant_field_value_type(&path, &field_name),
                    )
                    .with_script(self.enum_variant_field_fact(&path, &field_name));
                    self.bind_syntax_pattern_locals_inner(
                        field_value,
                        &field,
                        body_span,
                        field_facts,
                        kind,
                        cursor,
                    )?;
                }
                Ok(())
            }
            SyntaxPatternKind::Wildcard | SyntaxPatternKind::Literal | SyntaxPatternKind::Path => {
                Ok(())
            }
        }
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

    fn required_hir_pattern_at_source_origin(
        &self,
        source: SourceId,
        range: TextRange,
    ) -> CompileResult<HirPatternId> {
        self.hir_pattern_at_source_origin(source, range)
            .ok_or_else(|| {
                CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "HIR pattern source origin",
                ))
                .with_span(span_for_range(source, range))
            })
    }

    fn hir_pattern_at_source_origin(
        &self,
        source: SourceId,
        range: TextRange,
    ) -> Option<HirPatternId> {
        let span = span_for_range(source, range);
        self.hir_bodies
            .iter()
            .flat_map(|body| body.patterns.values())
            .find_map(|pattern| (pattern.origin.span == span).then_some(pattern.id))
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

    pub(in crate::compiler) fn hir_pattern_path(
        &self,
        source: SourceId,
        pattern: &SyntaxPattern,
    ) -> Option<Vec<String>> {
        let pattern = self.hir_pattern_at_source_origin(source, pattern.syntax().text_range())?;
        self.hir_bodies
            .iter()
            .flat_map(|body| body.paths.iter())
            .find_map(|path| {
                (path.kind == HirPathKind::Pattern && path.owner == HirPathOwner::Pattern(pattern))
                    .then(|| path.path.clone())
            })
    }

    pub(in crate::compiler) fn required_hir_pattern_path(
        &self,
        source: SourceId,
        pattern: &SyntaxPattern,
    ) -> CompileResult<Vec<String>> {
        self.hir_pattern_path(source, pattern).ok_or_else(|| {
            CompileError::new(CompileErrorKind::UnsupportedSyntax("path pattern"))
                .with_span(span_for_range(source, pattern.syntax().text_range()))
        })
    }
}

pub(in crate::compiler) struct HirPatternCursor<'a> {
    patterns: &'a [HirPatternId],
    index: usize,
}

impl<'a> HirPatternCursor<'a> {
    pub(in crate::compiler) const fn new(patterns: &'a [HirPatternId]) -> Self {
        Self { patterns, index: 0 }
    }

    fn next(&mut self, span: Span) -> CompileResult<HirPatternId> {
        let Some(pattern) = self.patterns.get(self.index).copied() else {
            return Err(
                CompileError::new(CompileErrorKind::UnsupportedSyntax("HIR pattern"))
                    .with_span(span),
            );
        };
        self.index = self.index.saturating_add(1);
        Ok(pattern)
    }
}

fn required_syntax_pattern_kind(
    pattern: &SyntaxPattern,
    context: &'static str,
) -> CompileResult<SyntaxPatternKind> {
    pattern
        .pattern_kind()
        .ok_or_else(|| CompileError::new(CompileErrorKind::UnsupportedSyntax(context)))
}

fn syntax_record_pattern_field_name(field: &SyntaxRecordPatternField) -> CompileResult<String> {
    field.label_text().ok_or_else(|| {
        CompileError::new(CompileErrorKind::UnsupportedSyntax("record pattern field"))
    })
}

fn span_for_range(source: vela_common::SourceId, range: TextRange) -> Span {
    Span::new(source, range.start().into(), range.end().into())
}

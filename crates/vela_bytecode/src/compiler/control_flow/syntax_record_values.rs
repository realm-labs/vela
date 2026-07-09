use std::collections::BTreeSet;

use vela_common::{SourceId, Span};
use vela_syntax::ast::{AstNode, SyntaxExpression, SyntaxRecordExpr, SyntaxRecordExprField};

use super::spans::syntax_expression_span;
use crate::compiler::body_payloads::expression_syntax_literal;
use crate::compiler::const_eval::compile_literal_constant_for_type;
use crate::compiler::constructors::schema_default_fields;
use crate::compiler::expected_exprs::guard_location_and_name;
use crate::compiler::patterns::enum_variant_path;
use crate::compiler::schema_defaults::{
    ConstructorFieldUse, record_constructor_field_diagnostics, unknown_enum_variant_diagnostic,
};
use crate::compiler::value_types::{
    ExpectedTypeOutcome, RuntimeTypeFact, TypeContractContext, check_expected_type,
};
use crate::compiler::{CompileResult, Compiler, type_guard_plan_for_runtime_type};
use crate::{
    GuardKind, Register, UnlinkedGuardContext, UnlinkedInstructionKind, UnlinkedTypeGuard,
};

impl Compiler<'_, '_> {
    pub(super) fn compile_syntax_record_literal(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        record: &SyntaxRecordExpr,
    ) -> CompileResult<Option<Register>> {
        let path = record.path_segments();
        if path.is_empty() {
            return Ok(None);
        }
        let span = syntax_expression_span(source, expression);
        let syntax_fields = record.fields();
        let field_uses = syntax_record_field_uses(source, &syntax_fields)?;
        let (enum_constructor, shape) = if let Some((enum_name, variant)) = enum_variant_path(&path)
        {
            let resolved_enum_name = self.type_symbol_at_span(span);
            let enum_name = resolved_enum_name.clone().unwrap_or(enum_name);
            if resolved_enum_name.is_some()
                && !self.enum_constructor_variant_exists(&enum_name, &variant)
            {
                return Err(self.constructor_diagnostics_error(vec![
                    unknown_enum_variant_diagnostic(&enum_name, &variant, span),
                ]));
            }
            let shape = self.enum_constructor_shape(&enum_name, &variant);
            self.reject_constructor_diagnostics(record_constructor_field_diagnostics(
                &format!("{enum_name}::{variant}"),
                shape.as_ref(),
                &field_uses,
                span,
            ))?;
            (Some((enum_name, variant)), shape)
        } else {
            let type_name = self
                .type_symbol_at_span(span)
                .unwrap_or_else(|| path.join("::"));
            let shape = self.record_constructor_shape(&type_name);
            self.reject_constructor_diagnostics(record_constructor_field_diagnostics(
                &type_name,
                shape.as_ref(),
                &field_uses,
                span,
            ))?;
            (None, shape)
        };
        let mut fields = Vec::new();
        let mut explicit_names = BTreeSet::new();
        for field in &syntax_fields {
            let Some(name) = field.label_text() else {
                return Ok(None);
            };
            let value = if let Some(expression) = field.expression() {
                let expected = shape
                    .as_ref()
                    .and_then(|shape| shape.field_value_type(&name));
                let Some(value) = self.compile_syntax_constructor_field_value(
                    source,
                    &expression,
                    expected,
                    &name,
                )?
                else {
                    return Ok(None);
                };
                value
            } else if field.is_shorthand() {
                self.local_register_at_span(syntax_record_field_label_span(source, field), &name)?
            } else {
                return Ok(None);
            };
            explicit_names.insert(name.clone());
            fields.push((name, value));
        }
        let dst = self.alloc_register()?;
        if let Some((enum_name, variant)) = enum_constructor {
            self.compile_schema_default_fields(
                &mut fields,
                &explicit_names,
                schema_default_fields(shape.as_ref()),
                shape.as_ref(),
            )?;
            self.emit(UnlinkedInstructionKind::MakeEnum {
                dst,
                enum_name,
                variant,
                fields,
            });
        } else {
            let type_name = self
                .type_symbol_at_span(span)
                .unwrap_or_else(|| path.join("::"));
            self.compile_schema_default_fields(
                &mut fields,
                &explicit_names,
                schema_default_fields(shape.as_ref()),
                shape.as_ref(),
            )?;
            self.emit(UnlinkedInstructionKind::MakeRecord {
                dst,
                type_name,
                fields,
            });
        }
        Ok(Some(dst))
    }

    fn compile_syntax_constructor_field_value(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        expected: Option<RuntimeTypeFact>,
        field_name: &str,
    ) -> CompileResult<Option<Register>> {
        let Some(expected) = expected else {
            return self.compile_syntax_expression(source, expression);
        };
        let span = syntax_expression_span(source, expression);
        let context = TypeContractContext::Field {
            name: field_name.to_owned(),
        };
        let static_type = self.syntax_static_type_for_expression(Some(source), expression);
        let outcome = check_expected_type(static_type, expected, span, context.clone())?;
        if let ExpectedTypeOutcome::Contextualized(RuntimeTypeFact::Primitive(tag)) = &outcome
            && let Some(literal) = expression_syntax_literal(expression)
            && let Some(constant) = compile_literal_constant_for_type(&literal, *tag)
                .map_err(|error| error.with_span(span))?
        {
            return self.emit_constant(constant).map(Some);
        }
        let Some(value) = self.compile_syntax_expression(source, expression)? else {
            return Ok(None);
        };
        if let ExpectedTypeOutcome::RequiresRuntimeGuard(expected) = &outcome
            && let Some((location, name)) = guard_location_and_name(context)
            && let Some(plan) = type_guard_plan_for_runtime_type(expected)
        {
            self.emit_spanned(
                UnlinkedInstructionKind::GuardType {
                    src: value,
                    guard: UnlinkedTypeGuard::new(
                        plan,
                        UnlinkedGuardContext::new(GuardKind::Contract, location, name),
                    ),
                },
                span,
            );
        }
        Ok(Some(value))
    }
}

fn syntax_record_field_uses(
    source: SourceId,
    fields: &[SyntaxRecordExprField],
) -> CompileResult<Vec<ConstructorFieldUse>> {
    fields
        .iter()
        .map(|field| {
            let Some(name) = field.label_text() else {
                return Ok(None);
            };
            Ok(Some(ConstructorFieldUse {
                name,
                span: syntax_record_field_span(source, field),
            }))
        })
        .collect::<CompileResult<Option<Vec<_>>>>()
        .map(Option::unwrap_or_default)
}

fn syntax_record_field_span(source: SourceId, field: &SyntaxRecordExprField) -> Span {
    let range = field.syntax().text_range();
    Span::new(source, range.start().into(), range.end().into())
}

fn syntax_record_field_label_span(source: SourceId, field: &SyntaxRecordExprField) -> Span {
    let Some(label) = field.label_token() else {
        return syntax_record_field_span(source, field);
    };
    let range = label.text_range();
    Span::new(source, range.start().into(), range.end().into())
}

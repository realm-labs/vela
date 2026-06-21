use std::collections::BTreeSet;

use vela_common::{SourceId, Span};
use vela_syntax::ast::{AstNode, SyntaxExpression, SyntaxRecordExpr, SyntaxRecordExprField};

use crate::{
    GuardKind, GuardLocation, Register, UnlinkedGuardContext, UnlinkedInstructionKind,
    UnlinkedTypeGuard,
};

use crate::compiler::constructors::schema_default_fields;
use crate::compiler::patterns::enum_variant_path;
use crate::compiler::schema_defaults::{
    ConstructorFieldUse, ConstructorShape, record_constructor_field_diagnostics,
    unknown_enum_variant_diagnostic,
};
use crate::compiler::value_types::{
    ExpectedTypeOutcome, RuntimeTypeFact, TypeContractContext, check_expected_type,
};
use crate::compiler::{
    CompileError, CompileErrorKind, CompileResult, Compiler, type_guard_plan_for_runtime_type,
};

use super::{
    param_default_cst_lowering_covers, param_default_unsupported, span_for, span_for_range,
};

impl Compiler<'_, '_> {
    pub(super) fn compile_param_default_record(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        record: &SyntaxRecordExpr,
    ) -> CompileResult<Register> {
        let path = record.path_segments();
        if path.is_empty() {
            return Err(param_default_unsupported(source, expression));
        }
        let span = span_for(source, expression);
        let fields = record_fields(source, record)?;
        let dst = self.alloc_register()?;

        if let Some((enum_name, variant)) = enum_variant_path(&path) {
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
            self.compile_param_default_record_fields(
                source,
                format!("{enum_name}::{variant}"),
                shape.as_ref(),
                fields,
                span,
            )
            .map(|fields| {
                self.emit(UnlinkedInstructionKind::MakeEnum {
                    dst,
                    enum_name,
                    variant,
                    fields,
                });
                dst
            })
        } else {
            let type_name = self
                .type_symbol_at_span(span)
                .unwrap_or_else(|| path.join("::"));
            let shape = self.record_constructor_shape(&type_name);
            self.compile_param_default_record_fields(
                source,
                type_name.clone(),
                shape.as_ref(),
                fields,
                span,
            )
            .map(|fields| {
                self.emit(UnlinkedInstructionKind::MakeRecord {
                    dst,
                    type_name,
                    fields,
                });
                dst
            })
        }
    }

    fn compile_param_default_record_fields(
        &mut self,
        source: SourceId,
        owner: String,
        shape: Option<&ConstructorShape>,
        fields: Vec<ParamDefaultRecordField>,
        constructor_span: Span,
    ) -> CompileResult<Vec<(String, Register)>> {
        let field_uses = fields
            .iter()
            .map(|field| ConstructorFieldUse {
                name: field.name.clone(),
                span: field.span,
            })
            .collect::<Vec<_>>();
        self.reject_constructor_diagnostics(record_constructor_field_diagnostics(
            &owner,
            shape,
            &field_uses,
            constructor_span,
        ))?;

        let mut compiled = Vec::new();
        let mut explicit_names = BTreeSet::new();
        for field in fields {
            explicit_names.insert(field.name.clone());
            let expected = shape.and_then(|shape| shape.field_value_type(&field.name));
            let value = self.compile_param_default_record_field(source, &field, expected)?;
            compiled.push((field.name, value));
        }
        self.compile_schema_default_fields(
            &mut compiled,
            &explicit_names,
            schema_default_fields(shape),
            shape,
        )?;
        Ok(compiled)
    }

    fn compile_param_default_record_field(
        &mut self,
        source: SourceId,
        field: &ParamDefaultRecordField,
        expected: Option<RuntimeTypeFact>,
    ) -> CompileResult<Register> {
        let Some(value) = field.value.as_ref() else {
            return self.local_register_at_span(field.span, &field.name);
        };
        let Some(expected) = expected else {
            return self.compile_param_default_expression(source, value);
        };
        let context = TypeContractContext::Field {
            name: field.name.clone(),
        };
        let outcome = check_expected_type(
            self.param_default_static_type(source, value),
            expected,
            span_for(source, value),
            context,
        )?;
        let register = self.compile_param_default_initializer(source, value, Some(&outcome))?;
        if let ExpectedTypeOutcome::RequiresRuntimeGuard(expected) = &outcome
            && let Some(plan) = type_guard_plan_for_runtime_type(expected)
        {
            self.emit_spanned(
                UnlinkedInstructionKind::GuardType {
                    src: register,
                    guard: UnlinkedTypeGuard::new(
                        plan,
                        UnlinkedGuardContext::new(
                            GuardKind::Contract,
                            GuardLocation::Field,
                            field.name.clone(),
                        ),
                    ),
                },
                span_for(source, value),
            );
        }
        Ok(register)
    }
}

pub(super) fn param_default_record_cst_lowering_covers(expression: &SyntaxExpression) -> bool {
    expression.as_record().is_some_and(|record| {
        !record.path_segments().is_empty()
            && record.fields().into_iter().all(|field| {
                field.label_text().is_some()
                    && match field.expression() {
                        Some(value) => param_default_cst_lowering_covers(&value),
                        None => field.is_shorthand(),
                    }
            })
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ParamDefaultRecordField {
    name: String,
    span: Span,
    value: Option<SyntaxExpression>,
}

fn record_fields(
    source: SourceId,
    record: &SyntaxRecordExpr,
) -> CompileResult<Vec<ParamDefaultRecordField>> {
    record
        .fields()
        .into_iter()
        .map(|field| syntax_record_field(source, &field))
        .collect()
}

fn syntax_record_field(
    source: SourceId,
    field: &SyntaxRecordExprField,
) -> CompileResult<ParamDefaultRecordField> {
    let Some(name) = field.label_text() else {
        return Err(
            CompileError::new(CompileErrorKind::UnsupportedSyntax("record field"))
                .with_span(span_for_range(source, field.syntax().text_range())),
        );
    };
    let span = field
        .label_token()
        .map(|label| {
            Span::new(
                source,
                label.text_range().start().into(),
                label.text_range().end().into(),
            )
        })
        .unwrap_or_else(|| span_for_range(source, field.syntax().text_range()));
    Ok(ParamDefaultRecordField {
        name,
        span,
        value: field.expression(),
    })
}

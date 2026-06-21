use std::collections::BTreeSet;

use vela_common::{Diagnostic, Span};
use vela_syntax::ast::Argument;

use crate::Register;

use super::body_payloads::{
    CompilerArgumentPayload, CompilerExpressionPayload, CompilerRecordFieldPayload,
};
use super::const_eval::evaluate_syntax_const_expr;
use super::patterns::tuple_variant_field_name;
use super::schema_defaults::{
    ConstructorShape, SchemaFieldDefault, resolve_tuple_constructor_arguments,
    tuple_constructor_diagnostics, unknown_enum_variant_diagnostic,
};
use super::value_types::{
    RuntimeTypeFact, StaticExprType, TypeContractContext, check_expected_type,
};
use super::{CompileError, CompileErrorKind, CompileResult, Compiler};

pub(super) fn record_field_names(
    fields: &[vela_syntax::ast::RecordField],
    payloads: Option<&[CompilerRecordFieldPayload<'_>]>,
) -> Option<Vec<Option<String>>> {
    payloads?;
    Some(
        fields
            .iter()
            .enumerate()
            .map(|(index, field)| {
                payloads
                    .and_then(|payloads| payloads.get(index))
                    .and_then(CompilerRecordFieldPayload::syntax_label_name)
                    .or_else(|| Some(field.name.clone()))
            })
            .collect(),
    )
}

impl<'ast, 'registry> Compiler<'ast, 'registry> {
    pub(super) fn compile_tuple_variant_fields(
        &mut self,
        constructor_span: Span,
        enum_name: &str,
        variant: &str,
        args: &[Argument],
        arg_payloads: Option<&[CompilerArgumentPayload<'_>]>,
    ) -> CompileResult<Vec<(String, Register)>> {
        if !self.enum_constructor_variant_exists(enum_name, variant) {
            return Err(
                self.constructor_diagnostics_error(vec![unknown_enum_variant_diagnostic(
                    enum_name,
                    variant,
                    constructor_span,
                )]),
            );
        }
        let shape = self.enum_constructor_shape(enum_name, variant);
        let arg_names = argument_names(args, arg_payloads);
        self.reject_constructor_diagnostics(tuple_constructor_diagnostics(
            enum_name,
            variant,
            shape.as_ref(),
            args,
            arg_names.as_deref(),
            constructor_span,
        ))?;
        let mut fields = Vec::new();
        let mut explicit_names = BTreeSet::new();
        if let Some(shape) = shape.as_ref() {
            let owner = format!("{enum_name}::{variant}");
            let slots = resolve_tuple_constructor_arguments(
                shape,
                &owner,
                args,
                arg_names.as_deref(),
                constructor_span,
            )
            .map_err(|diagnostics| self.constructor_diagnostics_error(diagnostics))?;
            for (index, arg) in slots.into_iter().enumerate() {
                let Some(arg) = arg else {
                    continue;
                };
                let name = shape
                    .field_name_at(index)
                    .map(str::to_owned)
                    .unwrap_or_else(|| tuple_variant_field_name(index));
                let payload = argument_expression_payload(args, arg_payloads, arg);
                let value = self.compile_constructor_value(
                    &arg.value,
                    &name,
                    shape.field_value_type_at(index),
                    payload,
                )?;
                explicit_names.insert(name.clone());
                fields.push((name, value));
            }
        } else {
            for (index, arg) in args.iter().enumerate() {
                if argument_name(args, arg_payloads, arg).is_some() {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "named tuple variant argument",
                    )));
                }
                let name = tuple_variant_field_name(index);
                let payload = arg_payloads
                    .and_then(|payloads| payloads.get(index))
                    .map(CompilerArgumentPayload::value_expression_payload);
                let value = self.compile_expr_with_payload(&arg.value, payload.as_ref())?;
                explicit_names.insert(name.clone());
                fields.push((name, value));
            }
        }
        let defaults = schema_default_fields(shape.as_ref());
        self.compile_schema_default_fields(&mut fields, &explicit_names, defaults, shape.as_ref())?;
        Ok(fields)
    }

    pub(super) fn compile_record_fields(
        &mut self,
        fields: &[vela_syntax::ast::RecordField],
        defaults: Vec<SchemaFieldDefault>,
        shape: Option<&ConstructorShape>,
        payloads: Option<&[CompilerRecordFieldPayload<'_>]>,
    ) -> CompileResult<Vec<(String, Register)>> {
        let mut compiled = Vec::new();
        let mut explicit_names = BTreeSet::new();
        for (index, field) in fields.iter().enumerate() {
            let payload = payloads.and_then(|payloads| payloads.get(index));
            let field_name = payload
                .and_then(CompilerRecordFieldPayload::syntax_label_name)
                .unwrap_or_else(|| field.name.clone());
            explicit_names.insert(field_name.clone());
            compiled.push(self.compile_record_field(
                field,
                &field_name,
                shape.and_then(|shape| shape.field_value_type(&field_name)),
                payload,
            )?);
        }
        self.compile_schema_default_fields(&mut compiled, &explicit_names, defaults, shape)?;
        Ok(compiled)
    }

    pub(super) fn record_constructor_shape(&self, type_name: &str) -> Option<ConstructorShape> {
        self.facts.schema_defaults.record(type_name).cloned()
    }

    pub(super) fn enum_constructor_shape(
        &self,
        type_name: &str,
        variant: &str,
    ) -> Option<ConstructorShape> {
        self.facts
            .schema_defaults
            .enum_variant(type_name, variant)
            .cloned()
    }

    pub(super) fn enum_constructor_variant_exists(&self, type_name: &str, variant: &str) -> bool {
        self.facts
            .schema_defaults
            .enum_contains_variant(type_name, variant)
    }

    pub(super) fn reject_constructor_diagnostics(
        &self,
        diagnostics: Vec<Diagnostic>,
    ) -> CompileResult<()> {
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(self.constructor_diagnostics_error(diagnostics))
        }
    }

    pub(super) fn constructor_diagnostics_error(
        &self,
        diagnostics: Vec<Diagnostic>,
    ) -> CompileError {
        CompileError::new(CompileErrorKind::SemanticDiagnostics(diagnostics))
    }

    fn compile_record_field(
        &mut self,
        field: &vela_syntax::ast::RecordField,
        field_name: &str,
        expected: Option<RuntimeTypeFact>,
        payload: Option<&CompilerRecordFieldPayload<'_>>,
    ) -> CompileResult<(String, Register)> {
        let value = if let Some(value) = &field.value {
            self.compile_constructor_value(
                value,
                field_name,
                expected,
                payload.and_then(CompilerRecordFieldPayload::value_expression_payload),
            )?
        } else {
            self.local_register_at_span(field.span, field_name)?
        };
        Ok((field_name.to_owned(), value))
    }

    fn compile_constructor_value(
        &mut self,
        value: &vela_syntax::ast::Expr,
        field_name: &str,
        expected: Option<RuntimeTypeFact>,
        payload: Option<CompilerExpressionPayload<'_>>,
    ) -> CompileResult<Register> {
        match expected {
            Some(expected) => self.compile_expr_with_expected_type_and_payload(
                value,
                expected,
                TypeContractContext::Field {
                    name: field_name.to_owned(),
                },
                payload.as_ref(),
            ),
            None => self.compile_expr_with_payload(value, payload.as_ref()),
        }
    }

    pub(super) fn compile_schema_default_fields(
        &mut self,
        fields: &mut Vec<(String, Register)>,
        explicit_names: &BTreeSet<String>,
        defaults: Vec<SchemaFieldDefault>,
        shape: Option<&ConstructorShape>,
    ) -> CompileResult<()> {
        for default in defaults {
            if explicit_names.contains(&default.name) {
                continue;
            }
            let value = self.compile_schema_field_default(
                &default,
                shape.and_then(|shape| shape.field_value_type(&default.name)),
            )?;
            fields.push((default.name, value));
        }
        Ok(())
    }

    fn compile_schema_field_default(
        &mut self,
        default: &SchemaFieldDefault,
        expected: Option<RuntimeTypeFact>,
    ) -> CompileResult<Register> {
        if let Some(value) = evaluate_syntax_const_expr(
            default.value.source(),
            default.value.syntax(),
            &default.constants,
        )? {
            if let Some(expected) = expected {
                check_expected_type(
                    static_type_for_constant(&value),
                    expected,
                    default.value.span(),
                    TypeContractContext::Field {
                        name: default.name.clone(),
                    },
                )?;
            }
            return self.emit_constant(value);
        }
        Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "non-constant CST schema default expression",
        ))
        .with_span(default.value.span()))
    }
}

fn argument_expression_payload<'ast>(
    args: &[Argument],
    arg_payloads: Option<&[CompilerArgumentPayload<'ast>]>,
    arg: &Argument,
) -> Option<CompilerExpressionPayload<'ast>> {
    let index = args
        .iter()
        .position(|candidate| std::ptr::eq(candidate, arg))?;
    arg_payloads?
        .get(index)
        .map(CompilerArgumentPayload::value_expression_payload)
}

fn argument_names(
    args: &[Argument],
    arg_payloads: Option<&[CompilerArgumentPayload<'_>]>,
) -> Option<Vec<Option<String>>> {
    arg_payloads.map(|_| {
        args.iter()
            .map(|arg| argument_name(args, arg_payloads, arg))
            .collect()
    })
}

fn argument_name(
    args: &[Argument],
    arg_payloads: Option<&[CompilerArgumentPayload<'_>]>,
    arg: &Argument,
) -> Option<String> {
    let Some(arg_payloads) = arg_payloads else {
        return arg.name.clone();
    };
    let index = args
        .iter()
        .position(|candidate| std::ptr::eq(candidate, arg))?;
    arg_payloads
        .get(index)
        .and_then(CompilerArgumentPayload::syntax_name)
}

pub(super) fn schema_default_fields(shape: Option<&ConstructorShape>) -> Vec<SchemaFieldDefault> {
    shape.map_or_else(Vec::new, ConstructorShape::default_fields)
}

fn static_type_for_constant(value: &crate::Constant) -> StaticExprType {
    let Some(fact) = runtime_type_for_constant(value) else {
        return StaticExprType::Dynamic;
    };
    StaticExprType::Exact(fact)
}

fn runtime_type_for_constant(value: &crate::Constant) -> Option<RuntimeTypeFact> {
    match value {
        crate::Constant::Null => Some(RuntimeTypeFact::primitive(vela_common::PrimitiveTag::Null)),
        crate::Constant::Bool(_) => {
            Some(RuntimeTypeFact::primitive(vela_common::PrimitiveTag::Bool))
        }
        crate::Constant::Char(_) => {
            Some(RuntimeTypeFact::primitive(vela_common::PrimitiveTag::Char))
        }
        crate::Constant::Scalar(value) => Some(RuntimeTypeFact::primitive(value.primitive_tag())),
        crate::Constant::String(_) => Some(RuntimeTypeFact::primitive(
            vela_common::PrimitiveTag::String,
        )),
        crate::Constant::Bytes(_) => {
            Some(RuntimeTypeFact::primitive(vela_common::PrimitiveTag::Bytes))
        }
        crate::Constant::Array(_) | crate::Constant::Map(_) => None,
    }
}

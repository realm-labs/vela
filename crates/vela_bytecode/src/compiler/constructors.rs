use std::collections::BTreeSet;

use vela_common::{Diagnostic, Span};
use vela_syntax::ast::Argument;

use crate::Register;

use super::const_eval::evaluate_const_expr;
use super::patterns::tuple_variant_field_name;
use super::schema_defaults::{
    ConstructorShape, SchemaFieldDefault, resolve_tuple_constructor_arguments,
    tuple_constructor_diagnostics, unknown_enum_variant_diagnostic,
};
use super::{CompileError, CompileErrorKind, CompileResult, Compiler};

impl Compiler<'_> {
    pub(super) fn compile_tuple_variant_fields(
        &mut self,
        constructor_span: Span,
        enum_name: &str,
        variant: &str,
        args: &[Argument],
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
        self.reject_constructor_diagnostics(tuple_constructor_diagnostics(
            enum_name,
            variant,
            shape.as_ref(),
            args,
            constructor_span,
        ))?;
        let mut fields = Vec::new();
        let mut explicit_names = BTreeSet::new();
        if let Some(shape) = shape.as_ref() {
            let owner = format!("{enum_name}.{variant}");
            let slots = resolve_tuple_constructor_arguments(shape, &owner, args, constructor_span)
                .map_err(|diagnostics| self.constructor_diagnostics_error(diagnostics))?;
            for (index, arg) in slots.into_iter().enumerate() {
                let Some(arg) = arg else {
                    continue;
                };
                let name = shape
                    .field_name_at(index)
                    .map(str::to_owned)
                    .unwrap_or_else(|| tuple_variant_field_name(index));
                let value = self.compile_expr(&arg.value)?;
                explicit_names.insert(name.clone());
                fields.push((name, value));
            }
        } else {
            for (index, arg) in args.iter().enumerate() {
                if arg.name.is_some() {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "named tuple variant argument",
                    )));
                }
                let name = tuple_variant_field_name(index);
                let value = self.compile_expr(&arg.value)?;
                explicit_names.insert(name.clone());
                fields.push((name, value));
            }
        }
        let defaults = schema_default_fields(shape.as_ref());
        self.compile_schema_default_fields(&mut fields, &explicit_names, defaults)?;
        Ok(fields)
    }

    pub(super) fn compile_record_fields(
        &mut self,
        fields: &[vela_syntax::ast::RecordField],
        defaults: Vec<SchemaFieldDefault>,
    ) -> CompileResult<Vec<(String, Register)>> {
        let mut compiled = Vec::new();
        let mut explicit_names = BTreeSet::new();
        for field in fields {
            explicit_names.insert(field.name.clone());
            compiled.push(self.compile_record_field(field)?);
        }
        self.compile_schema_default_fields(&mut compiled, &explicit_names, defaults)?;
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
    ) -> CompileResult<(String, Register)> {
        let value = if let Some(value) = &field.value {
            self.compile_expr(value)?
        } else {
            self.local_register_at_span(field.span, &field.name)?
        };
        Ok((field.name.clone(), value))
    }

    fn compile_schema_default_fields(
        &mut self,
        fields: &mut Vec<(String, Register)>,
        explicit_names: &BTreeSet<String>,
        defaults: Vec<SchemaFieldDefault>,
    ) -> CompileResult<()> {
        for default in defaults {
            if explicit_names.contains(&default.name) {
                continue;
            }
            let value = self.compile_schema_field_default(&default)?;
            fields.push((default.name, value));
        }
        Ok(())
    }

    fn compile_schema_field_default(
        &mut self,
        default: &SchemaFieldDefault,
    ) -> CompileResult<Register> {
        if let Some(value) = evaluate_const_expr(&default.value, &default.constants)? {
            return self.emit_constant(value);
        }
        self.compile_expr(&default.value)
    }
}

pub(super) fn schema_default_fields(shape: Option<&ConstructorShape>) -> Vec<SchemaFieldDefault> {
    shape.map_or_else(Vec::new, |shape| shape.defaults().cloned().collect())
}

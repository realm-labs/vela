use std::collections::BTreeSet;

use vela_common::{SourceId, Span};
use vela_syntax::ast::{SyntaxArgument, SyntaxExpression};

use crate::compiler::body_payloads::expression_syntax_literal;
use crate::compiler::call_args::SyntaxCallArgument;
use crate::compiler::const_eval::compile_literal_constant_for_type;
use crate::compiler::constructors::schema_default_fields;
use crate::compiler::control_flow::syntax_statement_values::syntax_expression_span;
use crate::compiler::expected_exprs::guard_location_and_name;
use crate::compiler::patterns::tuple_variant_field_name;
use crate::compiler::schema_defaults::{
    resolve_syntax_tuple_constructor_arguments, syntax_tuple_constructor_diagnostics,
    unknown_enum_variant_diagnostic,
};
use crate::compiler::value_types::{
    ExpectedTypeOutcome, RuntimeTypeFact, StaticExprType, TypeContractContext, check_expected_type,
};
use crate::compiler::{CompileError, CompileErrorKind, CompileResult, Compiler};
use crate::{
    GuardKind, Register, UnlinkedGuardContext, UnlinkedInstructionKind, UnlinkedTypeGuard,
};

impl Compiler<'_, '_> {
    pub(in crate::compiler::control_flow) fn compile_syntax_tuple_variant_fields(
        &mut self,
        source: SourceId,
        constructor_span: Span,
        enum_name: &str,
        variant: &str,
        arguments: &[SyntaxArgument],
    ) -> CompileResult<Option<Vec<(String, Register)>>> {
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
        let Some(syntax_args) = syntax_constructor_arguments(source, arguments) else {
            return Ok(None);
        };
        self.reject_constructor_diagnostics(syntax_tuple_constructor_diagnostics(
            enum_name,
            variant,
            shape.as_ref(),
            &syntax_args,
            constructor_span,
        ))?;

        let mut fields = Vec::new();
        let mut explicit_names = BTreeSet::new();
        if let Some(shape) = shape.as_ref() {
            let owner = format!("{enum_name}::{variant}");
            let slots = resolve_syntax_tuple_constructor_arguments(
                shape,
                &owner,
                &syntax_args,
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
                let Some(value) = self.compile_syntax_constructor_value(
                    source,
                    &arg.value,
                    shape.field_value_type_at(index),
                    &name,
                )?
                else {
                    return Ok(None);
                };
                explicit_names.insert(name.clone());
                fields.push((name, value));
            }
        } else {
            for (index, argument) in arguments.iter().enumerate() {
                if argument.name_text().is_some() {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "named tuple variant argument",
                    )));
                }
                let Some(expression) = argument.expression() else {
                    return Ok(None);
                };
                let name = tuple_variant_field_name(index);
                let Some(value) =
                    self.compile_syntax_constructor_value(source, &expression, None, &name)?
                else {
                    return Ok(None);
                };
                explicit_names.insert(name.clone());
                fields.push((name, value));
            }
        }
        let defaults = schema_default_fields(shape.as_ref());
        self.compile_schema_default_fields(&mut fields, &explicit_names, defaults, shape.as_ref())?;
        Ok(Some(fields))
    }

    fn compile_syntax_constructor_value(
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
        let static_type = self
            .syntax_value_type_for_expression(Some(source), expression)
            .map(StaticExprType::Exact)
            .unwrap_or(StaticExprType::Dynamic);
        let outcome = check_expected_type(static_type, expected, span, context.clone())?;
        if let ExpectedTypeOutcome::Contextualized(RuntimeTypeFact::Primitive(tag)) = &outcome
            && let Some(literal) = expression_syntax_literal(expression)
            && let Some(constant) = compile_literal_constant_for_type(&literal, *tag)
                .map_err(|error| error.with_span(syntax_expression_span(source, expression)))?
        {
            return self.emit_constant(constant).map(Some);
        }
        let Some(value) = self.compile_syntax_expression(source, expression)? else {
            return Ok(None);
        };
        if let ExpectedTypeOutcome::RequiresRuntimeGuard(expected) = &outcome
            && let Some((location, name)) = guard_location_and_name(context)
            && let Some(plan) = crate::compiler::type_guard_plan_for_runtime_type(expected)
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

fn syntax_constructor_arguments(
    source: SourceId,
    arguments: &[SyntaxArgument],
) -> Option<Vec<SyntaxCallArgument>> {
    arguments
        .iter()
        .map(|argument| {
            let value = argument.expression()?;
            Some(SyntaxCallArgument {
                name: argument.name_text(),
                span: syntax_expression_span(source, &value),
                value,
            })
        })
        .collect()
}

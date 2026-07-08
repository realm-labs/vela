use std::collections::BTreeSet;

use vela_common::Diagnostic;

use crate::Register;

use super::const_eval::evaluate_syntax_const_expr;
use super::schema_defaults::{ConstructorShape, SchemaFieldDefault};
use super::value_types::{
    RuntimeTypeFact, StaticExprType, TypeContractContext, check_expected_type,
};
use super::{CompileError, CompileErrorKind, CompileResult, Compiler};

impl<'ast, 'registry> Compiler<'ast, 'registry> {
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

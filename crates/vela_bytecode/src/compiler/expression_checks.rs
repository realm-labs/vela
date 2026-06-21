use vela_common::{Diagnostic, Span};
use vela_def::MethodId;
use vela_syntax::ast::{BinaryOp, Expr};

use super::body_payloads::CompilerExpressionPayload;
use super::record_shapes::ValueShape;
use super::value_types::{RuntimeTypeFact, StandardRuntimeType};
use super::{CompileError, CompileErrorKind, CompileResult, Compiler};

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn reject_static_identity_comparison_operands(
        &self,
        op: BinaryOp,
        span: Span,
        left: &Expr,
        right: &Expr,
        left_payload: Option<&CompilerExpressionPayload<'_>>,
        right_payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> CompileResult<()> {
        if !matches!(op, BinaryOp::IdentityEqual | BinaryOp::IdentityNotEqual) {
            return Ok(());
        }
        for (side, expr, payload) in [
            ("left", left, left_payload),
            ("right", right, right_payload),
        ] {
            if let Some(type_name) = self.static_non_identity_operand_type(expr, payload) {
                return Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
                    vec![
                        Diagnostic::error(format!(
                            "`{}` requires reference identity operands, but the {side} operand has type `{type_name}`",
                            binary_op_source_name(op)
                        ))
                        .with_code("compiler::invalid_identity_comparison")
                        .with_span(span)
                        .with_label(span, "identity comparison requires reference operands")
                        .with_label(
                            expr.span,
                            format!("{side} operand is statically `{type_name}`"),
                        ),
                    ],
                )));
            }
        }
        Ok(())
    }

    pub(in crate::compiler) fn reject_static_comparison_without_trait(
        &self,
        op: BinaryOp,
        span: Span,
        left: &Expr,
        left_payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> CompileResult<()> {
        let Some(requirement) = ComparisonTraitRequirement::for_op(op) else {
            return Ok(());
        };
        let Some(type_name) = self.script_type_for_expr_with_payload(left, left_payload) else {
            return Ok(());
        };
        if !self.is_declared_script_type(&type_name)
            || self.type_implements_builtin_trait_method(
                &type_name,
                requirement.trait_name,
                requirement.method_name,
            )
        {
            return Ok(());
        }
        Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
            vec![
                Diagnostic::error(format!(
                    "`{type_name}` does not implement `{}` for `{}`",
                    requirement.trait_name, requirement.operator
                ))
                .with_code("compiler::missing_comparison_trait")
                .with_span(span)
                .with_label(
                    span,
                    format!(
                        "static `{}` comparison requires `{}`",
                        requirement.operator, requirement.trait_name
                    ),
                )
                .with_label(
                    span,
                    format!(
                        "add `impl {} for {type_name}` or make the value dynamic",
                        requirement.trait_name
                    ),
                ),
            ],
        )))
    }

    pub(super) fn is_declared_script_type(&self, type_name: &str) -> bool {
        self.facts
            .type_symbols
            .values()
            .any(|known| known == type_name)
    }

    pub(super) fn type_implements_builtin_trait_method(
        &self,
        type_name: &str,
        trait_name: &str,
        method_name: &str,
    ) -> bool {
        self.script_method_id_for_type(type_name, method_name)
            == Some(builtin_trait_method_id(trait_name, method_name))
            || self
                .facts
                .derived_operator_traits
                .get(type_name)
                .is_some_and(|traits| traits.contains(trait_name))
    }

    fn static_non_identity_operand_type(
        &self,
        expr: &Expr,
        payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> Option<String> {
        if let Some(fact) = self.value_type_for_expr_with_payload(expr, payload) {
            return (!runtime_type_is_identity_operand(&fact)).then(|| fact.source_type_display());
        }
        if let Some(shape) = self.value_shape_for_expr_with_payload(expr, payload) {
            return non_identity_shape_type(&shape);
        }
        None
    }
}

fn runtime_type_is_identity_operand(fact: &RuntimeTypeFact) -> bool {
    match fact {
        RuntimeTypeFact::Primitive(_) | RuntimeTypeFact::Standard(StandardRuntimeType::Range) => {
            false
        }
        RuntimeTypeFact::Standard(
            StandardRuntimeType::Array
            | StandardRuntimeType::Map
            | StandardRuntimeType::Set
            | StandardRuntimeType::Function
            | StandardRuntimeType::Closure
            | StandardRuntimeType::Iterator
            | StandardRuntimeType::Option
            | StandardRuntimeType::Result,
        )
        | RuntimeTypeFact::Array(_)
        | RuntimeTypeFact::Map { .. }
        | RuntimeTypeFact::Set(_)
        | RuntimeTypeFact::Iterator(_)
        | RuntimeTypeFact::Option(_)
        | RuntimeTypeFact::Result { .. } => true,
    }
}

fn non_identity_shape_type(shape: &ValueShape) -> Option<String> {
    match shape {
        ValueShape::Scalar(type_name) => Some(type_name.clone()),
        ValueShape::Unknown
        | ValueShape::Record(_)
        | ValueShape::Array(_)
        | ValueShape::Iterator(_)
        | ValueShape::Map { .. }
        | ValueShape::Set(_)
        | ValueShape::Option(_)
        | ValueShape::Result { .. } => None,
    }
}

struct ComparisonTraitRequirement {
    trait_name: &'static str,
    method_name: &'static str,
    operator: &'static str,
}

impl ComparisonTraitRequirement {
    fn for_op(op: BinaryOp) -> Option<Self> {
        match op {
            BinaryOp::Equal | BinaryOp::NotEqual => Some(Self {
                trait_name: "PartialEq",
                method_name: "eq",
                operator: binary_op_source_name(op),
            }),
            BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => {
                Some(Self {
                    trait_name: "PartialOrd",
                    method_name: "partial_cmp",
                    operator: binary_op_source_name(op),
                })
            }
            BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Rem
            | BinaryOp::Range
            | BinaryOp::RangeInclusive
            | BinaryOp::Or
            | BinaryOp::And
            | BinaryOp::IdentityEqual
            | BinaryOp::IdentityNotEqual => None,
        }
    }
}

fn binary_op_source_name(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Rem => "%",
        BinaryOp::Equal => "==",
        BinaryOp::NotEqual => "!=",
        BinaryOp::IdentityEqual => "===",
        BinaryOp::IdentityNotEqual => "!==",
        BinaryOp::Less => "<",
        BinaryOp::LessEqual => "<=",
        BinaryOp::Greater => ">",
        BinaryOp::GreaterEqual => ">=",
        BinaryOp::Range => "..",
        BinaryOp::RangeInclusive => "..=",
        BinaryOp::Or => "||",
        BinaryOp::And => "&&",
    }
}

fn builtin_trait_method_id(trait_name: &str, method_name: &str) -> MethodId {
    MethodId::new(u128::from(vela_common::stable_id(
        "trait_method",
        trait_name,
        method_name,
    )))
}

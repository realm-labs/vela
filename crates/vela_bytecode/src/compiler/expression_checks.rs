use vela_common::{Diagnostic, Span};
use vela_def::script_trait_method_id;
use vela_hir::body::HirBinaryOp;

use super::{CompileError, CompileErrorKind, CompileResult, Compiler};

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn reject_static_script_path_binary_operands(
        &self,
        op: HirBinaryOp,
        span: Span,
        left_type_name: Option<&str>,
        right_type_name: Option<&str>,
    ) -> CompileResult<()> {
        if matches!(
            op,
            HirBinaryOp::IdentityEqual | HirBinaryOp::IdentityNotEqual
        ) {
            for (side, type_name) in [("left", left_type_name), ("right", right_type_name)] {
                let Some(type_name) = type_name else {
                    continue;
                };
                if self.is_declared_script_type(type_name) {
                    return Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
                        vec![
                            Diagnostic::error(format!(
                                "`{}` requires reference identity operands, but the {side} operand has type `{type_name}`",
                                binary_op_source_name(op)
                            ))
                            .with_code("compiler::invalid_identity_comparison")
                            .with_span(span)
                            .with_label(span, "identity comparison requires reference operands"),
                        ],
                    )));
                }
            }
        }

        let Some(requirement) = ComparisonTraitRequirement::for_op(op) else {
            return Ok(());
        };
        let Some(type_name) = left_type_name else {
            return Ok(());
        };
        if !self.is_declared_script_type(type_name)
            || self.type_implements_builtin_trait_method(
                type_name,
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
            == Some(script_trait_method_id(trait_name, method_name))
            || self
                .facts
                .derived_operator_traits
                .get(type_name)
                .is_some_and(|traits| traits.contains(trait_name))
    }
}

struct ComparisonTraitRequirement {
    trait_name: &'static str,
    method_name: &'static str,
    operator: &'static str,
}

impl ComparisonTraitRequirement {
    fn for_op(op: HirBinaryOp) -> Option<Self> {
        match op {
            HirBinaryOp::Equal | HirBinaryOp::NotEqual => Some(Self {
                trait_name: "PartialEq",
                method_name: "eq",
                operator: binary_op_source_name(op),
            }),
            HirBinaryOp::Less
            | HirBinaryOp::LessEqual
            | HirBinaryOp::Greater
            | HirBinaryOp::GreaterEqual => Some(Self {
                trait_name: "PartialOrd",
                method_name: "partial_cmp",
                operator: binary_op_source_name(op),
            }),
            HirBinaryOp::Add
            | HirBinaryOp::Sub
            | HirBinaryOp::Mul
            | HirBinaryOp::Div
            | HirBinaryOp::Rem
            | HirBinaryOp::Range
            | HirBinaryOp::RangeInclusive
            | HirBinaryOp::Or
            | HirBinaryOp::And
            | HirBinaryOp::IdentityEqual
            | HirBinaryOp::IdentityNotEqual => None,
        }
    }
}

fn binary_op_source_name(op: HirBinaryOp) -> &'static str {
    match op {
        HirBinaryOp::Add => "+",
        HirBinaryOp::Sub => "-",
        HirBinaryOp::Mul => "*",
        HirBinaryOp::Div => "/",
        HirBinaryOp::Rem => "%",
        HirBinaryOp::Equal => "==",
        HirBinaryOp::NotEqual => "!=",
        HirBinaryOp::IdentityEqual => "===",
        HirBinaryOp::IdentityNotEqual => "!==",
        HirBinaryOp::Less => "<",
        HirBinaryOp::LessEqual => "<=",
        HirBinaryOp::Greater => ">",
        HirBinaryOp::GreaterEqual => ">=",
        HirBinaryOp::Range => "..",
        HirBinaryOp::RangeInclusive => "..=",
        HirBinaryOp::Or => "||",
        HirBinaryOp::And => "&&",
    }
}

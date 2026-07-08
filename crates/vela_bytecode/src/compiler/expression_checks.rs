use vela_common::{Diagnostic, Span};
use vela_def::MethodId;
use vela_syntax::ast::BinaryOp;

use super::{CompileError, CompileErrorKind, CompileResult, Compiler};

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn reject_static_script_path_binary_operands(
        &self,
        op: BinaryOp,
        span: Span,
        left_type_name: Option<&str>,
        right_type_name: Option<&str>,
    ) -> CompileResult<()> {
        if matches!(op, BinaryOp::IdentityEqual | BinaryOp::IdentityNotEqual) {
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
            == Some(builtin_trait_method_id(trait_name, method_name))
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

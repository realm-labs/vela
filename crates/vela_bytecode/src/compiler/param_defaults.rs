use vela_hir::body::HirBodyRoot;
use vela_hir::ids::HirBodyId;
use vela_hir::type_hint::FunctionSignature;

use crate::{Constant, Register};

use super::value_types::{RuntimeTypeFact, TypeContractContext, type_hint_value_type};
use super::{CompileError, CompileErrorKind, CompileResult, Compiler};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ParamDefaultValue {
    pub(super) body: HirBodyId,
    pub(super) expected: Option<RuntimeTypeFact>,
    pub(super) name: String,
}

pub(super) fn param_default_values(
    body: &vela_hir::body::HirBody,
    signature: &FunctionSignature,
) -> Vec<Option<ParamDefaultValue>> {
    body.params
        .iter()
        .zip(&signature.params)
        .map(|(param, hint)| {
            Some(ParamDefaultValue {
                body: param.default_body?,
                expected: hint.type_hint.as_ref().and_then(type_hint_value_type),
                name: hint.name.clone(),
            })
        })
        .collect()
}

impl Compiler<'_, '_> {
    pub(super) fn compile_param_default_value(
        &mut self,
        default: &ParamDefaultValue,
    ) -> CompileResult<Register> {
        let root = self
            .hir_bodies
            .iter()
            .find(|body| body.id == default.body)
            .map(|body| body.root)
            .ok_or_else(|| {
                CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "parameter default body",
                ))
            })?;
        match root {
            HirBodyRoot::Expr(expression) => {
                if let Some(expected) = default.expected.clone() {
                    return self
                        .compile_hir_expression_for_expected_type(
                            expression,
                            expected,
                            TypeContractContext::FunctionParameter {
                                name: default.name.clone(),
                            },
                            &[],
                        )
                        .map(|(value, _)| value);
                }
                self.compile_hir_expression(expression)
            }
            HirBodyRoot::Block(block) => {
                let dst = self.alloc_register()?;
                self.compile_hir_block_value_to(block, dst)?;
                Ok(dst)
            }
            HirBodyRoot::Empty => self.emit_constant(Constant::Unit),
        }
    }
}

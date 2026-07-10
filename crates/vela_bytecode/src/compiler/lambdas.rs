use vela_common::Span;
use vela_hir::body::{HirBody, HirBodyOwner};
use vela_hir::ids::{HirExprId, HirLocalId};

use crate::{Register, UnlinkedInstructionKind};

use super::record_shapes::ValueShape;
use super::{
    CompileError, CompileErrorKind, CompileResult, Compiler, CompilerHirContext,
    LambdaCompilerInput,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LambdaCapture {
    pub local: HirLocalId,
    pub name: String,
    pub register: Register,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LambdaParam {
    pub local: HirLocalId,
    pub name: String,
    pub span: Span,
}

impl<'ast> Compiler<'ast, '_> {
    pub(in crate::compiler) fn compile_hir_lambda(
        &mut self,
        expression: HirExprId,
        callback_shapes: &[Option<ValueShape>],
    ) -> CompileResult<Register> {
        let hir_body = self
            .hir_bodies
            .iter()
            .copied()
            .find(|body| {
                matches!(
                    body.owner,
                    HirBodyOwner::Lambda {
                        expression: lambda_expression,
                        ..
                    } if lambda_expression == expression
                )
            })
            .ok_or_else(|| {
                CompileError::new(CompileErrorKind::UnsupportedSyntax("lambda HIR body"))
            })?;
        let lambda_span = hir_body.origin.span;
        let params = self.lambda_params_from_hir(hir_body)?;
        let captures = self.lambda_captures_from_hir(hir_body)?;
        let capture_registers = captures
            .iter()
            .map(|capture| capture.register)
            .collect::<Vec<_>>();
        let mut lambda_compiler = Compiler::new_lambda(
            LambdaCompilerInput {
                name: format!("{}::<lambda@{}>", self.code.name, lambda_span.start),
                function: self.function,
                root_origin: vela_mir::MirSourceOrigin::body(hir_body.id, lambda_span),
                params: &params,
                body: hir_body.id,
                captures: &captures,
            },
            CompilerHirContext {
                bindings: self.bindings,
                bodies: self.hir_bodies.clone(),
            },
            self.facts.clone(),
        )?;
        for capture in &captures {
            if let Some(script_fact) = self.script_types.local_fact(capture.local) {
                lambda_compiler.script_types.set_local_fact(
                    capture.local,
                    &capture.name,
                    Some(script_fact),
                );
            }
            if let Some(value_type) = self.value_types.local(capture.local) {
                lambda_compiler.value_types.set_local(
                    capture.local,
                    &capture.name,
                    Some(value_type),
                );
            }
            if let Some(value_shape) = self.value_shapes.local(capture.local) {
                lambda_compiler.value_shapes.set_local(
                    capture.local,
                    &capture.name,
                    Some(value_shape),
                );
            }
        }
        for (index, shape) in callback_shapes.iter().enumerate() {
            let (Some(shape), Some(param)) = (shape, params.get(index)) else {
                continue;
            };
            lambda_compiler
                .value_types
                .set_local(param.local, &param.name, shape.value_type());
            lambda_compiler
                .value_shapes
                .set_local(param.local, &param.name, Some(shape.clone()));
        }
        let code = lambda_compiler.compile_hir_value_body(hir_body.id)?;
        let function = self.code.push_nested_function(code);
        let dst = self.alloc_register()?;
        self.emit(UnlinkedInstructionKind::MakeClosure {
            dst,
            function,
            captures: capture_registers,
        });
        Ok(dst)
    }

    pub(in crate::compiler) fn lambda_params_from_hir(
        &self,
        body: &HirBody,
    ) -> CompileResult<Vec<LambdaParam>> {
        body.params
            .iter()
            .map(|param| {
                let local = self.bindings.local(param.local).ok_or_else(|| {
                    CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "lambda parameter local",
                    ))
                    .with_span(param.origin.span)
                })?;
                Ok(LambdaParam {
                    local: param.local,
                    name: local.name.clone(),
                    span: param.origin.span,
                })
            })
            .collect()
    }

    fn lambda_captures_from_hir(&self, body: &HirBody) -> CompileResult<Vec<LambdaCapture>> {
        body.captures
            .iter()
            .map(|capture| {
                let local = self.bindings.local(capture.local).ok_or_else(|| {
                    CompileError::new(CompileErrorKind::UnsupportedSyntax("lambda capture local"))
                        .with_span(body.origin.span)
                })?;
                let register = self
                    .hir_locals
                    .get(&capture.local)
                    .copied()
                    .ok_or_else(|| {
                        CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "lambda capture register",
                        ))
                        .with_span(body.origin.span)
                    })?;
                Ok(LambdaCapture {
                    local: capture.local,
                    name: local.name.clone(),
                    register,
                })
            })
            .collect()
    }
}

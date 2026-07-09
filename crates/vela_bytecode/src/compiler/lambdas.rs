use vela_common::{SourceId, Span};
use vela_hir::body::{HirBody, HirBodyOwner};
use vela_hir::ids::HirLocalId;
use vela_syntax::ast::{AstNode, SyntaxExpression, SyntaxLambdaBody};

use crate::{Register, UnlinkedCodeObject, UnlinkedInstructionKind};

use super::body_payloads::CompilerBodyPayload;
use super::record_shapes::ValueShape;
use super::{CompileError, CompileErrorKind, CompileResult, Compiler, CompilerHirContext};

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
    pub(in crate::compiler) fn compile_syntax_lambda_with_callback_shapes(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        callback_shapes: &[Option<ValueShape>],
    ) -> CompileResult<Option<Register>> {
        let Some(lambda) = expression.as_lambda() else {
            return Ok(None);
        };
        let Some(body) = lambda.body() else {
            return Ok(None);
        };
        let lambda_span = syntax_expr_span(source, expression);
        let hir_body = self.hir_lambda_body(lambda_span)?;
        let params = self.lambda_params_from_hir(hir_body)?;
        let captures = self.lambda_captures_from_hir(hir_body)?;
        let capture_registers = captures
            .iter()
            .map(|capture| capture.register)
            .collect::<Vec<_>>();
        let mut lambda_compiler = Compiler::new_lambda(
            format!("{}::<lambda@{}>", self.code.name, lambda_span.start),
            lambda_span,
            &params,
            self.body.clone(),
            &captures,
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
            let Some(shape) = shape else {
                continue;
            };
            let Some(param) = params.get(index) else {
                continue;
            };
            lambda_compiler
                .value_types
                .set_local(param.local, &param.name, shape.value_type());
            lambda_compiler
                .value_shapes
                .set_local(param.local, &param.name, Some(shape.clone()));
        }
        let code = lambda_compiler.compile_syntax_lambda_body(source, body, hir_body)?;
        let function = self.code.push_nested_function(code);
        let dst = self.alloc_register()?;
        self.emit(UnlinkedInstructionKind::MakeClosure {
            dst,
            function,
            captures: capture_registers,
        });
        Ok(Some(dst))
    }

    pub(in crate::compiler) fn hir_lambda_body(
        &self,
        lambda_span: Span,
    ) -> CompileResult<&HirBody> {
        let expression = self.expression_at_span(lambda_span).ok_or_else(|| {
            CompileError::new(CompileErrorKind::UnsupportedSyntax("lambda HIR expression"))
                .with_span(lambda_span)
        })?;
        self.hir_bodies
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
                    .with_span(lambda_span)
            })
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

    fn compile_syntax_lambda_body(
        mut self,
        source: SourceId,
        body: SyntaxLambdaBody,
        hir_body: &'ast HirBody,
    ) -> CompileResult<UnlinkedCodeObject> {
        self.compile_param_defaults()?;
        match body {
            SyntaxLambdaBody::Expression(expression) => {
                let value = self
                    .compile_syntax_expression(source, &expression)?
                    .ok_or_else(|| {
                        CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "unsupported lambda expression body",
                        ))
                        .with_span(syntax_expr_span(source, &expression))
                    })?;
                self.emit(UnlinkedInstructionKind::Return { src: value });
            }
            SyntaxLambdaBody::Block(block) => {
                let dst = self.alloc_register()?;
                let body = CompilerBodyPayload::hir_body(source, block, hir_body);
                let returned = self.compile_block_payload_value_to(&body, dst)?;
                if !returned {
                    self.emit(UnlinkedInstructionKind::Return { src: dst });
                }
            }
        }
        self.code.register_count = self.next_register;
        Ok(self.code)
    }
}

fn syntax_expr_span(source: SourceId, expression: &SyntaxExpression) -> Span {
    let range = expression.syntax().text_range();
    Span::new(source, range.start().into(), range.end().into())
}

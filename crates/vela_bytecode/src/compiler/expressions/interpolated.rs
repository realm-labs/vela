use vela_syntax::ast::InterpolatedStringPart;

use crate::{FormatStringPart, Register, UnlinkedInstructionKind};

use crate::compiler::body_payloads::CompilerInterpolationPayload;
use crate::compiler::{CompileError, CompileErrorKind, CompileResult, Compiler};

pub(in crate::compiler) fn interpolated_expression_payload_at(
    payloads: Option<&[CompilerInterpolationPayload]>,
    index: usize,
) -> CompileResult<Option<&CompilerInterpolationPayload>> {
    let Some(payloads) = payloads else {
        return Ok(None);
    };
    payloads.get(index).map(Some).ok_or_else(|| {
        CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "missing CST interpolation expression",
        ))
    })
}

impl Compiler<'_, '_> {
    pub(super) fn compile_interpolated_string(
        &mut self,
        parts: &[InterpolatedStringPart],
        payloads: Option<&[CompilerInterpolationPayload]>,
    ) -> CompileResult<Register> {
        let expression_count = parts
            .iter()
            .filter(|part| matches!(part, InterpolatedStringPart::Expr(_)))
            .count();
        if payloads.is_some_and(|payloads| payloads.len() > expression_count) {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "mismatched CST interpolation expressions",
            )));
        }
        let mut compiled = Vec::with_capacity(parts.len());
        let mut expression_index = 0;
        for part in parts {
            match part {
                InterpolatedStringPart::Text(value) => {
                    let constant = self
                        .code
                        .push_constant(crate::Constant::String(value.clone()));
                    compiled.push(FormatStringPart::Text(constant));
                }
                InterpolatedStringPart::Expr(expr) => {
                    let payload = interpolated_expression_payload_at(payloads, expression_index)?;
                    expression_index += 1;
                    let payload =
                        payload.map(CompilerInterpolationPayload::value_expression_payload);
                    if payload
                        .as_ref()
                        .is_some_and(|payload| payload.syntax_expression().is_none())
                    {
                        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "missing CST interpolation expression",
                        )));
                    }
                    compiled.push(FormatStringPart::Value(
                        self.compile_expr_with_payload(expr, payload.as_ref())?,
                    ));
                }
            }
        }
        let dst = self.alloc_register()?;
        self.emit(UnlinkedInstructionKind::FormatString {
            dst,
            parts: compiled,
        });
        Ok(dst)
    }
}

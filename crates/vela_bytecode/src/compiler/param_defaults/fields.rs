use vela_common::SourceId;
use vela_syntax::ast::{SyntaxExpression, SyntaxExpressionKind, SyntaxFieldExpr};

use crate::{Register, UnlinkedInstructionKind};

use crate::compiler::{CompileResult, Compiler};

use super::{param_default_cst_lowering_covers, param_default_unsupported, records, span_for};

impl Compiler<'_, '_> {
    pub(super) fn compile_param_default_field(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        field: &SyntaxFieldExpr,
    ) -> CompileResult<Register> {
        if !param_default_field_cst_lowering_covers(expression) {
            return Err(param_default_unsupported(source, expression));
        }
        let Some(receiver) = field.receiver() else {
            return Err(param_default_unsupported(source, expression));
        };
        let Some(name) = field.name_text() else {
            return Err(param_default_unsupported(source, expression));
        };

        let slot = self
            .record_shape_for_syntax_expression(Some(source), &receiver)
            .and_then(|shape| shape.field_slot(&name));
        let record = self.compile_param_default_expression(source, &receiver)?;
        let dst = self.alloc_register()?;
        if let Some(slot) = slot {
            self.emit(UnlinkedInstructionKind::GetRecordSlot {
                dst,
                record,
                field: name,
                slot,
            });
        } else {
            self.emit_spanned(
                UnlinkedInstructionKind::GetRecordField {
                    dst,
                    record,
                    field: name,
                },
                span_for(source, expression),
            );
        }
        Ok(dst)
    }
}

pub(super) fn param_default_field_cst_lowering_covers(expression: &SyntaxExpression) -> bool {
    expression.as_field().is_some_and(|field| {
        field.name_token().is_some()
            && field.receiver().is_some_and(|receiver| {
                param_default_cst_lowering_covers(&receiver)
                    && field_receiver_is_record_literal_chain(&receiver)
            })
    })
}

fn field_receiver_is_record_literal_chain(expression: &SyntaxExpression) -> bool {
    match expression.expression_kind() {
        SyntaxExpressionKind::Record => {
            records::param_default_record_cst_lowering_covers(expression)
        }
        SyntaxExpressionKind::Paren => expression
            .as_paren()
            .and_then(|paren| paren.expression())
            .is_some_and(|inner| field_receiver_is_record_literal_chain(&inner)),
        SyntaxExpressionKind::Field => expression.as_field().is_some_and(|field| {
            field.name_token().is_some()
                && field
                    .receiver()
                    .is_some_and(|receiver| field_receiver_is_record_literal_chain(&receiver))
        }),
        SyntaxExpressionKind::Literal
        | SyntaxExpressionKind::Path
        | SyntaxExpressionKind::Unary
        | SyntaxExpressionKind::Binary
        | SyntaxExpressionKind::Array
        | SyntaxExpressionKind::Map
        | SyntaxExpressionKind::Try
        | SyntaxExpressionKind::Block
        | SyntaxExpressionKind::If
        | SyntaxExpressionKind::Index
        | SyntaxExpressionKind::Call
        | SyntaxExpressionKind::Assign
        | SyntaxExpressionKind::Lambda
        | SyntaxExpressionKind::Match => false,
    }
}

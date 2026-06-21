use vela_common::SourceId;
use vela_syntax::ast::{SyntaxExpression, SyntaxExpressionKind, SyntaxFieldExpr};

use crate::{Register, UnlinkedInstructionKind};

use crate::compiler::host_paths::{HostPath, HostPathPart, HostPathRoot};
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

        if let Some(path) = self.param_default_host_field_path(source, expression) {
            let root = self.compile_host_path_root(&path.root)?;
            let dst = self.alloc_register()?;
            self.emit_host_read(dst, root, path, span_for(source, expression))?;
            return Ok(dst);
        }

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

    fn param_default_host_field_path<'ast>(
        &self,
        source: SourceId,
        expression: &'ast SyntaxExpression,
    ) -> Option<HostPath<'ast>> {
        let (root, span, fields) = syntax_host_field_path(source, expression)?;
        let mut current_type = self.host_local_type_name(&root, span)?;
        let mut segments = Vec::with_capacity(fields.len());
        let last_index = fields.len().checked_sub(1)?;
        for (index, field_name) in fields.into_iter().enumerate() {
            let field = self.host_field_info(Some(&current_type), &field_name)?;
            segments.push(if field.variant_field {
                HostPathPart::VariantField(field.id)
            } else {
                HostPathPart::Field(field.id)
            });
            if index != last_index {
                current_type = field.type_hint?;
            }
        }
        Some(HostPath {
            root: HostPathRoot::OwnedLocalPath { name: root, span },
            segments,
        })
    }
}

pub(super) fn param_default_field_cst_lowering_covers(expression: &SyntaxExpression) -> bool {
    expression.as_field().is_some_and(|field| {
        field.name_token().is_some()
            && field.receiver().is_some_and(|receiver| {
                param_default_cst_lowering_covers(&receiver)
                    && (field_receiver_is_record_literal_chain(&receiver)
                        || field_receiver_is_path_chain(&receiver))
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

fn field_receiver_is_path_chain(expression: &SyntaxExpression) -> bool {
    match expression.expression_kind() {
        SyntaxExpressionKind::Path => expression
            .as_path()
            .is_some_and(|path| !path.path_segments().is_empty()),
        SyntaxExpressionKind::Paren => expression
            .as_paren()
            .and_then(|paren| paren.expression())
            .is_some_and(|inner| field_receiver_is_path_chain(&inner)),
        SyntaxExpressionKind::Field => expression.as_field().is_some_and(|field| {
            field.name_token().is_some()
                && field
                    .receiver()
                    .is_some_and(|receiver| field_receiver_is_path_chain(&receiver))
        }),
        SyntaxExpressionKind::Literal
        | SyntaxExpressionKind::Unary
        | SyntaxExpressionKind::Binary
        | SyntaxExpressionKind::Array
        | SyntaxExpressionKind::Map
        | SyntaxExpressionKind::Record
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

fn syntax_host_field_path(
    source: SourceId,
    expression: &SyntaxExpression,
) -> Option<(String, vela_common::Span, Vec<String>)> {
    match expression.expression_kind() {
        SyntaxExpressionKind::Path => {
            let path = expression.as_path()?;
            let mut segments = path.path_segments();
            let root = segments.first()?.clone();
            let fields = segments.split_off(1);
            Some((root, span_for(source, expression), fields))
        }
        SyntaxExpressionKind::Paren => {
            let inner = expression.as_paren()?.expression()?;
            syntax_host_field_path(source, &inner)
        }
        SyntaxExpressionKind::Field => {
            let field = expression.as_field()?;
            let receiver = field.receiver()?;
            let name = field.name_text()?;
            let (root, span, mut fields) = syntax_host_field_path(source, &receiver)?;
            fields.push(name);
            Some((root, span, fields))
        }
        SyntaxExpressionKind::Literal
        | SyntaxExpressionKind::Unary
        | SyntaxExpressionKind::Binary
        | SyntaxExpressionKind::Array
        | SyntaxExpressionKind::Map
        | SyntaxExpressionKind::Record
        | SyntaxExpressionKind::Try
        | SyntaxExpressionKind::Block
        | SyntaxExpressionKind::If
        | SyntaxExpressionKind::Index
        | SyntaxExpressionKind::Call
        | SyntaxExpressionKind::Assign
        | SyntaxExpressionKind::Lambda
        | SyntaxExpressionKind::Match => None,
    }
}

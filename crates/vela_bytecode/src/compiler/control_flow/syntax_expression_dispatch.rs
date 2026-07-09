use vela_common::SourceId;
use vela_syntax::SyntaxKind;
use vela_syntax::ast::{SyntaxExpression, SyntaxExpressionKind, SyntaxLiteral};
use vela_syntax::token::{InterpolatedStringTokenPart, TokenKind};

use crate::compiler::body_payloads::{
    CompilerBodyPayload, expression_syntax_literal, expression_syntax_path_field,
    expression_syntax_path_or_self,
};
use crate::compiler::patterns::enum_variant_path;
use crate::compiler::{CompileResult, Compiler};
use crate::{Constant, FormatStringPart};
use crate::{Register, UnlinkedInstructionKind};

use super::spans::syntax_expression_span;

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn compile_syntax_expression(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        match expression.expression_kind() {
            SyntaxExpressionKind::Paren => {
                let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) else {
                    return Ok(None);
                };
                self.compile_syntax_expression(source, &inner)
            }
            SyntaxExpressionKind::Literal => {
                self.compile_syntax_literal_expression(source, expression)
            }
            SyntaxExpressionKind::Path => self.compile_syntax_path_expression(source, expression),
            SyntaxExpressionKind::Field => self.compile_syntax_field_expression(source, expression),
            SyntaxExpressionKind::Unary => self.compile_syntax_unary(source, expression),
            SyntaxExpressionKind::Lambda => {
                self.compile_syntax_lambda_with_callback_shapes(source, expression, &[])
            }
            SyntaxExpressionKind::Block => self.compile_syntax_block_expression(source, expression),
            SyntaxExpressionKind::Index => self.compile_syntax_index(source, expression),
            SyntaxExpressionKind::Assign => self.compile_syntax_assignment(source, expression),
            SyntaxExpressionKind::Call => self.compile_syntax_call(source, expression),
            SyntaxExpressionKind::Try => self.compile_syntax_try(source, expression),
            SyntaxExpressionKind::Array
            | SyntaxExpressionKind::Map
            | SyntaxExpressionKind::Record => self.compile_syntax_container(source, expression),
            SyntaxExpressionKind::If => self.compile_syntax_if_value(source, expression),
            SyntaxExpressionKind::Match => self.compile_syntax_match_value(source, expression),
            SyntaxExpressionKind::Binary => {
                self.compile_syntax_binary_expression(source, expression)
            }
        }
    }

    fn compile_syntax_literal_expression(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        if let Some(literal) = expression_syntax_literal(expression) {
            return self
                .compile_literal(Some(syntax_expression_span(source, expression)), &literal)
                .map(Some);
        }
        self.compile_syntax_interpolated_string(source, expression)
    }

    fn compile_syntax_path_expression(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        let Some(path) = expression_syntax_path_or_self(expression) else {
            return Ok(None);
        };
        self.compile_path_expr(syntax_expression_span(source, expression), &path)
            .map(Some)
    }

    fn compile_syntax_field_expression(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        if let Some(path) = expression_syntax_path_field(expression) {
            return self
                .compile_path_expr(syntax_expression_span(source, expression), &path)
                .map(Some);
        }
        self.compile_syntax_field_read(source, expression)
    }

    fn compile_syntax_block_expression(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        let Some(block) = expression.as_block() else {
            return Ok(None);
        };
        let dst = self.alloc_register()?;
        let body = CompilerBodyPayload::nested_syntax(source, block);
        self.compile_block_payload_value_to(&body, dst)?;
        Ok(Some(dst))
    }

    fn compile_syntax_interpolated_string(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        let Some(literal) = expression.as_literal() else {
            return Ok(None);
        };
        if literal.token_kind() != Some(SyntaxKind::InterpolatedString) {
            return Ok(None);
        }
        let Some(parts) = interpolated_string_parts(&literal) else {
            return Ok(None);
        };
        let mut interpolation_expressions = literal.interpolation_expressions();
        let mut compiled = Vec::with_capacity(parts.len());
        for part in parts {
            match part {
                InterpolatedStringTokenPart::Text(value) => {
                    let constant = self.code.push_constant(Constant::String(value));
                    compiled.push(FormatStringPart::Text(constant));
                }
                InterpolatedStringTokenPart::Expr { .. } => {
                    let Some(expression) = interpolation_expressions.next() else {
                        return Ok(None);
                    };
                    let Some(value) = self.compile_syntax_expression(source, &expression)? else {
                        return Ok(None);
                    };
                    compiled.push(FormatStringPart::Value(value));
                }
            }
        }
        if interpolation_expressions.next().is_some() {
            return Ok(None);
        }
        let dst = self.alloc_register()?;
        self.emit(UnlinkedInstructionKind::FormatString {
            dst,
            parts: compiled,
        });
        Ok(Some(dst))
    }

    fn compile_syntax_field_read(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        let Some(field) = expression.as_field() else {
            return Ok(None);
        };
        if let Some(register) = self.compile_syntax_host_field_read(source, expression)? {
            return Ok(Some(register));
        }
        let Some(receiver_expression) = field.receiver() else {
            return Ok(None);
        };
        let Some(field_name) = field.name_text() else {
            return Ok(None);
        };
        let receiver_span = syntax_expression_span(source, &receiver_expression);
        let record_slot = expression_syntax_path_or_self(&receiver_expression)
            .and_then(|path| {
                let [root] = path.as_slice() else {
                    return None;
                };
                self.script_record_field_slot_for_path_root(receiver_span, root, &field_name)
            })
            .or_else(|| {
                self.script_fact_for_syntax_expression(source, &receiver_expression)
                    .and_then(|fact| {
                        self.script_record_field_slot_for_type(&fact.type_name, &field_name)
                    })
            })
            .or_else(|| {
                self.value_shape_for_syntax_expression(Some(source), &receiver_expression)
                    .and_then(|shape| {
                        shape
                            .as_record()
                            .and_then(|shape| shape.field_slot(&field_name))
                    })
            });
        let enum_slot = self
            .syntax_record_enum_slot(source, &receiver_expression, &field_name)
            .or_else(|| {
                self.script_fact_for_syntax_expression(source, &receiver_expression)
                    .and_then(|fact| {
                        let variant = fact.enum_variant.as_deref()?;
                        self.facts.script_field_slots.enum_variant(
                            &fact.type_name,
                            variant,
                            &field_name,
                        )
                    })
            });
        let Some(record) = self.compile_syntax_expression(source, &receiver_expression)? else {
            return Ok(None);
        };
        let dst = self.alloc_register()?;
        if let Some(slot) = enum_slot {
            self.emit(UnlinkedInstructionKind::GetEnumSlot {
                dst,
                value: record,
                field: field_name,
                slot,
            });
        } else if let Some(slot) = record_slot {
            self.emit(UnlinkedInstructionKind::GetRecordSlot {
                dst,
                record,
                field: field_name,
                slot,
            });
        } else {
            self.emit(UnlinkedInstructionKind::GetRecordField {
                dst,
                record,
                field: field_name,
            });
        }
        Ok(Some(dst))
    }

    fn syntax_record_enum_slot(
        &self,
        source: SourceId,
        expression: &SyntaxExpression,
        field_name: &str,
    ) -> Option<usize> {
        let record = expression.as_record()?;
        let (enum_name, variant) = enum_variant_path(&record.path_segments())?;
        let type_name = self
            .type_symbol_at_span(syntax_expression_span(source, expression))
            .unwrap_or(enum_name);
        self.facts
            .script_field_slots
            .enum_variant(&type_name, &variant, field_name)
            .or_else(|| Self::syntax_record_literal_field_slot(&record, field_name))
    }

    fn syntax_record_literal_field_slot(
        record: &vela_syntax::ast::SyntaxRecordExpr,
        field_name: &str,
    ) -> Option<usize> {
        let mut names = record
            .field_list()?
            .fields()
            .filter_map(|field| field.label_text())
            .collect::<Vec<_>>();
        names.sort_unstable();
        names.iter().position(|name| name == field_name)
    }

    fn compile_syntax_try(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        let Some(operand_expression) = expression
            .as_try()
            .and_then(|try_expression| try_expression.expression())
        else {
            return Ok(None);
        };
        let Some(src) = self.compile_syntax_expression(source, &operand_expression)? else {
            return Ok(None);
        };
        let dst = self.alloc_register()?;
        self.emit_spanned(
            UnlinkedInstructionKind::TryPropagate { dst, src },
            syntax_expression_span(source, expression),
        );
        Ok(Some(dst))
    }
}

pub(super) fn syntax_block_expression(
    expression: &SyntaxExpression,
) -> Option<vela_syntax::ast::SyntaxBlock> {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_block_expression(&inner);
    }
    expression.as_block()
}

fn interpolated_string_parts(literal: &SyntaxLiteral) -> Option<Vec<InterpolatedStringTokenPart>> {
    let text = literal.token_text()?;
    vela_syntax::lexer::lex(SourceId::new(0), &text)
        .tokens
        .into_iter()
        .find_map(|token| match token.kind {
            TokenKind::InterpolatedString(parts) => Some(parts),
            _ => None,
        })
}

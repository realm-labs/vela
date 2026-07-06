use vela_common::{PrimitiveTag, Span};
use vela_syntax::ast::{BinaryOp, Expr, ExprKind, Literal, SyntaxExpressionKind, UnaryOp};

use crate::{BinaryLiteralSide, Register, UnlinkedInstructionKind};

use super::assignments::{AssignmentTargetSyntax, AssignmentValuePayloads, AssignmentValueSyntax};
use super::body_payloads::CompilerExpressionPayload;
use super::const_eval::{
    compile_literal_constant, compile_literal_constant_for_type, compile_negated_literal_constant,
};
use super::expression_checks::{
    UnsuffixedNumericLiteral, expressions_are_i64, payload_syntax_overlaps_expr,
    reject_missing_binary_operand_payload, reject_missing_expression_payload,
    unsuffixed_numeric_literal_with_payload,
};
use super::expression_facts::{
    expression_path_is_self, expression_syntax_kind, payload_stored_kind_matches_expression_facts,
};
use super::host_paths::HostPath;
use super::operators::{
    binary_literal_op, i64_binary_instruction, i64_immediate_instruction,
    i64_immediate_op_supported, non_logical_binary_instruction,
};
use super::patterns::enum_variant_path;
use super::value_types::RuntimeTypeFact;
use super::{CompileError, CompileErrorKind, CompileResult, Compiler};

pub(in crate::compiler) mod interpolated;
#[cfg(test)]
pub(in crate::compiler) use interpolated::interpolated_expression_payload_at;

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn compile_expr_with_payload(
        &mut self,
        expr: &Expr,
        payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> CompileResult<Register> {
        if let Some(payload) = payload
            && let Some(kind) = payload.stored_syntax_kind()
            && payload_stored_kind_matches_expression_facts(
                payload,
                expression_syntax_kind(expr),
                expression_path_is_self(expr),
                false,
            )
        {
            return self.compile_expr_with_payload_kind(expr, payload, kind);
        }
        if payload.is_some_and(|payload| payload.syntax_expression().is_none())
            && payload.is_some_and(CompilerExpressionPayload::rejects_missing_payload)
        {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST expression payload",
            )));
        }
        if payload.is_some_and(|payload| payload.stored_syntax_kind().is_some())
            && payload.is_some_and(CompilerExpressionPayload::requires_matching_payload)
        {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "mismatched CST expression payload",
            )));
        }
        self.compile_expr(expr)
    }

    fn compile_expr_with_payload_kind(
        &mut self,
        expr: &Expr,
        payload: &CompilerExpressionPayload<'_>,
        kind: SyntaxExpressionKind,
    ) -> CompileResult<Register> {
        match kind {
            SyntaxExpressionKind::Paren => {
                let inner_payload = payload.paren_inner_payload();
                reject_missing_expression_payload(
                    inner_payload.as_ref(),
                    "missing CST parenthesized expression",
                )?;
                self.compile_expr_with_payload(expr, inner_payload.as_ref())
            }
            SyntaxExpressionKind::Block => {
                let dst = self.alloc_register()?;
                if let Some(body_payload) = payload.block_body_payload() {
                    self.compile_block_payload_value_to(&body_payload, dst)?;
                } else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST block expression body payload",
                    )));
                }
                Ok(dst)
            }
            SyntaxExpressionKind::If => {
                let dst = self.alloc_register()?;
                let Some(source) = payload.source() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST if expression payload",
                    )));
                };
                let Some(if_expr) = payload
                    .syntax_expression()
                    .and_then(vela_syntax::ast::SyntaxExpression::as_if)
                else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST if expression payload",
                    )));
                };
                let Some(_) = self.compile_syntax_if_value_to(source, &if_expr, dst)? else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST if expression payload",
                    )));
                };
                Ok(dst)
            }
            SyntaxExpressionKind::Match => {
                let dst = self.alloc_register()?;
                if self
                    .compile_syntax_match_payload_value_to(payload, dst)?
                    .is_some()
                {
                    return Ok(dst);
                }
                Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "missing CST match expression payload",
                )))
            }
            SyntaxExpressionKind::Path => {
                let Some(source) = payload.source() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST path expression payload",
                    )));
                };
                let Some(expression) = payload.syntax_expression() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST path expression payload",
                    )));
                };
                let Some(register) = self.compile_syntax_expression(source, expression)? else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST path expression payload",
                    )));
                };
                Ok(register)
            }
            SyntaxExpressionKind::Array => {
                let Some(source) = payload.source() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST array expression payload",
                    )));
                };
                let Some(expression) = payload.syntax_expression() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST array expression payload",
                    )));
                };
                let Some(register) = self.compile_syntax_expression(source, expression)? else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST array expression payload",
                    )));
                };
                Ok(register)
            }
            SyntaxExpressionKind::Map => {
                let Some(source) = payload.source() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST map expression payload",
                    )));
                };
                let Some(expression) = payload.syntax_expression() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST map expression payload",
                    )));
                };
                let Some(register) = self.compile_syntax_expression(source, expression)? else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST map expression payload",
                    )));
                };
                Ok(register)
            }
            SyntaxExpressionKind::Record => {
                let Some(source) = payload.source() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST record expression payload",
                    )));
                };
                let Some(expression) = payload.syntax_expression() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST record expression payload",
                    )));
                };
                let Some(register) = self.compile_syntax_expression(source, expression)? else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST record expression payload",
                    )));
                };
                Ok(register)
            }
            SyntaxExpressionKind::Assign => {
                let ExprKind::Assign { .. } = &expr.kind else {
                    unreachable!("validated CST assignment expression payload kind");
                };
                if !payload_syntax_overlaps_expr(payload, expr) {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST assignment expression payload",
                    )));
                }
                let target_payload = payload.assignment_target_payload();
                let value_payload = payload.assignment_value_payload();
                let value_body = value_payload
                    .as_ref()
                    .and_then(CompilerExpressionPayload::block_body_payload);
                self.compile_assignment_with_payloads(
                    expr,
                    AssignmentTargetSyntax::new(target_payload.as_ref()),
                    AssignmentValueSyntax::new(
                        value_payload
                            .as_ref()
                            .and_then(CompilerExpressionPayload::syntax_kind),
                        payload.syntax_assignment_operator(),
                        value_payload.as_ref(),
                        AssignmentValuePayloads::new(value_body.as_ref()),
                    ),
                )
            }
            SyntaxExpressionKind::Binary => {
                let Some(source) = payload.source() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST binary expression payload",
                    )));
                };
                let Some(expression) = payload.syntax_expression() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST binary expression payload",
                    )));
                };
                let Some(register) = self.compile_syntax_expression(source, expression)? else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST binary expression payload",
                    )));
                };
                Ok(register)
            }
            SyntaxExpressionKind::Call => {
                let ExprKind::Call { callee, args } = &expr.kind else {
                    unreachable!("validated CST call expression payload kind");
                };
                let callee_payload = payload.call_callee_payload();
                reject_missing_expression_payload(
                    callee_payload.as_ref(),
                    "missing CST call callee",
                )?;
                let arg_payloads = payload.call_argument_payloads();
                self.compile_call_expr_with_arg_payloads(
                    expr,
                    callee,
                    args,
                    callee_payload.as_ref(),
                    arg_payloads.as_deref(),
                )
            }
            SyntaxExpressionKind::Field => {
                let Some(source) = payload.source() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST field expression payload",
                    )));
                };
                let Some(expression) = payload.syntax_expression() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST field expression payload",
                    )));
                };
                let Some(field) = expression.as_field() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST field expression payload",
                    )));
                };
                if field.receiver().is_none() {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST field receiver",
                    )));
                }
                if field.name_text().is_none() {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "field expression",
                    )));
                }
                let Some(register) = self.compile_syntax_expression(source, expression)? else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST field expression payload",
                    )));
                };
                Ok(register)
            }
            SyntaxExpressionKind::Index => {
                let ExprKind::Index { base, index } = &expr.kind else {
                    unreachable!("validated CST index expression payload kind");
                };
                if !payload_syntax_overlaps_expr(payload, expr) {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST index expression payload",
                    )));
                }
                let operand_payloads = payload.index_operand_payloads();
                let (base_payload, index_payload) = operand_payloads
                    .as_ref()
                    .map_or((None, None), |(base, index)| (Some(base), Some(index)));
                reject_missing_expression_payload(base_payload, "missing CST index receiver")?;
                reject_missing_expression_payload(index_payload, "missing CST index operand")?;
                self.compile_index_expr(
                    expr,
                    base,
                    index,
                    base_payload,
                    index_payload,
                    Some(payload),
                )
            }
            SyntaxExpressionKind::Lambda => {
                let Some(source) = payload.source() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST lambda expression payload",
                    )));
                };
                let Some(expression) = payload.syntax_expression() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST lambda expression payload",
                    )));
                };
                let Some(lambda) = expression.as_lambda() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST lambda expression payload",
                    )));
                };
                if lambda.body().is_none() {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST lambda body",
                    )));
                }
                let Some(register) =
                    self.compile_syntax_lambda_with_callback_shapes(source, expression, &[])?
                else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST lambda expression payload",
                    )));
                };
                Ok(register)
            }
            SyntaxExpressionKind::Unary => {
                let Some(source) = payload.source() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST unary expression payload",
                    )));
                };
                let Some(expression) = payload.syntax_expression() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST unary expression payload",
                    )));
                };
                let Some(register) = self.compile_syntax_expression(source, expression)? else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST unary expression payload",
                    )));
                };
                Ok(register)
            }
            SyntaxExpressionKind::Try => {
                let Some(source) = payload.source() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST try expression payload",
                    )));
                };
                let Some(expression) = payload.syntax_expression() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST try expression payload",
                    )));
                };
                let Some(register) = self.compile_syntax_expression(source, expression)? else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST try expression payload",
                    )));
                };
                Ok(register)
            }
            SyntaxExpressionKind::Literal => {
                let Some(source) = payload.source() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST literal expression",
                    )));
                };
                let Some(expression) = payload.syntax_expression() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST literal expression",
                    )));
                };
                let Some(register) = self.compile_syntax_expression(source, expression)? else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST literal expression",
                    )));
                };
                Ok(register)
            }
        }
    }

    pub(super) fn compile_expr(&mut self, expr: &Expr) -> CompileResult<Register> {
        match &expr.kind {
            ExprKind::Literal(literal) => self.compile_literal(Some(expr.span), literal),
            ExprKind::InterpolatedString(parts) => self.compile_interpolated_string(parts, None),
            ExprKind::Path(path) => self.compile_path_expr(expr.span, path),
            ExprKind::Binary { op, left, right } => {
                if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST logical operand payload",
                    )));
                }
                self.compile_binary(*op, expr.span, left, right, None, None)
            }
            ExprKind::Unary { op, expr } => self.compile_unary(*op, expr.span, expr, None),
            ExprKind::Field { base, name } => self.compile_field_expr(expr, base, name, None, None),
            ExprKind::Index { base, index } => {
                self.compile_index_expr(expr, base, index, None, None, None)
            }
            ExprKind::Call { callee, args } => self.compile_call_expr(expr, callee, args),
            ExprKind::Lambda { params, body } => self.compile_lambda(expr, params, body, None),
            ExprKind::Try(value) => {
                let src = self.compile_expr(value)?;
                let dst = self.alloc_register()?;
                self.emit(UnlinkedInstructionKind::TryPropagate { dst, src });
                Ok(dst)
            }
            ExprKind::Block(_) => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST block expression payload",
            ))),
            ExprKind::Array(_) => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST array element payload",
            ))),
            ExprKind::Map(_) => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST map entry payload",
            ))),
            ExprKind::Record { .. } => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST record field payload",
            ))),
            ExprKind::If(_) => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST if expression payload",
            ))),
            ExprKind::Assign { .. } => self.compile_assignment(expr),
            ExprKind::SelfValue => self.local_register_at_span(expr.span, "self"),
            ExprKind::Error => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "expression",
            ))),
            ExprKind::Match(_) => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST match expression payload",
            ))),
        }
    }

    fn compile_field_expr(
        &mut self,
        expr: &Expr,
        base: &Expr,
        name: &str,
        base_payload: Option<&CompilerExpressionPayload<'_>>,
        expr_payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> CompileResult<Register> {
        let receiver_type = self.script_type_for_expr_with_payload(base, base_payload);
        let typed_record_slot = receiver_type
            .as_deref()
            .and_then(|type_name| self.script_record_field_slot_for_type(type_name, name))
            .or_else(|| {
                self.record_shape_for_expr_with_payload(base, base_payload)
                    .and_then(|shape| shape.field_slot(name))
            });
        let typed_enum_slot = self
            .script_fact_for_expr_with_payload(base, base_payload)
            .and_then(|fact| {
                let variant = fact.enum_variant.as_deref()?;
                self.facts
                    .script_field_slots
                    .enum_variant(&fact.type_name, variant, name)
            });
        if let Some((slot_kind, slot)) = record_literal_field_slot(base, name) {
            let root = self.compile_expr_with_payload(base, base_payload)?;
            let dst = self.alloc_register()?;
            match slot_kind {
                LiteralFieldSlotKind::Record => self.emit(UnlinkedInstructionKind::GetRecordSlot {
                    dst,
                    record: root,
                    field: name.to_owned(),
                    slot,
                }),
                LiteralFieldSlotKind::Enum => self.emit(UnlinkedInstructionKind::GetEnumSlot {
                    dst,
                    value: root,
                    field: name.to_owned(),
                    slot,
                }),
            }
            Ok(dst)
        } else if let Some(slot) = typed_record_slot {
            let root = self.compile_expr_with_payload(base, base_payload)?;
            let dst = self.alloc_register()?;
            self.emit(UnlinkedInstructionKind::GetRecordSlot {
                dst,
                record: root,
                field: name.to_owned(),
                slot,
            });
            Ok(dst)
        } else if let Some(slot) = typed_enum_slot {
            let root = self.compile_expr_with_payload(base, base_payload)?;
            let dst = self.alloc_register()?;
            self.emit(UnlinkedInstructionKind::GetEnumSlot {
                dst,
                value: root,
                field: name.to_owned(),
                slot,
            });
            Ok(dst)
        } else {
            if let Some(path) = self.host_field_path_with_payload(expr, expr_payload)
                && path.requires_path_instruction()
            {
                let root = self.compile_host_path_root(&path.root)?;
                let dst = self.alloc_register()?;
                self.emit_host_read(dst, root, path, expr.span)?;
                return Ok(dst);
            }
            let root = self.compile_expr_with_payload(base, base_payload)?;
            let dst = self.alloc_register()?;
            if let Some(field) = self
                .host_field_info(receiver_type.as_deref(), name)
                .map(|field| field.id)
            {
                let path = HostPath {
                    root: super::host_paths::HostPathRoot::Expr {
                        expr: base,
                        payload: base_payload.cloned(),
                    },
                    segments: vec![super::host_paths::HostPathPart::Field(field)],
                };
                self.emit_host_read(dst, root, path, expr.span)?;
            } else {
                self.emit(UnlinkedInstructionKind::GetRecordField {
                    dst,
                    record: root,
                    field: name.to_owned(),
                });
            }
            Ok(dst)
        }
    }

    fn compile_index_expr(
        &mut self,
        expr: &Expr,
        base: &Expr,
        index: &Expr,
        base_payload: Option<&CompilerExpressionPayload<'_>>,
        index_payload: Option<&CompilerExpressionPayload<'_>>,
        expr_payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> CompileResult<Register> {
        if let Some(path) = self.host_field_path_with_payload(expr, expr_payload)
            && !path.segments.is_empty()
        {
            self.reject_invalid_host_index_read_with_payload(
                expr,
                base,
                index,
                base_payload,
                index_payload,
            )?;
            let root = self.compile_host_path_root(&path.root)?;
            let dst = self.alloc_register()?;
            self.emit_host_read(dst, root, path, expr.span)?;
            return Ok(dst);
        }
        self.reject_invalid_host_index_read_with_payload(
            expr,
            base,
            index,
            base_payload,
            index_payload,
        )?;
        let base = self.compile_expr_with_payload(base, base_payload)?;
        let dst = self.alloc_register()?;
        if let Some(key) = literal_string_with_payload(index, index_payload) {
            let key = self.code.push_constant(crate::Constant::String(key));
            self.emit(UnlinkedInstructionKind::GetStringKeyIndex { dst, base, key });
        } else {
            let index = self.compile_expr_with_payload(index, index_payload)?;
            self.emit(UnlinkedInstructionKind::GetIndex { dst, base, index });
        }
        Ok(dst)
    }

    pub(super) fn compile_literal(
        &mut self,
        span: Option<Span>,
        literal: &Literal,
    ) -> CompileResult<Register> {
        let constant = compile_literal_constant(literal).map_err(|error| match span {
            Some(span) => error.with_span(span),
            None => error,
        })?;
        self.emit_constant(constant)
    }

    fn compile_binary(
        &mut self,
        op: BinaryOp,
        span: Span,
        left: &Expr,
        right: &Expr,
        left_payload: Option<&CompilerExpressionPayload<'_>>,
        right_payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> CompileResult<Register> {
        reject_missing_binary_operand_payload(left_payload, right_payload)?;
        match op {
            BinaryOp::Range => {
                return self.compile_range(left, right, false, left_payload, right_payload);
            }
            BinaryOp::RangeInclusive => {
                return self.compile_range(left, right, true, left_payload, right_payload);
            }
            _ => {}
        }
        self.reject_static_identity_comparison_operands(
            op,
            span,
            left,
            right,
            left_payload,
            right_payload,
        )?;
        self.reject_static_comparison_without_trait(op, span, left, left_payload)?;

        if let Some(register) = self.compile_binary_with_inline_literal(
            op,
            span,
            left,
            right,
            left_payload,
            right_payload,
        )? {
            return Ok(register);
        }

        let lhs = self.compile_expr_with_payload(left, left_payload)?;
        let rhs = self.compile_expr_with_payload(right, right_payload)?;
        let dst = self.alloc_register()?;
        let instruction = if expressions_are_i64(
            self.value_type_for_expr_with_payload(left, left_payload),
            self.value_type_for_expr_with_payload(right, right_payload),
        ) {
            i64_binary_instruction(op, dst, lhs, rhs)
        } else {
            None
        }
        .or_else(|| non_logical_binary_instruction(op, dst, lhs, rhs))
        .expect("logical operators handled above");
        self.emit_spanned(instruction, span);
        Ok(dst)
    }

    fn compile_binary_with_inline_literal(
        &mut self,
        op: BinaryOp,
        span: Span,
        left: &Expr,
        right: &Expr,
        left_payload: Option<&CompilerExpressionPayload<'_>>,
        right_payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> CompileResult<Option<Register>> {
        if let Some(literal) = unsuffixed_numeric_literal_with_payload(left_payload) {
            return self.compile_binary_literal_candidate(
                op,
                span,
                right,
                right_payload,
                literal,
                BinaryLiteralSide::Left,
            );
        }
        if let Some(literal) = unsuffixed_numeric_literal_with_payload(right_payload) {
            return self.compile_binary_literal_candidate(
                op,
                span,
                left,
                left_payload,
                literal,
                BinaryLiteralSide::Right,
            );
        }
        Ok(None)
    }

    fn compile_binary_literal_candidate(
        &mut self,
        op: BinaryOp,
        span: Span,
        value_expr: &Expr,
        value_payload: Option<&CompilerExpressionPayload<'_>>,
        literal: UnsuffixedNumericLiteral,
        side: BinaryLiteralSide,
    ) -> CompileResult<Option<Register>> {
        let value_type = self.value_type_for_expr_with_payload(value_expr, value_payload);
        if side == BinaryLiteralSide::Right
            && value_type == Some(RuntimeTypeFact::Primitive(PrimitiveTag::I64))
            && let Some(imm) = self.i64_immediate_literal(&literal, span)?
            && i64_immediate_op_supported(op, imm)
        {
            let value = self.compile_expr_with_payload(value_expr, value_payload)?;
            let dst = self.alloc_register()?;
            let instruction = i64_immediate_instruction(op, dst, value, imm)
                .expect("support was checked before compiling the value expression");
            self.emit_spanned(instruction, span);
            return Ok(Some(dst));
        }

        let Some(literal_op) = binary_literal_op(op) else {
            return Ok(None);
        };
        if let Some(RuntimeTypeFact::Primitive(tag)) = value_type.as_ref()
            && literal.matches_primitive_tag(*tag)
        {
            let value = self.compile_expr_with_payload(value_expr, value_payload)?;
            let literal = self.compile_inline_numeric_literal_as(&literal, *tag, span)?;
            let rhs_or_lhs = self.emit_constant(literal)?;
            let dst = self.alloc_register()?;
            let instruction = match side {
                BinaryLiteralSide::Left => {
                    non_logical_binary_instruction(op, dst, rhs_or_lhs, value)
                }
                BinaryLiteralSide::Right => {
                    non_logical_binary_instruction(op, dst, value, rhs_or_lhs)
                }
            }
            .expect("literal op excludes logical and range operators");
            self.emit_spanned(instruction, span);
            return Ok(Some(dst));
        }

        if value_type.is_none() {
            let value = self.compile_expr_with_payload(value_expr, value_payload)?;
            let dst = self.alloc_register()?;
            match literal {
                UnsuffixedNumericLiteral::Integer(text) => {
                    self.emit_spanned(
                        UnlinkedInstructionKind::BinaryIntLiteral {
                            dst,
                            op: literal_op,
                            value,
                            literal: text.to_owned(),
                            side,
                        },
                        span,
                    );
                }
                UnsuffixedNumericLiteral::Float(text) => {
                    self.emit_spanned(
                        UnlinkedInstructionKind::BinaryFloatLiteral {
                            dst,
                            op: literal_op,
                            value,
                            literal: text.to_owned(),
                            side,
                        },
                        span,
                    );
                }
            }
            return Ok(Some(dst));
        }

        Ok(None)
    }

    fn i64_immediate_literal(
        &self,
        literal: &UnsuffixedNumericLiteral,
        span: Span,
    ) -> CompileResult<Option<i64>> {
        let UnsuffixedNumericLiteral::Integer(_) = literal else {
            return Ok(None);
        };
        let constant = self.compile_inline_numeric_literal_as(literal, PrimitiveTag::I64, span)?;
        let crate::Constant::Scalar(vela_common::ScalarValue::I64(value)) = constant else {
            return Ok(None);
        };
        Ok(Some(value))
    }

    fn compile_inline_numeric_literal_as(
        &self,
        literal: &UnsuffixedNumericLiteral,
        tag: PrimitiveTag,
        span: Span,
    ) -> CompileResult<crate::Constant> {
        match literal {
            UnsuffixedNumericLiteral::Integer(text) => compile_literal_constant_for_type(
                &Literal::Integer(vela_syntax::ast::IntegerLiteral::unsuffixed(text)),
                tag,
            ),
            UnsuffixedNumericLiteral::Float(text) => compile_literal_constant_for_type(
                &Literal::Float(vela_syntax::ast::FloatLiteral::unsuffixed(text)),
                tag,
            ),
        }
        .map_err(|error| error.with_span(span))
        .map(|constant| constant.expect("literal kind and primitive tag were checked by caller"))
    }

    fn compile_range(
        &mut self,
        left: &Expr,
        right: &Expr,
        inclusive: bool,
        left_payload: Option<&CompilerExpressionPayload<'_>>,
        right_payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> CompileResult<Register> {
        let start = self.compile_expr_with_payload(left, left_payload)?;
        let end = self.compile_expr_with_payload(right, right_payload)?;
        let dst = self.alloc_register()?;
        self.emit(UnlinkedInstructionKind::MakeRange {
            dst,
            start,
            end,
            inclusive,
        });
        Ok(dst)
    }

    pub(super) fn emit_truthy_to_bool(
        &mut self,
        dst: Register,
        src: Register,
    ) -> CompileResult<()> {
        self.emit(UnlinkedInstructionKind::Truthy { dst, src });
        Ok(())
    }

    fn compile_unary(
        &mut self,
        op: UnaryOp,
        span: Span,
        expr: &Expr,
        payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> CompileResult<Register> {
        if op == UnaryOp::Not
            && let Some(register) = self.compile_negated_equality(span, expr, payload)?
        {
            return Ok(register);
        }
        if op == UnaryOp::Negate
            && let ExprKind::Literal(literal) = &expr.kind
            && let Some(constant) = compile_negated_literal_constant(literal)
                .map_err(|error| error.with_span(expr.span))?
        {
            return self.emit_constant(constant);
        }

        let src = self.compile_expr_with_payload(expr, payload)?;
        let dst = self.alloc_register()?;
        let instruction = match op {
            UnaryOp::Not => UnlinkedInstructionKind::Not { dst, src },
            UnaryOp::Negate => UnlinkedInstructionKind::Negate { dst, src },
        };
        self.emit_spanned(instruction, span);
        Ok(dst)
    }

    fn compile_negated_equality(
        &mut self,
        span: Span,
        expr: &Expr,
        payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> CompileResult<Option<Register>> {
        let ExprKind::Binary {
            op:
                equality_op @ (BinaryOp::Equal
                | BinaryOp::NotEqual
                | BinaryOp::IdentityEqual
                | BinaryOp::IdentityNotEqual),
            left,
            right,
        } = &expr.kind
        else {
            return Ok(None);
        };

        let operand_payloads = payload.and_then(|payload| payload.binary_operand_payloads());
        let (left_payload, right_payload) = operand_payloads
            .as_ref()
            .map_or((None, None), |(left, right)| (Some(left), Some(right)));
        let inverse = match equality_op {
            BinaryOp::Equal => BinaryOp::NotEqual,
            BinaryOp::NotEqual => BinaryOp::Equal,
            BinaryOp::IdentityEqual => BinaryOp::IdentityNotEqual,
            BinaryOp::IdentityNotEqual => BinaryOp::IdentityEqual,
            _ => unreachable!("binary equality was matched above"),
        };
        self.compile_binary(inverse, span, left, right, left_payload, right_payload)
            .map(Some)
    }
}

pub(super) fn literal_string_with_payload(
    _expr: &Expr,
    payload: Option<&CompilerExpressionPayload<'_>>,
) -> Option<String> {
    if let Some(Literal::String(value)) =
        payload.and_then(CompilerExpressionPayload::syntax_literal)
    {
        return Some(value);
    }
    None
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LiteralFieldSlotKind {
    Record,
    Enum,
}

fn record_literal_field_slot(expr: &Expr, field: &str) -> Option<(LiteralFieldSlotKind, usize)> {
    let ExprKind::Record { path, fields } = &expr.kind else {
        return None;
    };
    let slot = sorted_field_slot(fields, field)?;
    let kind = if enum_variant_path(path).is_some() {
        LiteralFieldSlotKind::Enum
    } else {
        LiteralFieldSlotKind::Record
    };
    Some((kind, slot))
}

fn sorted_field_slot(fields: &[vela_syntax::ast::RecordField], field: &str) -> Option<usize> {
    let mut names = fields
        .iter()
        .map(|field| field.name.as_str())
        .collect::<Vec<_>>();
    names.sort_unstable();
    names.iter().position(|name| *name == field)
}

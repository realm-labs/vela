use vela_common::{PrimitiveTag, Span};
use vela_syntax::ast::{
    BinaryOp, Expr, ExprKind, InterpolatedStringPart, Literal, RecordField, SyntaxExpressionKind,
    UnaryOp,
};

use crate::{BinaryLiteralSide, FormatStringPart, Register, UnlinkedInstructionKind};

use super::body_payloads::{CompilerExpressionPayload, CompilerRecordFieldPayload};
use super::const_eval::{
    compile_literal_constant, compile_literal_constant_for_type, compile_negated_literal_constant,
};
use super::constructors::{record_field_names, schema_default_fields};
use super::expression_payload_kinds::expression_payload_kind_matches;
use super::host_paths::HostPath;
use super::operators::{
    binary_literal_op, i64_binary_instruction, i64_immediate_instruction,
    i64_immediate_op_supported, non_logical_binary_instruction,
};
use super::patterns::enum_variant_path;
use super::schema_defaults::{record_constructor_diagnostics, unknown_enum_variant_diagnostic};
use super::value_types::RuntimeTypeFact;
use super::{CompileError, CompileErrorKind, CompileResult, Compiler};

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn compile_expr_with_payload(
        &mut self,
        expr: &Expr,
        payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> CompileResult<Register> {
        if let Some(payload) = payload
            && let Some(kind) = payload.kind()
            && expression_payload_kind_matches(kind, expr)
        {
            return self.compile_expr_with_payload_kind(expr, payload, kind);
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
                self.compile_expr_with_payload(expr, inner_payload.as_ref())
            }
            SyntaxExpressionKind::Block => {
                let ExprKind::Block(block) = &expr.kind else {
                    unreachable!("validated CST block expression payload kind");
                };
                let dst = self.alloc_register()?;
                if let Some(body_payload) = payload.block_body_payload() {
                    self.compile_block_payload_value_to(&body_payload, dst)?;
                } else {
                    self.compile_block_value_to(block, dst)?;
                }
                Ok(dst)
            }
            SyntaxExpressionKind::If => {
                let ExprKind::If(if_expr) = &expr.kind else {
                    unreachable!("validated CST if expression payload kind");
                };
                let dst = self.alloc_register()?;
                let if_payload = payload.if_payload();
                self.compile_if_value_with_payloads(if_expr, dst, if_payload.as_ref())?;
                Ok(dst)
            }
            SyntaxExpressionKind::Match => {
                let ExprKind::Match(match_expr) = &expr.kind else {
                    unreachable!("validated CST match expression payload kind");
                };
                let dst = self.alloc_register()?;
                let scrutinee_payload = payload.match_scrutinee_payload();
                let arm_payloads = payload.match_arm_payloads();
                self.compile_match_value_with_payloads(
                    match_expr,
                    dst,
                    scrutinee_payload.as_ref(),
                    arm_payloads.as_deref(),
                )?;
                Ok(dst)
            }
            SyntaxExpressionKind::Path => {
                let ExprKind::Path(path) = &expr.kind else {
                    unreachable!("validated CST path expression payload kind");
                };
                let path = payload
                    .syntax_path_segments()
                    .unwrap_or_else(|| path.to_owned());
                self.compile_path_expr(expr.span, &path)
            }
            SyntaxExpressionKind::Array => {
                let ExprKind::Array(items) = &expr.kind else {
                    unreachable!("validated CST array expression payload kind");
                };
                let element_payloads = payload.array_element_payloads();
                self.compile_array(items, element_payloads.as_deref())
            }
            SyntaxExpressionKind::Map => {
                let ExprKind::Map(entries) = &expr.kind else {
                    unreachable!("validated CST map expression payload kind");
                };
                let entry_payloads = payload.map_entry_payloads();
                self.compile_map(entries, entry_payloads.as_deref())
            }
            SyntaxExpressionKind::Record => {
                let ExprKind::Record { path, fields } = &expr.kind else {
                    unreachable!("validated CST record expression payload kind");
                };
                let field_payloads = payload.record_field_payloads();
                let path = payload
                    .syntax_record_path_segments()
                    .unwrap_or_else(|| path.to_owned());
                self.compile_record(expr, &path, fields, field_payloads.as_deref())
            }
            SyntaxExpressionKind::Binary => {
                let ExprKind::Binary { op, left, right } = &expr.kind else {
                    unreachable!("validated CST binary expression payload kind");
                };
                if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    let operand_payloads = payload.logical_chain_operand_payloads(*op);
                    return self.compile_logical_chain(*op, expr, operand_payloads.as_deref());
                }
                let operand_payloads = payload.binary_operand_payloads();
                let (left_payload, right_payload) = operand_payloads
                    .as_ref()
                    .map_or((None, None), |(left, right)| (Some(left), Some(right)));
                self.compile_binary(*op, expr.span, left, right, left_payload, right_payload)
            }
            SyntaxExpressionKind::Call => {
                let ExprKind::Call { callee, args } = &expr.kind else {
                    unreachable!("validated CST call expression payload kind");
                };
                let callee_payload = payload.call_callee_payload();
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
                let ExprKind::Field { base, name } = &expr.kind else {
                    unreachable!("validated CST field expression payload kind");
                };
                let base_payload = payload.field_base_payload();
                let name = payload
                    .syntax_field_name()
                    .unwrap_or_else(|| name.to_owned());
                self.compile_field_expr(expr, base, &name, base_payload.as_ref(), Some(payload))
            }
            SyntaxExpressionKind::Index => {
                let ExprKind::Index { base, index } = &expr.kind else {
                    unreachable!("validated CST index expression payload kind");
                };
                let operand_payloads = payload.index_operand_payloads();
                let (base_payload, index_payload) = operand_payloads
                    .as_ref()
                    .map_or((None, None), |(base, index)| (Some(base), Some(index)));
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
                let ExprKind::Lambda { params, body } = &expr.kind else {
                    unreachable!("validated CST lambda expression payload kind");
                };
                let body_payload = payload.lambda_body_payload();
                self.compile_lambda(expr, params, body, body_payload.as_ref())
            }
            SyntaxExpressionKind::Unary => {
                let ExprKind::Unary { op, expr: operand } = &expr.kind else {
                    unreachable!("validated CST unary expression payload kind");
                };
                let operand_payload = payload.unary_operand_payload();
                self.compile_unary(*op, operand.span, operand, operand_payload.as_ref())
            }
            SyntaxExpressionKind::Try => {
                let ExprKind::Try(operand) = &expr.kind else {
                    unreachable!("validated CST try expression payload kind");
                };
                let operand_payload = payload.try_operand_payload();
                let src = self.compile_expr_with_payload(operand, operand_payload.as_ref())?;
                let dst = self.alloc_register()?;
                self.emit(UnlinkedInstructionKind::TryPropagate { dst, src });
                Ok(dst)
            }
            SyntaxExpressionKind::Literal => {
                if let ExprKind::InterpolatedString(parts) = &expr.kind {
                    let part_payloads = payload.interpolated_expression_payloads();
                    return self.compile_interpolated_string(parts, part_payloads.as_deref());
                }
                let ExprKind::Literal(_) = &expr.kind else {
                    unreachable!("validated CST literal expression payload kind");
                };
                let literal = payload.syntax_literal().ok_or_else(|| {
                    CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "mismatched CST literal expression",
                    ))
                })?;
                self.compile_literal(Some(expr.span), &literal)
            }
            _ => self.compile_expr(expr),
        }
    }

    pub(super) fn compile_expr(&mut self, expr: &Expr) -> CompileResult<Register> {
        match &expr.kind {
            ExprKind::Literal(literal) => self.compile_literal(Some(expr.span), literal),
            ExprKind::InterpolatedString(parts) => self.compile_interpolated_string(parts, None),
            ExprKind::Path(path) => self.compile_path_expr(expr.span, path),
            ExprKind::Binary { op, left, right } => {
                if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    return self.compile_logical_chain(*op, expr, None);
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
            ExprKind::Block(block) => {
                let dst = self.alloc_register()?;
                self.compile_block_value_to(block, dst)?;
                Ok(dst)
            }
            ExprKind::Array(items) => self.compile_array(items, None),
            ExprKind::Map(entries) => self.compile_map(entries, None),
            ExprKind::Record { path, fields } => self.compile_record(expr, path, fields, None),
            ExprKind::If(if_expr) => {
                let dst = self.alloc_register()?;
                self.compile_if_value_to(if_expr, dst)?;
                Ok(dst)
            }
            ExprKind::Assign { .. } => self.compile_assignment(expr),
            ExprKind::SelfValue => self.local_register_at_span(expr.span, "self"),
            ExprKind::Error => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "expression",
            ))),
            ExprKind::Match(match_expr) => {
                let dst = self.alloc_register()?;
                self.compile_match_value_to(match_expr, dst)?;
                Ok(dst)
            }
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

    fn compile_array(
        &mut self,
        items: &[Expr],
        payloads: Option<&[CompilerExpressionPayload<'_>]>,
    ) -> CompileResult<Register> {
        let elements = items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                self.compile_expr_with_payload(
                    item,
                    payloads.and_then(|payloads| payloads.get(index)),
                )
            })
            .collect::<CompileResult<Vec<_>>>()?;
        let dst = self.alloc_register()?;
        self.emit(UnlinkedInstructionKind::MakeArray { dst, elements });
        Ok(dst)
    }

    fn compile_map(
        &mut self,
        entries: &[vela_syntax::ast::MapEntry],
        payloads: Option<&[super::body_payloads::CompilerMapEntryPayload<'_>]>,
    ) -> CompileResult<Register> {
        let entries = entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                self.compile_map_entry(entry, payloads.and_then(|payloads| payloads.get(index)))
            })
            .collect::<CompileResult<Vec<_>>>()?;
        let dst = self.alloc_register()?;
        self.emit(UnlinkedInstructionKind::MakeMap { dst, entries });
        Ok(dst)
    }

    fn compile_record(
        &mut self,
        expr: &Expr,
        path: &[String],
        fields: &[RecordField],
        payloads: Option<&[CompilerRecordFieldPayload<'_>]>,
    ) -> CompileResult<Register> {
        let dst = self.alloc_register()?;
        if let Some((enum_name, variant)) = enum_variant_path(path) {
            let resolved_enum_name = self.type_symbol_at_span(expr.span);
            let enum_name = resolved_enum_name.clone().unwrap_or(enum_name);
            if resolved_enum_name.is_some()
                && !self.enum_constructor_variant_exists(&enum_name, &variant)
            {
                return Err(self.constructor_diagnostics_error(vec![
                    unknown_enum_variant_diagnostic(&enum_name, &variant, expr.span),
                ]));
            }
            let shape = self.enum_constructor_shape(&enum_name, &variant);
            let field_names = record_field_names(fields, payloads);
            self.reject_constructor_diagnostics(record_constructor_diagnostics(
                &format!("{enum_name}::{variant}"),
                shape.as_ref(),
                fields,
                field_names.as_deref(),
                expr.span,
            ))?;
            let defaults = schema_default_fields(shape.as_ref());
            let fields = self.compile_record_fields(fields, defaults, shape.as_ref(), payloads)?;
            self.emit(UnlinkedInstructionKind::MakeEnum {
                dst,
                enum_name,
                variant,
                fields,
            });
        } else {
            let type_name = self
                .type_symbol_at_span(expr.span)
                .unwrap_or_else(|| path.join("::"));
            let shape = self.record_constructor_shape(&type_name);
            let field_names = record_field_names(fields, payloads);
            self.reject_constructor_diagnostics(record_constructor_diagnostics(
                &type_name,
                shape.as_ref(),
                fields,
                field_names.as_deref(),
                expr.span,
            ))?;
            let defaults = schema_default_fields(shape.as_ref());
            let fields = self.compile_record_fields(fields, defaults, shape.as_ref(), payloads)?;
            self.emit(UnlinkedInstructionKind::MakeRecord {
                dst,
                type_name,
                fields,
            });
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
        if let Some(literal) = unsuffixed_numeric_literal(left) {
            return self.compile_binary_literal_candidate(
                op,
                span,
                right,
                right_payload,
                literal,
                BinaryLiteralSide::Left,
            );
        }
        if let Some(literal) = unsuffixed_numeric_literal(right) {
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
        literal: UnsuffixedNumericLiteral<'_>,
        side: BinaryLiteralSide,
    ) -> CompileResult<Option<Register>> {
        let value_type = self.value_type_for_expr_with_payload(value_expr, value_payload);
        if side == BinaryLiteralSide::Right
            && value_type == Some(RuntimeTypeFact::Primitive(PrimitiveTag::I64))
            && let Some(imm) = self.i64_immediate_literal(literal, span)?
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
            let literal = self.compile_inline_numeric_literal_as(literal, *tag, span)?;
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
        literal: UnsuffixedNumericLiteral<'_>,
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
        literal: UnsuffixedNumericLiteral<'_>,
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

    fn compile_logical_chain(
        &mut self,
        op: BinaryOp,
        expr: &Expr,
        payloads: Option<&[CompilerExpressionPayload<'_>]>,
    ) -> CompileResult<Register> {
        let operands = logical_chain_operands(op, expr);
        match op {
            BinaryOp::And => self.compile_logical_and_chain(&operands, payloads),
            BinaryOp::Or => self.compile_logical_or_chain(&operands, payloads),
            _ => unreachable!("logical chain only supports && and ||"),
        }
    }

    fn compile_logical_and_chain(
        &mut self,
        operands: &[&Expr],
        payloads: Option<&[CompilerExpressionPayload<'_>]>,
    ) -> CompileResult<Register> {
        let dst = self.alloc_register()?;
        let Some((last, prefix)) = operands.split_last() else {
            self.emit_bool_constant_to(dst, true);
            return Ok(dst);
        };

        let mut false_branches = Vec::with_capacity(prefix.len());
        for (index, operand) in prefix.iter().enumerate() {
            let value =
                self.compile_expr_with_payload(operand, payloads.and_then(|p| p.get(index)))?;
            false_branches.push(self.emit_jump_if_false(value));
        }

        let last =
            self.compile_expr_with_payload(last, payloads.and_then(|p| p.get(prefix.len())))?;
        self.emit_truthy_to_bool(dst, last)?;
        let end = self.emit_jump();

        for false_branch in false_branches {
            self.patch_jump(false_branch, self.current_offset())?;
        }
        self.emit_bool_constant_to(dst, false);
        self.patch_jump(end, self.current_offset())?;

        Ok(dst)
    }

    fn compile_logical_or_chain(
        &mut self,
        operands: &[&Expr],
        payloads: Option<&[CompilerExpressionPayload<'_>]>,
    ) -> CompileResult<Register> {
        let dst = self.alloc_register()?;
        let Some((last, prefix)) = operands.split_last() else {
            self.emit_bool_constant_to(dst, false);
            return Ok(dst);
        };

        let mut end_jumps = Vec::with_capacity(prefix.len());
        for (index, operand) in prefix.iter().enumerate() {
            let value =
                self.compile_expr_with_payload(operand, payloads.and_then(|p| p.get(index)))?;
            let next_operand = self.emit_jump_if_false(value);
            self.emit_bool_constant_to(dst, true);
            end_jumps.push(self.emit_jump());
            self.patch_jump(next_operand, self.current_offset())?;
        }

        let last =
            self.compile_expr_with_payload(last, payloads.and_then(|p| p.get(prefix.len())))?;
        self.emit_truthy_to_bool(dst, last)?;
        for end in end_jumps {
            self.patch_jump(end, self.current_offset())?;
        }

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

    fn compile_interpolated_string(
        &mut self,
        parts: &[InterpolatedStringPart],
        payloads: Option<&[CompilerExpressionPayload<'_>]>,
    ) -> CompileResult<Register> {
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
                    let payload = payloads.and_then(|payloads| payloads.get(expression_index));
                    expression_index += 1;
                    compiled.push(FormatStringPart::Value(
                        self.compile_expr_with_payload(expr, payload)?,
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

        let operand_payloads = payload.and_then(CompilerExpressionPayload::binary_operand_payloads);
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

pub(super) fn literal_string(expr: &Expr) -> Option<&str> {
    match &expr.kind {
        ExprKind::Literal(Literal::String(value)) => Some(value),
        _ => None,
    }
}

pub(super) fn literal_string_with_payload(
    expr: &Expr,
    payload: Option<&CompilerExpressionPayload<'_>>,
) -> Option<String> {
    if let Some(Literal::String(value)) =
        payload.and_then(CompilerExpressionPayload::syntax_literal)
    {
        return Some(value);
    }
    literal_string(expr).map(ToOwned::to_owned)
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

fn logical_chain_operands(op: BinaryOp, expr: &Expr) -> Vec<&Expr> {
    let mut operands = Vec::new();
    let mut stack = vec![expr];
    while let Some(expr) = stack.pop() {
        if let ExprKind::Binary {
            op: expr_op,
            left,
            right,
        } = &expr.kind
            && *expr_op == op
        {
            stack.push(right);
            stack.push(left);
            continue;
        }
        operands.push(expr);
    }
    operands
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UnsuffixedNumericLiteral<'a> {
    Integer(&'a str),
    Float(&'a str),
}

impl UnsuffixedNumericLiteral<'_> {
    fn matches_primitive_tag(self, tag: PrimitiveTag) -> bool {
        match self {
            Self::Integer(_) => matches!(
                tag,
                PrimitiveTag::I8
                    | PrimitiveTag::I16
                    | PrimitiveTag::I32
                    | PrimitiveTag::I64
                    | PrimitiveTag::U8
                    | PrimitiveTag::U16
                    | PrimitiveTag::U32
                    | PrimitiveTag::U64
            ),
            Self::Float(_) => matches!(tag, PrimitiveTag::F32 | PrimitiveTag::F64),
        }
    }
}

fn unsuffixed_numeric_literal(expr: &Expr) -> Option<UnsuffixedNumericLiteral<'_>> {
    match &expr.kind {
        ExprKind::Literal(Literal::Integer(value)) if value.suffix.is_none() => {
            Some(UnsuffixedNumericLiteral::Integer(value.source_text()))
        }
        ExprKind::Literal(Literal::Float(value)) if value.suffix.is_none() => {
            Some(UnsuffixedNumericLiteral::Float(value.source_text()))
        }
        _ => None,
    }
}

fn expressions_are_i64(left: Option<RuntimeTypeFact>, right: Option<RuntimeTypeFact>) -> bool {
    matches!(
        (left, right),
        (
            Some(RuntimeTypeFact::Primitive(PrimitiveTag::I64)),
            Some(RuntimeTypeFact::Primitive(PrimitiveTag::I64))
        )
    )
}

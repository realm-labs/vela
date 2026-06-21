use vela_common::{Diagnostic, PrimitiveTag, Span};
use vela_def::MethodId;
use vela_syntax::ast::{
    BinaryOp, Expr, ExprKind, InterpolatedStringPart, Literal, SyntaxExpressionKind, UnaryOp,
};

use crate::{
    BinaryLiteralSide, FormatStringPart, GuardKind, GuardLocation, Register, UnlinkedGuardContext,
    UnlinkedInstructionKind, UnlinkedTypeGuard,
};

use super::body_payloads::CompilerExpressionPayload;
use super::const_eval::{
    compile_literal_constant, compile_literal_constant_for_type, compile_negated_literal_constant,
};
use super::constructors::schema_default_fields;
use super::host_paths::{HostIndexAccessKind, HostPath};
use super::operators::{
    binary_literal_op, i64_binary_instruction, i64_immediate_instruction,
    i64_immediate_op_supported, non_logical_binary_instruction,
};
use super::patterns::enum_variant_path;
use super::record_shapes::ValueShape;
use super::schema_defaults::{record_constructor_diagnostics, unknown_enum_variant_diagnostic};
use super::value_types::{
    ExpectedTypeOutcome, RuntimeTypeFact, StandardRuntimeType, TypeContractContext,
};
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
                let arm_payloads = payload.match_arm_payloads();
                self.compile_match_value_with_payloads(match_expr, dst, arm_payloads.as_deref())?;
                Ok(dst)
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
            _ => self.compile_expr(expr),
        }
    }

    pub(super) fn compile_expr(&mut self, expr: &Expr) -> CompileResult<Register> {
        match &expr.kind {
            ExprKind::Literal(literal) => self.compile_literal(Some(expr.span), literal),
            ExprKind::InterpolatedString(parts) => self.compile_interpolated_string(parts),
            ExprKind::Path(path) => self.compile_path_expr(expr.span, path),
            ExprKind::Binary { op, left, right } => {
                if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    return self.compile_logical_chain(*op, expr);
                }
                self.compile_binary(*op, expr.span, left, right)
            }
            ExprKind::Unary { op, expr } => self.compile_unary(*op, expr.span, expr),
            ExprKind::Field { base, name } => {
                let typed_record_slot = self
                    .script_record_field_slot_for_receiver(base, name)
                    .or_else(|| self.record_field_shape_slot_for_receiver(base, name));
                let typed_enum_slot = self.script_enum_field_slot_for_receiver(base, name);
                if let Some((slot_kind, slot)) = record_literal_field_slot(base, name) {
                    let root = self.compile_expr(base)?;
                    let dst = self.alloc_register()?;
                    match slot_kind {
                        LiteralFieldSlotKind::Record => {
                            self.emit(UnlinkedInstructionKind::GetRecordSlot {
                                dst,
                                record: root,
                                field: name.clone(),
                                slot,
                            })
                        }
                        LiteralFieldSlotKind::Enum => {
                            self.emit(UnlinkedInstructionKind::GetEnumSlot {
                                dst,
                                value: root,
                                field: name.clone(),
                                slot,
                            })
                        }
                    }
                    Ok(dst)
                } else if let Some(slot) = typed_record_slot {
                    let root = self.compile_expr(base)?;
                    let dst = self.alloc_register()?;
                    self.emit(UnlinkedInstructionKind::GetRecordSlot {
                        dst,
                        record: root,
                        field: name.clone(),
                        slot,
                    });
                    Ok(dst)
                } else if let Some(slot) = typed_enum_slot {
                    let root = self.compile_expr(base)?;
                    let dst = self.alloc_register()?;
                    self.emit(UnlinkedInstructionKind::GetEnumSlot {
                        dst,
                        value: root,
                        field: name.clone(),
                        slot,
                    });
                    Ok(dst)
                } else {
                    if let Some(path) = self.host_field_path(expr)
                        && path.requires_path_instruction()
                    {
                        let root = self.compile_host_path_root(path.root)?;
                        let dst = self.alloc_register()?;
                        self.emit_host_read(dst, root, path, expr.span)?;
                        return Ok(dst);
                    }
                    let root = self.compile_expr(base)?;
                    let dst = self.alloc_register()?;
                    let receiver_type = self.script_type_for_expr(base);
                    if let Some(field) = self
                        .host_field_info(receiver_type.as_deref(), name)
                        .map(|field| field.id)
                    {
                        let path = HostPath {
                            root: super::host_paths::HostPathRoot::Expr(base),
                            segments: vec![super::host_paths::HostPathPart::Field(field)],
                        };
                        self.emit_host_read(dst, root, path, expr.span)?;
                    } else {
                        self.emit(UnlinkedInstructionKind::GetRecordField {
                            dst,
                            record: root,
                            field: name.clone(),
                        });
                    }
                    Ok(dst)
                }
            }
            ExprKind::Index { base, index } => {
                if let Some(path) = self.host_field_path(expr)
                    && !path.segments.is_empty()
                {
                    self.reject_invalid_host_index_access(
                        expr,
                        base,
                        index,
                        HostIndexAccessKind::Read,
                    )?;
                    let root = self.compile_host_path_root(path.root)?;
                    let dst = self.alloc_register()?;
                    self.emit_host_read(dst, root, path, expr.span)?;
                    return Ok(dst);
                }
                self.reject_invalid_host_index_access(
                    expr,
                    base,
                    index,
                    HostIndexAccessKind::Read,
                )?;
                let base = self.compile_expr(base)?;
                let dst = self.alloc_register()?;
                if let Some(key) = literal_string(index) {
                    let key = self
                        .code
                        .push_constant(crate::Constant::String(key.to_owned()));
                    self.emit(UnlinkedInstructionKind::GetStringKeyIndex { dst, base, key });
                } else {
                    let index = self.compile_expr(index)?;
                    self.emit(UnlinkedInstructionKind::GetIndex { dst, base, index });
                }
                Ok(dst)
            }
            ExprKind::Call { callee, args } => self.compile_call_expr(expr, callee, args),
            ExprKind::Lambda { params, body } => self.compile_lambda(expr, params, body),
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
            ExprKind::Record { path, fields } => {
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
                    self.reject_constructor_diagnostics(record_constructor_diagnostics(
                        &format!("{enum_name}::{variant}"),
                        shape.as_ref(),
                        fields,
                        expr.span,
                    ))?;
                    let defaults = schema_default_fields(shape.as_ref());
                    let fields = self.compile_record_fields(fields, defaults, shape.as_ref())?;
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
                    self.reject_constructor_diagnostics(record_constructor_diagnostics(
                        &type_name,
                        shape.as_ref(),
                        fields,
                        expr.span,
                    ))?;
                    let defaults = schema_default_fields(shape.as_ref());
                    let fields = self.compile_record_fields(fields, defaults, shape.as_ref())?;
                    self.emit(UnlinkedInstructionKind::MakeRecord {
                        dst,
                        type_name,
                        fields,
                    });
                }
                Ok(dst)
            }
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

    pub(super) fn compile_expr_with_expected_type(
        &mut self,
        expr: &Expr,
        expected: RuntimeTypeFact,
        context: TypeContractContext,
    ) -> CompileResult<Register> {
        let outcome = self.expected_type_for_expr(expr, expected, context.clone())?;
        if let ExpectedTypeOutcome::Contextualized(RuntimeTypeFact::Primitive(tag)) = &outcome
            && let ExprKind::Literal(literal) = &expr.kind
            && let Some(constant) = compile_literal_constant_for_type(literal, *tag)
                .map_err(|error| error.with_span(expr.span))?
        {
            return self.emit_constant(constant);
        }
        let register = self.compile_expr(expr)?;
        if let ExpectedTypeOutcome::RequiresRuntimeGuard(expected) = &outcome
            && let Some((location, name)) = guard_location_and_name(context)
            && let Some(plan) = super::type_guard_plan_for_runtime_type(expected)
        {
            self.emit_spanned(
                UnlinkedInstructionKind::GuardType {
                    src: register,
                    guard: UnlinkedTypeGuard::new(
                        plan,
                        UnlinkedGuardContext::new(GuardKind::Contract, location, name),
                    ),
                },
                expr.span,
            );
        }
        Ok(register)
    }

    fn compile_binary(
        &mut self,
        op: BinaryOp,
        span: Span,
        left: &Expr,
        right: &Expr,
    ) -> CompileResult<Register> {
        match op {
            BinaryOp::Range => return self.compile_range(left, right, false),
            BinaryOp::RangeInclusive => return self.compile_range(left, right, true),
            _ => {}
        }
        self.reject_static_identity_comparison_operands(op, span, left, right)?;
        self.reject_static_comparison_without_trait(op, span, left)?;

        if let Some(register) = self.compile_binary_with_inline_literal(op, span, left, right)? {
            return Ok(register);
        }

        let lhs = self.compile_expr(left)?;
        let rhs = self.compile_expr(right)?;
        let dst = self.alloc_register()?;
        let instruction = if expressions_are_i64(
            self.value_type_for_expr(left),
            self.value_type_for_expr(right),
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

    fn reject_static_identity_comparison_operands(
        &self,
        op: BinaryOp,
        span: Span,
        left: &Expr,
        right: &Expr,
    ) -> CompileResult<()> {
        if !matches!(op, BinaryOp::IdentityEqual | BinaryOp::IdentityNotEqual) {
            return Ok(());
        }
        for (side, expr) in [("left", left), ("right", right)] {
            if let Some(type_name) = self.static_non_identity_operand_type(expr) {
                return Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
                    vec![
                        Diagnostic::error(format!(
                            "`{}` requires reference identity operands, but the {side} operand has type `{type_name}`",
                            op.source_name()
                        ))
                        .with_code("compiler::invalid_identity_comparison")
                        .with_span(span)
                        .with_label(span, "identity comparison requires reference operands")
                        .with_label(
                            expr.span,
                            format!("{side} operand is statically `{type_name}`"),
                        ),
                    ],
                )));
            }
        }
        Ok(())
    }

    fn static_non_identity_operand_type(&self, expr: &Expr) -> Option<String> {
        if let Some(fact) = self.value_type_for_expr(expr) {
            return (!runtime_type_is_identity_operand(&fact)).then(|| fact.source_type_display());
        }
        if let Some(shape) = self.value_shape_for_expr(expr) {
            return non_identity_shape_type(&shape);
        }
        None
    }

    fn reject_static_comparison_without_trait(
        &self,
        op: BinaryOp,
        span: Span,
        left: &Expr,
    ) -> CompileResult<()> {
        let Some(requirement) = ComparisonTraitRequirement::for_op(op) else {
            return Ok(());
        };
        let Some(type_name) = self.script_type_for_expr(left) else {
            return Ok(());
        };
        if !self.is_declared_script_type(&type_name)
            || self.type_implements_builtin_trait_method(
                &type_name,
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

    fn compile_binary_with_inline_literal(
        &mut self,
        op: BinaryOp,
        span: Span,
        left: &Expr,
        right: &Expr,
    ) -> CompileResult<Option<Register>> {
        if let Some(literal) = unsuffixed_numeric_literal(left) {
            return self.compile_binary_literal_candidate(
                op,
                span,
                right,
                literal,
                BinaryLiteralSide::Left,
            );
        }
        if let Some(literal) = unsuffixed_numeric_literal(right) {
            return self.compile_binary_literal_candidate(
                op,
                span,
                left,
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
        literal: UnsuffixedNumericLiteral<'_>,
        side: BinaryLiteralSide,
    ) -> CompileResult<Option<Register>> {
        if side == BinaryLiteralSide::Right
            && self.value_type_for_expr(value_expr)
                == Some(RuntimeTypeFact::Primitive(PrimitiveTag::I64))
            && let Some(imm) = self.i64_immediate_literal(literal, span)?
            && i64_immediate_op_supported(op, imm)
        {
            let value = self.compile_expr(value_expr)?;
            let dst = self.alloc_register()?;
            let instruction = i64_immediate_instruction(op, dst, value, imm)
                .expect("support was checked before compiling the value expression");
            self.emit_spanned(instruction, span);
            return Ok(Some(dst));
        }

        let Some(literal_op) = binary_literal_op(op) else {
            return Ok(None);
        };
        if let Some(RuntimeTypeFact::Primitive(tag)) = self.value_type_for_expr(value_expr)
            && literal.matches_primitive_tag(tag)
        {
            let value = self.compile_expr(value_expr)?;
            let literal = self.compile_inline_numeric_literal_as(literal, tag, span)?;
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

        if self.value_type_for_expr(value_expr).is_none() {
            let value = self.compile_expr(value_expr)?;
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
    ) -> CompileResult<Register> {
        let start = self.compile_expr(left)?;
        let end = self.compile_expr(right)?;
        let dst = self.alloc_register()?;
        self.emit(UnlinkedInstructionKind::MakeRange {
            dst,
            start,
            end,
            inclusive,
        });
        Ok(dst)
    }

    fn compile_logical_chain(&mut self, op: BinaryOp, expr: &Expr) -> CompileResult<Register> {
        let operands = logical_chain_operands(op, expr);
        match op {
            BinaryOp::And => self.compile_logical_and_chain(&operands),
            BinaryOp::Or => self.compile_logical_or_chain(&operands),
            _ => unreachable!("logical chain only supports && and ||"),
        }
    }

    fn compile_logical_and_chain(&mut self, operands: &[&Expr]) -> CompileResult<Register> {
        let dst = self.alloc_register()?;
        let Some((last, prefix)) = operands.split_last() else {
            self.emit_bool_constant_to(dst, true);
            return Ok(dst);
        };

        let mut false_branches = Vec::with_capacity(prefix.len());
        for operand in prefix {
            let value = self.compile_expr(operand)?;
            false_branches.push(self.emit_jump_if_false(value));
        }

        let last = self.compile_expr(last)?;
        self.emit_truthy_to_bool(dst, last)?;
        let end = self.emit_jump();

        for false_branch in false_branches {
            self.patch_jump(false_branch, self.current_offset())?;
        }
        self.emit_bool_constant_to(dst, false);
        self.patch_jump(end, self.current_offset())?;

        Ok(dst)
    }

    fn compile_logical_or_chain(&mut self, operands: &[&Expr]) -> CompileResult<Register> {
        let dst = self.alloc_register()?;
        let Some((last, prefix)) = operands.split_last() else {
            self.emit_bool_constant_to(dst, false);
            return Ok(dst);
        };

        let mut end_jumps = Vec::with_capacity(prefix.len());
        for operand in prefix {
            let value = self.compile_expr(operand)?;
            let next_operand = self.emit_jump_if_false(value);
            self.emit_bool_constant_to(dst, true);
            end_jumps.push(self.emit_jump());
            self.patch_jump(next_operand, self.current_offset())?;
        }

        let last = self.compile_expr(last)?;
        self.emit_truthy_to_bool(dst, last)?;
        for end in end_jumps {
            self.patch_jump(end, self.current_offset())?;
        }

        Ok(dst)
    }

    fn emit_truthy_to_bool(&mut self, dst: Register, src: Register) -> CompileResult<()> {
        self.emit(UnlinkedInstructionKind::Truthy { dst, src });
        Ok(())
    }

    fn compile_interpolated_string(
        &mut self,
        parts: &[InterpolatedStringPart],
    ) -> CompileResult<Register> {
        let mut compiled = Vec::with_capacity(parts.len());
        for part in parts {
            match part {
                InterpolatedStringPart::Text(value) => {
                    let constant = self
                        .code
                        .push_constant(crate::Constant::String(value.clone()));
                    compiled.push(FormatStringPart::Text(constant));
                }
                InterpolatedStringPart::Expr(expr) => {
                    compiled.push(FormatStringPart::Value(self.compile_expr(expr)?));
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

    fn compile_unary(&mut self, op: UnaryOp, span: Span, expr: &Expr) -> CompileResult<Register> {
        if op == UnaryOp::Not
            && let Some(register) = self.compile_negated_equality(span, expr)?
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

        let src = self.compile_expr(expr)?;
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

        let inverse = match equality_op {
            BinaryOp::Equal => BinaryOp::NotEqual,
            BinaryOp::NotEqual => BinaryOp::Equal,
            BinaryOp::IdentityEqual => BinaryOp::IdentityNotEqual,
            BinaryOp::IdentityNotEqual => BinaryOp::IdentityEqual,
            _ => unreachable!("binary equality was matched above"),
        };
        self.compile_binary(inverse, span, left, right).map(Some)
    }
}

impl Compiler<'_, '_> {
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

fn runtime_type_is_identity_operand(fact: &RuntimeTypeFact) -> bool {
    match fact {
        RuntimeTypeFact::Primitive(_) | RuntimeTypeFact::Standard(StandardRuntimeType::Range) => {
            false
        }
        RuntimeTypeFact::Standard(
            StandardRuntimeType::Array
            | StandardRuntimeType::Map
            | StandardRuntimeType::Set
            | StandardRuntimeType::Function
            | StandardRuntimeType::Closure
            | StandardRuntimeType::Iterator
            | StandardRuntimeType::Option
            | StandardRuntimeType::Result,
        )
        | RuntimeTypeFact::Array(_)
        | RuntimeTypeFact::Map { .. }
        | RuntimeTypeFact::Set(_)
        | RuntimeTypeFact::Iterator(_)
        | RuntimeTypeFact::Option(_)
        | RuntimeTypeFact::Result { .. } => true,
    }
}

fn expression_payload_kind_matches(kind: SyntaxExpressionKind, expr: &Expr) -> bool {
    match kind {
        SyntaxExpressionKind::Block => matches!(expr.kind, ExprKind::Block(_)),
        SyntaxExpressionKind::If => matches!(expr.kind, ExprKind::If(_)),
        SyntaxExpressionKind::Match => matches!(expr.kind, ExprKind::Match(_)),
        SyntaxExpressionKind::Array => matches!(expr.kind, ExprKind::Array(_)),
        SyntaxExpressionKind::Map => matches!(expr.kind, ExprKind::Map(_)),
        _ => !matches!(
            expr.kind,
            ExprKind::Block(_)
                | ExprKind::If(_)
                | ExprKind::Match(_)
                | ExprKind::Array(_)
                | ExprKind::Map(_)
        ),
    }
}

fn non_identity_shape_type(shape: &ValueShape) -> Option<String> {
    match shape {
        ValueShape::Scalar(type_name) => Some(type_name.clone()),
        ValueShape::Unknown
        | ValueShape::Record(_)
        | ValueShape::Array(_)
        | ValueShape::Iterator(_)
        | ValueShape::Map { .. }
        | ValueShape::Set(_)
        | ValueShape::Option(_)
        | ValueShape::Result { .. } => None,
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
                operator: op.source_name(),
            }),
            BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => {
                Some(Self {
                    trait_name: "PartialOrd",
                    method_name: "partial_cmp",
                    operator: op.source_name(),
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

trait BinaryOpName {
    fn source_name(self) -> &'static str;
}

impl BinaryOpName for BinaryOp {
    fn source_name(self) -> &'static str {
        match self {
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
}

fn builtin_trait_method_id(trait_name: &str, method_name: &str) -> MethodId {
    MethodId::new(u128::from(vela_common::stable_id(
        "trait_method",
        trait_name,
        method_name,
    )))
}

pub(super) fn literal_string(expr: &Expr) -> Option<&str> {
    match &expr.kind {
        ExprKind::Literal(Literal::String(value)) => Some(value),
        _ => None,
    }
}

fn guard_location_and_name(context: TypeContractContext) -> Option<(GuardLocation, String)> {
    match context {
        TypeContractContext::TypedLet { name } => Some((GuardLocation::Local, name)),
        TypeContractContext::Field { name } => Some((GuardLocation::Field, name)),
        TypeContractContext::NativeParameter { name, index, .. } => {
            Some((GuardLocation::Parameter { index }, name))
        }
        TypeContractContext::FunctionParameter { .. } | TypeContractContext::Return => None,
    }
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

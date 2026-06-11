use vela_common::{PrimitiveTag, Span};
use vela_syntax::ast::{BinaryOp, Expr, ExprKind, Literal, UnaryOp};

use crate::{
    BinaryLiteralSide, GuardKind, GuardLocation, Register, UnlinkedGuardContext,
    UnlinkedInstructionKind, UnlinkedTypeGuard, UnlinkedTypeGuardPlan,
};

use super::const_eval::{
    compile_literal_constant, compile_literal_constant_for_type, compile_negated_literal_constant,
};
use super::constructors::schema_default_fields;
use super::host_paths::{HostIndexAccessKind, HostPath};
use super::operators::{binary_literal_op, non_logical_binary_instruction};
use super::patterns::enum_variant_path;
use super::schema_defaults::{record_constructor_diagnostics, unknown_enum_variant_diagnostic};
use super::value_types::{ExpectedTypeOutcome, RuntimeTypeFact, TypeContractContext};
use super::{CompileError, CompileErrorKind, CompileResult, Compiler};

impl Compiler<'_, '_> {
    pub(super) fn compile_expr(&mut self, expr: &Expr) -> CompileResult<Register> {
        match &expr.kind {
            ExprKind::Literal(literal) => self.compile_literal(Some(expr.span), literal),
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
                let index = self.compile_expr(index)?;
                let dst = self.alloc_register()?;
                self.emit(UnlinkedInstructionKind::GetIndex { dst, base, index });
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
            ExprKind::Array(items) => {
                let elements = items
                    .iter()
                    .map(|item| self.compile_expr(item))
                    .collect::<CompileResult<Vec<_>>>()?;
                let dst = self.alloc_register()?;
                self.emit(UnlinkedInstructionKind::MakeArray { dst, elements });
                Ok(dst)
            }
            ExprKind::Map(entries) => {
                let entries = entries
                    .iter()
                    .map(|entry| self.compile_map_entry(entry))
                    .collect::<CompileResult<Vec<_>>>()?;
                let dst = self.alloc_register()?;
                self.emit(UnlinkedInstructionKind::MakeMap { dst, entries });
                Ok(dst)
            }
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
        if let ExpectedTypeOutcome::RequiresRuntimeGuard(RuntimeTypeFact::Primitive(tag)) = &outcome
            && let Some((location, name)) = guard_location_and_name(context)
        {
            self.emit_spanned(
                UnlinkedInstructionKind::GuardType {
                    src: register,
                    guard: UnlinkedTypeGuard::new(
                        UnlinkedTypeGuardPlan::Primitive(*tag),
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

        if let Some(register) = self.compile_binary_with_inline_literal(op, span, left, right)? {
            return Ok(register);
        }

        let lhs = self.compile_expr(left)?;
        let rhs = self.compile_expr(right)?;
        let dst = self.alloc_register()?;
        let instruction = non_logical_binary_instruction(op, dst, lhs, rhs)
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
            op: equality_op @ (BinaryOp::Equal | BinaryOp::NotEqual),
            left,
            right,
        } = &expr.kind
        else {
            return Ok(None);
        };

        let inverse = match equality_op {
            BinaryOp::Equal => BinaryOp::NotEqual,
            BinaryOp::NotEqual => BinaryOp::Equal,
            _ => unreachable!("binary equality was matched above"),
        };
        self.compile_binary(inverse, span, left, right).map(Some)
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

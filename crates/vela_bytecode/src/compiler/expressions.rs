use vela_common::Span;
use vela_syntax::ast::{BinaryOp, Expr, ExprKind, Literal, UnaryOp};

use crate::{Register, UnlinkedInstructionKind};

use super::const_eval::compile_literal_constant;
use super::constructors::schema_default_fields;
use super::host_paths::HostPath;
use super::operators::non_logical_binary_instruction;
use super::patterns::enum_variant_path;
use super::schema_defaults::{record_constructor_diagnostics, unknown_enum_variant_diagnostic};
use super::{CompileError, CompileErrorKind, CompileResult, Compiler};

impl Compiler<'_, '_> {
    pub(super) fn compile_expr(&mut self, expr: &Expr) -> CompileResult<Register> {
        match &expr.kind {
            ExprKind::Literal(literal) => self.compile_literal(literal),
            ExprKind::Path(path) => self.compile_path_expr(expr.span, path),
            ExprKind::Binary { op, left, right } => {
                if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    return self.compile_logical_chain(*op, expr);
                }
                self.compile_binary(*op, expr.span, left, right)
            }
            ExprKind::Unary { op, expr } => self.compile_unary(*op, expr.span, expr),
            ExprKind::Field { base, name } => {
                let typed_record_slot = self.script_record_field_slot_for_receiver(base, name);
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
                    let root = self.compile_host_path_root(path.root)?;
                    let dst = self.alloc_register()?;
                    self.emit_host_read(dst, root, path, expr.span)?;
                    return Ok(dst);
                }
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
                    let fields = self.compile_record_fields(fields, defaults)?;
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
                    let fields = self.compile_record_fields(fields, defaults)?;
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

    pub(super) fn compile_literal(&mut self, literal: &Literal) -> CompileResult<Register> {
        self.emit_constant(compile_literal_constant(literal)?)
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

        let lhs = self.compile_expr(left)?;
        let rhs = self.compile_expr(right)?;
        let dst = self.alloc_register()?;
        let instruction = non_logical_binary_instruction(op, dst, lhs, rhs)
            .expect("logical operators handled above");
        self.emit_spanned(instruction, span);
        Ok(dst)
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

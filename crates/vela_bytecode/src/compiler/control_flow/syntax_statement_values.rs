use std::collections::BTreeSet;

use vela_common::{PrimitiveTag, SourceId, Span};
use vela_hir::binding::LocalBindingKind;
use vela_syntax::SyntaxKind;
use vela_syntax::ast::{
    AssignOp, AstNode, BinaryOp, Literal, SyntaxElseBranch, SyntaxExpression, SyntaxIfExpr,
    SyntaxLiteral, UnaryOp,
};
use vela_syntax::token::{InterpolatedStringTokenPart, TokenKind};

use crate::compiler::body_payloads::{
    CompilerBodyPayload, expression_syntax_literal, expression_syntax_path_field,
    expression_syntax_path_or_field, expression_syntax_path_or_self,
};
use crate::compiler::calls::unresolved_static_method_error;
use crate::compiler::const_eval::{
    compile_literal_constant_for_type, compile_negated_literal_constant,
};
use crate::compiler::constructors::schema_default_fields;
use crate::compiler::expected_exprs::guard_location_and_name;
use crate::compiler::operators::{
    binary_literal_op, compound_assignment_instruction, i64_compound_assignment_instruction,
    i64_immediate_instruction, i64_immediate_op_supported, non_logical_binary_instruction,
};
use crate::compiler::param_defaults::syntax_map_key_name;
use crate::compiler::patterns::enum_variant_path;
use crate::compiler::schema_defaults::unknown_enum_variant_diagnostic;
use crate::compiler::value_types::{
    ExpectedTypeOutcome, RuntimeTypeFact, StaticExprType, TypeContractContext, check_expected_type,
};
use crate::compiler::{CompileResult, Compiler, frame_slot_kind, type_guard_plan_for_runtime_type};
use crate::function_id_for_script_name;
use crate::{BinaryLiteralSide, CallArgument, Constant, FormatStringPart, ScriptCallMode};
use crate::{
    GuardKind, Register, UnlinkedGuardContext, UnlinkedInstructionKind, UnlinkedTypeGuard,
};

impl Compiler<'_, '_> {
    pub(in crate::compiler::control_flow) fn compile_let_syntax_expression(
        &mut self,
        source: SourceId,
        name: String,
        span: Span,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<bool>> {
        let Some(register) = self.compile_syntax_expression(source, expression)? else {
            return Ok(None);
        };
        let value_shape = self.value_shape_for_syntax_expression(Some(source), expression);
        let value_type = self
            .syntax_value_type_for_expression(Some(source), expression)
            .or_else(|| value_shape.as_ref().and_then(|shape| shape.value_type()));
        self.locals.insert(name.clone(), register);
        let local_binding = self
            .bindings
            .local_named_at(&name, LocalBindingKind::Let, span);
        if let Some(local) = local_binding {
            self.hir_locals.insert(local, register);
            self.script_types.set_local_fact(local, name.clone(), None);
            self.value_types.set_local(local, name.clone(), value_type);
            self.value_shapes
                .set_local(local, name.clone(), value_shape);
        } else {
            self.script_types.set_name_fact(name.clone(), None);
            self.value_types.set_name(name.clone(), value_type);
            self.value_shapes.set_name(name.clone(), value_shape);
        }
        self.record_frame_slot(
            name,
            register,
            frame_slot_kind(LocalBindingKind::Let),
            local_binding,
            Some(span),
        );
        Ok(Some(false))
    }

    pub(in crate::compiler::control_flow) fn compile_return_syntax_expression(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<bool>> {
        let Some(register) = self.compile_syntax_expression(source, expression)? else {
            return Ok(None);
        };
        self.emit(UnlinkedInstructionKind::Return { src: register });
        Ok(Some(true))
    }

    pub(in crate::compiler::control_flow) fn compile_syntax_value_expr_statement(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<bool>> {
        let Some(_register) = self.compile_syntax_expression(source, expression)? else {
            return Ok(None);
        };
        Ok(Some(false))
    }

    pub(in crate::compiler::control_flow) fn compile_syntax_if_statement(
        &mut self,
        source: SourceId,
        if_expr: &SyntaxIfExpr,
    ) -> CompileResult<Option<bool>> {
        let Some(condition_expression) = if_expr.condition() else {
            return Ok(None);
        };
        let Some(then_block) = if_expr.then_block() else {
            return Ok(None);
        };
        let then_body = CompilerBodyPayload::nested_syntax(source, then_block);

        let jump_to_else = if let Some(jump) =
            self.try_emit_syntax_i64_immediate_jump_if_false(source, &condition_expression)?
        {
            jump
        } else {
            let Some(condition) = self.compile_syntax_expression(source, &condition_expression)?
            else {
                return Ok(None);
            };
            self.emit_jump_if_false(condition)
        };
        let then_returned = self.compile_body_payload_statements(&then_body)?;
        let jump_to_end = if then_returned {
            None
        } else {
            Some(self.emit_jump())
        };

        self.patch_jump(jump_to_else, self.current_offset())?;
        let else_returned = match if_expr.else_branch() {
            Some(SyntaxElseBranch::If(else_if)) => {
                let Some(returned) = self.compile_syntax_if_statement(source, &else_if)? else {
                    return Ok(None);
                };
                returned
            }
            Some(SyntaxElseBranch::Block(block)) => {
                let else_body = CompilerBodyPayload::nested_syntax(source, block);
                self.compile_body_payload_statements(&else_body)?
            }
            None => false,
        };

        if let Some(jump_to_end) = jump_to_end {
            self.patch_jump(jump_to_end, self.current_offset())?;
        }
        Ok(Some(then_returned && else_returned))
    }

    fn try_emit_syntax_i64_immediate_jump_if_false(
        &mut self,
        source: SourceId,
        condition: &SyntaxExpression,
    ) -> CompileResult<Option<usize>> {
        let Some(binary) = condition.as_binary() else {
            return Ok(None);
        };
        let Some(op) = binary
            .operator()
            .and_then(crate::compiler::operators::i64_compare_op)
        else {
            return Ok(None);
        };
        let Some(lhs_expression) = binary.lhs() else {
            return Ok(None);
        };
        let Some(rhs_expression) = binary.rhs() else {
            return Ok(None);
        };
        let Some(lhs_path) = expression_syntax_path_or_field(&lhs_expression) else {
            return Ok(None);
        };
        let lhs_span = syntax_expression_span(source, &lhs_expression);
        if self.value_type_for_path(lhs_span, &lhs_path)
            != Some(RuntimeTypeFact::Primitive(PrimitiveTag::I64))
        {
            return Ok(None);
        }
        let Some(Literal::Integer(value)) = expression_syntax_literal(&rhs_expression) else {
            return Ok(None);
        };
        let Some(Constant::Scalar(vela_common::ScalarValue::I64(imm))) =
            compile_literal_constant_for_type(&Literal::Integer(value), PrimitiveTag::I64)
                .map_err(|error| {
                    error.with_span(syntax_expression_span(source, &rhs_expression))
                })?
        else {
            return Ok(None);
        };
        let lhs = self.compile_path_expr(lhs_span, &lhs_path)?;
        let offset = self.current_offset();
        self.emit(UnlinkedInstructionKind::I64CmpImmJumpIfFalse {
            op,
            lhs,
            imm,
            target: crate::InstructionOffset(usize::MAX),
        });
        Ok(Some(offset))
    }

    pub(in crate::compiler::control_flow) fn compile_syntax_value_expr_to(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        dst: Register,
    ) -> CompileResult<Option<bool>> {
        let Some(value) = self.compile_syntax_expression(source, expression)? else {
            return Ok(None);
        };
        if value != dst {
            self.emit(UnlinkedInstructionKind::Move { dst, src: value });
        }
        Ok(Some(false))
    }

    pub(in crate::compiler) fn compile_syntax_expression(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
            return self.compile_syntax_expression(source, &inner);
        }
        if let Some(literal) = expression_syntax_literal(expression) {
            return self
                .compile_literal(Some(syntax_expression_span(source, expression)), &literal)
                .map(Some);
        }
        if let Some(register) = self.compile_syntax_interpolated_string(source, expression)? {
            return Ok(Some(register));
        }
        if let Some(path) = expression_syntax_path_or_self(expression) {
            return self
                .compile_path_expr(syntax_expression_span(source, expression), &path)
                .map(Some);
        }
        if let Some(path) = expression_syntax_path_field(expression) {
            return self
                .compile_path_expr(syntax_expression_span(source, expression), &path)
                .map(Some);
        }
        if let Some(register) = self.compile_syntax_field_read(source, expression)? {
            return Ok(Some(register));
        }
        if let Some(register) = self.compile_syntax_unary(source, expression)? {
            return Ok(Some(register));
        }
        if let Some(register) =
            self.compile_syntax_lambda_with_callback_shapes(source, expression, &[])?
        {
            return Ok(Some(register));
        }
        if let Some(block) = expression.as_block() {
            let dst = self.alloc_register()?;
            let body = CompilerBodyPayload::nested_syntax(source, block);
            self.compile_block_payload_value_to(&body, dst)?;
            return Ok(Some(dst));
        }
        if let Some(register) = self.compile_syntax_index(source, expression)? {
            return Ok(Some(register));
        }
        if let Some(register) = self.compile_syntax_assignment(source, expression)? {
            return Ok(Some(register));
        }
        if let Some(register) = self.compile_syntax_call(source, expression)? {
            return Ok(Some(register));
        }
        if let Some(register) = self.compile_syntax_try(source, expression)? {
            return Ok(Some(register));
        }
        if let Some(register) = self.compile_syntax_container(source, expression)? {
            return Ok(Some(register));
        }
        if let Some(register) = self.compile_syntax_if_value(source, expression)? {
            return Ok(Some(register));
        }
        if let Some(register) = self.compile_syntax_match_value(source, expression)? {
            return Ok(Some(register));
        }
        let Some(binary) = expression.as_binary() else {
            return Ok(None);
        };
        let Some(op) = binary.operator() else {
            return Ok(None);
        };
        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            return self.compile_syntax_logical_chain(source, expression, op);
        }
        let Some(lhs_expression) = binary.lhs() else {
            return Ok(None);
        };
        let Some(rhs_expression) = binary.rhs() else {
            return Ok(None);
        };
        if matches!(op, BinaryOp::Range | BinaryOp::RangeInclusive) {
            let inclusive = op == BinaryOp::RangeInclusive;
            return self
                .compile_syntax_range_value(source, &lhs_expression, &rhs_expression, inclusive)
                .map(Some);
        }
        if let Some(register) = self.compile_syntax_path_numeric_literal_binary(
            source,
            op,
            expression,
            &lhs_expression,
            &rhs_expression,
        )? {
            return Ok(Some(register));
        }
        self.reject_static_syntax_path_binary_operands(
            source,
            op,
            expression,
            &lhs_expression,
            &rhs_expression,
        )?;
        let Some(lhs) = self.compile_syntax_expression(source, &lhs_expression)? else {
            return Ok(None);
        };
        let Some(rhs) = self.compile_syntax_expression(source, &rhs_expression)? else {
            return Ok(None);
        };
        let dst = self.alloc_register()?;
        let Some(instruction) = non_logical_binary_instruction(op, dst, lhs, rhs) else {
            return Ok(None);
        };
        self.emit_spanned(instruction, syntax_expression_span(source, expression));
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
            .script_fact_for_syntax_expression(source, &receiver_expression)
            .and_then(|fact| {
                let variant = fact.enum_variant.as_deref()?;
                self.facts
                    .script_field_slots
                    .enum_variant(&fact.type_name, variant, &field_name)
            });
        let Some(record) = self.compile_syntax_expression(source, &receiver_expression)? else {
            return Ok(None);
        };
        let dst = self.alloc_register()?;
        if let Some(slot) = record_slot {
            self.emit(UnlinkedInstructionKind::GetRecordSlot {
                dst,
                record,
                field: field_name,
                slot,
            });
        } else if let Some(slot) = enum_slot {
            self.emit(UnlinkedInstructionKind::GetEnumSlot {
                dst,
                value: record,
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

    fn compile_syntax_logical_chain(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        op: BinaryOp,
    ) -> CompileResult<Option<Register>> {
        let Some(operands) = logical_chain_syntax_operands(expression, op) else {
            return Ok(None);
        };
        match op {
            BinaryOp::And => self.compile_syntax_logical_and_chain(source, &operands),
            BinaryOp::Or => self.compile_syntax_logical_or_chain(source, &operands),
            _ => unreachable!("logical chain only supports && and ||"),
        }
    }

    fn compile_syntax_logical_and_chain(
        &mut self,
        source: SourceId,
        operands: &[SyntaxExpression],
    ) -> CompileResult<Option<Register>> {
        let dst = self.alloc_register()?;
        let Some((last, prefix)) = operands.split_last() else {
            self.emit_bool_constant_to(dst, true);
            return Ok(Some(dst));
        };

        let mut false_branches = Vec::with_capacity(prefix.len());
        for operand in prefix {
            let Some(value) = self.compile_syntax_expression(source, operand)? else {
                return Err(crate::compiler::CompileError::new(
                    crate::compiler::CompileErrorKind::UnsupportedSyntax(
                        "unsupported CST logical operand",
                    ),
                )
                .with_span(syntax_expression_span(source, operand)));
            };
            false_branches.push(self.emit_jump_if_false(value));
        }

        let Some(last) = self.compile_syntax_expression(source, last)? else {
            return Err(crate::compiler::CompileError::new(
                crate::compiler::CompileErrorKind::UnsupportedSyntax(
                    "unsupported CST logical operand",
                ),
            )
            .with_span(syntax_expression_span(source, last)));
        };
        self.emit_truthy_to_bool(dst, last)?;
        let end = self.emit_jump();

        for false_branch in false_branches {
            self.patch_jump(false_branch, self.current_offset())?;
        }
        self.emit_bool_constant_to(dst, false);
        self.patch_jump(end, self.current_offset())?;

        Ok(Some(dst))
    }

    fn compile_syntax_logical_or_chain(
        &mut self,
        source: SourceId,
        operands: &[SyntaxExpression],
    ) -> CompileResult<Option<Register>> {
        let dst = self.alloc_register()?;
        let Some((last, prefix)) = operands.split_last() else {
            self.emit_bool_constant_to(dst, false);
            return Ok(Some(dst));
        };

        let mut end_jumps = Vec::with_capacity(prefix.len());
        for operand in prefix {
            let Some(value) = self.compile_syntax_expression(source, operand)? else {
                return Err(crate::compiler::CompileError::new(
                    crate::compiler::CompileErrorKind::UnsupportedSyntax(
                        "unsupported CST logical operand",
                    ),
                )
                .with_span(syntax_expression_span(source, operand)));
            };
            let next_operand = self.emit_jump_if_false(value);
            self.emit_bool_constant_to(dst, true);
            end_jumps.push(self.emit_jump());
            self.patch_jump(next_operand, self.current_offset())?;
        }

        let Some(last) = self.compile_syntax_expression(source, last)? else {
            return Err(crate::compiler::CompileError::new(
                crate::compiler::CompileErrorKind::UnsupportedSyntax(
                    "unsupported CST logical operand",
                ),
            )
            .with_span(syntax_expression_span(source, last)));
        };
        self.emit_truthy_to_bool(dst, last)?;
        for end in end_jumps {
            self.patch_jump(end, self.current_offset())?;
        }

        Ok(Some(dst))
    }

    fn reject_static_syntax_path_binary_operands(
        &self,
        source: SourceId,
        op: BinaryOp,
        expression: &SyntaxExpression,
        lhs_expression: &SyntaxExpression,
        rhs_expression: &SyntaxExpression,
    ) -> CompileResult<()> {
        let Some(lhs_path) = expression_syntax_path_or_self(lhs_expression) else {
            return Ok(());
        };
        let Some(rhs_path) = expression_syntax_path_or_self(rhs_expression) else {
            return Ok(());
        };
        let lhs_span = syntax_expression_span(source, lhs_expression);
        let rhs_span = syntax_expression_span(source, rhs_expression);
        let lhs_type = self
            .script_fact_for_path(lhs_span, &lhs_path)
            .map(|fact| fact.type_name);
        let rhs_type = self
            .script_fact_for_path(rhs_span, &rhs_path)
            .map(|fact| fact.type_name);
        self.reject_static_script_path_binary_operands(
            op,
            syntax_expression_span(source, expression),
            lhs_type.as_deref(),
            rhs_type.as_deref(),
        )
    }

    fn compile_syntax_unary(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        let Some(unary) = expression.as_unary() else {
            return Ok(None);
        };
        let Some(op) = unary.operator() else {
            return Ok(None);
        };
        let Some(operand_expression) = unary.expression() else {
            return Ok(None);
        };
        if op == UnaryOp::Negate
            && let Some(literal) = operand_expression
                .as_literal()
                .and_then(|literal| literal.literal())
            && let Some(constant) = compile_negated_literal_constant(&literal)
                .map_err(|error| error.with_span(syntax_expression_span(source, expression)))?
        {
            return self.emit_constant(constant).map(Some);
        }
        let Some(src) = self.compile_syntax_expression(source, &operand_expression)? else {
            return Ok(None);
        };
        let dst = self.alloc_register()?;
        let instruction = match op {
            UnaryOp::Not => UnlinkedInstructionKind::Not { dst, src },
            UnaryOp::Negate => UnlinkedInstructionKind::Negate { dst, src },
        };
        self.emit_spanned(instruction, syntax_expression_span(source, expression));
        Ok(Some(dst))
    }
    fn compile_syntax_assignment(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        let Some(assign) = expression.as_assign() else {
            return Ok(None);
        };
        let Some(target_expression) = assign.target() else {
            return Ok(None);
        };
        let Some(value_expression) = assign.value() else {
            return Ok(None);
        };
        let Some(op) = assign.operator() else {
            return Ok(None);
        };
        let value_type = self.syntax_value_type_for_expression(Some(source), &value_expression);
        if let Some(target_path) = expression_syntax_path_or_self(&target_expression) {
            let [target_name] = target_path.as_slice() else {
                return Ok(None);
            };
            let target_span = syntax_expression_span(source, &target_expression);
            let target_type = self.value_type_for_path(target_span, &target_path);
            let assigned_type = syntax_assignment_value_type(op, target_type, value_type);
            let target = self.local_register_at_span(target_span, target_name)?;
            let Some(value) = self.compile_syntax_expression(source, &value_expression)? else {
                return Ok(None);
            };
            return self.compile_syntax_local_assignment(op, target, value, assigned_type);
        }
        if let Some(index_target) = target_expression.as_index() {
            let Some(value) = self.compile_syntax_expression(source, &value_expression)? else {
                return Ok(None);
            };
            if let Some(assigned) = self.compile_syntax_host_index_assignment(
                source,
                expression,
                &target_expression,
                op,
                value,
            )? {
                return Ok(Some(assigned));
            }
            return self.compile_syntax_index_assignment(source, op, &index_target, value);
        }
        let Some(value) = self.compile_syntax_expression(source, &value_expression)? else {
            return Ok(None);
        };
        if let Some(assigned) = self.compile_syntax_host_field_assignment(
            source,
            expression,
            &target_expression,
            op,
            value,
        )? {
            return Ok(Some(assigned));
        }
        if let Some(assigned) = self.compile_syntax_record_field_assignment(
            source,
            &target_expression,
            op,
            &value_expression,
            value,
        )? {
            return Ok(Some(assigned));
        }
        Ok(None)
    }

    fn compile_syntax_record_field_assignment(
        &mut self,
        source: SourceId,
        target_expression: &SyntaxExpression,
        op: AssignOp,
        value_expression: &SyntaxExpression,
        value: Register,
    ) -> CompileResult<Option<Register>> {
        let Some(field) = target_expression.as_field() else {
            return Ok(None);
        };
        let Some(receiver_expression) = field.receiver() else {
            return Ok(None);
        };
        let Some(field_name) = field.name_text() else {
            return Ok(None);
        };
        let receiver_span = syntax_expression_span(source, &receiver_expression);
        let field_slot = expression_syntax_path_or_self(&receiver_expression)
            .and_then(|path| {
                let [root] = path.as_slice() else {
                    return None;
                };
                self.script_record_field_slot_for_path_root(receiver_span, root, &field_name)
            })
            .or_else(|| {
                self.value_shape_for_syntax_expression(Some(source), &receiver_expression)
                    .and_then(|shape| {
                        shape
                            .as_record()
                            .and_then(|shape| shape.field_slot(&field_name))
                    })
            });
        let value_type = self.syntax_record_field_assignment_value_type(
            source,
            target_expression,
            &receiver_expression,
        );
        let Some(record) = self.compile_syntax_expression(source, &receiver_expression)? else {
            return Ok(None);
        };
        let assigned = match op {
            AssignOp::Set => self.compile_syntax_record_field_value(
                source,
                value_expression,
                value_type,
                field_name.clone(),
                value,
            )?,
            AssignOp::Add | AssignOp::Sub | AssignOp::Mul | AssignOp::Div | AssignOp::Rem => {
                let current = self.alloc_register()?;
                if let Some(slot) = field_slot {
                    self.emit(UnlinkedInstructionKind::GetRecordSlot {
                        dst: current,
                        record,
                        field: field_name.clone(),
                        slot,
                    });
                } else {
                    self.emit(UnlinkedInstructionKind::GetRecordField {
                        dst: current,
                        record,
                        field: field_name.clone(),
                    });
                }
                let dst = self.alloc_register()?;
                let instruction = compound_assignment_instruction(op, dst, current, value)
                    .ok_or_else(|| {
                        crate::compiler::CompileError::new(
                            crate::compiler::CompileErrorKind::UnsupportedSyntax(
                                "compound assignment",
                            ),
                        )
                    })?;
                self.emit(instruction);
                dst
            }
        };
        if let Some(slot) = field_slot {
            self.emit(UnlinkedInstructionKind::SetRecordSlot {
                record,
                field: field_name,
                slot,
                src: assigned,
            });
        } else {
            self.emit(UnlinkedInstructionKind::SetRecordField {
                record,
                field: field_name,
                src: assigned,
            });
        }
        Ok(Some(assigned))
    }

    fn syntax_record_field_assignment_value_type(
        &self,
        source: SourceId,
        target_expression: &SyntaxExpression,
        receiver_expression: &SyntaxExpression,
    ) -> Option<RuntimeTypeFact> {
        let path = expression_syntax_path_or_field(target_expression)?;
        let (root, fields) = path.split_first()?;
        let root_span = expression_syntax_path_or_self(receiver_expression)
            .filter(|receiver| receiver.as_slice() == [root.as_str()])
            .map_or_else(
                || syntax_expression_span(source, target_expression),
                |_| syntax_expression_span(source, receiver_expression),
            );
        let root_type = self.script_type_for_path_root(root_span, root)?;
        self.schema_record_field_value_type(Some(root_type.as_str()), fields)
    }

    fn compile_syntax_record_field_value(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        expected: Option<RuntimeTypeFact>,
        field_name: String,
        value: Register,
    ) -> CompileResult<Register> {
        let Some(expected) = expected else {
            return Ok(value);
        };
        let span = syntax_expression_span(source, expression);
        let context = TypeContractContext::Field { name: field_name };
        let static_type = self
            .syntax_value_type_for_expression(Some(source), expression)
            .map(StaticExprType::Exact)
            .unwrap_or(StaticExprType::Dynamic);
        let outcome = check_expected_type(static_type, expected, span, context.clone())?;
        if let ExpectedTypeOutcome::RequiresRuntimeGuard(expected) = &outcome
            && let Some((location, name)) = guard_location_and_name(context)
            && let Some(plan) = type_guard_plan_for_runtime_type(expected)
        {
            self.emit_spanned(
                UnlinkedInstructionKind::GuardType {
                    src: value,
                    guard: UnlinkedTypeGuard::new(
                        plan,
                        UnlinkedGuardContext::new(GuardKind::Contract, location, name),
                    ),
                },
                span,
            );
        }
        Ok(value)
    }

    fn compile_syntax_local_assignment(
        &mut self,
        op: AssignOp,
        target: Register,
        value: Register,
        assigned_type: Option<RuntimeTypeFact>,
    ) -> CompileResult<Option<Register>> {
        let assigned = match op {
            AssignOp::Set => {
                self.emit(UnlinkedInstructionKind::Move {
                    dst: target,
                    src: value,
                });
                value
            }
            AssignOp::Add | AssignOp::Sub | AssignOp::Mul | AssignOp::Div | AssignOp::Rem => {
                let dst = self.alloc_register()?;
                let instruction = if assigned_type
                    == Some(RuntimeTypeFact::Primitive(PrimitiveTag::I64))
                {
                    i64_compound_assignment_instruction(op, dst, target, value)
                } else {
                    None
                }
                .or_else(|| compound_assignment_instruction(op, dst, target, value))
                .ok_or_else(|| {
                    crate::compiler::CompileError::new(
                        crate::compiler::CompileErrorKind::UnsupportedSyntax("compound assignment"),
                    )
                })?;
                self.emit(instruction);
                self.emit(UnlinkedInstructionKind::Move {
                    dst: target,
                    src: dst,
                });
                dst
            }
        };
        Ok(Some(assigned))
    }
    fn compile_syntax_index_assignment(
        &mut self,
        source: SourceId,
        op: AssignOp,
        target: &vela_syntax::ast::SyntaxIndexExpr,
        value: Register,
    ) -> CompileResult<Option<Register>> {
        let Some(receiver_expression) = target.receiver() else {
            return Ok(None);
        };
        let Some(index_expression) = target.index() else {
            return Ok(None);
        };
        let Some(base) = self.compile_syntax_expression(source, &receiver_expression)? else {
            return Ok(None);
        };
        let Some(index) = self.compile_syntax_expression(source, &index_expression)? else {
            return Ok(None);
        };
        let assigned = match op {
            AssignOp::Set => value,
            AssignOp::Add | AssignOp::Sub | AssignOp::Mul | AssignOp::Div | AssignOp::Rem => {
                let current = self.alloc_register()?;
                self.emit(UnlinkedInstructionKind::GetIndex {
                    dst: current,
                    base,
                    index,
                });
                let dst = self.alloc_register()?;
                let instruction = compound_assignment_instruction(op, dst, current, value)
                    .ok_or_else(|| {
                        crate::compiler::CompileError::new(
                            crate::compiler::CompileErrorKind::UnsupportedSyntax(
                                "compound assignment",
                            ),
                        )
                    })?;
                self.emit(instruction);
                dst
            }
        };
        self.emit(UnlinkedInstructionKind::SetIndex {
            base,
            index,
            src: assigned,
        });
        Ok(Some(assigned))
    }

    fn compile_syntax_index(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        let Some(index) = expression.as_index() else {
            return Ok(None);
        };
        if let Some(register) = self.compile_syntax_host_index(source, expression)? {
            return Ok(Some(register));
        }
        let Some(receiver_expression) = index.receiver() else {
            return Ok(None);
        };
        let Some(index_expression) = index.index() else {
            return Ok(None);
        };
        let Some(base) = self.compile_syntax_expression(source, &receiver_expression)? else {
            return Ok(None);
        };
        let Some(index) = self.compile_syntax_expression(source, &index_expression)? else {
            return Ok(None);
        };
        let dst = self.alloc_register()?;
        self.emit(UnlinkedInstructionKind::GetIndex { dst, base, index });
        Ok(Some(dst))
    }

    fn compile_syntax_call(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        let Some(call) = expression.as_call() else {
            return Ok(None);
        };
        let Some(callee) = call.callee() else {
            return Ok(None);
        };
        let call_span = syntax_expression_span(source, expression);
        let callee_span = syntax_expression_span(source, &callee);
        let arguments = call.arguments();

        if let Some(field) = callee.as_field() {
            let Some(receiver_expression) = field.receiver() else {
                return Ok(None);
            };
            let Some(method) = field.name_text() else {
                return Ok(None);
            };
            if let Some(register) = self.compile_syntax_host_index_remove_call(
                source,
                &receiver_expression,
                method.as_str(),
                arguments.is_empty(),
                call_span,
            )? {
                return Ok(Some(register));
            }
            if let Some(register) = self.compile_syntax_host_method_call(
                source,
                &receiver_expression,
                method.as_str(),
                &arguments,
                call_span,
            )? {
                return Ok(Some(register));
            }
            let receiver_type = self
                .script_fact_for_syntax_expression(source, &receiver_expression)
                .map(|fact| fact.type_name);
            let receiver_shape =
                self.value_shape_for_syntax_expression(Some(source), &receiver_expression);
            let value_receiver_type = self
                .syntax_value_type_for_expression(Some(source), &receiver_expression)
                .or_else(|| receiver_shape.as_ref().and_then(|shape| shape.value_type()));
            if let Some(method_id) = receiver_type
                .as_deref()
                .and_then(|type_name| self.script_method_id_for_type(type_name, &method))
            {
                let Some(receiver) =
                    self.compile_syntax_expression(source, &receiver_expression)?
                else {
                    return Ok(None);
                };
                let Some(args) = self.compile_syntax_script_method_call_arguments(
                    source,
                    receiver_type
                        .as_deref()
                        .expect("receiver type checked above"),
                    &method,
                    &arguments,
                    call_span,
                )?
                else {
                    return Ok(None);
                };
                let dst = self.alloc_register()?;
                self.emit_spanned(
                    UnlinkedInstructionKind::CallMethodId {
                        dst,
                        receiver,
                        method,
                        method_id,
                        args,
                    },
                    call_span,
                );
                return Ok(Some(dst));
            }
            if let Some(method_id) = value_receiver_type
                .as_ref()
                .and_then(|receiver_type| self.value_method_id_for_type(receiver_type, &method))
            {
                let Some(receiver) =
                    self.compile_syntax_expression(source, &receiver_expression)?
                else {
                    return Ok(None);
                };
                let Some(args) = self.compile_syntax_value_method_call_arguments(
                    source,
                    receiver_shape.as_ref(),
                    value_receiver_type.as_ref(),
                    &method,
                    &arguments,
                    call_span,
                )?
                else {
                    return Ok(None);
                };
                let dst = self.alloc_register()?;
                self.emit_spanned(
                    UnlinkedInstructionKind::CallMethodId {
                        dst,
                        receiver,
                        method,
                        method_id,
                        args,
                    },
                    call_span,
                );
                return Ok(Some(dst));
            }
            if receiver_type.is_some() {
                return Err(unresolved_static_method_error(&method, call_span));
            }
            let Some(receiver) = self.compile_syntax_expression(source, &receiver_expression)?
            else {
                return Ok(None);
            };
            let Some(args) = self.compile_syntax_dynamic_call_arguments(source, &arguments)? else {
                return Ok(None);
            };
            let dst = self.alloc_register()?;
            self.emit_spanned(
                UnlinkedInstructionKind::CallDynamicMethod {
                    dst,
                    receiver,
                    method,
                    args,
                },
                call_span,
            );
            return Ok(Some(dst));
        }

        let Some(path) = expression_syntax_path_or_self(&callee) else {
            return Ok(None);
        };
        if path.is_empty() {
            return Ok(None);
        }
        let dst = self.alloc_register()?;
        if let Some((_declaration, name)) = self.script_function_call_at_span(callee_span) {
            if arguments
                .iter()
                .any(|argument| argument.name_text().is_some())
            {
                return Ok(None);
            }
            let Some(args) = self.compile_syntax_call_arguments(source, &arguments)? else {
                return Ok(None);
            };
            self.emit_spanned(
                UnlinkedInstructionKind::CallFunction {
                    dst,
                    target: function_id_for_script_name(&name),
                    name,
                    mode: ScriptCallMode::Unchecked,
                    args: args.into_iter().map(CallArgument::Register).collect(),
                },
                call_span,
            );
            return Ok(Some(dst));
        }

        if self.local_callee_at_span(callee_span).is_some() {
            if arguments
                .iter()
                .any(|argument| argument.name_text().is_some())
            {
                return Ok(None);
            }
            let Some(callee) = self.compile_syntax_expression(source, &callee)? else {
                return Ok(None);
            };
            let Some(args) = self.compile_syntax_call_arguments(source, &arguments)? else {
                return Ok(None);
            };
            self.emit_spanned(
                UnlinkedInstructionKind::CallClosure { dst, callee, args },
                call_span,
            );
            return Ok(Some(dst));
        }

        let callee_name = path.join("::");
        let native = self.resolve_native_function_id(&callee_name, callee_span)?;
        let Some(args) = self.compile_syntax_native_call_arguments(
            source,
            &callee_name,
            native,
            &arguments,
            call_span,
        )?
        else {
            return Ok(None);
        };
        self.emit_spanned(
            UnlinkedInstructionKind::CallNative {
                dst: Some(dst),
                name: callee_name,
                native,
                cache_site: None,
                args,
            },
            call_span,
        );
        Ok(Some(dst))
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

    fn compile_syntax_container(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
            return self.compile_syntax_container(source, &inner);
        }
        if let Some(array) = expression.as_array() {
            let elements = array
                .expressions()
                .map(|element| self.compile_syntax_expression(source, &element))
                .collect::<CompileResult<Option<Vec<_>>>>()?;
            let Some(elements) = elements else {
                return Ok(None);
            };
            let dst = self.alloc_register()?;
            self.emit(UnlinkedInstructionKind::MakeArray { dst, elements });
            return Ok(Some(dst));
        }
        if let Some(map) = expression.as_map() {
            let entries = map
                .entries()
                .map(|entry| {
                    let Some(key) = entry.key() else {
                        return Ok(None);
                    };
                    let Some(value) = entry.value() else {
                        return Ok(None);
                    };
                    let key = syntax_map_key_name(source, &key)?;
                    let Some(value) = self.compile_syntax_expression(source, &value)? else {
                        return Ok(None);
                    };
                    Ok(Some((key, value)))
                })
                .collect::<CompileResult<Option<Vec<_>>>>()?;
            let Some(entries) = entries else {
                return Ok(None);
            };
            let dst = self.alloc_register()?;
            self.emit(UnlinkedInstructionKind::MakeMap { dst, entries });
            return Ok(Some(dst));
        }
        if let Some(record) = expression.as_record() {
            let path = record.path_segments();
            if path.is_empty() {
                return Ok(None);
            }
            let mut explicit_names = BTreeSet::new();
            let fields = record
                .fields()
                .into_iter()
                .map(|field| {
                    let Some(name) = field.label_text() else {
                        return Ok(None);
                    };
                    let value = if let Some(expression) = field.expression() {
                        let Some(value) = self.compile_syntax_expression(source, &expression)?
                        else {
                            return Ok(None);
                        };
                        value
                    } else if field.is_shorthand() {
                        self.compile_path_expr(
                            syntax_expression_span(source, expression),
                            std::slice::from_ref(&name),
                        )?
                    } else {
                        return Ok(None);
                    };
                    explicit_names.insert(name.clone());
                    Ok(Some((name, value)))
                })
                .collect::<CompileResult<Option<Vec<_>>>>()?;
            let Some(mut fields) = fields else {
                return Ok(None);
            };
            let dst = self.alloc_register()?;
            if let Some((enum_name, variant)) = enum_variant_path(&path) {
                let span = syntax_expression_span(source, expression);
                let resolved_enum_name = self.type_symbol_at_span(span);
                let enum_name = resolved_enum_name.clone().unwrap_or(enum_name);
                if resolved_enum_name.is_some()
                    && !self.enum_constructor_variant_exists(&enum_name, &variant)
                {
                    return Err(self.constructor_diagnostics_error(vec![
                        unknown_enum_variant_diagnostic(&enum_name, &variant, span),
                    ]));
                }
                let shape = self.enum_constructor_shape(&enum_name, &variant);
                self.compile_schema_default_fields(
                    &mut fields,
                    &explicit_names,
                    schema_default_fields(shape.as_ref()),
                    shape.as_ref(),
                )?;
                self.emit(UnlinkedInstructionKind::MakeEnum {
                    dst,
                    enum_name,
                    variant,
                    fields,
                });
            } else {
                let type_name = self
                    .type_symbol_at_span(syntax_expression_span(source, expression))
                    .unwrap_or_else(|| path.join("::"));
                let shape = self.record_constructor_shape(&type_name);
                self.compile_schema_default_fields(
                    &mut fields,
                    &explicit_names,
                    schema_default_fields(shape.as_ref()),
                    shape.as_ref(),
                )?;
                self.emit(UnlinkedInstructionKind::MakeRecord {
                    dst,
                    type_name,
                    fields,
                });
            }
            return Ok(Some(dst));
        }
        Ok(None)
    }

    fn compile_syntax_path_numeric_literal_binary(
        &mut self,
        source: SourceId,
        op: BinaryOp,
        expression: &SyntaxExpression,
        lhs_expression: &SyntaxExpression,
        rhs_expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        let Some((path_expression, literal_expression, side)) =
            syntax_path_numeric_literal_operands(lhs_expression, rhs_expression)
        else {
            return Ok(None);
        };
        let Some(path) = expression_syntax_path_or_field(path_expression) else {
            return Ok(None);
        };
        let literal = expression_syntax_literal(literal_expression)
            .and_then(InlineNumericLiteral::from_literal)
            .expect("numeric literal operand helper checks literal availability");
        let span = syntax_expression_span(source, expression);
        let path_span = syntax_expression_span(source, path_expression);
        let script_type = self
            .script_fact_for_path(path_span, &path)
            .map(|fact| fact.type_name);
        self.reject_static_script_path_binary_operands(op, span, script_type.as_deref(), None)?;
        let value_type = self.value_type_for_path(path_span, &path);
        if side == BinaryLiteralSide::Right
            && value_type == Some(RuntimeTypeFact::Primitive(PrimitiveTag::I64))
            && let Some(imm) = i64_immediate_value(&literal, span)?
            && i64_immediate_op_supported(op, imm)
        {
            let value = self.compile_path_expr(path_span, &path)?;
            let dst = self.alloc_register()?;
            let instruction = i64_immediate_instruction(op, dst, value, imm)
                .expect("support was checked before compiling the syntax value expression");
            self.emit_spanned(instruction, span);
            return Ok(Some(dst));
        }
        if let Some(RuntimeTypeFact::Primitive(tag)) = value_type.as_ref()
            && literal.matches_primitive_tag(*tag)
        {
            let value = self.compile_path_expr(path_span, &path)?;
            let literal_register =
                self.emit_constant(inline_numeric_literal_as(&literal, *tag, span)?)?;
            let dst = self.alloc_register()?;
            let Some(instruction) = (match side {
                BinaryLiteralSide::Left => {
                    non_logical_binary_instruction(op, dst, literal_register, value)
                }
                BinaryLiteralSide::Right => {
                    non_logical_binary_instruction(op, dst, value, literal_register)
                }
            }) else {
                return Ok(None);
            };
            self.emit_spanned(instruction, span);
            return Ok(Some(dst));
        }
        if value_type.is_none()
            && let Some(literal_op) = binary_literal_op(op)
        {
            let value = self.compile_path_expr(path_span, &path)?;
            let dst = self.alloc_register()?;
            match literal {
                InlineNumericLiteral::Integer(text) => {
                    self.emit_spanned(
                        UnlinkedInstructionKind::BinaryIntLiteral {
                            dst,
                            op: literal_op,
                            value,
                            literal: text,
                            side,
                        },
                        span,
                    );
                }
                InlineNumericLiteral::Float(text) => {
                    self.emit_spanned(
                        UnlinkedInstructionKind::BinaryFloatLiteral {
                            dst,
                            op: literal_op,
                            value,
                            literal: text,
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
}

fn syntax_assignment_value_type(
    op: AssignOp,
    target_type: Option<RuntimeTypeFact>,
    value_type: Option<RuntimeTypeFact>,
) -> Option<RuntimeTypeFact> {
    match op {
        AssignOp::Set => value_type,
        AssignOp::Add | AssignOp::Sub | AssignOp::Mul | AssignOp::Rem
            if target_type == Some(RuntimeTypeFact::Primitive(PrimitiveTag::I64))
                && value_type == Some(RuntimeTypeFact::Primitive(PrimitiveTag::I64)) =>
        {
            Some(RuntimeTypeFact::Primitive(PrimitiveTag::I64))
        }
        AssignOp::Div | AssignOp::Add | AssignOp::Sub | AssignOp::Mul | AssignOp::Rem => None,
    }
}

fn syntax_path_numeric_literal_operands<'expression>(
    lhs: &'expression SyntaxExpression,
    rhs: &'expression SyntaxExpression,
) -> Option<(
    &'expression SyntaxExpression,
    &'expression SyntaxExpression,
    BinaryLiteralSide,
)> {
    if expression_syntax_path_or_field(lhs).is_some()
        && expression_syntax_literal(rhs)
            .and_then(InlineNumericLiteral::from_literal)
            .is_some()
    {
        return Some((lhs, rhs, BinaryLiteralSide::Right));
    }
    if expression_syntax_literal(lhs)
        .and_then(InlineNumericLiteral::from_literal)
        .is_some()
        && expression_syntax_path_or_field(rhs).is_some()
    {
        return Some((rhs, lhs, BinaryLiteralSide::Left));
    }
    None
}

fn logical_chain_syntax_operands(
    expression: &SyntaxExpression,
    op: BinaryOp,
) -> Option<Vec<SyntaxExpression>> {
    fn collect(
        expression: SyntaxExpression,
        op: BinaryOp,
        operands: &mut Vec<SyntaxExpression>,
    ) -> Option<()> {
        if let Some(binary) = expression.as_binary()
            && binary.operator() == Some(op)
        {
            collect(binary.lhs()?, op, operands)?;
            collect(binary.rhs()?, op, operands)?;
            return Some(());
        }

        operands.push(expression);
        Some(())
    }

    let mut operands = Vec::new();
    collect(expression.clone(), op, &mut operands)?;
    Some(operands)
}

pub(in crate::compiler::control_flow) fn syntax_expression_span(
    source: SourceId,
    expression: &SyntaxExpression,
) -> Span {
    let range = expression.syntax().text_range();
    Span::new(source, range.start().into(), range.end().into())
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

#[derive(Clone)]
enum InlineNumericLiteral {
    Integer(String),
    Float(String),
}

impl InlineNumericLiteral {
    fn from_literal(literal: Literal) -> Option<Self> {
        match literal {
            Literal::Integer(value) if value.suffix.is_none() => {
                Some(Self::Integer(value.source_text().to_owned()))
            }
            Literal::Float(value) if value.suffix.is_none() => {
                Some(Self::Float(value.source_text().to_owned()))
            }
            _ => None,
        }
    }

    fn matches_primitive_tag(&self, tag: PrimitiveTag) -> bool {
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

fn i64_immediate_value(literal: &InlineNumericLiteral, span: Span) -> CompileResult<Option<i64>> {
    let InlineNumericLiteral::Integer(_) = literal else {
        return Ok(None);
    };
    let Constant::Scalar(vela_common::ScalarValue::I64(value)) =
        inline_numeric_literal_as(literal, PrimitiveTag::I64, span)?
    else {
        return Ok(None);
    };
    Ok(Some(value))
}

fn inline_numeric_literal_as(
    literal: &InlineNumericLiteral,
    tag: PrimitiveTag,
    span: Span,
) -> CompileResult<Constant> {
    match literal {
        InlineNumericLiteral::Integer(text) => compile_literal_constant_for_type(
            &Literal::Integer(vela_syntax::ast::IntegerLiteral::unsuffixed(text)),
            tag,
        ),
        InlineNumericLiteral::Float(text) => compile_literal_constant_for_type(
            &Literal::Float(vela_syntax::ast::FloatLiteral::unsuffixed(text)),
            tag,
        ),
    }
    .map_err(|error| error.with_span(span))
    .map(|constant| constant.expect("literal kind and primitive tag were checked by caller"))
}

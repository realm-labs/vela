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
use crate::compiler::const_eval::compile_literal_constant_for_type;
use crate::compiler::operators::{
    binary_literal_op, compound_assignment_instruction, i64_immediate_instruction,
    i64_immediate_op_supported, non_logical_binary_instruction,
};
use crate::compiler::param_defaults::syntax_map_key_name;
use crate::compiler::value_types::RuntimeTypeFact;
use crate::compiler::{CompileResult, Compiler, frame_slot_kind};
use crate::function_id_for_script_name;
use crate::{
    BinaryLiteralSide, CallArgument, Constant, DynamicCallArgument, FormatStringPart,
    ScriptCallMode,
};
use crate::{Register, UnlinkedInstructionKind};

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
        self.locals.insert(name.clone(), register);
        let local_binding = self
            .bindings
            .local_named_at(&name, LocalBindingKind::Let, span);
        if let Some(local) = local_binding {
            self.hir_locals.insert(local, register);
            self.script_types.set_local_fact(local, name.clone(), None);
            self.value_types.set_local(local, name.clone(), None);
            self.value_shapes.set_local(local, name.clone(), None);
        } else {
            self.script_types.set_name_fact(name.clone(), None);
            self.value_types.set_name(name.clone(), None);
            self.value_shapes.set_name(name.clone(), None);
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

    fn compile_syntax_expression(
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
        if let Some(register) = self.compile_syntax_path_unary(source, expression)? {
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
        let Some(binary) = expression.as_binary() else {
            return Ok(None);
        };
        let Some(op) = binary.operator() else {
            return Ok(None);
        };
        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            return self.compile_syntax_logical_chain(source, expression, op);
        }
        if matches!(op, BinaryOp::Range | BinaryOp::RangeInclusive) {
            return Ok(None);
        }
        let Some(lhs_expression) = binary.lhs() else {
            return Ok(None);
        };
        let Some(rhs_expression) = binary.rhs() else {
            return Ok(None);
        };
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

    fn compile_syntax_if_value(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        let Some(if_expr) = expression.as_if() else {
            return Ok(None);
        };
        if !syntax_if_value_lowering_covers(&if_expr) {
            return Ok(None);
        }
        let dst = self.alloc_register()?;
        let Some(returned) = self.compile_syntax_if_value_to(source, &if_expr, dst)? else {
            return Ok(None);
        };
        let _ = returned;
        Ok(Some(dst))
    }

    pub(in crate::compiler::control_flow) fn compile_syntax_if_value_to(
        &mut self,
        source: SourceId,
        if_expr: &SyntaxIfExpr,
        dst: Register,
    ) -> CompileResult<Option<bool>> {
        let Some(condition_expression) = if_expr.condition() else {
            return Ok(None);
        };
        let Some(condition) = self.compile_syntax_expression(source, &condition_expression)? else {
            return Ok(None);
        };
        let Some(then_block) = if_expr.then_block() else {
            return Ok(None);
        };
        let then_body = CompilerBodyPayload::nested_syntax(source, then_block);

        let jump_to_else = self.emit_jump_if_false(condition);
        let then_returned = self.compile_block_payload_value_to(&then_body, dst)?;
        let jump_to_end = if then_returned {
            None
        } else {
            Some(self.emit_jump())
        };

        self.patch_jump(jump_to_else, self.current_offset())?;
        let else_returned = match if_expr.else_branch() {
            Some(SyntaxElseBranch::If(else_if)) => {
                let Some(returned) = self.compile_syntax_if_value_to(source, &else_if, dst)? else {
                    return Ok(None);
                };
                returned
            }
            Some(SyntaxElseBranch::Block(block)) => {
                let else_body = CompilerBodyPayload::nested_syntax(source, block);
                self.compile_block_payload_value_to(&else_body, dst)?
            }
            None => {
                self.emit_constant_to(dst, Constant::Null);
                false
            }
        };

        if let Some(jump_to_end) = jump_to_end {
            self.patch_jump(jump_to_end, self.current_offset())?;
        }
        Ok(Some(then_returned && else_returned))
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
                return Ok(None);
            };
            false_branches.push(self.emit_jump_if_false(value));
        }

        let Some(last) = self.compile_syntax_expression(source, last)? else {
            return Ok(None);
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
                return Ok(None);
            };
            let next_operand = self.emit_jump_if_false(value);
            self.emit_bool_constant_to(dst, true);
            end_jumps.push(self.emit_jump());
            self.patch_jump(next_operand, self.current_offset())?;
        }

        let Some(last) = self.compile_syntax_expression(source, last)? else {
            return Ok(None);
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

    fn compile_syntax_path_unary(
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
        let Some(path) = expression_syntax_path_or_field(&operand_expression) else {
            return Ok(None);
        };
        let src =
            self.compile_path_expr(syntax_expression_span(source, &operand_expression), &path)?;
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
        let Some(value) = self.compile_syntax_expression(source, &value_expression)? else {
            return Ok(None);
        };
        if let Some(target_path) = expression_syntax_path_or_self(&target_expression) {
            let [target_name] = target_path.as_slice() else {
                return Ok(None);
            };
            let target_span = syntax_expression_span(source, &target_expression);
            let target = self.local_register_at_span(target_span, target_name)?;
            return self.compile_syntax_local_assignment(op, target, value);
        }
        if let Some(index_target) = target_expression.as_index() {
            return self.compile_syntax_index_assignment(source, op, &index_target, value);
        }
        Ok(None)
    }
    fn compile_syntax_local_assignment(
        &mut self,
        op: AssignOp,
        target: Register,
        value: Register,
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
                let instruction = compound_assignment_instruction(op, dst, target, value)
                    .ok_or_else(|| {
                        crate::compiler::CompileError::new(
                            crate::compiler::CompileErrorKind::UnsupportedSyntax(
                                "compound assignment",
                            ),
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
        if arguments
            .iter()
            .any(|argument| argument.name_text().is_some())
        {
            return Ok(None);
        }

        let dst = self.alloc_register()?;
        if let Some((_declaration, name)) = self.script_function_call_at_span(callee_span) {
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
        let Some(args) = self.compile_syntax_call_arguments(source, &arguments)? else {
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

    fn compile_syntax_call_arguments(
        &mut self,
        source: SourceId,
        arguments: &[vela_syntax::ast::SyntaxArgument],
    ) -> CompileResult<Option<Vec<Register>>> {
        arguments
            .iter()
            .map(|argument| {
                let Some(expression) = argument.expression() else {
                    return Ok(None);
                };
                self.compile_syntax_expression(source, &expression)
            })
            .collect::<CompileResult<Option<Vec<_>>>>()
    }

    fn compile_syntax_dynamic_call_arguments(
        &mut self,
        source: SourceId,
        arguments: &[vela_syntax::ast::SyntaxArgument],
    ) -> CompileResult<Option<Vec<DynamicCallArgument>>> {
        arguments
            .iter()
            .map(|argument| {
                let Some(expression) = argument.expression() else {
                    return Ok(None);
                };
                let Some(value) = self.compile_syntax_expression(source, &expression)? else {
                    return Ok(None);
                };
                Ok(Some(DynamicCallArgument {
                    name: argument.name_text(),
                    value,
                }))
            })
            .collect::<CompileResult<Option<Vec<_>>>>()
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
            let type_name = record.path_segments().join("::");
            if type_name.is_empty() {
                return Ok(None);
            }
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
                    Ok(Some((name, value)))
                })
                .collect::<CompileResult<Option<Vec<_>>>>()?;
            let Some(fields) = fields else {
                return Ok(None);
            };
            let dst = self.alloc_register()?;
            self.emit(UnlinkedInstructionKind::MakeRecord {
                dst,
                type_name,
                fields,
            });
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

fn syntax_expression_span(source: SourceId, expression: &SyntaxExpression) -> Span {
    let range = expression.syntax().text_range();
    Span::new(source, range.start().into(), range.end().into())
}

fn syntax_if_value_lowering_covers(if_expr: &SyntaxIfExpr) -> bool {
    if if_expr.condition().is_none() || if_expr.then_block().is_none() {
        return false;
    }
    if if_expr
        .then_block()
        .is_some_and(|block| CompilerBodyPayload::requires_body_block_lookup(&block))
    {
        return false;
    }
    match if_expr.else_branch() {
        Some(SyntaxElseBranch::If(else_if)) => syntax_if_value_lowering_covers(&else_if),
        Some(SyntaxElseBranch::Block(block)) => {
            !CompilerBodyPayload::requires_body_block_lookup(&block)
        }
        None => true,
    }
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

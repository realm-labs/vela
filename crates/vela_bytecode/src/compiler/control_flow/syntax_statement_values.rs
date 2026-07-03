use vela_common::{PrimitiveTag, SourceId, Span};
use vela_hir::binding::LocalBindingKind;
use vela_syntax::SyntaxKind;
use vela_syntax::ast::{AstNode, BinaryOp, Literal, SyntaxExpression, SyntaxLiteral, UnaryOp};
use vela_syntax::token::{InterpolatedStringTokenPart, TokenKind};

use crate::compiler::body_payloads::{
    expression_syntax_literal, expression_syntax_path_field, expression_syntax_path_or_self,
};
use crate::compiler::const_eval::compile_literal_constant_for_type;
use crate::compiler::operators::{
    binary_literal_op, i64_immediate_instruction, i64_immediate_op_supported,
    non_logical_binary_instruction,
};
use crate::compiler::param_defaults::syntax_map_key_name;
use crate::compiler::value_types::RuntimeTypeFact;
use crate::compiler::{CompileResult, Compiler, frame_slot_kind};
use crate::{BinaryLiteralSide, Constant, FormatStringPart};
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
        if let Some(register) = self.compile_syntax_try(source, expression)? {
            return Ok(Some(register));
        }
        if let Some(register) = self.compile_syntax_container(source, expression)? {
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
        let Some(path) = expression_syntax_path_or_self(&operand_expression) else {
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
        let Some(path) = expression_syntax_path_or_self(path_expression) else {
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
    if expression_syntax_path_or_self(lhs).is_some()
        && expression_syntax_literal(rhs)
            .and_then(InlineNumericLiteral::from_literal)
            .is_some()
    {
        return Some((lhs, rhs, BinaryLiteralSide::Right));
    }
    if expression_syntax_literal(lhs)
        .and_then(InlineNumericLiteral::from_literal)
        .is_some()
        && expression_syntax_path_or_self(rhs).is_some()
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

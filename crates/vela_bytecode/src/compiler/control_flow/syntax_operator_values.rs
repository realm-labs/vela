use vela_common::{Diagnostic, PrimitiveTag, SourceId, Span};
use vela_syntax::ast::{BinaryOp, Literal, SyntaxExpression, UnaryOp};

use super::spans::syntax_expression_span;
use crate::compiler::body_payloads::expression_syntax_literal;
use crate::compiler::const_eval::{
    compile_literal_constant_for_type, compile_negated_literal_constant,
};
use crate::compiler::operators::{
    binary_literal_op, i64_binary_instruction, i64_immediate_instruction,
    i64_immediate_op_supported, non_logical_binary_instruction,
};
use crate::compiler::record_shapes::ValueShape;
use crate::compiler::value_types::{RuntimeTypeFact, StandardRuntimeType};
use crate::compiler::{CompileError, CompileErrorKind, CompileResult, Compiler};
use crate::{BinaryLiteralSide, Constant, Register, UnlinkedInstructionKind};

impl Compiler<'_, '_> {
    pub(super) fn compile_syntax_binary_expression(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
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
        if let Some(register) = self.compile_syntax_unknown_numeric_literal_binary(
            source,
            op,
            expression,
            &lhs_expression,
            &rhs_expression,
        )? {
            return Ok(Some(register));
        }
        self.reject_static_syntax_binary_operands(
            source,
            op,
            expression,
            &lhs_expression,
            &rhs_expression,
        )?;
        if self.syntax_value_type_for_expression(Some(source), &lhs_expression)
            == Some(RuntimeTypeFact::Primitive(PrimitiveTag::I64))
            && self.syntax_value_type_for_expression(Some(source), &rhs_expression)
                == Some(RuntimeTypeFact::Primitive(PrimitiveTag::I64))
            && matches!(
                op,
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Rem
            )
            && let Some(register) = self.compile_syntax_i64_binary(
                source,
                op,
                expression,
                &lhs_expression,
                &rhs_expression,
            )?
        {
            return Ok(Some(register));
        }
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

    fn compile_syntax_i64_binary(
        &mut self,
        source: SourceId,
        op: BinaryOp,
        expression: &SyntaxExpression,
        lhs_expression: &SyntaxExpression,
        rhs_expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        let Some(lhs) = self.compile_syntax_expression(source, lhs_expression)? else {
            return Ok(None);
        };
        let Some(rhs) = self.compile_syntax_expression(source, rhs_expression)? else {
            return Ok(None);
        };
        let dst = self.alloc_register()?;
        let Some(instruction) = i64_binary_instruction(op, dst, lhs, rhs) else {
            return Ok(None);
        };
        self.emit_spanned(instruction, syntax_expression_span(source, expression));
        Ok(Some(dst))
    }

    pub(super) fn compile_syntax_unary(
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
        if op == UnaryOp::Not
            && let Some(register) =
                self.compile_syntax_negated_equality(source, expression, &operand_expression)?
        {
            return Ok(Some(register));
        }
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

    fn compile_syntax_negated_equality(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        operand_expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        let mut equality_expression = operand_expression.clone();
        while let Some(inner) = equality_expression
            .as_paren()
            .and_then(|paren| paren.expression())
        {
            equality_expression = inner;
        }
        let Some(binary) = equality_expression.as_binary() else {
            return Ok(None);
        };
        let Some(equality_op) = binary.operator() else {
            return Ok(None);
        };
        let inverse = match equality_op {
            BinaryOp::Equal => BinaryOp::NotEqual,
            BinaryOp::NotEqual => BinaryOp::Equal,
            BinaryOp::IdentityEqual => BinaryOp::IdentityNotEqual,
            BinaryOp::IdentityNotEqual => BinaryOp::IdentityEqual,
            _ => return Ok(None),
        };
        let Some(lhs_expression) = binary.lhs() else {
            return Ok(None);
        };
        let Some(rhs_expression) = binary.rhs() else {
            return Ok(None);
        };
        if let Some(register) = self.compile_syntax_path_numeric_literal_binary(
            source,
            inverse,
            expression,
            &lhs_expression,
            &rhs_expression,
        )? {
            return Ok(Some(register));
        }
        if let Some(register) = self.compile_syntax_unknown_numeric_literal_binary(
            source,
            inverse,
            expression,
            &lhs_expression,
            &rhs_expression,
        )? {
            return Ok(Some(register));
        }
        self.reject_static_syntax_binary_operands(
            source,
            inverse,
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
        let Some(instruction) = non_logical_binary_instruction(inverse, dst, lhs, rhs) else {
            return Ok(None);
        };
        self.emit_spanned(instruction, syntax_expression_span(source, expression));
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
                return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "unsupported logical operand",
                ))
                .with_span(syntax_expression_span(source, operand)));
            };
            false_branches.push(self.emit_jump_if_false(value));
        }

        let Some(last) = self.compile_syntax_expression(source, last)? else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "unsupported logical operand",
            ))
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
                return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "unsupported logical operand",
                ))
                .with_span(syntax_expression_span(source, operand)));
            };
            let next_operand = self.emit_jump_if_false(value);
            self.emit_bool_constant_to(dst, true);
            end_jumps.push(self.emit_jump());
            self.patch_jump(next_operand, self.current_offset())?;
        }

        let Some(last) = self.compile_syntax_expression(source, last)? else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "unsupported logical operand",
            ))
            .with_span(syntax_expression_span(source, last)));
        };
        self.emit_truthy_to_bool(dst, last)?;
        for end in end_jumps {
            self.patch_jump(end, self.current_offset())?;
        }

        Ok(Some(dst))
    }

    fn reject_static_syntax_binary_operands(
        &self,
        source: SourceId,
        op: BinaryOp,
        expression: &SyntaxExpression,
        lhs_expression: &SyntaxExpression,
        rhs_expression: &SyntaxExpression,
    ) -> CompileResult<()> {
        if matches!(op, BinaryOp::IdentityEqual | BinaryOp::IdentityNotEqual) {
            return self.reject_static_syntax_identity_binary_operands(
                source,
                op,
                expression,
                lhs_expression,
                rhs_expression,
            );
        }
        let lhs_type = self
            .script_fact_for_syntax_expression(source, lhs_expression)
            .map(|fact| fact.type_name);
        let rhs_type = self
            .script_fact_for_syntax_expression(source, rhs_expression)
            .map(|fact| fact.type_name);
        self.reject_static_script_path_binary_operands(
            op,
            syntax_expression_span(source, expression),
            lhs_type.as_deref(),
            rhs_type.as_deref(),
        )
    }

    fn reject_static_syntax_identity_binary_operands(
        &self,
        source: SourceId,
        op: BinaryOp,
        expression: &SyntaxExpression,
        lhs_expression: &SyntaxExpression,
        rhs_expression: &SyntaxExpression,
    ) -> CompileResult<()> {
        let span = syntax_expression_span(source, expression);
        for (side, operand) in [("left", lhs_expression), ("right", rhs_expression)] {
            let type_name = self
                .syntax_value_type_for_expression(Some(source), operand)
                .and_then(non_identity_runtime_type_name)
                .or_else(|| {
                    self.value_shape_for_syntax_expression(Some(source), operand)
                        .and_then(non_identity_value_shape_name)
                });
            let Some(type_name) = type_name else {
                continue;
            };
            return Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
                vec![
                    Diagnostic::error(format!(
                        "`{}` requires reference identity operands, but the {side} operand has type `{type_name}`",
                        syntax_binary_op_source_name(op)
                    ))
                    .with_code("compiler::invalid_identity_comparison")
                    .with_span(span)
                    .with_label(span, "identity comparison requires reference operands")
                    .with_label(
                        syntax_expression_span(source, operand),
                        format!("{side} operand is statically `{type_name}`"),
                    ),
                ],
            )));
        }
        Ok(())
    }

    fn compile_syntax_unknown_numeric_literal_binary(
        &mut self,
        source: SourceId,
        op: BinaryOp,
        expression: &SyntaxExpression,
        lhs_expression: &SyntaxExpression,
        rhs_expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        let Some((value_expression, literal_expression, side)) =
            syntax_numeric_literal_operands(lhs_expression, rhs_expression)
        else {
            return Ok(None);
        };
        if self
            .syntax_value_type_for_expression(Some(source), value_expression)
            .is_some()
        {
            return Ok(None);
        }
        let Some(literal_op) = binary_literal_op(op) else {
            return Ok(None);
        };
        let literal = expression_syntax_literal(literal_expression)
            .and_then(InlineNumericLiteral::from_literal)
            .expect("numeric literal operand helper checks literal availability");
        let span = syntax_expression_span(source, expression);
        let Some(value) = self.compile_syntax_expression(source, value_expression)? else {
            return Ok(None);
        };
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
        Ok(Some(dst))
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
        let literal = expression_syntax_literal(literal_expression)
            .and_then(InlineNumericLiteral::from_literal)
            .expect("numeric literal operand helper checks literal availability");
        let span = syntax_expression_span(source, expression);
        let path_span = syntax_expression_span(source, path_expression);
        let Some(path) = self.hir_value_path_for_span(path_span) else {
            return Ok(None);
        };
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
    if expression_syntax_literal(rhs)
        .and_then(InlineNumericLiteral::from_literal)
        .is_some()
    {
        return Some((lhs, rhs, BinaryLiteralSide::Right));
    }
    if expression_syntax_literal(lhs)
        .and_then(InlineNumericLiteral::from_literal)
        .is_some()
    {
        return Some((rhs, lhs, BinaryLiteralSide::Left));
    }
    None
}

fn syntax_numeric_literal_operands<'expression>(
    lhs: &'expression SyntaxExpression,
    rhs: &'expression SyntaxExpression,
) -> Option<(
    &'expression SyntaxExpression,
    &'expression SyntaxExpression,
    BinaryLiteralSide,
)> {
    let lhs_literal = expression_syntax_literal(lhs).and_then(InlineNumericLiteral::from_literal);
    let rhs_literal = expression_syntax_literal(rhs).and_then(InlineNumericLiteral::from_literal);
    match (lhs_literal.is_some(), rhs_literal.is_some()) {
        (false, true) => Some((lhs, rhs, BinaryLiteralSide::Right)),
        (true, false) => Some((rhs, lhs, BinaryLiteralSide::Left)),
        (true, true) | (false, false) => None,
    }
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

fn non_identity_runtime_type_name(fact: RuntimeTypeFact) -> Option<String> {
    match fact {
        RuntimeTypeFact::Primitive(_) | RuntimeTypeFact::Standard(StandardRuntimeType::Range) => {
            Some(fact.source_type_display())
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
        | RuntimeTypeFact::Tuple(_)
        | RuntimeTypeFact::Option(_)
        | RuntimeTypeFact::Result { .. } => None,
    }
}

fn non_identity_value_shape_name(shape: ValueShape) -> Option<String> {
    match shape {
        ValueShape::Scalar(type_name) => Some(type_name),
        ValueShape::Unknown
        | ValueShape::Record(_)
        | ValueShape::Array(_)
        | ValueShape::Iterator(_)
        | ValueShape::Map { .. }
        | ValueShape::Set(_)
        | ValueShape::Tuple(_)
        | ValueShape::Option(_)
        | ValueShape::Result { .. } => None,
    }
}

fn syntax_binary_op_source_name(op: BinaryOp) -> &'static str {
    match op {
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

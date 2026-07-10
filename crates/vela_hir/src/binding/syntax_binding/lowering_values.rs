use vela_common::{SourceId, Span};
use vela_syntax::ast::{
    AssignOp, AstNode, BinaryOp, FloatSuffix, IntRadix, IntegerSuffix, Literal, SyntaxTypeHint,
    UnaryOp,
};
use vela_syntax::{SyntaxKind, SyntaxToken, TextRange};

use crate::binding::PathUsage;
use crate::body::{
    HirAssignOp, HirBinaryOp, HirFloatLiteral, HirFloatSuffix, HirIntRadix, HirIntegerLiteral,
    HirIntegerSuffix, HirLiteral, HirPathKind, HirUnaryOp,
};
use crate::type_hint::HirTypeHint;

pub(super) fn hir_literal(literal: Literal) -> HirLiteral {
    match literal {
        Literal::Bool(value) => HirLiteral::Bool(value),
        Literal::Integer(value) => HirLiteral::Integer(HirIntegerLiteral {
            text: value.text,
            radix: match value.radix {
                IntRadix::Binary => HirIntRadix::Binary,
                IntRadix::Decimal => HirIntRadix::Decimal,
                IntRadix::Hex => HirIntRadix::Hex,
            },
            suffix: value.suffix.map(|suffix| match suffix {
                IntegerSuffix::I8 => HirIntegerSuffix::I8,
                IntegerSuffix::I16 => HirIntegerSuffix::I16,
                IntegerSuffix::I32 => HirIntegerSuffix::I32,
                IntegerSuffix::I64 => HirIntegerSuffix::I64,
                IntegerSuffix::U8 => HirIntegerSuffix::U8,
                IntegerSuffix::U16 => HirIntegerSuffix::U16,
                IntegerSuffix::U32 => HirIntegerSuffix::U32,
                IntegerSuffix::U64 => HirIntegerSuffix::U64,
            }),
        }),
        Literal::Float(value) => HirLiteral::Float(HirFloatLiteral {
            text: value.text,
            suffix: value.suffix.map(|suffix| match suffix {
                FloatSuffix::F32 => HirFloatSuffix::F32,
                FloatSuffix::F64 => HirFloatSuffix::F64,
            }),
        }),
        Literal::Char(value) => HirLiteral::Char(value),
        Literal::String(value) => HirLiteral::String(value),
        Literal::Bytes(value) => HirLiteral::Bytes(value),
    }
}

pub(super) const fn hir_unary_op(op: UnaryOp) -> HirUnaryOp {
    match op {
        UnaryOp::Not => HirUnaryOp::Not,
        UnaryOp::Negate => HirUnaryOp::Negate,
    }
}

pub(super) const fn hir_binary_op(op: BinaryOp) -> HirBinaryOp {
    match op {
        BinaryOp::Or => HirBinaryOp::Or,
        BinaryOp::And => HirBinaryOp::And,
        BinaryOp::Equal => HirBinaryOp::Equal,
        BinaryOp::NotEqual => HirBinaryOp::NotEqual,
        BinaryOp::IdentityEqual => HirBinaryOp::IdentityEqual,
        BinaryOp::IdentityNotEqual => HirBinaryOp::IdentityNotEqual,
        BinaryOp::Less => HirBinaryOp::Less,
        BinaryOp::LessEqual => HirBinaryOp::LessEqual,
        BinaryOp::Greater => HirBinaryOp::Greater,
        BinaryOp::GreaterEqual => HirBinaryOp::GreaterEqual,
        BinaryOp::Range => HirBinaryOp::Range,
        BinaryOp::RangeInclusive => HirBinaryOp::RangeInclusive,
        BinaryOp::Add => HirBinaryOp::Add,
        BinaryOp::Sub => HirBinaryOp::Sub,
        BinaryOp::Mul => HirBinaryOp::Mul,
        BinaryOp::Div => HirBinaryOp::Div,
        BinaryOp::Rem => HirBinaryOp::Rem,
    }
}

pub(super) const fn hir_assign_op(op: AssignOp) -> HirAssignOp {
    match op {
        AssignOp::Set => HirAssignOp::Set,
        AssignOp::Add => HirAssignOp::Add,
        AssignOp::Sub => HirAssignOp::Sub,
        AssignOp::Mul => HirAssignOp::Mul,
        AssignOp::Div => HirAssignOp::Div,
        AssignOp::Rem => HirAssignOp::Rem,
    }
}

pub(super) fn hir_type_hint(source: SourceId, hint: &SyntaxTypeHint) -> HirTypeHint {
    let span = span_for(source, hint.syntax().text_range());
    if hint.is_unit() {
        return HirTypeHint {
            path: vec![HirTypeHint::UNIT_PATH.to_owned()],
            args: Vec::new(),
            span,
        };
    }
    let tuple_elements = hint.tuple_element_hints().collect::<Vec<_>>();
    if hint.is_tuple() {
        return HirTypeHint {
            path: vec![HirTypeHint::UNIT_PATH.to_owned()],
            args: tuple_elements
                .iter()
                .map(|arg| hir_type_hint(source, arg))
                .collect(),
            span,
        };
    }
    if hint.l_paren_token().is_some() && tuple_elements.len() == 1 {
        return hir_type_hint(source, &tuple_elements[0]);
    }
    HirTypeHint {
        path: hint.path_segments(),
        args: hint
            .type_arg_list()
            .into_iter()
            .flat_map(|args| args.type_hints())
            .map(|arg| hir_type_hint(source, &arg))
            .collect(),
        span,
    }
}

pub(super) fn span_for(source: SourceId, range: TextRange) -> Span {
    Span::new(source, range.start().into(), range.end().into())
}

pub(super) fn last_segment_span(source: SourceId, tokens: Vec<SyntaxToken>) -> Option<Span> {
    tokens
        .into_iter()
        .rev()
        .find(|token| token.kind() == SyntaxKind::Ident)
        .map(|token| span_for(source, token.text_range()))
}

pub(super) const fn hir_path_kind_for_usage(usage: PathUsage) -> HirPathKind {
    match usage {
        PathUsage::Callee => HirPathKind::Callee,
        PathUsage::Value | PathUsage::FieldBase | PathUsage::AssignmentTarget => HirPathKind::Value,
    }
}

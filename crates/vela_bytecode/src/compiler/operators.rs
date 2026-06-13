use vela_syntax::ast::{AssignOp, BinaryOp};

use crate::{BinaryLiteralOp, Register, UnlinkedInstructionKind};

pub(super) fn binary_literal_op(op: BinaryOp) -> Option<BinaryLiteralOp> {
    match op {
        BinaryOp::Add => Some(BinaryLiteralOp::Add),
        BinaryOp::Sub => Some(BinaryLiteralOp::Sub),
        BinaryOp::Mul => Some(BinaryLiteralOp::Mul),
        BinaryOp::Div => Some(BinaryLiteralOp::Div),
        BinaryOp::Rem => Some(BinaryLiteralOp::Rem),
        BinaryOp::Less => Some(BinaryLiteralOp::Less),
        BinaryOp::LessEqual => Some(BinaryLiteralOp::LessEqual),
        BinaryOp::Greater => Some(BinaryLiteralOp::Greater),
        BinaryOp::GreaterEqual => Some(BinaryLiteralOp::GreaterEqual),
        BinaryOp::Equal
        | BinaryOp::NotEqual
        | BinaryOp::Range
        | BinaryOp::RangeInclusive
        | BinaryOp::Or
        | BinaryOp::And => None,
    }
}

pub(super) fn non_logical_binary_instruction(
    op: BinaryOp,
    dst: Register,
    lhs: Register,
    rhs: Register,
) -> Option<UnlinkedInstructionKind> {
    match op {
        BinaryOp::Add => Some(UnlinkedInstructionKind::Add { dst, lhs, rhs }),
        BinaryOp::Sub => Some(UnlinkedInstructionKind::Sub { dst, lhs, rhs }),
        BinaryOp::Mul => Some(UnlinkedInstructionKind::Mul { dst, lhs, rhs }),
        BinaryOp::Div => Some(UnlinkedInstructionKind::Div { dst, lhs, rhs }),
        BinaryOp::Rem => Some(UnlinkedInstructionKind::Rem { dst, lhs, rhs }),
        BinaryOp::Equal => Some(UnlinkedInstructionKind::Equal { dst, lhs, rhs }),
        BinaryOp::NotEqual => Some(UnlinkedInstructionKind::NotEqual { dst, lhs, rhs }),
        BinaryOp::Less => Some(UnlinkedInstructionKind::Less { dst, lhs, rhs }),
        BinaryOp::LessEqual => Some(UnlinkedInstructionKind::LessEqual { dst, lhs, rhs }),
        BinaryOp::Greater => Some(UnlinkedInstructionKind::Greater { dst, lhs, rhs }),
        BinaryOp::GreaterEqual => Some(UnlinkedInstructionKind::GreaterEqual { dst, lhs, rhs }),
        BinaryOp::Range | BinaryOp::RangeInclusive | BinaryOp::Or | BinaryOp::And => None,
    }
}

pub(super) fn i64_binary_instruction(
    op: BinaryOp,
    dst: Register,
    lhs: Register,
    rhs: Register,
) -> Option<UnlinkedInstructionKind> {
    match op {
        BinaryOp::Add => Some(UnlinkedInstructionKind::I64Add { dst, lhs, rhs }),
        BinaryOp::Sub => Some(UnlinkedInstructionKind::I64Sub { dst, lhs, rhs }),
        BinaryOp::Mul => Some(UnlinkedInstructionKind::I64Mul { dst, lhs, rhs }),
        BinaryOp::Rem => Some(UnlinkedInstructionKind::I64Rem { dst, lhs, rhs }),
        _ => None,
    }
}

pub(super) fn i64_immediate_instruction(
    op: BinaryOp,
    dst: Register,
    lhs: Register,
    imm: i64,
) -> Option<UnlinkedInstructionKind> {
    if !i64_immediate_op_supported(op, imm) {
        return None;
    }
    match op {
        BinaryOp::Add => Some(UnlinkedInstructionKind::I64AddImm { dst, lhs, imm }),
        BinaryOp::Sub => Some(UnlinkedInstructionKind::I64SubImm { dst, lhs, imm }),
        BinaryOp::Mul => Some(UnlinkedInstructionKind::I64MulImm { dst, lhs, imm }),
        BinaryOp::Rem => Some(UnlinkedInstructionKind::I64RemImm { dst, lhs, imm }),
        BinaryOp::Equal => Some(UnlinkedInstructionKind::I64EqImm { dst, lhs, imm }),
        BinaryOp::Greater => Some(UnlinkedInstructionKind::I64GtImm { dst, lhs, imm }),
        _ => None,
    }
}

pub(super) fn i64_immediate_op_supported(op: BinaryOp, imm: i64) -> bool {
    matches!(
        op,
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Equal | BinaryOp::Greater
    ) || matches!(op, BinaryOp::Rem if imm != 0)
}

pub(super) fn compound_assignment_instruction(
    op: AssignOp,
    dst: Register,
    lhs: Register,
    rhs: Register,
) -> Option<UnlinkedInstructionKind> {
    match op {
        AssignOp::Add => Some(UnlinkedInstructionKind::Add { dst, lhs, rhs }),
        AssignOp::Sub => Some(UnlinkedInstructionKind::Sub { dst, lhs, rhs }),
        AssignOp::Mul => Some(UnlinkedInstructionKind::Mul { dst, lhs, rhs }),
        AssignOp::Div => Some(UnlinkedInstructionKind::Div { dst, lhs, rhs }),
        AssignOp::Rem => Some(UnlinkedInstructionKind::Rem { dst, lhs, rhs }),
        AssignOp::Set => None,
    }
}

pub(super) fn i64_compound_assignment_instruction(
    op: AssignOp,
    dst: Register,
    lhs: Register,
    rhs: Register,
) -> Option<UnlinkedInstructionKind> {
    match op {
        AssignOp::Add => Some(UnlinkedInstructionKind::I64Add { dst, lhs, rhs }),
        AssignOp::Sub => Some(UnlinkedInstructionKind::I64Sub { dst, lhs, rhs }),
        AssignOp::Mul => Some(UnlinkedInstructionKind::I64Mul { dst, lhs, rhs }),
        AssignOp::Rem => Some(UnlinkedInstructionKind::I64Rem { dst, lhs, rhs }),
        AssignOp::Div | AssignOp::Set => None,
    }
}

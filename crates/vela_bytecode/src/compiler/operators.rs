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

use vela_syntax::{AssignOp, BinaryOp};

use crate::{InstructionKind, Register};

pub(super) fn non_logical_binary_instruction(
    op: BinaryOp,
    dst: Register,
    lhs: Register,
    rhs: Register,
) -> Option<InstructionKind> {
    match op {
        BinaryOp::Add => Some(InstructionKind::Add { dst, lhs, rhs }),
        BinaryOp::Sub => Some(InstructionKind::Sub { dst, lhs, rhs }),
        BinaryOp::Mul => Some(InstructionKind::Mul { dst, lhs, rhs }),
        BinaryOp::Div => Some(InstructionKind::Div { dst, lhs, rhs }),
        BinaryOp::Rem => Some(InstructionKind::Rem { dst, lhs, rhs }),
        BinaryOp::Equal => Some(InstructionKind::Equal { dst, lhs, rhs }),
        BinaryOp::NotEqual => Some(InstructionKind::NotEqual { dst, lhs, rhs }),
        BinaryOp::Less => Some(InstructionKind::Less { dst, lhs, rhs }),
        BinaryOp::LessEqual => Some(InstructionKind::LessEqual { dst, lhs, rhs }),
        BinaryOp::Greater => Some(InstructionKind::Greater { dst, lhs, rhs }),
        BinaryOp::GreaterEqual => Some(InstructionKind::GreaterEqual { dst, lhs, rhs }),
        BinaryOp::Or | BinaryOp::And => None,
    }
}

pub(super) fn compound_assignment_instruction(
    op: AssignOp,
    dst: Register,
    lhs: Register,
    rhs: Register,
) -> Option<InstructionKind> {
    match op {
        AssignOp::Add => Some(InstructionKind::Add { dst, lhs, rhs }),
        AssignOp::Sub => Some(InstructionKind::Sub { dst, lhs, rhs }),
        AssignOp::Mul => Some(InstructionKind::Mul { dst, lhs, rhs }),
        AssignOp::Div => Some(InstructionKind::Div { dst, lhs, rhs }),
        AssignOp::Rem => Some(InstructionKind::Rem { dst, lhs, rhs }),
        AssignOp::Set => None,
    }
}

use vela_hir::body::HirBinaryOp;

use crate::{BinaryLiteralOp, I64CompareOp, Register, UnlinkedInstructionKind};

pub(super) fn binary_literal_op(op: HirBinaryOp) -> Option<BinaryLiteralOp> {
    match op {
        HirBinaryOp::Add => Some(BinaryLiteralOp::Add),
        HirBinaryOp::Sub => Some(BinaryLiteralOp::Sub),
        HirBinaryOp::Mul => Some(BinaryLiteralOp::Mul),
        HirBinaryOp::Div => Some(BinaryLiteralOp::Div),
        HirBinaryOp::Rem => Some(BinaryLiteralOp::Rem),
        HirBinaryOp::Less => Some(BinaryLiteralOp::Less),
        HirBinaryOp::LessEqual => Some(BinaryLiteralOp::LessEqual),
        HirBinaryOp::Greater => Some(BinaryLiteralOp::Greater),
        HirBinaryOp::GreaterEqual => Some(BinaryLiteralOp::GreaterEqual),
        HirBinaryOp::Equal
        | HirBinaryOp::NotEqual
        | HirBinaryOp::IdentityEqual
        | HirBinaryOp::IdentityNotEqual
        | HirBinaryOp::Range
        | HirBinaryOp::RangeInclusive
        | HirBinaryOp::Or
        | HirBinaryOp::And => None,
    }
}

pub(super) fn i64_immediate_instruction(
    op: HirBinaryOp,
    dst: Register,
    lhs: Register,
    imm: i64,
) -> Option<UnlinkedInstructionKind> {
    if !i64_immediate_op_supported(op, imm) {
        return None;
    }
    match op {
        HirBinaryOp::Add => Some(UnlinkedInstructionKind::I64AddImm { dst, lhs, imm }),
        HirBinaryOp::Sub => Some(UnlinkedInstructionKind::I64SubImm { dst, lhs, imm }),
        HirBinaryOp::Mul => Some(UnlinkedInstructionKind::I64MulImm { dst, lhs, imm }),
        HirBinaryOp::Rem => Some(UnlinkedInstructionKind::I64RemImm { dst, lhs, imm }),
        HirBinaryOp::Equal
        | HirBinaryOp::NotEqual
        | HirBinaryOp::Less
        | HirBinaryOp::LessEqual
        | HirBinaryOp::Greater
        | HirBinaryOp::GreaterEqual => Some(UnlinkedInstructionKind::I64CmpImm {
            dst,
            op: i64_compare_op(op)?,
            lhs,
            imm,
        }),
        _ => None,
    }
}

pub(super) fn i64_immediate_op_supported(op: HirBinaryOp, imm: i64) -> bool {
    matches!(
        op,
        HirBinaryOp::Add
            | HirBinaryOp::Sub
            | HirBinaryOp::Mul
            | HirBinaryOp::Equal
            | HirBinaryOp::NotEqual
            | HirBinaryOp::Less
            | HirBinaryOp::LessEqual
            | HirBinaryOp::Greater
            | HirBinaryOp::GreaterEqual
    ) || matches!(op, HirBinaryOp::Rem if imm != 0)
}

pub(super) fn i64_compare_op(op: HirBinaryOp) -> Option<I64CompareOp> {
    match op {
        HirBinaryOp::Equal => Some(I64CompareOp::Equal),
        HirBinaryOp::NotEqual => Some(I64CompareOp::NotEqual),
        HirBinaryOp::IdentityEqual | HirBinaryOp::IdentityNotEqual => None,
        HirBinaryOp::Less => Some(I64CompareOp::Less),
        HirBinaryOp::LessEqual => Some(I64CompareOp::LessEqual),
        HirBinaryOp::Greater => Some(I64CompareOp::Greater),
        HirBinaryOp::GreaterEqual => Some(I64CompareOp::GreaterEqual),
        _ => None,
    }
}

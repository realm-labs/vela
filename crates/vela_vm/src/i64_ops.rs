use crate::{VmError, VmErrorKind, VmResult};

#[inline]
pub(crate) fn add_raw(lhs: i64, rhs: i64) -> VmResult<i64> {
    lhs.checked_add(rhs).ok_or_else(|| overflow("add"))
}

#[inline]
pub(crate) fn sub_raw(lhs: i64, rhs: i64) -> VmResult<i64> {
    lhs.checked_sub(rhs).ok_or_else(|| overflow("sub"))
}

#[inline]
pub(crate) fn mul_raw(lhs: i64, rhs: i64) -> VmResult<i64> {
    lhs.checked_mul(rhs).ok_or_else(|| overflow("mul"))
}

#[inline]
pub(crate) fn rem_raw(lhs: i64, rhs: i64) -> VmResult<i64> {
    if rhs == 0 {
        return Err(VmError::new(VmErrorKind::DivisionByZero));
    }
    lhs.checked_rem(rhs).ok_or_else(|| overflow("rem"))
}

#[inline]
fn overflow(operation: &'static str) -> VmError {
    VmError::new(VmErrorKind::ArithmeticOverflow { operation })
}

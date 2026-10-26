use super::*;
use rstest::rstest;
use vela_common::ScalarValue;

#[rstest]
#[case::i32_from_i16(
    r#"
fn main() {
    return i32::from_i16(-12i16);
}
"#,
    ScalarValue::I32(-12)
)]
#[case::i64_from_i32(
    r#"
fn main() {
    return i64::from_i32(12);
}
"#,
    ScalarValue::I64(12)
)]
#[case::u32_from_u16(
    r#"
fn main() {
    return u32::from_u16(255u16);
}
"#,
    ScalarValue::U32(255)
)]
#[case::u64_from_u32(
    r#"
fn main() {
    return u64::from_u32(255u32);
}
"#,
    ScalarValue::U64(255)
)]
#[case::f64_from_f32(
    r#"
fn main() {
    return f64::from_f32(1.5f32);
}
"#,
    ScalarValue::F64(1.5)
)]
fn numeric_widening_conversions_return_wider_scalar_tags(
    #[case] source: &str,
    #[case] expected: ScalarValue,
) {
    assert_eq!(
        run_standard_source(source),
        Ok(OwnedValue::Scalar(expected))
    );
}

#[rstest]
#[case::i64_add(
    r#"
fn main() {
    return i64::from_i32(1i32) + 2;
}
"#,
    ScalarValue::I64(3)
)]
#[case::f64_add(
    r#"
fn main() {
    return f64::from_f32(1.5f32) + 2.5;
}
"#,
    ScalarValue::F64(4.0)
)]
fn explicit_numeric_conversions_repair_mixed_numeric_code(
    #[case] source: &str,
    #[case] expected: ScalarValue,
) {
    assert_eq!(
        run_standard_source(source),
        Ok(OwnedValue::Scalar(expected))
    );
}

#[rstest]
#[case::i32_max(
    r#"
fn main() {
    return result::unwrap_or(i32::try_from_i64(2147483647), 0i32);
}
"#,
    ScalarValue::I32(i32::MAX)
)]
#[case::i16_negative(
    r#"
fn main() {
    return result::unwrap_or(i16::try_from_i64(-1024), 0i16);
}
"#,
    ScalarValue::I16(-1024)
)]
#[case::i8_negative(
    r#"
fn main() {
    return result::unwrap_or(i8::try_from_i64(-12), 0i8);
}
"#,
    ScalarValue::I8(-12)
)]
#[case::u16(
    r#"
fn main() {
    return result::unwrap_or(u16::try_from_u64(1024u64), 0u16);
}
"#,
    ScalarValue::U16(1024)
)]
#[case::u8(
    r#"
fn main() {
    return result::unwrap_or(u8::try_from_u64(200u64), 0u8);
}
"#,
    ScalarValue::U8(200)
)]
#[case::f32(
    r#"
fn main() {
    return result::unwrap_or(f32::try_from_f64(1.25), 0.0f32);
}
"#,
    ScalarValue::F32(1.25)
)]
fn numeric_try_conversions_return_result_ok_with_narrow_scalar_tags(
    #[case] source: &str,
    #[case] expected: ScalarValue,
) {
    assert_eq!(
        run_standard_source(source),
        Ok(OwnedValue::Scalar(expected))
    );
}

#[rstest]
#[case::i32(
    r#"
fn main() {
    return result::is_err(i32::try_from_i64(2147483648));
}
"#
)]
#[case::i16(
    r#"
fn main() {
    return result::is_err(i16::try_from_i64(32768));
}
"#
)]
#[case::i8(
    r#"
fn main() {
    return result::is_err(i8::try_from_i64(128));
}
"#
)]
#[case::u16(
    r#"
fn main() {
    return result::is_err(u16::try_from_u64(65536u64));
}
"#
)]
#[case::u8(
    r#"
fn main() {
    return result::is_err(u8::try_from_u64(256u64));
}
"#
)]
#[case::f32(
    r#"
fn main() {
    return result::is_err(f32::try_from_f64(1.0e40));
}
"#
)]
fn numeric_try_conversions_return_result_err_out_of_range(#[case] source: &str) {
    assert_eq!(run_standard_source(source), Ok(OwnedValue::Bool(true)));
}

#[test]
fn numeric_conversions_reject_wrong_source_scalar_tags() {
    let error = run_standard_source(
        r#"
fn convert(value) {
    return i64::from_i32(value);
}

fn main() {
    return convert(12i64);
}
"#,
    )
    .expect_err("i64::from_i32 requires an i32 contract");

    assert!(
        matches!(error.kind(), VmErrorKind::TypeContractViolation { .. }),
        "got {:?}",
        error.kind()
    );
}

#[test]
fn numeric_wrapping_helpers_wrap_without_operator_overflow() {
    assert_eq!(
        run_standard_source(
            r#"
fn main() {
    return [
        u8::wrapping_add(255u8, 1u8),
        u32::wrapping_mul(65536u32, 65536u32),
        i8::wrapping_add(127i8, 1i8),
    ];
}
"#
        ),
        Ok(OwnedValue::Array(vec![
            OwnedValue::Scalar(ScalarValue::U8(0)),
            OwnedValue::Scalar(ScalarValue::U32(0)),
            OwnedValue::Scalar(ScalarValue::I8(-128)),
        ]))
    );
}

#[test]
fn u8_bit_helpers_apply_explicit_protocol_operations() {
    assert_eq!(
        run_standard_source(
            r#"
fn main() {
    return [
        u8::bit_and(10u8, 12u8),
        u8::bit_or(10u8, 12u8),
        u8::bit_xor(10u8, 12u8),
        u8::shift_left(1u8, 3u32),
        u8::shift_left(1u8, 8u32),
        u8::shift_right(128u8, 7u32),
        u8::rotate_left(129u8, 1u32),
        u8::rotate_right(3u8, 1u32),
    ];
}
"#
        ),
        Ok(OwnedValue::Array(vec![
            OwnedValue::Scalar(ScalarValue::U8(8)),
            OwnedValue::Scalar(ScalarValue::U8(14)),
            OwnedValue::Scalar(ScalarValue::U8(6)),
            OwnedValue::Scalar(ScalarValue::U8(8)),
            OwnedValue::Scalar(ScalarValue::U8(0)),
            OwnedValue::Scalar(ScalarValue::U8(1)),
            OwnedValue::Scalar(ScalarValue::U8(3)),
            OwnedValue::Scalar(ScalarValue::U8(129)),
        ]))
    );
}

#[test]
fn numeric_bit_helpers_reject_wrong_source_scalar_tags() {
    let error = run_standard_source(
        r#"
fn mask(value) {
    return u8::bit_and(value, 1u8);
}

fn main() {
    return mask(1u32);
}
"#,
    )
    .expect_err("u8::bit_and requires u8 operands");

    assert!(
        matches!(error.kind(), VmErrorKind::TypeContractViolation { .. }),
        "got {:?}",
        error.kind()
    );
}

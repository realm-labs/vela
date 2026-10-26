use super::*;
use rstest::rstest;

#[test]
fn bytes_index_returns_u8_scalar() {
    assert_eq!(
        run_standard_source(
            r#"
fn main() {
    return b"abc"[0];
}
"#
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::U8(97)))
    );
}

#[rstest]
#[case::negative_index(
    r#"
fn main() {
    return b"abc"[-1];
}
"#,
    -1,
    3
)]
#[case::index_at_length(
    r#"
fn main() {
    return b"abc"[3];
}
"#,
    3,
    3
)]
#[case::short_endian_buffer(
    r#"
fn main() {
    return b"\x01\x02\x03".read_u32_le(0);
}
"#,
    0,
    3
)]
fn bytes_reads_reject_out_of_bounds(#[case] source: &str, #[case] index: i64, #[case] len: usize) {
    let error = run_standard_source(source).expect_err("bytes read should be out of bounds");
    assert_eq!(error.kind(), VmErrorKind::IndexOutOfBounds { index, len });
}

#[test]
fn bytes_index_assignment_is_rejected() {
    let error = run_standard_source(
        r#"
fn main() {
    let data = b"abc";
    data[0] = 42u8;
    return data[0];
}
"#,
    )
    .expect_err("bytes must remain immutable");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "index assignment"
        }
    );
}

#[test]
fn bytes_methods_cover_length_slice_get_and_hex() {
    assert_eq!(
        run_standard_source(
            r##"
fn main() {
    let data = b"\x00\x01\x02\xff";
    let middle = data.slice(1, 3);
    if data.len() == 4 && b"".is_empty() && data.get(3) == 255u8 && middle == b"\x01\x02" && data.to_hex() == "000102ff" {
        return middle;
    }
    return b"";
}
"##
        ),
        Ok(OwnedValue::Bytes(vec![1, 2]))
    );
}

#[test]
fn bytes_iterators_yield_u8_values() {
    assert_eq!(
        run_standard_source(
            r#"
fn main() {
    let total = 0;
    for byte in b"AZ" {
        if byte == 65u8 {
            total += 10;
        }
        if byte == 90u8 {
            total += 100;
        }
    }
    let iter_values = b"\x01\x02\x03".iter().collect_array();
    let byte_values = b"\x04\x05".values().collect_array();
    if iter_values[0] == 1u8 && iter_values[2] == 3u8 && byte_values[1] == 5u8 {
        total += iter_values.len() + byte_values.len();
    }
    return total;
}
"#
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(115)))
    );
}

#[test]
fn bytes_endian_reads_return_u32_scalars() {
    assert_eq!(
        run_standard_source(
            r#"
fn main() {
    let data = b"\x01\x02\x03\x04";
    if data.read_u32_le(0) == 0x04030201u32 && data.read_u32_be(0) == 0x01020304u32 {
        return data.read_u32_be(0);
    }
    return 0u32;
}
"#
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::U32(
            0x0102_0304
        )))
    );
}

#[test]
fn bytes_from_hex_returns_result_ok_bytes() {
    assert_eq!(
        run_standard_source(
            r#"
fn main() {
    return result::unwrap_or(bytes::from_hex("00Ff"), b"bad");
}
"#
        ),
        Ok(OwnedValue::Bytes(vec![0, 255]))
    );
}

#[test]
fn bytes_from_hex_returns_result_err_for_invalid_hex() {
    assert_eq!(
        run_standard_source(
            r#"
fn main() {
    return result::is_err(bytes::from_hex("0x"));
}
"#
        ),
        Ok(OwnedValue::Bool(true))
    );
}

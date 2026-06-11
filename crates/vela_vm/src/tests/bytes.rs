use super::*;

fn run_bytes_source(source: &str) -> VmResult<OwnedValue> {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let program =
        compile_program_source_with_registry(SourceId::new(1), source, registry.compile_view())
            .expect("bytes source compiles");
    let mut linker = Linker::with_registry(&registry);
    let vm = Vm::new().with_standard_natives();
    vm.native_ids
        .keys()
        .copied()
        .for_each(|id| linker.add_native_implementation(id));
    let linked = linker
        .link_program(&program)
        .expect("bytes source should link");
    vm.run_linked_program(&linked, "main", &[])
}

#[test]
fn bytes_index_returns_u8_scalar() {
    assert_eq!(
        run_bytes_source(
            r#"
fn main() {
    return b"abc"[0];
}
"#
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::U8(97)))
    );
}

#[test]
fn bytes_index_rejects_negative_indexes() {
    let error = run_bytes_source(
        r#"
fn main() {
    return b"abc"[-1];
}
"#,
    )
    .expect_err("negative bytes index should fail");

    assert_eq!(
        error.kind(),
        VmErrorKind::IndexOutOfBounds { index: -1, len: 3 }
    );
}

#[test]
fn bytes_index_rejects_out_of_bounds_indexes() {
    let error = run_bytes_source(
        r#"
fn main() {
    return b"abc"[3];
}
"#,
    )
    .expect_err("out-of-bounds bytes index should fail");

    assert_eq!(
        error.kind(),
        VmErrorKind::IndexOutOfBounds { index: 3, len: 3 }
    );
}

#[test]
fn bytes_index_assignment_is_rejected() {
    let error = run_bytes_source(
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
        run_bytes_source(
            r#"
fn main() {
    let data = b"\x00\x01\x02\xff";
    let middle = data.slice(1, 3);
    if data.len() == 4 && b"".is_empty() && data.get(3) == 255u8 && middle == b"\x01\x02" && data.to_hex() == "000102ff" {
        return middle;
    }
    return b"";
}
"#
        ),
        Ok(OwnedValue::Bytes(vec![1, 2]))
    );
}

#[test]
fn bytes_endian_reads_return_u32_scalars() {
    assert_eq!(
        run_bytes_source(
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
fn bytes_endian_reads_reject_short_buffers() {
    let error = run_bytes_source(
        r#"
fn main() {
    return b"\x01\x02\x03".read_u32_le(0);
}
"#,
    )
    .expect_err("u32 read should need four bytes");

    assert_eq!(
        error.kind(),
        VmErrorKind::IndexOutOfBounds { index: 0, len: 3 }
    );
}

#[test]
fn bytes_from_hex_returns_result_ok_bytes() {
    assert_eq!(
        run_bytes_source(
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
        run_bytes_source(
            r#"
fn main() {
    return result::is_err(bytes::from_hex("0x"));
}
"#
        ),
        Ok(OwnedValue::Bool(true))
    );
}

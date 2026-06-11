use super::*;

fn run_bytes_source(source: &str) -> VmResult<OwnedValue> {
    let program = compile_program_source(SourceId::new(1), source).expect("bytes source compiles");
    let linked = link_test_program(&program);
    Vm::new().run_linked_program(&linked, "main", &[])
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

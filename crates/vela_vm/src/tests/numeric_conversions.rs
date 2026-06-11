use super::*;
use vela_bytecode::compiler::compile_program_source_with_registry;
use vela_common::ScalarValue;

fn run_conversion_source(source: &str) -> VmResult<OwnedValue> {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let program =
        compile_program_source_with_registry(SourceId::new(1), source, registry.compile_view())
            .expect("conversion source compiles");
    let mut linker = Linker::with_registry(&registry);
    let vm = Vm::new().with_standard_natives();
    vm.native_ids
        .keys()
        .copied()
        .for_each(|id| linker.add_native_implementation(id));
    let linked = linker
        .link_program(&program)
        .expect("conversion source should link");
    vm.run_linked_program(&linked, "main", &[])
}

#[test]
fn numeric_widening_conversions_return_wider_scalar_tags() {
    assert_eq!(
        run_conversion_source(
            r#"
fn main() {
    return i64::from_i32(12);
}
"#
        ),
        Ok(OwnedValue::Scalar(ScalarValue::I64(12)))
    );
    assert_eq!(
        run_conversion_source(
            r#"
fn main() {
    return u64::from_u32(255u32);
}
"#
        ),
        Ok(OwnedValue::Scalar(ScalarValue::U64(255)))
    );
    assert_eq!(
        run_conversion_source(
            r#"
fn main() {
    return f64::from_f32(1.5f32);
}
"#
        ),
        Ok(OwnedValue::Scalar(ScalarValue::F64(1.5)))
    );
}

#[test]
fn numeric_try_conversions_return_result_ok_with_narrow_scalar_tags() {
    assert_eq!(
        run_conversion_source(
            r#"
fn main() {
    return result::unwrap_or(i8::try_from_i64(-12), 0i8);
}
"#
        ),
        Ok(OwnedValue::Scalar(ScalarValue::I8(-12)))
    );
    assert_eq!(
        run_conversion_source(
            r#"
fn main() {
    return result::unwrap_or(u8::try_from_u64(200u64), 0u8);
}
"#
        ),
        Ok(OwnedValue::Scalar(ScalarValue::U8(200)))
    );
    assert_eq!(
        run_conversion_source(
            r#"
fn main() {
    return result::unwrap_or(f32::try_from_f64(1.25), 0.0f32);
}
"#
        ),
        Ok(OwnedValue::Scalar(ScalarValue::F32(1.25)))
    );
}

#[test]
fn numeric_try_conversions_return_result_err_out_of_range() {
    assert_eq!(
        run_conversion_source(
            r#"
fn main() {
    return result::is_err(i8::try_from_i64(128));
}
"#
        ),
        Ok(OwnedValue::Bool(true))
    );
    assert_eq!(
        run_conversion_source(
            r#"
fn main() {
    return result::is_err(u8::try_from_u64(256u64));
}
"#
        ),
        Ok(OwnedValue::Bool(true))
    );
    assert_eq!(
        run_conversion_source(
            r#"
fn main() {
    return result::is_err(f32::try_from_f64(1.0e40));
}
"#
        ),
        Ok(OwnedValue::Bool(true))
    );
}

#[test]
fn numeric_conversions_reject_wrong_source_scalar_tags() {
    let error = run_conversion_source(
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

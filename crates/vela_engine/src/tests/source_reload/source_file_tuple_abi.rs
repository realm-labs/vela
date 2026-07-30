use super::*;

#[test]
fn runtime_accepts_source_file_equivalent_tuple_signature_update() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
fn split_pair(value: String) -> Option<(String, i64)> {
    return ();
}

fn main() -> i64 {
    return 1;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(
        &mut runtime,
        r#"
fn split_pair(value: String) -> Option< ( String , i64 ) > {
    return ();
}

fn main() -> i64 {
    return 2;
}
"#,
    );

    let report = runtime
        .activate_reload()
        .expect("check reload at safe point")
        .expect("staged equivalent tuple signature update report");

    assert!(report.accepted);
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn runtime_rejects_source_file_tuple_signature_abi_change_until_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
fn split_pair(value: String) -> Option<(String, i64)> {
    return ();
}

fn main() -> i64 {
    return 1;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(
        &mut runtime,
        r#"
fn split_pair(value: String) -> Option<(String, i64, bool)> {
    return ();
}

fn main() -> i64 {
    return 2;
}
"#,
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .activate_reload()
        .expect("check reload at safe point")
        .expect("staged tuple signature ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.return_abi_changed");
    let HotReloadErrorKind::ChangedFunctionReturnAbi {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed function return ABI");
    };
    assert_eq!(function, "split_pair");
    assert_eq!(old.as_deref(), Some("Option<(String, i64)>"));
    assert_eq!(new.as_deref(), Some("Option<(String, i64, bool)>"));
    assert!(source_span.is_some());
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

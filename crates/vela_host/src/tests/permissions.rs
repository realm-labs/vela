use super::*;

#[test]
fn read_denied_path_fails_without_hiding_freshness_checks() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    adapter.insert_value(path.clone(), HostValue::Int(9));
    adapter.deny_read(path.clone());

    let error = adapter
        .read_path(&path)
        .expect_err("read denied path should fail");

    assert_eq!(
        error.kind,
        HostErrorKind::PermissionDenied {
            path,
            action: "read"
        }
    );
}

#[test]
fn write_denied_patch_fails_validation_before_mutation() {
    let mut adapter = MockStateAdapter::new();
    let level = level_path();
    let rewards = rewards_path();
    adapter.insert_value(level.clone(), HostValue::Int(9));
    adapter.insert_value(rewards.clone(), HostValue::Int(1));
    adapter.deny_write(rewards.clone());
    let mut tx = PatchTx::new();

    tx.set_path(level.clone(), HostValue::Int(10), None)
        .expect("set level");
    tx.set_path(rewards.clone(), HostValue::Int(2), None)
        .expect("set rewards");

    let error = tx
        .apply(&mut adapter)
        .expect_err("write denied patch should fail validation");

    assert_eq!(
        error.kind,
        HostErrorKind::PermissionDenied {
            path: rewards.clone(),
            action: "write"
        }
    );
    assert_eq!(adapter.read_path(&level), Ok(HostValue::Int(9)));
    assert_eq!(adapter.read_path(&rewards), Ok(HostValue::Int(1)));
}

#[test]
fn denied_patch_validation_error_keeps_source_span() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    let span = test_span();
    adapter.insert_value(path.clone(), HostValue::Int(9));
    adapter.deny_write(path.clone());
    let mut tx = PatchTx::new();

    tx.set_path(path.clone(), HostValue::Int(10), Some(span))
        .expect("set path");
    let error = tx
        .apply(&mut adapter)
        .expect_err("denied write should fail validation");

    assert_eq!(error.source_span, Some(span));
    assert_eq!(
        error.kind,
        HostErrorKind::PermissionDenied {
            path,
            action: "write"
        }
    );
}

#[test]
fn call_denied_patch_fails_validation_before_method_call() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    let method = HostMethodId::new(4);
    adapter.insert_value(path.clone(), HostValue::Int(9));
    adapter.insert_method_return(method, HostValue::Null);
    adapter.deny_call(path.clone());
    let mut tx = PatchTx::new();

    tx.call_method(path.clone(), method, vec![HostValue::Int(1)], None)
        .expect("record method call");

    let error = tx
        .apply(&mut adapter)
        .expect_err("call denied patch should fail validation");

    assert_eq!(
        error.kind,
        HostErrorKind::PermissionDenied {
            path,
            action: "call"
        }
    );
    assert!(adapter.method_calls().is_empty());
}

#[test]
fn preview_method_return_copies_configured_value_without_calling_method() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    let method = HostMethodId::new(4);
    adapter.insert_method_return(method, HostValue::Int(12));

    let value = adapter
        .preview_method_return(&path, method, &[HostValue::Int(1)])
        .expect("preview method return");

    assert_eq!(value, HostValue::Int(12));
    assert!(adapter.method_calls().is_empty());
}

#[test]
fn adapter_rejects_stale_generation_on_read_and_apply() {
    let mut adapter = MockStateAdapter::new();
    let fresh_path = level_path();
    adapter.insert_value(fresh_path, HostValue::Int(9));
    let stale_path = HostPath::new(player_ref(2)).field(FieldId::new(2));
    let mut tx = PatchTx::new();

    let read_error = adapter
        .read_path(&stale_path)
        .expect_err("stale read should fail");
    assert_eq!(
        read_error.kind,
        HostErrorKind::StaleGeneration {
            expected: 2,
            actual: 3
        }
    );

    tx.set_path(stale_path, HostValue::Int(10), None)
        .expect("patch recording does not touch adapter");
    let apply_error = tx.apply(&mut adapter).expect_err("stale apply should fail");
    assert_eq!(
        apply_error.kind,
        HostErrorKind::StaleGeneration {
            expected: 2,
            actual: 3
        }
    );
}

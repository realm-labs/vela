use super::*;

#[test]
fn read_denied_path_fails_without_hiding_freshness_checks() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    adapter.insert_diagnostic_path_value(path.clone(), HostValue::Int(9));
    adapter.deny_diagnostic_path_read(path.clone());

    let error = adapter
        .read_diagnostic_path(&path)
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
fn write_denied_path_fails_before_mutation_and_keeps_previous_writes() {
    let mut adapter = MockStateAdapter::new();
    let level = level_path();
    let rewards = rewards_path();
    adapter.insert_diagnostic_path_value(level.clone(), HostValue::Int(9));
    adapter.insert_diagnostic_path_value(rewards.clone(), HostValue::Int(1));
    adapter.deny_diagnostic_path_write(rewards.clone());
    let mut tx = HostAccess::new();

    tx.set_path(&mut adapter, level.clone(), HostValue::Int(10), None)
        .expect("set level");
    let error = tx
        .set_path(&mut adapter, rewards.clone(), HostValue::Int(2), None)
        .expect_err("write denied path should fail");

    assert_eq!(
        error.kind,
        HostErrorKind::PermissionDenied {
            path: rewards.clone(),
            action: "write"
        }
    );
    assert_eq!(adapter.read_diagnostic_path(&level), Ok(HostValue::Int(10)));
    assert_eq!(
        adapter.read_diagnostic_path(&rewards),
        Ok(HostValue::Int(1))
    );
}

#[test]
fn denied_write_error_keeps_source_span() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    let span = test_span();
    adapter.insert_diagnostic_path_value(path.clone(), HostValue::Int(9));
    adapter.deny_diagnostic_path_write(path.clone());
    let mut tx = HostAccess::new();

    let error = tx
        .set_path(&mut adapter, path.clone(), HostValue::Int(10), Some(span))
        .expect_err("denied write should fail");

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
fn call_denied_path_fails_before_method_call() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    let method = HostMethodId::new(4);
    adapter.insert_diagnostic_path_value(path.clone(), HostValue::Int(9));
    adapter.insert_method_return(method, HostValue::Null);
    adapter.deny_diagnostic_path_call(path.clone());
    let mut tx = HostAccess::new();

    let error = tx
        .call_method(
            &mut adapter,
            path.clone(),
            method,
            vec![HostValue::Int(1)],
            None,
        )
        .expect_err("call denied path should fail");

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
fn adapter_rejects_stale_generation_on_read_and_write() {
    let mut adapter = MockStateAdapter::new();
    let fresh_path = level_path();
    adapter.insert_diagnostic_path_value(fresh_path, HostValue::Int(9));
    let stale_path = HostPath::new(player_ref(2)).field(FieldId::new(2));
    let mut tx = HostAccess::new();

    let read_error = adapter
        .read_diagnostic_path(&stale_path)
        .expect_err("stale read should fail");
    assert_eq!(
        read_error.kind,
        HostErrorKind::StaleGeneration {
            expected: 2,
            actual: 3
        }
    );

    let write_error = tx
        .set_path(&mut adapter, stale_path, HostValue::Int(10), None)
        .expect_err("stale write should fail");
    assert_eq!(
        write_error.kind,
        HostErrorKind::StaleGeneration {
            expected: 2,
            actual: 3
        }
    );
}

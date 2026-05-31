use super::*;

#[test]
fn apply_commits_set_add_sub_and_push_at_safe_point() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    adapter.insert_value(path.clone(), HostValue::Int(9));
    let mut tx = PatchTx::new();

    tx.set_path(path.clone(), HostValue::Int(10), None)
        .expect("set path");
    tx.add_path(path.clone(), HostValue::Int(2), HostValue::Int(0), None)
        .expect("add path");
    tx.sub_path(path.clone(), HostValue::Int(5), HostValue::Int(0), None)
        .expect("sub path");
    assert_eq!(adapter.read_path(&path), Ok(HostValue::Int(9)));

    tx.apply(&mut adapter).expect("apply transaction");

    assert_eq!(adapter.read_path(&path), Ok(HostValue::Int(7)));
}

#[test]
fn apply_commits_push_at_safe_point() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    adapter.insert_value(
        path.clone(),
        HostValue::Array(vec![HostValue::String("xp".into())]),
    );
    let mut tx = PatchTx::new();

    tx.push_path(
        path.clone(),
        HostValue::String("gold".into()),
        HostValue::Array(vec![HostValue::String("xp".into())]),
        None,
    )
    .expect("push path");
    assert_eq!(
        adapter.read_path(&path),
        Ok(HostValue::Array(vec![HostValue::String("xp".into())]))
    );

    tx.apply(&mut adapter).expect("apply push transaction");

    assert_eq!(
        adapter.read_path(&path),
        Ok(HostValue::Array(vec![
            HostValue::String("xp".into()),
            HostValue::String("gold".into())
        ]))
    );
}

#[test]
fn apply_commits_remove_at_safe_point() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    adapter.insert_value(path.clone(), HostValue::Int(9));
    let mut tx = PatchTx::new();

    tx.remove_path(path.clone(), None).expect("remove path");
    assert_eq!(adapter.read_path(&path), Ok(HostValue::Int(9)));

    tx.apply(&mut adapter).expect("apply remove transaction");

    assert_eq!(
        adapter.read_path(&path),
        Err(crate::HostError::new(HostErrorKind::MissingPath { path }))
    );
}

#[test]
fn failed_apply_leaves_mock_adapter_state_unchanged() {
    let mut adapter = MockStateAdapter::new();
    let level = level_path();
    let rewards = rewards_path();
    adapter.insert_value(level.clone(), HostValue::Int(9));
    adapter.insert_value(rewards.clone(), HostValue::Int(1));
    let mut tx = PatchTx::new();

    tx.set_path(level.clone(), HostValue::Int(10), None)
        .expect("set path");
    tx.push_path(
        rewards.clone(),
        HostValue::String("gold".into()),
        HostValue::Array(Vec::new()),
        None,
    )
    .expect("record push path");

    let error = tx
        .apply(&mut adapter)
        .expect_err("push apply should fail against non-array adapter state");

    assert_eq!(
        error.kind,
        HostErrorKind::PatchConflict {
            path: rewards.clone(),
            expected: Box::new(HostValue::Array(Vec::new())),
            actual: Some(Box::new(HostValue::Int(1))),
        }
    );
    assert_eq!(adapter.read_path(&level), Ok(HostValue::Int(9)));
    assert_eq!(adapter.read_path(&rewards), Ok(HostValue::Int(1)));
}

#[test]
fn conflicting_base_value_reports_patch_conflict_before_apply() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    let span = test_span();
    adapter.insert_value(path.clone(), HostValue::Int(9));
    let mut tx = PatchTx::new();

    tx.add_path(
        path.clone(),
        HostValue::Int(1),
        HostValue::Int(9),
        Some(span),
    )
    .expect("record add path");
    adapter
        .write_path(&path, HostValue::Int(12))
        .expect("simulate host state changing before apply");

    let error = tx
        .apply(&mut adapter)
        .expect_err("changed base value should conflict");

    assert_eq!(error.source_span, Some(span));
    assert_eq!(
        error.kind,
        HostErrorKind::PatchConflict {
            path: path.clone(),
            expected: Box::new(HostValue::Int(9)),
            actual: Some(Box::new(HostValue::Int(12))),
        }
    );
    assert_eq!(adapter.read_path(&path), Ok(HostValue::Int(12)));
}

#[test]
fn apply_error_keeps_patch_source_span() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    let span = test_span();
    adapter.insert_value(path.clone(), HostValue::Int(9));
    let mut tx = PatchTx::new();

    tx.push_path(
        path.clone(),
        HostValue::String("gold".into()),
        HostValue::Array(Vec::new()),
        Some(span),
    )
    .expect("record push path");
    let error = tx
        .apply(&mut adapter)
        .expect_err("push apply should fail against non-array adapter state");

    assert_eq!(error.source_span, Some(span));
    assert_eq!(
        error.kind,
        HostErrorKind::PatchConflict {
            path,
            expected: Box::new(HostValue::Array(Vec::new())),
            actual: Some(Box::new(HostValue::Int(9))),
        }
    );
}

#[test]
fn call_method_patch_applies_at_safe_point() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    let method = HostMethodId::new(4);
    adapter.insert_value(path.clone(), HostValue::Int(9));
    adapter.insert_method_return(method, HostValue::Null);
    let mut tx = PatchTx::new();

    tx.call_method(
        path.clone(),
        method,
        vec![HostValue::String("gold".into())],
        None,
    )
    .expect("record method call");
    assert!(adapter.method_calls().is_empty());

    tx.apply(&mut adapter).expect("apply method call");

    assert_eq!(
        adapter.method_calls(),
        &[(path, method, vec![HostValue::String("gold".into())])]
    );
}

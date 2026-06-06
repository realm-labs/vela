use super::*;

#[test]
fn write_through_set_and_numeric_patches_mutate_immediately() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    adapter.insert_value(path.clone(), HostValue::Int(9));
    let mut tx = PatchTx::new();

    tx.set_path(&mut adapter, path.clone(), HostValue::Int(10), None)
        .expect("set path");
    assert_eq!(adapter.read_path(&path), Ok(HostValue::Int(10)));

    tx.add_path(&mut adapter, path.clone(), HostValue::Int(2), None)
        .expect("add path");
    tx.sub_path(&mut adapter, path.clone(), HostValue::Int(5), None)
        .expect("sub path");

    assert_eq!(adapter.read_path(&path), Ok(HostValue::Int(7)));
    assert_eq!(tx.patches().len(), 3);
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(10)));
    assert_eq!(tx.patches()[1].op, PatchOp::Add(HostValue::Int(2)));
    assert_eq!(tx.patches()[2].op, PatchOp::Sub(HostValue::Int(5)));
}

#[test]
fn write_through_push_remove_and_method_call_are_immediate() {
    let mut adapter = MockStateAdapter::new();
    let rewards = rewards_path();
    let method_path = level_path();
    let method = HostMethodId::new(4);
    adapter.insert_value(
        rewards.clone(),
        HostValue::Array(vec![HostValue::String("xp".into())]),
    );
    adapter.insert_value(method_path.clone(), HostValue::Int(9));
    adapter.insert_method_return(method, HostValue::String("ok".into()));
    let mut tx = PatchTx::new();

    tx.push_path(
        &mut adapter,
        rewards.clone(),
        HostValue::String("gold".into()),
        None,
    )
    .expect("push path");
    assert_eq!(
        adapter.read_path(&rewards),
        Ok(HostValue::Array(vec![
            HostValue::String("xp".into()),
            HostValue::String("gold".into())
        ]))
    );

    let result = tx
        .call_method(
            &mut adapter,
            method_path.clone(),
            method,
            vec![HostValue::Int(1)],
            None,
        )
        .expect("call method");
    assert_eq!(result, HostValue::String("ok".into()));
    assert_eq!(
        adapter.method_calls(),
        &[(method_path.clone(), method, vec![HostValue::Int(1)])]
    );

    tx.remove_path(&mut adapter, rewards.clone(), None)
        .expect("remove path");
    assert_eq!(
        adapter.read_path(&rewards),
        Err(HostError::new(HostErrorKind::MissingPath { path: rewards }))
    );
}

#[test]
fn write_through_error_keeps_previous_successful_writes() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    adapter.insert_value(path.clone(), HostValue::Int(9));
    let mut tx = PatchTx::new();

    tx.set_path(&mut adapter, path.clone(), HostValue::Int(10), None)
        .expect("set path");
    let error = tx
        .div_path(&mut adapter, path.clone(), HostValue::Int(0), None)
        .expect_err("division by zero should fail");

    assert_eq!(error.kind, HostErrorKind::InvalidDiv { path: path.clone() });
    assert_eq!(adapter.read_path(&path), Ok(HostValue::Int(10)));
    assert_eq!(tx.patches().len(), 1);
}

#[test]
fn write_through_error_keeps_source_span() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    let span = test_span();
    adapter.insert_value(path.clone(), HostValue::Int(9));
    let mut tx = PatchTx::new();

    let error = tx
        .push_path(
            &mut adapter,
            path.clone(),
            HostValue::String("gold".into()),
            Some(span),
        )
        .expect_err("push should fail against non-array host value");

    assert_eq!(error.source_span, Some(span));
    assert_eq!(error.kind, HostErrorKind::InvalidPush { path });
}

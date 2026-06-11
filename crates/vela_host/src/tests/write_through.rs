use super::*;

#[test]
fn write_through_set_and_numeric_mutations_mutate_immediately() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    adapter.insert_diagnostic_path_value(
        path.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(9)),
    );
    let mut tx = HostAccess::new();

    tx.write_diagnostic_path(
        &mut adapter,
        path.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(10)),
        None,
    )
    .expect("set path");
    assert_eq!(
        adapter.read_diagnostic_path(&path),
        Ok(HostValue::Scalar(vela_common::ScalarValue::I64(10)))
    );

    tx.add_diagnostic_path(
        &mut adapter,
        path.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(2)),
        None,
    )
    .expect("add path");
    tx.sub_diagnostic_path(
        &mut adapter,
        path.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(5)),
        None,
    )
    .expect("sub path");

    assert_eq!(
        adapter.read_diagnostic_path(&path),
        Ok(HostValue::Scalar(vela_common::ScalarValue::I64(7)))
    );
}

#[test]
fn write_through_rejects_push_and_keeps_method_call_remove_immediate() {
    let mut adapter = MockStateAdapter::new();
    let rewards = rewards_path();
    let method_path = level_path();
    let method = HostMethodId::new(4);
    adapter.insert_diagnostic_path_value(
        rewards.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(0)),
    );
    adapter.insert_diagnostic_path_value(
        method_path.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(9)),
    );
    adapter.insert_method_return(method, HostValue::String("ok".into()));
    let mut tx = HostAccess::new();

    let push_error = tx
        .push_diagnostic_path(
            &mut adapter,
            rewards.clone(),
            HostValue::String("gold".into()),
            None,
        )
        .expect_err("push path should reject scalar-only host values");
    assert_eq!(
        push_error.kind,
        HostErrorKind::InvalidPush {
            path: rewards.clone()
        }
    );
    assert_eq!(
        adapter.read_diagnostic_path(&rewards),
        Ok(HostValue::Scalar(vela_common::ScalarValue::I64(0)))
    );

    let result = tx
        .call_diagnostic_path_method(
            &mut adapter,
            method_path.clone(),
            method,
            vec![HostValue::Scalar(vela_common::ScalarValue::I64(1))],
            None,
        )
        .expect("call method");
    assert_eq!(result, HostValue::String("ok".into()));
    assert_eq!(adapter.method_calls().len(), 1);
    assert_eq!(adapter.method_calls()[0].diagnostic_path(), method_path);
    assert_eq!(adapter.method_calls()[0].method, method);
    assert_eq!(
        adapter.method_calls()[0].args,
        vec![HostValue::Scalar(vela_common::ScalarValue::I64(1))]
    );

    tx.remove_diagnostic_path(&mut adapter, rewards.clone(), None)
        .expect("remove path");
    assert_eq!(
        adapter.read_diagnostic_path(&rewards),
        Err(HostError::new(HostErrorKind::MissingPath { path: rewards }))
    );
}

#[test]
fn write_through_error_keeps_source_span() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    let span = test_span();
    adapter.insert_diagnostic_path_value(
        path.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(9)),
    );
    let mut tx = HostAccess::new();

    let error = tx
        .push_diagnostic_path(
            &mut adapter,
            path.clone(),
            HostValue::String("gold".into()),
            Some(span),
        )
        .expect_err("push should fail against scalar host value");

    assert_eq!(error.source_span, Some(span));
    assert_eq!(error.kind, HostErrorKind::InvalidPush { path });
}

#[test]
fn write_through_error_keeps_previous_successful_writes() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    adapter.insert_diagnostic_path_value(
        path.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(9)),
    );
    let mut tx = HostAccess::new();

    tx.write_diagnostic_path(
        &mut adapter,
        path.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(10)),
        None,
    )
    .expect("set path");
    let error = tx
        .div_diagnostic_path(
            &mut adapter,
            path.clone(),
            HostValue::Scalar(vela_common::ScalarValue::I64(0)),
            None,
        )
        .expect_err("division by zero should fail");

    assert_eq!(error.kind, HostErrorKind::InvalidDiv { path: path.clone() });
    assert_eq!(
        adapter.read_diagnostic_path(&path),
        Ok(HostValue::Scalar(vela_common::ScalarValue::I64(10)))
    );
}

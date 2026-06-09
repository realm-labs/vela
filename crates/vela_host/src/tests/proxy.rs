use super::*;

#[test]
fn path_proxy_routes_reads_and_writes_through_host_access() {
    let path = level_path();
    let proxy = PathProxy::from_diagnostic_path(path.clone());
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(path.clone(), HostValue::Int(9));
    let mut tx = HostAccess::new();

    assert_eq!(
        proxy.read(&mut adapter, &tx, None).expect("read host path"),
        HostValue::Int(9)
    );

    proxy
        .set(&mut adapter, &mut tx, HostValue::Int(10), None)
        .expect("set through proxy");

    assert_eq!(adapter.read_path(&path), Ok(HostValue::Int(10)));
    assert_eq!(
        proxy.read(&mut adapter, &tx, None).expect("read host path"),
        HostValue::Int(10)
    );
}

#[test]
fn path_proxy_records_rmw_remove_and_calls() {
    let level = level_path();
    let rewards = rewards_path();
    let method = HostMethodId::new(8);
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(level.clone(), HostValue::Int(9));
    adapter.insert_value(rewards.clone(), HostValue::Int(0));
    adapter.insert_method_return(method, HostValue::String("ok".into()));
    let mut tx = HostAccess::new();

    PathProxy::from_diagnostic_path(level.clone())
        .add(&mut adapter, &mut tx, HostValue::Int(2), None)
        .expect("add through proxy");
    PathProxy::from_diagnostic_path(level.clone())
        .sub(&mut adapter, &mut tx, HostValue::Int(1), None)
        .expect("sub through proxy");
    PathProxy::from_diagnostic_path(level.clone())
        .mul(&mut adapter, &mut tx, HostValue::Int(3), None)
        .expect("mul through proxy");
    PathProxy::from_diagnostic_path(level.clone())
        .div(&mut adapter, &mut tx, HostValue::Int(2), None)
        .expect("div through proxy");
    PathProxy::from_diagnostic_path(level.clone())
        .rem(&mut adapter, &mut tx, HostValue::Int(5), None)
        .expect("rem through proxy");
    let push_error = PathProxy::from_diagnostic_path(rewards.clone())
        .push(
            &mut adapter,
            &mut tx,
            HostValue::String("gold".into()),
            None,
        )
        .expect_err("push through proxy should reject scalar-only host values");
    assert_eq!(
        push_error.kind,
        HostErrorKind::InvalidPush {
            path: rewards.clone()
        }
    );
    let result = PathProxy::from_diagnostic_path(level.clone())
        .call_method(&mut adapter, &mut tx, method, vec![HostValue::Int(5)], None)
        .expect("method call through proxy");
    PathProxy::from_diagnostic_path(rewards.clone())
        .remove(&mut adapter, &mut tx, None)
        .expect("remove through proxy");

    assert_eq!(adapter.read_path(&level), Ok(HostValue::Int(0)));
    assert_eq!(
        adapter.read_path(&rewards),
        Err(HostError::new(HostErrorKind::MissingPath { path: rewards }))
    );
    assert_eq!(result, HostValue::String("ok".into()));
}

#[test]
fn path_proxy_uses_target_plan_and_owned_dynamic_args() {
    let root = player_ref(3);
    let static_path = HostPath::new(root)
        .field(FieldId::new(4))
        .index(2)
        .key("gold");
    let proxy = PathProxy::new(
        root,
        crate::target::HostTargetPlan::new(root.type_id).field(FieldId::new(4)),
    )
    .index(2)
    .key("gold");
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(static_path.clone(), HostValue::Int(11));
    let tx = HostAccess::new();

    assert_eq!(proxy.to_diagnostic_path(), static_path);
    assert_eq!(
        proxy.read(&mut adapter, &tx, None).expect("read proxy"),
        HostValue::Int(11)
    );
}

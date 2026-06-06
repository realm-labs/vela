use super::*;

#[test]
fn path_proxy_routes_reads_and_writes_through_patch_tx() {
    let path = level_path();
    let proxy = PathProxy::new(path.clone());
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(path.clone(), HostValue::Int(9));
    let mut tx = PatchTx::new();

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
    assert_eq!(tx.patches()[0].path, path);
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(10)));
}

#[test]
fn path_proxy_records_rmw_push_remove_and_calls() {
    let level = level_path();
    let rewards = rewards_path();
    let method = HostMethodId::new(8);
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(level.clone(), HostValue::Int(9));
    adapter.insert_value(rewards.clone(), HostValue::Array(Vec::new()));
    adapter.insert_method_return(method, HostValue::String("ok".into()));
    let mut tx = PatchTx::new();

    PathProxy::new(level.clone())
        .add(&mut adapter, &mut tx, HostValue::Int(2), None)
        .expect("add through proxy");
    PathProxy::new(level.clone())
        .sub(&mut adapter, &mut tx, HostValue::Int(1), None)
        .expect("sub through proxy");
    PathProxy::new(level.clone())
        .mul(&mut adapter, &mut tx, HostValue::Int(3), None)
        .expect("mul through proxy");
    PathProxy::new(level.clone())
        .div(&mut adapter, &mut tx, HostValue::Int(2), None)
        .expect("div through proxy");
    PathProxy::new(level.clone())
        .rem(&mut adapter, &mut tx, HostValue::Int(5), None)
        .expect("rem through proxy");
    PathProxy::new(rewards.clone())
        .push(
            &mut adapter,
            &mut tx,
            HostValue::String("gold".into()),
            None,
        )
        .expect("push through proxy");
    let result = PathProxy::new(level.clone())
        .call_method(&mut adapter, &mut tx, method, vec![HostValue::Int(5)], None)
        .expect("method call through proxy");
    PathProxy::new(rewards.clone())
        .remove(&mut adapter, &mut tx, None)
        .expect("remove through proxy");

    assert_eq!(adapter.read_path(&level), Ok(HostValue::Int(0)));
    assert_eq!(
        adapter.read_path(&rewards),
        Err(HostError::new(HostErrorKind::MissingPath { path: rewards }))
    );
    assert_eq!(result, HostValue::String("ok".into()));
    assert_eq!(tx.patches()[0].op, PatchOp::Add(HostValue::Int(2)));
    assert_eq!(tx.patches()[1].op, PatchOp::Sub(HostValue::Int(1)));
    assert_eq!(tx.patches()[2].op, PatchOp::Mul(HostValue::Int(3)));
    assert_eq!(tx.patches()[3].op, PatchOp::Div(HostValue::Int(2)));
    assert_eq!(tx.patches()[4].op, PatchOp::Rem(HostValue::Int(5)));
    assert_eq!(
        tx.patches()[5].op,
        PatchOp::Push(HostValue::String("gold".into()))
    );
    assert_eq!(
        tx.patches()[6].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::Int(5)]
        }
    );
    assert_eq!(tx.patches()[7].op, PatchOp::Remove);
}

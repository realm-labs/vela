use super::*;

#[test]
fn set_path_records_patch_and_overlay_value() {
    let mut tx = PatchTx::new();
    let path = level_path();

    tx.set_path(path.clone(), HostValue::Int(10), None)
        .expect("set path");

    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(10)));
    assert_eq!(tx.read_overlay(&path), Some(&HostValue::Int(10)));
}

#[test]
fn overlay_reads_through_equivalent_path_keys() {
    let mut tx = PatchTx::new();
    let write_path = level_path();
    let read_path = level_path();

    tx.set_path(write_path, HostValue::Int(10), None)
        .expect("set path");

    assert_eq!(tx.read_overlay(&read_path), Some(&HostValue::Int(10)));
}

#[test]
fn variant_field_paths_record_overlay_and_apply() {
    let path = quest_variant_count_path();
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(path.clone(), HostValue::Int(2));
    let mut tx = PatchTx::new();

    assert_eq!(
        tx.read_path(&adapter, &path)
            .expect("read variant field path"),
        HostValue::Int(2)
    );

    tx.add_path(path.clone(), HostValue::Int(1), HostValue::Int(2), None)
        .expect("record variant field add");

    assert_eq!(
        tx.read_path(&adapter, &path)
            .expect("read variant field overlay"),
        HostValue::Int(3)
    );
    assert_eq!(tx.patches()[0].path, path);
    assert_eq!(tx.patches()[0].op, PatchOp::Add(HostValue::Int(1)));

    tx.apply(&mut adapter)
        .expect("apply variant field patch at safe point");

    assert_eq!(adapter.read_path(&path), Ok(HostValue::Int(3)));
}

#[test]
fn add_path_records_patch_and_updates_overlay_from_base() {
    let mut tx = PatchTx::new();
    let path = level_path();

    tx.add_path(path.clone(), HostValue::Int(1), HostValue::Int(9), None)
        .expect("add path");

    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].op, PatchOp::Add(HostValue::Int(1)));
    assert_eq!(tx.patches()[0].expected_base, Some(HostValue::Int(9)));
    assert_eq!(tx.read_overlay(&path), Some(&HostValue::Int(10)));
}

#[test]
fn add_path_uses_previous_overlay_value() {
    let mut tx = PatchTx::new();
    let path = level_path();

    tx.set_path(path.clone(), HostValue::Int(10), None)
        .expect("set path");
    tx.add_path(path.clone(), HostValue::Int(5), HostValue::Int(0), None)
        .expect("add path");

    assert_eq!(tx.patches()[1].expected_base, None);
    assert_eq!(tx.read_overlay(&path), Some(&HostValue::Int(15)));
}

#[test]
fn sub_path_records_patch_and_updates_overlay_from_base() {
    let mut tx = PatchTx::new();
    let path = level_path();

    tx.sub_path(path.clone(), HostValue::Int(2), HostValue::Int(9), None)
        .expect("sub path");

    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].op, PatchOp::Sub(HostValue::Int(2)));
    assert_eq!(tx.read_overlay(&path), Some(&HostValue::Int(7)));
}

#[test]
fn sub_path_uses_previous_overlay_value() {
    let mut tx = PatchTx::new();
    let path = level_path();

    tx.set_path(path.clone(), HostValue::Int(10), None)
        .expect("set path");
    tx.sub_path(path.clone(), HostValue::Int(3), HostValue::Int(0), None)
        .expect("sub path");

    assert_eq!(tx.read_overlay(&path), Some(&HostValue::Int(7)));
}

#[test]
fn numeric_compound_paths_record_patches_and_update_overlay() {
    let mut tx = PatchTx::new();
    let path = level_path();

    tx.mul_path(path.clone(), HostValue::Int(3), HostValue::Int(4), None)
        .expect("mul path");
    tx.div_path(path.clone(), HostValue::Int(2), HostValue::Int(0), None)
        .expect("div path");
    tx.rem_path(path.clone(), HostValue::Int(5), HostValue::Int(0), None)
        .expect("rem path");

    assert_eq!(tx.patches()[0].op, PatchOp::Mul(HostValue::Int(3)));
    assert_eq!(tx.patches()[1].op, PatchOp::Div(HostValue::Int(2)));
    assert_eq!(tx.patches()[2].op, PatchOp::Rem(HostValue::Int(5)));
    assert_eq!(tx.read_overlay(&path), Some(&HostValue::Int(1)));
}

#[test]
fn push_path_records_patch_and_updates_overlay_from_base() {
    let mut tx = PatchTx::new();
    let path = level_path();

    tx.push_path(
        path.clone(),
        HostValue::String("gold".into()),
        HostValue::Array(vec![HostValue::String("xp".into())]),
        None,
    )
    .expect("push path");

    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::Push(HostValue::String("gold".into()))
    );
    assert_eq!(
        tx.read_overlay(&path),
        Some(&HostValue::Array(vec![
            HostValue::String("xp".into()),
            HostValue::String("gold".into())
        ]))
    );
}

#[test]
fn push_path_uses_previous_overlay_value() {
    let mut tx = PatchTx::new();
    let path = level_path();

    tx.set_path(path.clone(), HostValue::Array(Vec::new()), None)
        .expect("set path");
    tx.push_path(
        path.clone(),
        HostValue::Int(3),
        HostValue::Array(vec![HostValue::Int(1)]),
        None,
    )
    .expect("push path");

    assert_eq!(
        tx.read_overlay(&path),
        Some(&HostValue::Array(vec![HostValue::Int(3)]))
    );
}

#[test]
fn remove_path_records_patch_and_tombstone_overlay() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    adapter.insert_value(path.clone(), HostValue::Int(9));
    let mut tx = PatchTx::new();

    tx.remove_path(path.clone(), None).expect("remove path");

    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].op, PatchOp::Remove);
    assert_eq!(tx.read_overlay(&path), None);
    assert_eq!(
        tx.read_path(&adapter, &path),
        Err(HostError::new(HostErrorKind::MissingPath {
            path: path.clone()
        }))
    );
    assert_eq!(adapter.read_path(&path), Ok(HostValue::Int(9)));
}

#[test]
fn transaction_read_error_keeps_source_span() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    let span = test_span();
    adapter.insert_value(path.clone(), HostValue::Int(9));
    let mut tx = PatchTx::new();

    tx.remove_path(path.clone(), None).expect("remove path");
    let error = tx
        .read_path_at(&adapter, &path, Some(span))
        .expect_err("removed overlay should fail");

    assert_eq!(error.source_span, Some(span));
    assert_eq!(error.kind, HostErrorKind::MissingPath { path });
}

#[test]
fn set_path_overwrites_remove_overlay() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    adapter.insert_value(path.clone(), HostValue::Int(9));
    let mut tx = PatchTx::new();

    tx.remove_path(path.clone(), None).expect("remove path");
    tx.set_path(path.clone(), HostValue::Int(10), None)
        .expect("set path");

    assert_eq!(tx.read_path(&adapter, &path), Ok(HostValue::Int(10)));
    assert_eq!(tx.read_overlay(&path), Some(&HostValue::Int(10)));
}

#[test]
fn stale_generation_reports_error() {
    let host_ref = player_ref(3);
    let snapshot = HostObjectSnapshot {
        type_id: host_ref.type_id,
        object_id: host_ref.object_id,
        generation: 4,
    };

    let error = PatchTx::require_fresh_ref(host_ref, &snapshot).expect_err("stale ref");

    assert_eq!(
        error.kind,
        HostErrorKind::StaleGeneration {
            expected: 3,
            actual: 4
        }
    );
}

#[test]
fn transaction_read_prefers_overlay_before_adapter_snapshot() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    adapter.insert_value(path.clone(), HostValue::Int(9));
    let mut tx = PatchTx::new();

    assert_eq!(tx.read_path(&adapter, &path), Ok(HostValue::Int(9)));

    tx.set_path(path.clone(), HostValue::Int(10), None)
        .expect("set path");

    assert_eq!(tx.read_path(&adapter, &path), Ok(HostValue::Int(10)));
    assert_eq!(adapter.read_path(&path), Ok(HostValue::Int(9)));
}

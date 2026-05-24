use vela_common::{FieldId, HostMethodId, HostObjectId, HostTypeId};

use crate::{
    HostErrorKind, HostObjectSnapshot, HostPath, HostRef, HostValue, MockStateAdapter, PatchOp,
    PatchTx, ScriptStateAdapter,
};

fn player_ref(generation: u32) -> HostRef {
    HostRef::new(HostTypeId::new(1), HostObjectId::new(7), generation)
}

fn level_path() -> HostPath {
    HostPath::new(player_ref(3)).field(FieldId::new(2))
}

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
fn add_path_records_patch_and_updates_overlay_from_base() {
    let mut tx = PatchTx::new();
    let path = level_path();

    tx.add_path(path.clone(), HostValue::Int(1), HostValue::Int(9), None)
        .expect("add path");

    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].op, PatchOp::Add(HostValue::Int(1)));
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
        Err(crate::HostError::new(HostErrorKind::MissingPath {
            path: path.clone()
        }))
    );
    assert_eq!(adapter.read_path(&path), Ok(HostValue::Int(9)));
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
        HostValue::Array(Vec::new()),
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

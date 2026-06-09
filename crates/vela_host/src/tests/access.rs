use super::*;

#[test]
fn read_path_reads_current_adapter_state() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    adapter.insert_diagnostic_path_value(path.clone(), HostValue::Int(9));
    let mut tx = HostAccess::new();

    assert_eq!(tx.read_path(&adapter, &path), Ok(HostValue::Int(9)));

    tx.set_path(&mut adapter, path.clone(), HostValue::Int(10), None)
        .expect("set path");

    assert_eq!(tx.read_path(&adapter, &path), Ok(HostValue::Int(10)));
    assert_eq!(adapter.read_diagnostic_path(&path), Ok(HostValue::Int(10)));
}

#[test]
fn compound_write_validates_against_current_adapter_value() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    adapter.insert_diagnostic_path_value(path.clone(), HostValue::Int(9));
    let mut tx = HostAccess::new();

    tx.add_path(&mut adapter, path.clone(), HostValue::Int(1), None)
        .expect("add path");

    assert_eq!(adapter.read_diagnostic_path(&path), Ok(HostValue::Int(10)));
}

#[test]
fn repeated_alias_writes_read_current_host_state() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    adapter.insert_diagnostic_path_value(path.clone(), HostValue::Int(1));
    let mut tx = HostAccess::new();

    tx.add_path(&mut adapter, path.clone(), HostValue::Int(1), None)
        .expect("first alias add");
    tx.add_path(&mut adapter, path.clone(), HostValue::Int(2), None)
        .expect("second alias add");

    assert_eq!(adapter.read_diagnostic_path(&path), Ok(HostValue::Int(4)));
}

#[test]
fn variant_field_paths_write_through() {
    let path = quest_variant_count_path();
    let mut adapter = MockStateAdapter::new();
    adapter.insert_diagnostic_path_value(path.clone(), HostValue::Int(2));
    let mut tx = HostAccess::new();

    tx.add_path(&mut adapter, path.clone(), HostValue::Int(1), None)
        .expect("variant field add");

    assert_eq!(adapter.read_diagnostic_path(&path), Ok(HostValue::Int(3)));
}

#[test]
fn access_read_error_keeps_source_span() {
    let adapter = MockStateAdapter::new();
    let path = level_path();
    let span = test_span();
    let tx = HostAccess::new();

    let error = tx
        .read_path_at(&adapter, &path, Some(span))
        .expect_err("missing path should fail");

    assert_eq!(error.source_span, Some(span));
    assert_eq!(error.kind, HostErrorKind::MissingPath { path });
}

#[test]
fn stale_generation_reports_error() {
    let host_ref = player_ref(3);
    let snapshot = HostObjectSnapshot {
        type_id: host_ref.type_id,
        object_id: host_ref.object_id,
        generation: 4,
    };

    let error = HostAccess::require_fresh_ref(host_ref, &snapshot).expect_err("stale ref");

    assert_eq!(
        error.kind,
        HostErrorKind::StaleGeneration {
            expected: 3,
            actual: 4
        }
    );
}

#[test]
fn write_through_keeps_no_retained_journal() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    adapter.insert_diagnostic_path_value(path.clone(), HostValue::Int(9));
    let mut tx = HostAccess::new();

    tx.set_path(&mut adapter, path, HostValue::Int(10), None)
        .expect("set path");
}

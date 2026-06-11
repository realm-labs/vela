use super::*;

#[test]
fn read_target_reads_current_adapter_state() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    adapter.insert_diagnostic_path_value(
        path.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(9)),
    );
    let plan = target_plan(&path);
    let mut tx = HostAccess::new();

    assert_eq!(
        tx.read(&adapter, target_instance(&path, &plan), None),
        Ok(HostValue::Scalar(vela_common::ScalarValue::I64(9)))
    );

    tx.write(
        &mut adapter,
        target_instance(&path, &plan),
        HostValue::Scalar(vela_common::ScalarValue::I64(10)),
        None,
    )
    .expect("write target");

    assert_eq!(
        tx.read(&adapter, target_instance(&path, &plan), None),
        Ok(HostValue::Scalar(vela_common::ScalarValue::I64(10)))
    );
    assert_eq!(
        adapter.read_diagnostic_path(&path),
        Ok(HostValue::Scalar(vela_common::ScalarValue::I64(10)))
    );
}

#[test]
fn compound_write_validates_against_current_adapter_value() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    adapter.insert_diagnostic_path_value(
        path.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(9)),
    );
    let plan = target_plan(&path);
    let mut tx = HostAccess::new();

    tx.mutate(
        &mut adapter,
        target_instance(&path, &plan),
        HostMutationOp::Add,
        HostValue::Scalar(vela_common::ScalarValue::I64(1)),
        None,
    )
    .expect("add target");

    assert_eq!(
        adapter.read_diagnostic_path(&path),
        Ok(HostValue::Scalar(vela_common::ScalarValue::I64(10)))
    );
}

#[test]
fn host_value_conversions_preserve_exact_scalar_tags() {
    assert_eq!(
        1_i8.into_host_value(),
        Ok(HostValue::Scalar(ScalarValue::I8(1)))
    );
    assert_eq!(
        2_i16.into_host_value(),
        Ok(HostValue::Scalar(ScalarValue::I16(2)))
    );
    assert_eq!(
        3_i32.into_host_value(),
        Ok(HostValue::Scalar(ScalarValue::I32(3)))
    );
    assert_eq!(
        4_i64.into_host_value(),
        Ok(HostValue::Scalar(ScalarValue::I64(4)))
    );
    assert_eq!(
        5_u8.into_host_value(),
        Ok(HostValue::Scalar(ScalarValue::U8(5)))
    );
    assert_eq!(
        6_u16.into_host_value(),
        Ok(HostValue::Scalar(ScalarValue::U16(6)))
    );
    assert_eq!(
        7_u32.into_host_value(),
        Ok(HostValue::Scalar(ScalarValue::U32(7)))
    );
    assert_eq!(
        8_u64.into_host_value(),
        Ok(HostValue::Scalar(ScalarValue::U64(8)))
    );
    assert_eq!(
        1.5_f32.into_host_value(),
        Ok(HostValue::Scalar(ScalarValue::F32(1.5)))
    );
    assert_eq!(
        2.5_f64.into_host_value(),
        Ok(HostValue::Scalar(ScalarValue::F64(2.5)))
    );

    assert_eq!(
        u64::from_host_value(&HostValue::Scalar(ScalarValue::U64(9))),
        Ok(9)
    );
    assert_eq!(
        i64::from_host_value(&HostValue::Scalar(ScalarValue::I32(9)))
            .expect_err("i32 is not an i64 host value")
            .kind,
        HostErrorKind::InvalidArgument { expected: "i64" }
    );
}

#[test]
fn host_value_conversions_round_trip_byte_buffers_as_bytes() {
    assert_eq!(
        vec![0_u8, 1, 255].into_host_value(),
        Ok(HostValue::Bytes(vec![0, 1, 255]))
    );
    assert_eq!(
        (&[2_u8, 3, 4][..]).into_host_value(),
        Ok(HostValue::Bytes(vec![2, 3, 4]))
    );
    assert_eq!(
        Vec::<u8>::from_host_value(&HostValue::Bytes(vec![5, 6, 7])),
        Ok(vec![5, 6, 7])
    );
    assert_eq!(
        Vec::<u8>::from_host_value(&HostValue::Scalar(ScalarValue::U8(1)))
            .expect_err("scalar u8 is not bytes")
            .kind,
        HostErrorKind::InvalidArgument { expected: "bytes" }
    );
}

#[test]
fn byte_vector_host_fields_read_and_write_leaf_bytes() {
    let path = HostPath::new(player_ref(3));
    let plan = target_plan(&path);
    let mut bytes = vec![1_u8, 2, 3];

    assert_eq!(
        ScriptHostFieldAccess::read_host_target_from(&bytes, target_instance(&path, &plan), 0),
        Ok(HostValue::Bytes(vec![1, 2, 3]))
    );

    ScriptHostFieldAccess::write_host_target_from(
        &mut bytes,
        target_instance(&path, &plan),
        0,
        HostValue::Bytes(vec![4, 5]),
    )
    .expect("leaf byte vector write should replace bytes");

    assert_eq!(bytes, vec![4, 5]);

    let indexed_path = path.index(1);
    let indexed_plan = target_plan(&indexed_path);
    assert_eq!(
        ScriptHostFieldAccess::read_host_target_from(
            &bytes,
            target_instance(&indexed_path, &indexed_plan),
            0,
        ),
        Ok(HostValue::Scalar(ScalarValue::U8(5)))
    );
}

#[test]
fn host_access_arithmetic_requires_matching_scalar_tags() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    adapter.insert_diagnostic_path_value(path.clone(), HostValue::Scalar(ScalarValue::U8(9)));
    let plan = target_plan(&path);
    let mut tx = HostAccess::new();

    tx.mutate(
        &mut adapter,
        target_instance(&path, &plan),
        HostMutationOp::Add,
        HostValue::Scalar(ScalarValue::U8(1)),
        None,
    )
    .expect("matching u8 add should mutate");

    assert_eq!(
        adapter.read_diagnostic_path(&path),
        Ok(HostValue::Scalar(ScalarValue::U8(10)))
    );

    let error = tx
        .mutate(
            &mut adapter,
            target_instance(&path, &plan),
            HostMutationOp::Add,
            HostValue::Scalar(ScalarValue::I64(1)),
            None,
        )
        .expect_err("mixed scalar tags should reject");

    assert_eq!(error.kind, HostErrorKind::InvalidAdd { path: path.clone() });
    assert_eq!(
        adapter.read_diagnostic_path(&path),
        Ok(HostValue::Scalar(ScalarValue::U8(10)))
    );
}

#[test]
fn repeated_alias_writes_read_current_host_state() {
    let mut adapter = MockStateAdapter::new();
    let path = level_path();
    adapter.insert_diagnostic_path_value(
        path.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(1)),
    );
    let plan = target_plan(&path);
    let mut tx = HostAccess::new();

    tx.mutate(
        &mut adapter,
        target_instance(&path, &plan),
        HostMutationOp::Add,
        HostValue::Scalar(vela_common::ScalarValue::I64(1)),
        None,
    )
    .expect("first alias add");
    tx.mutate(
        &mut adapter,
        target_instance(&path, &plan),
        HostMutationOp::Add,
        HostValue::Scalar(vela_common::ScalarValue::I64(2)),
        None,
    )
    .expect("second alias add");

    assert_eq!(
        adapter.read_diagnostic_path(&path),
        Ok(HostValue::Scalar(vela_common::ScalarValue::I64(4)))
    );
}

#[test]
fn variant_field_paths_write_through() {
    let path = quest_variant_count_path();
    let mut adapter = MockStateAdapter::new();
    adapter.insert_diagnostic_path_value(
        path.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(2)),
    );
    let plan = target_plan(&path);
    let mut tx = HostAccess::new();

    tx.mutate(
        &mut adapter,
        target_instance(&path, &plan),
        HostMutationOp::Add,
        HostValue::Scalar(vela_common::ScalarValue::I64(1)),
        None,
    )
    .expect("variant field add");

    assert_eq!(
        adapter.read_diagnostic_path(&path),
        Ok(HostValue::Scalar(vela_common::ScalarValue::I64(3)))
    );
}

#[test]
fn access_read_error_keeps_source_span() {
    let adapter = MockStateAdapter::new();
    let path = level_path();
    let plan = target_plan(&path);
    let span = test_span();
    let tx = HostAccess::new();

    let error = tx
        .read(&adapter, target_instance(&path, &plan), Some(span))
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
    adapter.insert_diagnostic_path_value(
        path.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(9)),
    );
    let plan = target_plan(&path);
    let mut tx = HostAccess::new();

    tx.write(
        &mut adapter,
        target_instance(&path, &plan),
        HostValue::Scalar(vela_common::ScalarValue::I64(10)),
        None,
    )
    .expect("write target");
}

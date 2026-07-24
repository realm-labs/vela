use super::*;
use crate::protocol::{HostCollectionKey, HostCollectionMutation};

struct FailingMapValue;

impl ScriptHostFieldAccess for FailingMapValue {
    fn script_host_type_id(&self) -> HostTypeId {
        HostTypeId::new(0)
    }

    fn read_host_target_from(
        &self,
        target: HostTargetInstance<'_>,
        _offset: usize,
    ) -> Result<HostValue, HostError> {
        Err(HostError {
            kind: HostErrorKind::MissingPath {
                path: target.to_diagnostic_path().to_host_path(),
            },
            source_span: None,
        })
    }

    fn write_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        _offset: usize,
        _value: HostValue,
    ) -> Result<(), HostError> {
        Err(HostError {
            kind: HostErrorKind::MissingPath {
                path: target.to_diagnostic_path().to_host_path(),
            },
            source_span: None,
        })
    }
}

impl ScriptHostObject for FailingMapValue {
    fn host_type_id(&self) -> HostTypeId {
        ScriptHostFieldAccess::script_host_type_id(self)
    }

    fn read_resolved_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> Result<HostValue, HostError> {
        ScriptHostFieldAccess::read_resolved_host_target_from(self, access, target)
    }
}

#[test]
fn map_entry_absence_is_distinct_from_value_projection_failure() {
    let root = HostRef::new(HostTypeId::new(0), HostObjectId::new(1), 0);
    let plan = HostTargetPlan::new(root.type_id).dyn_key(0);
    let missing_key = [HostPathArg::Key(HostCollectionKeyRef::String("missing"))];
    let present_key = [HostPathArg::Key(HostCollectionKeyRef::String("present"))];
    let map = BTreeMap::from([("present".to_owned(), FailingMapValue)]);
    let access = ResolvedHostAccess::generic_target(HostSchemaEpoch::new(0));

    let missing = map
        .read_resolved_host(access, HostTargetInstance::new(root, &plan, &missing_key))
        .expect_err("missing map key should use the collection-entry error");
    assert!(matches!(
        missing.kind,
        HostErrorKind::MissingCollectionEntry { .. }
    ));

    let projection = map
        .read_resolved_host(access, HostTargetInstance::new(root, &plan, &present_key))
        .expect_err("present map value projection failure must propagate");
    assert!(matches!(projection.kind, HostErrorKind::MissingPath { .. }));
}

#[test]
fn hash_map_collection_snapshots_are_deterministic_and_exactly_typed() {
    let root = HostRef::new(HostTypeId::new(0), HostObjectId::new(1), 0);
    let plan = HostTargetPlan::new(root.type_id);
    let target = HostTargetInstance::new(root, &plan, &[]);
    let access = ResolvedHostAccess::generic_target(HostSchemaEpoch::new(0));
    let map = HashMap::from([(9_i32, 11_i64), (3_i32, 4_i64)]);

    assert_eq!(
        map.snapshot_collection_resolved_host(access, target, HostCollectionProjection::Entries,),
        Ok(HostCollectionSnapshot::Entries(vec![
            (
                HostValue::Scalar(ScalarValue::I32(3)),
                HostValue::Scalar(ScalarValue::I64(4)),
            ),
            (
                HostValue::Scalar(ScalarValue::I32(9)),
                HostValue::Scalar(ScalarValue::I64(11)),
            ),
        ]))
    );
}

#[test]
fn batch_map_extension_converts_every_entry_before_mutating_host_state() {
    let root = HostRef::new(HostTypeId::new(0), HostObjectId::new(1), 0);
    let plan = HostTargetPlan::new(root.type_id);
    let target = HostTargetInstance::new(root, &plan, &[]);
    let access = ResolvedHostAccess::generic_target(HostSchemaEpoch::new(0));
    let mut map = BTreeMap::from([(1_i32, 2_i64)]);
    let entries = [
        (
            HostCollectionKey::I32(3),
            HostValue::Scalar(ScalarValue::I64(5)),
        ),
        (
            HostCollectionKey::I32(8),
            HostValue::String("not an i64".to_owned()),
        ),
    ];

    let error = map
        .mutate_collection_resolved_host(
            access,
            target,
            HostCollectionMutation::ExtendMap(&entries),
        )
        .expect_err("one invalid value must reject the complete host mutation batch");

    assert_eq!(
        error.kind,
        HostErrorKind::InvalidArgument { expected: "i64" }
    );
    assert_eq!(map, BTreeMap::from([(1_i32, 2_i64)]));
}

#[test]
fn batch_sequence_extension_writes_exact_values_to_standard_vec() {
    let root = HostRef::new(HostTypeId::new(0), HostObjectId::new(1), 0);
    let plan = HostTargetPlan::new(root.type_id);
    let target = HostTargetInstance::new(root, &plan, &[]);
    let access = ResolvedHostAccess::generic_target(HostSchemaEpoch::new(0));
    let mut values = vec![2_i64];
    let extension = [
        HostValue::Scalar(ScalarValue::I64(3)),
        HostValue::Scalar(ScalarValue::I64(5)),
    ];

    values
        .mutate_collection_resolved_host(
            access,
            target,
            HostCollectionMutation::ExtendSequence(&extension),
        )
        .expect("exact sequence values should extend a standard Vec in one batch");

    assert_eq!(values, vec![2, 3, 5]);
}

#[test]
fn indexed_sequence_insertion_validates_before_mutating_standard_vec() {
    let root = HostRef::new(HostTypeId::new(0), HostObjectId::new(1), 0);
    let plan = HostTargetPlan::new(root.type_id);
    let target = HostTargetInstance::new(root, &plan, &[]);
    let access = ResolvedHostAccess::generic_target(HostSchemaEpoch::new(0));
    let mut values = vec![2_i64, 5];
    let three = HostValue::Scalar(ScalarValue::I64(3));

    values
        .mutate_collection_resolved_host(
            access,
            target,
            HostCollectionMutation::InsertSequence {
                index: 1,
                value: &three,
            },
        )
        .expect("an in-range sequence insertion should write through");
    assert_eq!(values, vec![2, 3, 5]);

    let invalid = HostValue::String("not an i64".to_owned());
    let conversion = values
        .mutate_collection_resolved_host(
            access,
            target,
            HostCollectionMutation::InsertSequence {
                index: 2,
                value: &invalid,
            },
        )
        .expect_err("an invalid inserted value should fail before mutation");
    assert_eq!(
        conversion.kind,
        HostErrorKind::InvalidArgument { expected: "i64" }
    );
    assert_eq!(values, vec![2, 3, 5]);

    let bounds = values
        .mutate_collection_resolved_host(
            access,
            target,
            HostCollectionMutation::InsertSequence {
                index: 4,
                value: &three,
            },
        )
        .expect_err("a sparse insertion should fail before mutation");
    assert_eq!(
        bounds.kind,
        HostErrorKind::InvalidArgument {
            expected: "array insertion index"
        }
    );
    assert_eq!(values, vec![2, 3, 5]);
}

#[test]
fn sequence_retain_applies_one_complete_mask_to_an_unchanged_vec() {
    let root = HostRef::new(HostTypeId::new(0), HostObjectId::new(1), 0);
    let plan = HostTargetPlan::new(root.type_id);
    let target = HostTargetInstance::new(root, &plan, &[]);
    let access = ResolvedHostAccess::generic_target(HostSchemaEpoch::new(0));
    let mut values = vec![2_i64, 3, 5, 8];
    let keep = [true, false, true, false];

    values
        .mutate_collection_resolved_host(
            access,
            target,
            HostCollectionMutation::RetainSequence {
                expected_len: 4,
                keep: &keep,
            },
        )
        .expect("one decision per unchanged Vec element should retain in place");

    assert_eq!(values, vec![2, 5]);
}

#[test]
fn sequence_retain_rejects_stale_or_incomplete_masks_without_mutation() {
    let root = HostRef::new(HostTypeId::new(0), HostObjectId::new(1), 0);
    let plan = HostTargetPlan::new(root.type_id);
    let target = HostTargetInstance::new(root, &plan, &[]);
    let access = ResolvedHostAccess::generic_target(HostSchemaEpoch::new(0));
    let mut values = vec![2_i64, 3, 5];

    let stale = [true, false];
    let stale_error = values
        .mutate_collection_resolved_host(
            access,
            target,
            HostCollectionMutation::RetainSequence {
                expected_len: 2,
                keep: &stale,
            },
        )
        .expect_err("a changed Vec length must invalidate retain decisions");
    assert_eq!(
        stale_error.kind,
        HostErrorKind::InvalidArgument {
            expected: "unchanged sequence snapshot"
        }
    );
    assert_eq!(values, vec![2, 3, 5]);

    let incomplete = [true, false];
    let incomplete_error = values
        .mutate_collection_resolved_host(
            access,
            target,
            HostCollectionMutation::RetainSequence {
                expected_len: 3,
                keep: &incomplete,
            },
        )
        .expect_err("retain must decide every captured Vec element");
    assert_eq!(
        incomplete_error.kind,
        HostErrorKind::InvalidArgument {
            expected: "retain decision for every sequence element"
        }
    );
    assert_eq!(values, vec![2, 3, 5]);
}

#[test]
fn map_retain_validates_the_complete_key_snapshot_before_mutation() {
    let root = HostRef::new(HostTypeId::new(0), HostObjectId::new(1), 0);
    let plan = HostTargetPlan::new(root.type_id);
    let target = HostTargetInstance::new(root, &plan, &[]);
    let access = ResolvedHostAccess::generic_target(HostSchemaEpoch::new(0));
    let expected = [
        HostCollectionKey::I32(2),
        HostCollectionKey::I32(3),
        HostCollectionKey::I32(5),
    ];
    let keep = [HostCollectionKey::I32(2), HostCollectionKey::I32(5)];

    let mut ordered = BTreeMap::from([(2_i32, 20_i64), (3, 30), (5, 50)]);
    ordered
        .mutate_collection_resolved_host(
            access,
            target,
            HostCollectionMutation::RetainKeys {
                expected: &expected,
                keep: &keep,
            },
        )
        .expect("an unchanged BTreeMap key snapshot should retain selected entries");
    assert_eq!(ordered, BTreeMap::from([(2_i32, 20_i64), (5, 50)]));

    let stale_expected = [HostCollectionKey::I32(2), HostCollectionKey::I32(3)];
    let mut hashed = HashMap::from([(2_i32, 20_i64), (3, 30), (5, 50)]);
    let error = hashed
        .mutate_collection_resolved_host(
            access,
            target,
            HostCollectionMutation::RetainKeys {
                expected: &stale_expected,
                keep: &keep,
            },
        )
        .expect_err("a changed HashMap key set must invalidate retain decisions");
    assert_eq!(
        error.kind,
        HostErrorKind::InvalidArgument {
            expected: "unchanged keyed collection snapshot"
        }
    );
    assert_eq!(hashed, HashMap::from([(2_i32, 20_i64), (3, 30), (5, 50)]));
}

#[test]
fn set_retain_converts_and_validates_all_keys_before_mutation() {
    use std::collections::{BTreeSet, HashSet};

    let root = HostRef::new(HostTypeId::new(0), HostObjectId::new(1), 0);
    let plan = HostTargetPlan::new(root.type_id);
    let target = HostTargetInstance::new(root, &plan, &[]);
    let access = ResolvedHostAccess::generic_target(HostSchemaEpoch::new(0));
    let expected = [
        HostCollectionKey::I32(2),
        HostCollectionKey::I32(3),
        HostCollectionKey::I32(5),
    ];
    let keep = [HostCollectionKey::I32(3)];

    let mut ordered = BTreeSet::from([2_i32, 3, 5]);
    ordered
        .mutate_collection_resolved_host(
            access,
            target,
            HostCollectionMutation::RetainKeys {
                expected: &expected,
                keep: &keep,
            },
        )
        .expect("an unchanged BTreeSet key snapshot should retain selected values");
    assert_eq!(ordered, BTreeSet::from([3_i32]));

    let invalid_keep = [
        HostCollectionKey::I32(3),
        HostCollectionKey::String("wrong key type".to_owned()),
    ];
    let mut hashed = HashSet::from([2_i32, 3, 5]);
    let error = hashed
        .mutate_collection_resolved_host(
            access,
            target,
            HostCollectionMutation::RetainKeys {
                expected: &expected,
                keep: &invalid_keep,
            },
        )
        .expect_err("every retained HashSet key must convert before mutation");
    assert_eq!(
        error.kind,
        HostErrorKind::InvalidArgument {
            expected: "i32 collection key"
        }
    );
    assert_eq!(hashed, HashSet::from([2_i32, 3, 5]));
}

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
fn mock_adapter_resolves_stable_extern_state_refs() {
    let host_ref = player_ref(3);
    let state = vela_def::StateId::new(101);
    let missing = vela_def::StateId::new(102);
    let mut adapter = MockStateAdapter::new();
    adapter.insert_extern_state_ref(state, host_ref);

    assert_eq!(
        adapter.extern_state_ref(ExternStateBinding {
            id: state,
            name: "main::state",
        }),
        Ok(host_ref)
    );
    assert_eq!(
        adapter
            .extern_state_ref(ExternStateBinding {
                id: missing,
                name: "main::missing",
            })
            .expect_err("missing extern state should fail")
            .kind,
        HostErrorKind::MissingExternState {
            name: "main::missing".to_owned()
        }
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

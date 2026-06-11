use super::*;

#[test]
fn runtime_stages_changed_file_defaulted_schema_addition_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_defaulted_schema_addition");
    let reward_file = write_schema_reward_modules(&root, 2, StructCountField::Absent);
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    write_schema_reward_module(&reward_file, 6, StructCountField::Defaulted);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file defaulted schema addition should be staged");
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file schema addition report");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["game::reward::grant"]);
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(6)))
    );
}

#[test]
fn runtime_stages_changed_file_stable_id_schema_renames_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_stable_id_schema_renames");
    let reward_file = write_stable_schema_rename_modules(&root, 2, false);
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    write_stable_schema_rename_module(&reward_file, 6, true);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file stable-id schema rename should be staged");
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file stable-id schema rename report");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["game::reward::grant"]);
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(6)))
    );
}

#[test]
fn runtime_stages_changed_file_required_schema_field_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_required_schema_field_rejection");
    let reward_file = write_schema_reward_modules(&root, 2, StructCountField::Absent);
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    write_schema_reward_module(&reward_file, 6, StructCountField::Required);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file schema field rejection should be staged");
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file schema field rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.schema.abi_changed");
    assert_eq!(
        report.errors[0].target.as_deref(),
        Some("game::reward::Reward")
    );
    assert_changed_schema_abi_repair_hint(&report);
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn runtime_stages_changed_file_removed_schema_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_removed_schema_rejection");
    let reward_file = write_schema_reward_modules(&root, 2, StructCountField::Required);
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    std::fs::write(
        &reward_file,
        r#"
pub fn grant() {
    return 6;
}
"#,
    )
    .expect("write schema-free reward module");
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file removed schema rejection should be staged");
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file removed schema rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.schema.removed");
    assert_eq!(
        report.errors[0].target.as_deref(),
        Some("game::reward::Reward")
    );
    assert_removed_schema_repair_hint(&report);
    let HotReloadErrorKind::RemovedSchema {
        type_name,
        old_hash,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected removed schema rejection");
    };
    assert_eq!(type_name, "game::reward::Reward");
    assert_ne!(*old_hash, 0);
    assert!(source_span.is_some());
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn runtime_stages_changed_file_schema_field_type_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_schema_field_type_rejection");
    let reward_file = write_schema_reward_modules(&root, 2, StructCountField::Required);
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    write_schema_reward_module(&reward_file, 6, StructCountField::Float);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file schema field type rejection should be staged");
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file schema field type rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.schema.abi_changed");
    assert_eq!(
        report.errors[0].target.as_deref(),
        Some("game::reward::Reward")
    );
    assert_changed_schema_abi_repair_hint(&report);
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn runtime_stages_changed_file_defaulted_enum_variant_field_addition_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_defaulted_enum_variant_field_addition");
    let reward_file = write_enum_reward_modules(&root, 2, EnumVariantCountField::Absent);
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    write_enum_reward_module(&reward_file, 6, EnumVariantCountField::Defaulted);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file defaulted enum variant field addition should be staged");
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file enum variant field addition report");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["game::reward::grant"]);
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(6)))
    );
}

#[test]
fn runtime_stages_changed_file_required_enum_variant_field_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_required_enum_variant_field_rejection");
    let reward_file = write_enum_reward_modules(&root, 2, EnumVariantCountField::Absent);
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    write_enum_reward_module(&reward_file, 6, EnumVariantCountField::Required);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file enum variant field rejection should be staged");
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file enum variant field rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.schema.abi_changed");
    assert_eq!(
        report.errors[0].target.as_deref(),
        Some("game::reward::QuestProgress")
    );
    assert_changed_schema_abi_repair_hint(&report);
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn runtime_stages_changed_file_enum_variant_field_type_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_enum_variant_field_type_rejection");
    let reward_file = write_enum_reward_modules(&root, 2, EnumVariantCountField::Required);
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    write_enum_reward_module(&reward_file, 6, EnumVariantCountField::Float);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file enum variant field type rejection should be staged");
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file enum variant field type rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.schema.abi_changed");
    assert_eq!(
        report.errors[0].target.as_deref(),
        Some("game::reward::QuestProgress")
    );
    assert_changed_schema_abi_repair_hint(&report);
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn runtime_stages_changed_file_removed_trait_impl_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_removed_trait_impl_rejection");
    let reward_file = write_trait_impl_modules(&root, 2, true);
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    write_trait_impl_module(&reward_file, 6, false);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file removed trait impl rejection should be staged");
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file removed trait impl rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.schema.abi_changed");
    assert_eq!(
        report.errors[0].target.as_deref(),
        Some("game::reward::Player")
    );
    assert_changed_schema_abi_repair_hint(&report);
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn runtime_stages_changed_file_added_trait_impl_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_added_trait_impl");
    let reward_file = write_trait_impl_modules(&root, 2, false);
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    write_trait_impl_module(&reward_file, 6, true);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file added trait impl update should be staged");
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file added trait impl report");

    assert!(report.accepted);
    assert!(
        report
            .changed_functions
            .contains(&"game::reward::grant".to_owned())
    );
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(6)))
    );
}

#[test]
fn runtime_stages_changed_file_removed_trait_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_removed_trait_rejection");
    let reward_file = write_trait_abi_modules(&root, 2, "int");
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    write_reward_module(&reward_file, 6);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file removed trait rejection should be staged");
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file removed trait rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.trait.removed_abi");
    assert_eq!(
        report.errors[0].target.as_deref(),
        Some("game::reward::Damageable")
    );
    assert_removed_trait_abi_repair_hint(&report);
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn runtime_stages_changed_file_trait_method_return_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_trait_method_return_rejection");
    let reward_file = write_trait_abi_modules(&root, 2, "int");
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    write_trait_abi_module(&reward_file, 6, "float");
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file trait method return rejection should be staged");
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file trait method return rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.trait.changed_abi");
    assert_eq!(
        report.errors[0].target.as_deref(),
        Some("game::reward::Damageable")
    );
    assert_changed_trait_abi_repair_hint(&report);
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn runtime_stages_changed_file_required_trait_method_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_required_trait_method_rejection");
    let reward_file = write_trait_abi_modules(&root, 2, "int");
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    write_trait_abi_module_with_required_method(&reward_file, 6);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file required trait method rejection should be staged");
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file required trait method rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.trait.changed_abi");
    assert_eq!(
        report.errors[0].target.as_deref(),
        Some("game::reward::Damageable")
    );
    assert_changed_trait_abi_repair_hint(&report);
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn runtime_stages_changed_file_defaulted_trait_method_addition_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_defaulted_trait_method_addition");
    let reward_file = write_trait_abi_modules(&root, 2, "int");
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    write_trait_abi_module_with_defaulted_method(&reward_file, 6);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file defaulted trait method addition should be staged");
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file defaulted trait method addition report");

    assert!(report.accepted);
    assert_eq!(report.errors, Vec::new());
    assert!(
        report
            .changed_functions
            .contains(&"game::reward::grant".to_owned())
    );
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(6)))
    );
}

#[test]
fn runtime_stages_changed_file_compile_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_compile_rejection");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    std::fs::write(
        &reward_file,
        r#"
const BAD = register_event("monster.kill");

pub fn grant() {
    return 6;
}
"#,
    )
    .expect("write side-effecting changed file");
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("compile rejection should be staged as a hot reload report");
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file compile rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.compile");
    assert!(
        report.errors[0]
            .source_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_deref() == Some("hir::top_level_side_effect"))
    );
    assert_top_level_side_effect_repair_label(&report);
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn runtime_returns_hot_reload_changed_file_source_errors_immediately() {
    let root = unique_test_dir("runtime_stage_changed_file_source_error");
    let _reward_file = write_reward_modules(&root, "return grant();", 2);
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let changed = root.join("game").join("reward.txt");
    std::fs::write(&changed, "not a vela source file").expect("write non-source file");

    let error = runtime
        .stage_hot_reload_update_changed_file(&root, &changed)
        .expect("runtime should be hot-reload enabled")
        .expect_err("invalid changed-file path should not stage a hot reload report");

    assert!(matches!(
        error.kind,
        EngineHotReloadSourceErrorKind::Source(crate::source::EngineSourceError {
            kind: EngineSourceErrorKind::InvalidSourcePath { .. }
        })
    ));
    assert!(
        !runtime
            .has_pending_hot_update()
            .expect("source error should not stage an update")
    );
}

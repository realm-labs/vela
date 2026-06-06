fn removed_script_function_rejection_kind(
    test_name: &str,
    workflow: ScriptFunctionReloadWorkflow,
) -> HotReloadErrorKind {
    let root = unique_test_dir(test_name);
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    write_reward_module_with_helper(&reward_file, 2);
    let engine = Engine::builder().execution_profile(ExecutionProfile::trusted()).build().expect("engine should build");
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
        Ok(OwnedValue::Int(2))
    );

    write_reward_module(&reward_file, 6);
    match workflow {
        ScriptFunctionReloadWorkflow::Directory => runtime
            .stage_hot_reload_update_dir(&root)
            .expect("runtime should be hot-reload enabled")
            .expect("dir removed function rejection should be staged"),
        ScriptFunctionReloadWorkflow::ChangedFile => runtime
            .stage_hot_reload_update_changed_file(&root, &reward_file)
            .expect("runtime should be hot-reload enabled")
            .expect("changed-file removed function rejection should be staged"),
    };
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged removed function rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.removed");
    assert_eq!(
        report.errors[0].target.as_deref(),
        Some("game::reward::helper")
    );
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );
    report.errors[0].error.kind.clone()
}

enum NativeDescriptorReloadWorkflow {
    Directory,
    ChangedFile,
}

fn removed_native_descriptor_rejection_kind(
    test_name: &str,
    workflow: NativeDescriptorReloadWorkflow,
) -> HotReloadErrorKind {
    let root = unique_test_dir(test_name);
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let old_engine = Engine::builder().execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
                .effects(EffectSet::host_read()),
            |_| Ok(OwnedValue::Null),
        )
        .build()
        .expect("old engine should build");
    let initial = old_engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let new_engine = Engine::builder().execution_profile(ExecutionProfile::trusted()).build().expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
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
        Ok(OwnedValue::Int(2))
    );

    write_reward_module(&reward_file, 6);
    match workflow {
        NativeDescriptorReloadWorkflow::Directory => runtime
            .stage_hot_reload_update_dir(&root)
            .expect("runtime should be hot-reload enabled")
            .expect("dir removed native descriptor ABI rejection should be staged"),
        NativeDescriptorReloadWorkflow::ChangedFile => runtime
            .stage_hot_reload_update_changed_file(&root, &reward_file)
            .expect("runtime should be hot-reload enabled")
            .expect("changed-file removed native descriptor ABI rejection should be staged"),
    };
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged removed native descriptor ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.removed_abi");
    assert_eq!(
        report.errors[0].target.as_deref(),
        Some("game::native::grant_bonus")
    );
    assert_removed_function_abi_repair_hint(&report);
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );
    report.errors[0].error.kind.clone()
}

fn native_stable_id_churn_rejection_kind(
    test_name: &str,
    workflow: NativeDescriptorReloadWorkflow,
) -> HotReloadErrorKind {
    let root = unique_test_dir(test_name);
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let old_engine = Engine::builder().execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
                .effects(EffectSet::host_read()),
            |_| Ok(OwnedValue::Null),
        )
        .build()
        .expect("old engine should build");
    let initial = old_engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let new_engine = Engine::builder().execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(23))
                .effects(EffectSet::host_read()),
            |_| Ok(OwnedValue::Null),
        )
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
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
        Ok(OwnedValue::Int(2))
    );

    write_reward_module(&reward_file, 6);
    match workflow {
        NativeDescriptorReloadWorkflow::Directory => runtime
            .stage_hot_reload_update_dir(&root)
            .expect("runtime should be hot-reload enabled")
            .expect("dir native stable-ID churn ABI rejection should be staged"),
        NativeDescriptorReloadWorkflow::ChangedFile => runtime
            .stage_hot_reload_update_changed_file(&root, &reward_file)
            .expect("runtime should be hot-reload enabled")
            .expect("changed-file native stable-ID churn ABI rejection should be staged"),
    };
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged native stable-ID churn ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.removed_abi");
    assert_eq!(
        report.errors[0].target.as_deref(),
        Some("game::native::grant_bonus")
    );
    assert_removed_function_abi_repair_hint(&report);
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );
    report.errors[0].error.kind.clone()
}

fn dir_native_rejection_kind(
    test_name: &str,
    old_desc: NativeFunctionDesc,
    new_desc: NativeFunctionDesc,
    expected_code: &str,
) -> HotReloadErrorKind {
    let root = unique_test_dir(test_name);
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let old_engine = Engine::builder().execution_profile(ExecutionProfile::trusted())
        .register_native_fn(old_desc, |_| Ok(OwnedValue::Null))
        .build()
        .expect("old engine should build");
    let initial = old_engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let new_engine = Engine::builder().execution_profile(ExecutionProfile::trusted())
        .register_native_fn(new_desc, |_| Ok(OwnedValue::Null))
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
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
        Ok(OwnedValue::Int(2))
    );

    write_reward_module(&reward_file, 6);
    runtime
        .stage_hot_reload_update_dir(&root)
        .expect("runtime should be hot-reload enabled")
        .expect("dir native descriptor ABI rejection should be staged");
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged dir native descriptor ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, expected_code);
    if expected_code == "reload.function.effects_changed" {
        assert_effect_abi_repair_hint(&report);
    }
    if expected_code == "reload.function.access_changed" {
        assert_access_abi_repair_hint(&report);
    }
    if expected_code == "reload.function.parameter_abi_changed" {
        assert_parameter_abi_repair_hint(&report);
    }
    if expected_code == "reload.function.return_abi_changed" {
        assert_function_return_repair_hint(&report);
    }
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );
    report.errors[0].error.kind.clone()
}

enum MethodDescriptorReloadWorkflow {
    Directory,
    ChangedFile,
}

fn removed_method_descriptor_rejection_kind(
    test_name: &str,
    workflow: MethodDescriptorReloadWorkflow,
) -> HotReloadErrorKind {
    let root = unique_test_dir(test_name);
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let old_engine = Engine::builder().execution_profile(ExecutionProfile::trusted())
        .register_type(type_with_reload_method(MethodDesc::new(
            HostMethodId::new(9),
            "grant_exp",
        )))
        .build()
        .expect("old engine should build");
    let initial = old_engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let new_engine = Engine::builder().execution_profile(ExecutionProfile::trusted())
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player")).host_type(HostTypeId::new(1)),
        )
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
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
        Ok(OwnedValue::Int(2))
    );

    write_reward_module(&reward_file, 6);
    match workflow {
        MethodDescriptorReloadWorkflow::Directory => runtime
            .stage_hot_reload_update_dir(&root)
            .expect("runtime should be hot-reload enabled")
            .expect("dir removed method descriptor ABI rejection should be staged"),
        MethodDescriptorReloadWorkflow::ChangedFile => runtime
            .stage_hot_reload_update_changed_file(&root, &reward_file)
            .expect("runtime should be hot-reload enabled")
            .expect("changed-file removed method descriptor ABI rejection should be staged"),
    };
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged removed method descriptor ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.method.removed_abi");
    assert_eq!(report.errors[0].target.as_deref(), Some("Player.grant_exp"));
    assert_removed_method_abi_repair_hint(&report);
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );
    report.errors[0].error.kind.clone()
}

fn method_stable_id_churn_rejection_kind(
    test_name: &str,
    workflow: MethodDescriptorReloadWorkflow,
) -> HotReloadErrorKind {
    let root = unique_test_dir(test_name);
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let old_engine = Engine::builder().execution_profile(ExecutionProfile::trusted())
        .register_type(type_with_reload_method(MethodDesc::new(
            HostMethodId::new(9),
            "grant_exp",
        )))
        .build()
        .expect("old engine should build");
    let initial = old_engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let new_engine = Engine::builder().execution_profile(ExecutionProfile::trusted())
        .register_type(type_with_reload_method(MethodDesc::new(
            HostMethodId::new(10),
            "grant_exp",
        )))
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
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
        Ok(OwnedValue::Int(2))
    );

    write_reward_module(&reward_file, 6);
    match workflow {
        MethodDescriptorReloadWorkflow::Directory => runtime
            .stage_hot_reload_update_dir(&root)
            .expect("runtime should be hot-reload enabled")
            .expect("dir method stable-ID churn ABI rejection should be staged"),
        MethodDescriptorReloadWorkflow::ChangedFile => runtime
            .stage_hot_reload_update_changed_file(&root, &reward_file)
            .expect("runtime should be hot-reload enabled")
            .expect("changed-file method stable-ID churn ABI rejection should be staged"),
    };
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged method stable-ID churn ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.method.removed_abi");
    assert_eq!(report.errors[0].target.as_deref(), Some("Player.grant_exp"));
    assert_removed_method_abi_repair_hint(&report);
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );
    report.errors[0].error.kind.clone()
}

fn dir_method_rejection_kind(
    test_name: &str,
    old_method: MethodDesc,
    new_method: MethodDesc,
    expected_code: &str,
) -> HotReloadErrorKind {
    let root = unique_test_dir(test_name);
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let old_engine = Engine::builder().execution_profile(ExecutionProfile::trusted())
        .register_type(type_with_reload_method(old_method))
        .build()
        .expect("old engine should build");
    let initial = old_engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let new_engine = Engine::builder().execution_profile(ExecutionProfile::trusted())
        .register_type(type_with_reload_method(new_method))
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
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
        Ok(OwnedValue::Int(2))
    );

    write_reward_module(&reward_file, 6);
    runtime
        .stage_hot_reload_update_dir(&root)
        .expect("runtime should be hot-reload enabled")
        .expect("dir method ABI rejection should be staged");
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged dir method ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, expected_code);
    if expected_code == "reload.method.effects_changed" {
        assert_effect_abi_repair_hint(&report);
    }
    if expected_code == "reload.method.access_changed" {
        assert_access_abi_repair_hint(&report);
    }
    if expected_code == "reload.method.parameter_abi_changed" {
        assert_method_parameter_abi_repair_hint(&report);
    }
    if expected_code == "reload.method.return_abi_changed" {
        assert_method_return_repair_hint(&report);
    }
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );
    report.errors[0].error.kind.clone()
}

fn changed_file_method_rejection_kind(
    test_name: &str,
    old_method: MethodDesc,
    new_method: MethodDesc,
    expected_code: &str,
) -> HotReloadErrorKind {
    let root = unique_test_dir(test_name);
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let old_engine = Engine::builder().execution_profile(ExecutionProfile::trusted())
        .register_type(type_with_reload_method(old_method))
        .build()
        .expect("old engine should build");
    let initial = old_engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let new_engine = Engine::builder().execution_profile(ExecutionProfile::trusted())
        .register_type(type_with_reload_method(new_method))
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
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
        Ok(OwnedValue::Int(2))
    );

    write_reward_module(&reward_file, 6);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file method ABI rejection should be staged");
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file method ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, expected_code);
    if expected_code == "reload.method.effects_changed" {
        assert_effect_abi_repair_hint(&report);
    }
    if expected_code == "reload.method.access_changed" {
        assert_access_abi_repair_hint(&report);
    }
    if expected_code == "reload.method.parameter_abi_changed" {
        assert_method_parameter_abi_repair_hint(&report);
    }
    if expected_code == "reload.method.return_abi_changed" {
        assert_method_return_repair_hint(&report);
    }
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );
    report.errors[0].error.kind.clone()
}


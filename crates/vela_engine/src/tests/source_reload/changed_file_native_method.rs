use super::*;

#[test]
fn runtime_stages_changed_file_native_effect_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_native_effect");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let old_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
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
    let new_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
                .effects(EffectSet::host_write()),
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
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file native effect ABI rejection should be staged");
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
        .expect("staged changed-file native effect ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.effects_changed");
    assert_effect_abi_repair_hint(&report);
    let HotReloadErrorKind::ChangedFunctionEffects {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed native function effects");
    };
    assert_eq!(function, "game::native::grant_bonus");
    assert!(old.reads_host);
    assert!(!old.writes_host);
    assert!(new.reads_host);
    assert!(new.writes_host);
    assert!(source_span.is_none());
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
}

#[test]
fn runtime_stages_changed_file_native_access_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_native_access");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let old_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
                .access(FunctionAccess::public().reflect_callable(true)),
            |_| Ok(OwnedValue::Null),
        )
        .build()
        .expect("old engine should build");
    let initial = old_engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let new_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
                .access(FunctionAccess::public().reflect_callable(false)),
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
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file native access ABI rejection should be staged");
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
        .expect("staged changed-file native access ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.access_changed");
    assert_access_abi_repair_hint(&report);
    let HotReloadErrorKind::ChangedFunctionAccess {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed native function access");
    };
    assert_eq!(function, "game::native::grant_bonus");
    assert!(old.callable);
    assert!(!new.callable);
    assert!(source_span.is_none());
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
}

#[test]
fn runtime_stages_changed_file_native_parameter_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_native_parameter");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let old_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
                .param("amount", TypeHint::Int),
            |_| Ok(OwnedValue::Null),
        )
        .build()
        .expect("old engine should build");
    let initial = old_engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let new_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
                .param("amount", TypeHint::Float),
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
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file native parameter ABI rejection should be staged");
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
        .expect("staged changed-file native parameter ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(
        report.errors[0].code,
        "reload.function.parameter_abi_changed"
    );
    assert_parameter_abi_repair_hint(&report);
    let HotReloadErrorKind::ChangedFunctionParameterAbi {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed native function parameter ABI");
    };
    assert_eq!(function, "game::native::grant_bonus");
    assert_eq!(old.len(), 1);
    assert_eq!(old[0].name, "amount");
    assert_eq!(old[0].type_hint.as_deref(), Some("int"));
    assert_eq!(new.len(), 1);
    assert_eq!(new[0].name, "amount");
    assert_eq!(new[0].type_hint.as_deref(), Some("float"));
    assert!(source_span.is_none());
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
}

#[test]
fn runtime_stages_changed_file_native_path_proxy_parameter_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_native_path_proxy_parameter");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let old_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::native::inspect_path", NativeFunctionId::new(23))
                .param("path", TypeHint::PathProxy),
            |_| Ok(OwnedValue::Null),
        )
        .build()
        .expect("old engine should build");
    let initial = old_engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let new_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::native::inspect_path", NativeFunctionId::new(23))
                .param("path", TypeHint::Int),
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
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file native path proxy ABI rejection should be staged");
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
        .expect("staged changed-file native path proxy ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(
        report.errors[0].code,
        "reload.function.parameter_abi_changed"
    );
    assert_parameter_abi_repair_hint(&report);
    let HotReloadErrorKind::ChangedFunctionParameterAbi {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed native function parameter ABI");
    };
    assert_eq!(function, "game::native::inspect_path");
    assert_eq!(old.len(), 1);
    assert_eq!(old[0].name, "path");
    assert_eq!(old[0].type_hint.as_deref(), Some("path_proxy"));
    assert_eq!(new.len(), 1);
    assert_eq!(new[0].name, "path");
    assert_eq!(new[0].type_hint.as_deref(), Some("int"));
    assert!(source_span.is_none());
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
}

#[test]
fn runtime_stages_changed_file_native_return_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_native_return");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let old_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
                .returns(TypeHint::Int),
            |_| Ok(OwnedValue::Null),
        )
        .build()
        .expect("old engine should build");
    let initial = old_engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let new_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
                .returns(TypeHint::Float),
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
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file native return ABI rejection should be staged");
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
        .expect("staged changed-file native return ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.return_abi_changed");
    assert_function_return_repair_hint(&report);
    let HotReloadErrorKind::ChangedFunctionReturnAbi {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed native function return ABI");
    };
    assert_eq!(function, "game::native::grant_bonus");
    assert_eq!(old.as_deref(), Some("int"));
    assert_eq!(new.as_deref(), Some("float"));
    assert!(source_span.is_none());
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
}

#[test]
fn runtime_stages_changed_file_native_path_proxy_return_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_native_path_proxy_return");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let old_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::native::inspect_path", NativeFunctionId::new(23))
                .returns(TypeHint::PathProxy),
            |_| Ok(OwnedValue::Null),
        )
        .build()
        .expect("old engine should build");
    let initial = old_engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let new_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::native::inspect_path", NativeFunctionId::new(23))
                .returns(TypeHint::Int),
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
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file native path proxy return ABI rejection should be staged");
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
        .expect("staged changed-file native path proxy return ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.return_abi_changed");
    assert_function_return_repair_hint(&report);
    let HotReloadErrorKind::ChangedFunctionReturnAbi {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed native function return ABI");
    };
    assert_eq!(function, "game::native::inspect_path");
    assert_eq!(old.as_deref(), Some("path_proxy"));
    assert_eq!(new.as_deref(), Some("int"));
    assert!(source_span.is_none());
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
}

#[test]
fn runtime_stages_changed_file_removed_native_function_rejection_until_safe_point() {
    let kind = removed_native_descriptor_rejection_kind(
        "runtime_stage_changed_file_removed_native_function",
        NativeDescriptorReloadWorkflow::ChangedFile,
    );

    let HotReloadErrorKind::RemovedFunctionAbi {
        function,
        source_span,
    } = kind
    else {
        panic!("expected removed native function ABI");
    };
    assert_eq!(function, "game::native::grant_bonus");
    assert!(source_span.is_none());
}

#[test]
fn runtime_stages_changed_file_native_stable_id_churn_rejection_until_safe_point() {
    let kind = native_stable_id_churn_rejection_kind(
        "runtime_stage_changed_file_native_stable_id_churn",
        NativeDescriptorReloadWorkflow::ChangedFile,
    );

    let HotReloadErrorKind::RemovedFunctionAbi {
        function,
        source_span,
    } = kind
    else {
        panic!("expected removed native function ABI");
    };
    assert_eq!(function, "game::native::grant_bonus");
    assert!(source_span.is_none());
}

#[test]
fn runtime_stages_changed_file_native_stable_id_rename_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_native_stable_id_rename");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    write_native_reward_module(&reward_file, "grant_bonus", "");
    let old_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
                .returns(TypeHint::Int)
                .effects(EffectSet::host_read()),
            |_| Ok(OwnedValue::Int(5)),
        )
        .build()
        .expect("old engine should build");
    let initial = old_engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let new_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::native::grant_bonus_v2", NativeFunctionId::new(22))
                .returns(TypeHint::Int)
                .effects(EffectSet::host_read()),
            |_| Ok(OwnedValue::Int(5)),
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
        Ok(OwnedValue::Int(5))
    );

    write_native_reward_module(&reward_file, "grant_bonus_v2", " + 1");
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file native stable-ID rename should be staged");

    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(5))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file native stable-ID rename report");

    assert!(report.accepted);
    assert!(report.errors.is_empty());
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(6))
    );
}

#[test]
fn runtime_stages_changed_file_method_effect_rejection_until_safe_point() {
    let kind = changed_file_method_rejection_kind(
        "runtime_stage_changed_file_method_effect",
        MethodDesc::new(HostMethodId::new(9), "grant_exp").effects(MethodEffectSet::host_read()),
        MethodDesc::new(HostMethodId::new(9), "grant_exp").effects(MethodEffectSet::host_write()),
        "reload.method.effects_changed",
    );

    let HotReloadErrorKind::ChangedMethodEffects {
        type_name,
        method,
        old,
        new,
        source_span,
    } = kind
    else {
        panic!("expected changed host method effects");
    };
    assert_eq!(type_name, "Player");
    assert_eq!(method, "grant_exp");
    assert!(old.reads_host);
    assert!(!old.writes_host);
    assert!(new.reads_host);
    assert!(new.writes_host);
    assert!(source_span.is_none());
}

#[test]
fn runtime_stages_changed_file_method_access_rejection_until_safe_point() {
    let kind = changed_file_method_rejection_kind(
        "runtime_stage_changed_file_method_access",
        MethodDesc::new(HostMethodId::new(9), "grant_exp")
            .access(MethodAccess::new().reflect_callable(true)),
        MethodDesc::new(HostMethodId::new(9), "grant_exp")
            .access(MethodAccess::new().reflect_callable(false)),
        "reload.method.access_changed",
    );

    let HotReloadErrorKind::ChangedMethodAccess {
        type_name,
        method,
        old,
        new,
        source_span,
    } = kind
    else {
        panic!("expected changed host method access");
    };
    assert_eq!(type_name, "Player");
    assert_eq!(method, "grant_exp");
    assert!(old.callable);
    assert!(!new.callable);
    assert!(source_span.is_none());
}

#[test]
fn runtime_stages_changed_file_method_parameter_rejection_until_safe_point() {
    let kind = changed_file_method_rejection_kind(
        "runtime_stage_changed_file_method_parameter",
        MethodDesc::new(HostMethodId::new(9), "grant_exp")
            .param(MethodParamDesc::new("amount").type_hint("int")),
        MethodDesc::new(HostMethodId::new(9), "grant_exp")
            .param(MethodParamDesc::new("amount").type_hint("float")),
        "reload.method.parameter_abi_changed",
    );

    let HotReloadErrorKind::ChangedMethodParameterAbi {
        type_name,
        method,
        old,
        new,
        source_span,
    } = kind
    else {
        panic!("expected changed host method parameter ABI");
    };
    assert_eq!(type_name, "Player");
    assert_eq!(method, "grant_exp");
    assert_eq!(old.len(), 1);
    assert_eq!(old[0].name, "amount");
    assert_eq!(old[0].type_hint.as_deref(), Some("int"));
    assert_eq!(new.len(), 1);
    assert_eq!(new[0].name, "amount");
    assert_eq!(new[0].type_hint.as_deref(), Some("float"));
    assert!(source_span.is_none());
}

#[test]
fn runtime_stages_changed_file_method_return_rejection_until_safe_point() {
    let kind = changed_file_method_rejection_kind(
        "runtime_stage_changed_file_method_return",
        MethodDesc::new(HostMethodId::new(9), "grant_exp").return_type("int"),
        MethodDesc::new(HostMethodId::new(9), "grant_exp").return_type("null"),
        "reload.method.return_abi_changed",
    );

    let HotReloadErrorKind::ChangedMethodReturnAbi {
        type_name,
        method,
        old,
        new,
        source_span,
    } = kind
    else {
        panic!("expected changed host method return ABI");
    };
    assert_eq!(type_name, "Player");
    assert_eq!(method, "grant_exp");
    assert_eq!(old.as_deref(), Some("int"));
    assert_eq!(new.as_deref(), Some("null"));
    assert!(source_span.is_none());
}

#[test]
fn runtime_stages_changed_file_removed_method_rejection_until_safe_point() {
    let kind = removed_method_descriptor_rejection_kind(
        "runtime_stage_changed_file_removed_method",
        MethodDescriptorReloadWorkflow::ChangedFile,
    );

    let HotReloadErrorKind::RemovedMethodAbi {
        type_name,
        method,
        source_span,
    } = kind
    else {
        panic!("expected removed host method ABI");
    };
    assert_eq!(type_name, "Player");
    assert_eq!(method, "grant_exp");
    assert!(source_span.is_none());
}

#[test]
fn runtime_stages_changed_file_method_stable_id_churn_rejection_until_safe_point() {
    let kind = method_stable_id_churn_rejection_kind(
        "runtime_stage_changed_file_method_stable_id_churn",
        MethodDescriptorReloadWorkflow::ChangedFile,
    );

    let HotReloadErrorKind::RemovedMethodAbi {
        type_name,
        method,
        source_span,
    } = kind
    else {
        panic!("expected removed host method ABI");
    };
    assert_eq!(type_name, "Player");
    assert_eq!(method, "grant_exp");
    assert!(source_span.is_none());
}

#[test]
fn runtime_stages_changed_file_method_stable_id_rename_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_method_stable_id_rename");
    let method = HostMethodId::new(9);
    let reward_file = write_host_method_reward_modules(&root, "grant_exp", 1);
    let old_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_type(type_with_reload_method(MethodDesc::new(
            method,
            "grant_exp",
        )))
        .build()
        .expect("old engine should build");
    let initial = old_engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let new_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_type(type_with_reload_method(MethodDesc::new(
            method,
            "award_exp",
        )))
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();

    let mut tx = HostAccess::new();
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[OwnedValue::HostRef(host_ref)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(1))
    );
    assert_host_method_access(&tx, method, 7);

    write_host_method_reward_module(&reward_file, "award_exp", 2);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file method stable-ID rename should be staged");

    let mut tx = HostAccess::new();
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[OwnedValue::HostRef(host_ref)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(1))
    );
    assert_host_method_access(&tx, method, 7);

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file method stable-ID rename report");

    assert!(report.accepted);
    assert!(report.errors.is_empty());
    let mut tx = HostAccess::new();
    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[OwnedValue::HostRef(host_ref)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );
    assert_host_method_access(&tx, method, 7);
}

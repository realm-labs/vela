use super::*;

#[test]
fn runtime_stages_source_file_native_effect_rejection_until_safe_point() {
    let old_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::reward::grant", NativeFunctionId::new(22))
                .effects(EffectSet::host_read()),
            |_| Ok(OwnedValue::Null),
        )
        .build()
        .expect("old engine should build");
    let initial = hot_reload_initial_from_source(&old_engine, "fn main() { return 1; }");
    let new_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::reward::grant", NativeFunctionId::new(22))
                .effects(EffectSet::host_write()),
            |_| Ok(OwnedValue::Null),
        )
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(&mut runtime, "fn main() { return 2; }");
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged native effect ABI rejection report");

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
    assert_eq!(function, "game::reward::grant");
    assert!(old.reads_host);
    assert!(!old.writes_host);
    assert!(new.reads_host);
    assert!(new.writes_host);
    assert!(source_span.is_none());
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn runtime_stages_source_file_native_access_rejection_until_safe_point() {
    let old_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::reward::grant", NativeFunctionId::new(22))
                .access(FunctionAccess::public().reflect_callable(true)),
            |_| Ok(OwnedValue::Null),
        )
        .build()
        .expect("old engine should build");
    let initial = hot_reload_initial_from_source(&old_engine, "fn main() { return 1; }");
    let new_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::reward::grant", NativeFunctionId::new(22))
                .access(FunctionAccess::public().reflect_callable(false)),
            |_| Ok(OwnedValue::Null),
        )
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(&mut runtime, "fn main() { return 2; }");
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged native access ABI rejection report");

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
    assert_eq!(function, "game::reward::grant");
    assert!(old.callable);
    assert!(!new.callable);
    assert!(source_span.is_none());
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn runtime_stages_source_file_native_parameter_rejection_until_safe_point() {
    let old_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::reward::grant", NativeFunctionId::new(22))
                .param("amount", TypeHint::i64()),
            |_| Ok(OwnedValue::Null),
        )
        .build()
        .expect("old engine should build");
    let initial = hot_reload_initial_from_source(&old_engine, "fn main() { return 1; }");
    let new_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::reward::grant", NativeFunctionId::new(22))
                .param("amount", TypeHint::f64()),
            |_| Ok(OwnedValue::Null),
        )
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(&mut runtime, "fn main() { return 2; }");
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged native parameter ABI rejection report");

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
    assert_eq!(function, "game::reward::grant");
    assert_eq!(old.len(), 1);
    assert_eq!(old[0].name, "amount");
    assert_eq!(old[0].type_hint.as_deref(), Some("i64"));
    assert_eq!(new.len(), 1);
    assert_eq!(new[0].name, "amount");
    assert_eq!(new[0].type_hint.as_deref(), Some("f64"));
    assert!(source_span.is_none());
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn runtime_stages_source_file_native_return_rejection_until_safe_point() {
    let old_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::reward::grant", NativeFunctionId::new(22))
                .returns(TypeHint::i64()),
            |_| Ok(OwnedValue::Null),
        )
        .build()
        .expect("old engine should build");
    let initial = hot_reload_initial_from_source(&old_engine, "fn main() { return 1; }");
    let new_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::reward::grant", NativeFunctionId::new(22))
                .returns(TypeHint::f64()),
            |_| Ok(OwnedValue::Null),
        )
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(&mut runtime, "fn main() { return 2; }");
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged native return ABI rejection report");

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
    assert_eq!(function, "game::reward::grant");
    assert_eq!(old.as_deref(), Some("i64"));
    assert_eq!(new.as_deref(), Some("f64"));
    assert!(source_span.is_none());
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn runtime_stages_source_file_removed_native_function_rejection_until_safe_point() {
    let old_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::reward::grant", NativeFunctionId::new(22))
                .effects(EffectSet::host_read()),
            |_| Ok(OwnedValue::Null),
        )
        .build()
        .expect("old engine should build");
    let initial = hot_reload_initial_from_source(&old_engine, "fn main() { return 1; }");
    let new_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(&mut runtime, "fn main() { return 2; }");
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged removed native function ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.removed_abi");
    assert_removed_function_abi_repair_hint(&report);
    assert_eq!(
        report.errors[0].target.as_deref(),
        Some("game::reward::grant")
    );
    let HotReloadErrorKind::RemovedFunctionAbi {
        function,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected removed native function ABI");
    };
    assert_eq!(function, "game::reward::grant");
    assert!(source_span.is_none());
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn runtime_stages_source_file_native_stable_id_churn_rejection_until_safe_point() {
    let old_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::reward::grant", NativeFunctionId::new(22))
                .effects(EffectSet::host_read()),
            |_| Ok(OwnedValue::Null),
        )
        .build()
        .expect("old engine should build");
    let initial = hot_reload_initial_from_source(&old_engine, "fn main() { return 1; }");
    let new_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::reward::grant", NativeFunctionId::new(23))
                .effects(EffectSet::host_read()),
            |_| Ok(OwnedValue::Null),
        )
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(&mut runtime, "fn main() { return 2; }");
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged native stable-ID churn ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.removed_abi");
    assert_removed_function_abi_repair_hint(&report);
    assert_eq!(
        report.errors[0].target.as_deref(),
        Some("game::reward::grant")
    );
    let HotReloadErrorKind::RemovedFunctionAbi {
        function,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected removed native function ABI");
    };
    assert_eq!(function, "game::reward::grant");
    assert!(source_span.is_none());
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn runtime_stages_source_file_native_stable_id_rename_until_safe_point() {
    let old_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
                .returns(TypeHint::i64())
                .effects(EffectSet::host_read()),
            |_| Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(5))),
        )
        .build()
        .expect("old engine should build");
    let initial = hot_reload_initial_from_source(
        &old_engine,
        r#"
fn main() {
    return game::native::grant_bonus();
}
"#,
    );
    let new_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_native_fn(
            NativeFunctionDesc::new("game::native::grant_bonus_v2", NativeFunctionId::new(22))
                .returns(TypeHint::i64())
                .effects(EffectSet::host_read()),
            |_| Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(5))),
        )
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(5)))
    );

    stage_source_update(
        &mut runtime,
        r#"
fn main() {
    return game::native::grant_bonus_v2() + 1;
}
"#,
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(5)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged source-file native stable-ID rename report");

    assert!(report.accepted);
    assert!(report.errors.is_empty());
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(6)))
    );
}

#[test]
fn runtime_stages_source_file_removed_method_rejection_until_safe_point() {
    let player_key = TypeKey::new(TypeId::new(1), "Player");
    let old_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_type(
            TypeDesc::new(player_key.clone())
                .host_type(HostTypeId::new(1))
                .method(MethodDesc::new(HostMethodId::new(9), "grant_exp")),
        )
        .build()
        .expect("old engine should build");
    let initial = hot_reload_initial_from_source(&old_engine, "fn main() { return 1; }");
    let new_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_type(TypeDesc::new(player_key).host_type(HostTypeId::new(1)))
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(&mut runtime, "fn main() { return 2; }");
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged removed host method ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.method.removed_abi");
    assert_removed_method_abi_repair_hint(&report);
    assert_eq!(report.errors[0].target.as_deref(), Some("Player.grant_exp"));
    let HotReloadErrorKind::RemovedMethodAbi {
        type_name,
        method,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected removed host method ABI");
    };
    assert_eq!(type_name, "Player");
    assert_eq!(method, "grant_exp");
    assert!(source_span.is_none());
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn runtime_stages_source_file_method_stable_id_churn_rejection_until_safe_point() {
    let old_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_type(type_with_reload_method(MethodDesc::new(
            HostMethodId::new(9),
            "grant_exp",
        )))
        .build()
        .expect("old engine should build");
    let initial = hot_reload_initial_from_source(&old_engine, "fn main() { return 1; }");
    let new_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_type(type_with_reload_method(MethodDesc::new(
            HostMethodId::new(10),
            "grant_exp",
        )))
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(&mut runtime, "fn main() { return 2; }");
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged method stable-ID churn ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.method.removed_abi");
    assert_removed_method_abi_repair_hint(&report);
    assert_eq!(report.errors[0].target.as_deref(), Some("Player.grant_exp"));
    let HotReloadErrorKind::RemovedMethodAbi {
        type_name,
        method,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected removed host method ABI");
    };
    assert_eq!(type_name, "Player");
    assert_eq!(method, "grant_exp");
    assert!(source_span.is_none());
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn runtime_stages_source_file_method_stable_id_rename_until_safe_point() {
    let method = HostMethodId::new(9);
    let old_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_type(type_with_reload_method(MethodDesc::new(
            method,
            "grant_exp",
        )))
        .build()
        .expect("old engine should build");
    let initial = hot_reload_initial_from_source(
        &old_engine,
        r#"
fn main(player: Player) {
    player.grant_exp(7);
    return 1;
}
"#,
    );
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
            "main",
            &[OwnedValue::HostRef(host_ref)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
    assert_host_method_access(&tx, method, 7);

    stage_source_update(
        &mut runtime,
        r#"
fn main(player: Player) {
    player.award_exp(7);
    return 2;
}
"#,
    );
    let mut tx = HostAccess::new();
    assert_eq!(
        runtime.call_raw(
            "main",
            &[OwnedValue::HostRef(host_ref)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
    assert_host_method_access(&tx, method, 7);

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged source-file method stable-ID rename report");

    assert!(report.accepted);
    assert!(report.errors.is_empty());
    let mut tx = HostAccess::new();
    assert_eq!(
        runtime.call_raw(
            "main",
            &[OwnedValue::HostRef(host_ref)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
    assert_host_method_access(&tx, method, 7);
}

#[test]
fn runtime_stages_source_file_method_effect_rejection_until_safe_point() {
    let player_key = TypeKey::new(TypeId::new(1), "Player");
    let old_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_type(
            TypeDesc::new(player_key.clone())
                .host_type(HostTypeId::new(1))
                .method(
                    MethodDesc::new(HostMethodId::new(9), "grant_exp")
                        .effects(MethodEffectSet::host_read()),
                ),
        )
        .build()
        .expect("old engine should build");
    let initial = hot_reload_initial_from_source(&old_engine, "fn main() { return 1; }");
    let new_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_type(
            TypeDesc::new(player_key)
                .host_type(HostTypeId::new(1))
                .method(
                    MethodDesc::new(HostMethodId::new(9), "grant_exp")
                        .effects(MethodEffectSet::host_write()),
                ),
        )
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(&mut runtime, "fn main() { return 2; }");
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged method effect ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.method.effects_changed");
    assert_effect_abi_repair_hint(&report);
    let HotReloadErrorKind::ChangedMethodEffects {
        type_name,
        method,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
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
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn runtime_stages_source_file_method_access_rejection_until_safe_point() {
    let player_key = TypeKey::new(TypeId::new(1), "Player");
    let old_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_type(
            TypeDesc::new(player_key.clone())
                .host_type(HostTypeId::new(1))
                .method(
                    MethodDesc::new(HostMethodId::new(9), "grant_exp")
                        .access(MethodAccess::new().reflect_callable(true)),
                ),
        )
        .build()
        .expect("old engine should build");
    let initial = hot_reload_initial_from_source(&old_engine, "fn main() { return 1; }");
    let new_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_type(
            TypeDesc::new(player_key)
                .host_type(HostTypeId::new(1))
                .method(
                    MethodDesc::new(HostMethodId::new(9), "grant_exp")
                        .access(MethodAccess::new().reflect_callable(false)),
                ),
        )
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(&mut runtime, "fn main() { return 2; }");
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged method access ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.method.access_changed");
    assert_access_abi_repair_hint(&report);
    let HotReloadErrorKind::ChangedMethodAccess {
        type_name,
        method,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed host method access");
    };
    assert_eq!(type_name, "Player");
    assert_eq!(method, "grant_exp");
    assert!(old.callable);
    assert!(!new.callable);
    assert!(source_span.is_none());
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn runtime_stages_source_file_method_parameter_rejection_until_safe_point() {
    let player_key = TypeKey::new(TypeId::new(1), "Player");
    let old_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_type(
            TypeDesc::new(player_key.clone())
                .host_type(HostTypeId::new(1))
                .method(
                    MethodDesc::new(HostMethodId::new(9), "grant_exp")
                        .param(MethodParamDesc::new("amount").type_hint("i64")),
                ),
        )
        .build()
        .expect("old engine should build");
    let initial = hot_reload_initial_from_source(&old_engine, "fn main() { return 1; }");
    let new_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_type(
            TypeDesc::new(player_key)
                .host_type(HostTypeId::new(1))
                .method(
                    MethodDesc::new(HostMethodId::new(9), "grant_exp")
                        .param(MethodParamDesc::new("amount").type_hint("f64")),
                ),
        )
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(&mut runtime, "fn main() { return 2; }");
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged method parameter ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.method.parameter_abi_changed");
    assert_method_parameter_abi_repair_hint(&report);
    let HotReloadErrorKind::ChangedMethodParameterAbi {
        type_name,
        method,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed host method parameter ABI");
    };
    assert_eq!(type_name, "Player");
    assert_eq!(method, "grant_exp");
    assert_eq!(old.len(), 1);
    assert_eq!(old[0].name, "amount");
    assert_eq!(old[0].type_hint.as_deref(), Some("i64"));
    assert_eq!(new.len(), 1);
    assert_eq!(new[0].name, "amount");
    assert_eq!(new[0].type_hint.as_deref(), Some("f64"));
    assert!(source_span.is_none());
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn runtime_stages_source_file_method_return_rejection_until_safe_point() {
    let player_key = TypeKey::new(TypeId::new(1), "Player");
    let old_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_type(
            TypeDesc::new(player_key.clone())
                .host_type(HostTypeId::new(1))
                .method(MethodDesc::new(HostMethodId::new(9), "grant_exp").return_type("i64")),
        )
        .build()
        .expect("old engine should build");
    let initial = hot_reload_initial_from_source(&old_engine, "fn main() { return 1; }");
    let new_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_type(
            TypeDesc::new(player_key)
                .host_type(HostTypeId::new(1))
                .method(MethodDesc::new(HostMethodId::new(9), "grant_exp").return_type("null")),
        )
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(&mut runtime, "fn main() { return 2; }");
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged method return ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.method.return_abi_changed");
    assert_method_return_repair_hint(&report);
    let HotReloadErrorKind::ChangedMethodReturnAbi {
        type_name,
        method,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed host method return ABI");
    };
    assert_eq!(type_name, "Player");
    assert_eq!(method, "grant_exp");
    assert_eq!(old.as_deref(), Some("i64"));
    assert_eq!(new.as_deref(), Some("null"));
    assert!(source_span.is_none());
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn runtime_stages_file_hot_reload_rejection_until_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .hot_reload_policy(HotReloadPolicy::locked_down())
        .build()
        .expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(engine, "fn main() { return 1; }");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(
        &mut runtime,
        r#"
fn helper() {
    return 2;
}

fn main() {
    return helper();
}
"#,
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert!(matches!(
        report.errors[0].error.kind,
        HotReloadErrorKind::NewFunctionDenied { ref function }
            if function == "helper"
    ));
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn runtime_stages_source_file_top_level_effect_rejection_until_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(engine, "fn main() { return 1; }");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(
        &mut runtime,
        r#"
const BAD = register_event("monster.kill");

fn main() {
    return 2;
}
"#,
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged compile rejection report");

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
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn runtime_returns_hot_reload_file_source_errors_immediately() {
    let root = unique_test_dir("runtime_stage_file_source_error");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let path = root.join("main.vela");
    std::fs::write(&path, "fn main() { return 1; }").expect("write initial source");
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_file(&path)
        .expect("initial hot reload file compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let missing = root.join("missing.vela");

    let error = runtime
        .stage_hot_reload_update_file(&missing)
        .expect("runtime should be hot-reload enabled")
        .expect_err("missing source should not stage a hot reload report");

    assert!(matches!(
        error.kind,
        EngineHotReloadSourceErrorKind::Source(crate::source::EngineSourceError {
            kind: EngineSourceErrorKind::Io { .. }
        })
    ));
    assert!(
        !runtime
            .has_pending_hot_update()
            .expect("source error should not stage an update")
    );
}

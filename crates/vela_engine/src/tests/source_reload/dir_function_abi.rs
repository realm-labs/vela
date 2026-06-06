use super::*;

#[test]
fn runtime_stages_dir_required_parameter_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_dir_required_parameter");
    let reward_file = write_typed_reward_modules(&root, "return 2;", "int", "2");
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

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

    write_reward_module_with_signature(&reward_file, "(amount: int) -> int", "amount");
    runtime
        .stage_hot_reload_update_dir(&root)
        .expect("runtime should be hot-reload enabled")
        .expect("dir required parameter rejection should be staged");
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
        .expect("staged dir required parameter rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(
        report.errors[0].code,
        "reload.function.required_added_parameters"
    );
    assert_required_parameter_repair_hint(&report);
    let HotReloadErrorKind::AddedFunctionParametersWithoutDefaults { function, added } =
        &report.errors[0].error.kind
    else {
        panic!("expected added required parameters");
    };
    assert_eq!(function, "game::reward::grant");
    assert_eq!(added, &vec!["amount".to_owned()]);
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
fn runtime_stages_dir_event_parameter_reorder_rejection_until_safe_point() {
    event_parameter_reorder_rejection(
        "runtime_stage_dir_event_parameter_reorder",
        EventReloadWorkflow::Directory,
    );
}

#[test]
fn runtime_stages_changed_file_event_parameter_reorder_rejection_until_safe_point() {
    event_parameter_reorder_rejection(
        "runtime_stage_changed_file_event_parameter_reorder",
        EventReloadWorkflow::ChangedFile,
    );
}

#[test]
fn runtime_stages_dir_event_target_rejection_until_safe_point() {
    event_target_rejection(
        "runtime_stage_dir_event_target",
        EventReloadWorkflow::Directory,
    );
}

#[test]
fn runtime_stages_changed_file_event_target_rejection_until_safe_point() {
    event_target_rejection(
        "runtime_stage_changed_file_event_target",
        EventReloadWorkflow::ChangedFile,
    );
}

#[test]
fn runtime_stages_dir_script_function_access_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_dir_script_function_access");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let main_file = root.join("game").join("main.vela");
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

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

    std::fs::write(
        &main_file,
        r#"
fn main() {
    return 3;
}
"#,
    )
    .expect("write main without reward import");
    std::fs::write(
        &reward_file,
        r#"
fn grant() {
    return 6;
}
"#,
    )
    .expect("write reward without public export");
    runtime
        .stage_hot_reload_update_dir(&root)
        .expect("runtime should be hot-reload enabled")
        .expect("dir script function access ABI rejection should be staged");
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
        .expect("staged dir script function access ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.access_changed");
    assert_changed_function_access_rejection(&report, "game::reward::grant");
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
fn runtime_stages_dir_removed_function_rejection_until_safe_point() {
    let kind = removed_script_function_rejection_kind(
        "runtime_stage_dir_removed_function",
        ScriptFunctionReloadWorkflow::Directory,
    );

    let HotReloadErrorKind::RemovedFunction { function } = kind else {
        panic!("expected removed script function");
    };
    assert_eq!(function, "game::reward::helper");
}

#[test]
fn runtime_stages_dir_native_effect_rejection_until_safe_point() {
    let kind = dir_native_rejection_kind(
        "runtime_stage_dir_native_effect",
        NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
            .effects(EffectSet::host_read()),
        NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
            .effects(EffectSet::host_write()),
        "reload.function.effects_changed",
    );

    let HotReloadErrorKind::ChangedFunctionEffects {
        function,
        old,
        new,
        source_span,
    } = kind
    else {
        panic!("expected changed native function effects");
    };
    assert_eq!(function, "game::native::grant_bonus");
    assert!(old.reads_host);
    assert!(!old.writes_host);
    assert!(new.reads_host);
    assert!(new.writes_host);
    assert!(source_span.is_none());
}

#[test]
fn runtime_stages_dir_native_access_rejection_until_safe_point() {
    let kind = dir_native_rejection_kind(
        "runtime_stage_dir_native_access",
        NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
            .access(FunctionAccess::public().reflect_callable(true)),
        NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
            .access(FunctionAccess::public().reflect_callable(false)),
        "reload.function.access_changed",
    );

    let HotReloadErrorKind::ChangedFunctionAccess {
        function,
        old,
        new,
        source_span,
    } = kind
    else {
        panic!("expected changed native function access");
    };
    assert_eq!(function, "game::native::grant_bonus");
    assert!(old.callable);
    assert!(!new.callable);
    assert!(source_span.is_none());
}

#[test]
fn runtime_stages_dir_native_parameter_rejection_until_safe_point() {
    let kind = dir_native_rejection_kind(
        "runtime_stage_dir_native_parameter",
        NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
            .param("amount", TypeHint::Int),
        NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
            .param("amount", TypeHint::Float),
        "reload.function.parameter_abi_changed",
    );

    let HotReloadErrorKind::ChangedFunctionParameterAbi {
        function,
        old,
        new,
        source_span,
    } = kind
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
}

#[test]
fn runtime_stages_dir_native_return_rejection_until_safe_point() {
    let kind = dir_native_rejection_kind(
        "runtime_stage_dir_native_return",
        NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
            .returns(TypeHint::Int),
        NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
            .returns(TypeHint::Float),
        "reload.function.return_abi_changed",
    );

    let HotReloadErrorKind::ChangedFunctionReturnAbi {
        function,
        old,
        new,
        source_span,
    } = kind
    else {
        panic!("expected changed native function return ABI");
    };
    assert_eq!(function, "game::native::grant_bonus");
    assert_eq!(old.as_deref(), Some("int"));
    assert_eq!(new.as_deref(), Some("float"));
    assert!(source_span.is_none());
}

#[test]
fn runtime_stages_dir_removed_native_function_rejection_until_safe_point() {
    let kind = removed_native_descriptor_rejection_kind(
        "runtime_stage_dir_removed_native_function",
        NativeDescriptorReloadWorkflow::Directory,
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
fn runtime_stages_dir_native_stable_id_churn_rejection_until_safe_point() {
    let kind = native_stable_id_churn_rejection_kind(
        "runtime_stage_dir_native_stable_id_churn",
        NativeDescriptorReloadWorkflow::Directory,
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
fn runtime_stages_dir_native_stable_id_rename_until_safe_point() {
    let root = unique_test_dir("runtime_stage_dir_native_stable_id_rename");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    std::fs::write(
        &reward_file,
        r#"
pub fn grant() {
    return game::native::grant_bonus();
}
"#,
    )
    .expect("write old native reward module");
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
    let mut tx = PatchTx::new();

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

    std::fs::write(
        &reward_file,
        r#"
pub fn grant() {
    return game::native::grant_bonus_v2();
}
"#,
    )
    .expect("write renamed native reward module");
    runtime
        .stage_hot_reload_update_dir(&root)
        .expect("runtime should be hot-reload enabled")
        .expect("dir native stable-ID rename should be staged");

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged dir native stable-ID rename report");

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
        Ok(OwnedValue::Int(5))
    );
}

#[test]
fn runtime_stages_dir_method_stable_id_rename_until_safe_point() {
    let root = unique_test_dir("runtime_stage_dir_method_stable_id_rename");
    let game_dir = root.join("game");
    std::fs::create_dir_all(&game_dir).expect("create module dir");
    std::fs::write(
        game_dir.join("main.vela"),
        r#"
use game::reward::grant

fn main(player: Player) {
    return grant(player);
}
"#,
    )
    .expect("write main module");
    let reward_file = game_dir.join("reward.vela");
    std::fs::write(
        &reward_file,
        r#"
pub fn grant(player: Player) {
    player.grant_exp(7);
    return 1;
}
"#,
    )
    .expect("write old method reward module");
    let method = HostMethodId::new(9);
    let old_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_type(
            player_type(TypeId::new(1), HostTypeId::new(1))
                .method(MethodDesc::new(method, "grant_exp")),
        )
        .build()
        .expect("old engine should build");
    let initial = old_engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let new_engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_type(
            player_type(TypeId::new(1), HostTypeId::new(1))
                .method(MethodDesc::new(method, "award_exp")),
        )
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();

    let mut tx = PatchTx::new();
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

    std::fs::write(
        &reward_file,
        r#"
pub fn grant(player: Player) {
    player.award_exp(7);
    return 2;
}
"#,
    )
    .expect("write renamed method reward module");
    runtime
        .stage_hot_reload_update_dir(&root)
        .expect("runtime should be hot-reload enabled")
        .expect("dir method stable-ID rename should be staged");

    let mut tx = PatchTx::new();
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

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged dir method stable-ID rename report");

    assert!(report.accepted);
    assert!(report.errors.is_empty());
    let mut tx = PatchTx::new();
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
}

#[test]
fn runtime_stages_dir_method_effect_rejection_until_safe_point() {
    let kind = dir_method_rejection_kind(
        "runtime_stage_dir_method_effect",
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
fn runtime_stages_dir_method_access_rejection_until_safe_point() {
    let kind = dir_method_rejection_kind(
        "runtime_stage_dir_method_access",
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
fn runtime_stages_dir_method_parameter_rejection_until_safe_point() {
    let kind = dir_method_rejection_kind(
        "runtime_stage_dir_method_parameter",
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
fn runtime_stages_dir_method_return_rejection_until_safe_point() {
    let kind = dir_method_rejection_kind(
        "runtime_stage_dir_method_return",
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
fn runtime_stages_dir_removed_method_rejection_until_safe_point() {
    let kind = removed_method_descriptor_rejection_kind(
        "runtime_stage_dir_removed_method",
        MethodDescriptorReloadWorkflow::Directory,
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
fn runtime_stages_dir_method_stable_id_churn_rejection_until_safe_point() {
    let kind = method_stable_id_churn_rejection_kind(
        "runtime_stage_dir_method_stable_id_churn",
        MethodDescriptorReloadWorkflow::Directory,
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

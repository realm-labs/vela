use super::*;
use vela_common::{FunctionId, HostMethodId, SourceId, TypeId};
use vela_reflect::{
    FunctionAccess, FunctionDesc, FunctionEffectSet, MethodAccess, MethodDesc, MethodEffectSet,
    SchemaHash, TypeDesc, TypeKey, TypeRegistry,
};
use vela_vm::{Value, Vm};

#[test]
fn new_calls_enter_new_code_after_update() {
    let initial =
        compile_initial(SourceId::new(1), "fn main() { return 20; }").expect("compile initial");
    let mut runtime = HotReloadRuntime::new(initial);
    let update = compile_update(
        &runtime.current(),
        SourceId::new(2),
        "fn main() { return 30; }",
    )
    .expect("compile update");

    runtime.apply_hot_update(update).expect("apply update");

    assert_eq!(
        Vm::new().run_program(&runtime.current().to_program(), "main", &[]),
        Ok(Value::Int(30))
    );
}

#[test]
fn old_version_lifetime_preserves_old_code() {
    let initial =
        compile_initial(SourceId::new(1), "fn main() { return 20; }").expect("compile initial");
    let mut runtime = HotReloadRuntime::new(initial);
    let old = runtime.current();
    let update =
        compile_update(&old, SourceId::new(2), "fn main() { return 30; }").expect("update");

    let new = runtime.apply_hot_update(update).expect("apply update");

    assert_eq!(
        Vm::new().run_program(&old.to_program(), "main", &[]),
        Ok(Value::Int(20))
    );
    assert_eq!(
        Vm::new().run_program(&new.to_program(), "main", &[]),
        Ok(Value::Int(30))
    );
}

#[test]
fn deleted_function_parameters_are_rejected() {
    let initial = compile_initial(SourceId::new(1), "fn main(value) { return value; }")
        .expect("compile initial");

    let error = compile_update(&initial, SourceId::new(2), "fn main() { return 0; }")
        .expect_err("deleted param");

    assert_eq!(
        error.kind,
        HotReloadErrorKind::DeletedFunctionParameters {
            function: "main".to_owned(),
            old: vec!["value".to_owned()],
            new: Vec::new(),
        }
    );
}

#[test]
fn new_private_helper_functions_are_accepted() {
    let initial = compile_initial(SourceId::new(1), "fn main() { return 1; }").expect("initial");
    let mut runtime = HotReloadRuntime::new(initial);
    let update = compile_update(
        &runtime.current(),
        SourceId::new(2),
        r#"
fn helper() {
    return 7;
}

fn main() {
    return helper();
}
"#,
    )
    .expect("helper update");

    runtime.apply_hot_update(update).expect("apply update");

    assert_eq!(
        Vm::new().run_program(&runtime.current().to_program(), "main", &[]),
        Ok(Value::Int(7))
    );
}

#[test]
fn schema_abi_changes_are_rejected() {
    let old_abi = HotReloadAbi::empty().schema(SchemaAbi::new("Reward", SchemaHash::new(0x1111)));
    let new_abi = HotReloadAbi::empty().schema(SchemaAbi::new("Reward", SchemaHash::new(0x2222)));
    let initial = compile_initial_with_abi(SourceId::new(1), "fn main() { return 1; }", old_abi)
        .expect("initial");

    let error = compile_update_with_abi(
        &initial,
        SourceId::new(2),
        "fn main() { return 2; }",
        new_abi,
    )
    .expect_err("schema change should fail");

    assert_eq!(
        error.kind,
        HotReloadErrorKind::ChangedSchema {
            type_name: "Reward".to_owned(),
            old_hash: 0x1111,
            new_hash: 0x2222,
        }
    );
}

#[test]
fn removed_schema_abi_is_rejected() {
    let old_abi = HotReloadAbi::empty().schema(SchemaAbi::new("Reward", SchemaHash::new(0x1111)));
    let initial = compile_initial_with_abi(SourceId::new(1), "fn main() { return 1; }", old_abi)
        .expect("initial");

    let error = compile_update_with_abi(
        &initial,
        SourceId::new(2),
        "fn main() { return 2; }",
        HotReloadAbi::empty(),
    )
    .expect_err("removed schema should fail");

    assert_eq!(
        error.kind,
        HotReloadErrorKind::RemovedSchema {
            type_name: "Reward".to_owned(),
            old_hash: 0x1111,
        }
    );
}

#[test]
fn function_effect_and_access_abi_changes_are_rejected() {
    let old_abi = HotReloadAbi::empty().function(FunctionAbi::new(
        "game.reward.grant",
        EffectAbi::host_read(),
        AccessAbi::new(true, true, vec!["reward.read".to_owned()]),
    ));
    let changed_effects = HotReloadAbi::empty().function(FunctionAbi::new(
        "game.reward.grant",
        EffectAbi::host_write(),
        AccessAbi::new(true, true, vec!["reward.read".to_owned()]),
    ));
    let initial =
        compile_initial_with_abi(SourceId::new(1), "fn main() { return 1; }", old_abi.clone())
            .expect("initial");

    let error = compile_update_with_abi(
        &initial,
        SourceId::new(2),
        "fn main() { return 2; }",
        changed_effects,
    )
    .expect_err("effect change should fail");
    assert_eq!(
        error.kind,
        HotReloadErrorKind::ChangedFunctionEffects {
            function: "game.reward.grant".to_owned(),
            old: EffectAbi::host_read(),
            new: EffectAbi::host_write(),
        }
    );

    let changed_access = HotReloadAbi::empty().function(FunctionAbi::new(
        "game.reward.grant",
        EffectAbi::host_read(),
        AccessAbi::new(true, true, vec!["reward.write".to_owned()]),
    ));
    let error = compile_update_with_abi(
        &initial,
        SourceId::new(3),
        "fn main() { return 3; }",
        changed_access,
    )
    .expect_err("access change should fail");
    assert_eq!(
        error.kind,
        HotReloadErrorKind::ChangedFunctionAccess {
            function: "game.reward.grant".to_owned(),
            old: AccessAbi::new(true, true, vec!["reward.read".to_owned()]),
            new: AccessAbi::new(true, true, vec!["reward.write".to_owned()]),
        }
    );
}

#[test]
fn method_effect_and_access_abi_changes_are_rejected() {
    let old_abi = HotReloadAbi::empty().method(MethodAbi::new(
        "Player",
        "grant_exp",
        EffectAbi::host_write(),
        AccessAbi::new(true, true, vec!["player.write".to_owned()]),
    ));
    let changed_effects = HotReloadAbi::empty().method(MethodAbi::new(
        "Player",
        "grant_exp",
        EffectAbi::host_read(),
        AccessAbi::new(true, true, vec!["player.write".to_owned()]),
    ));
    let initial =
        compile_initial_with_abi(SourceId::new(1), "fn main() { return 1; }", old_abi.clone())
            .expect("initial");

    let error = compile_update_with_abi(
        &initial,
        SourceId::new(2),
        "fn main() { return 2; }",
        changed_effects,
    )
    .expect_err("method effect change should fail");
    assert_eq!(
        error.kind,
        HotReloadErrorKind::ChangedMethodEffects {
            type_name: "Player".to_owned(),
            method: "grant_exp".to_owned(),
            old: EffectAbi::host_write(),
            new: EffectAbi::host_read(),
        }
    );

    let changed_access = HotReloadAbi::empty().method(MethodAbi::new(
        "Player",
        "grant_exp",
        EffectAbi::host_write(),
        AccessAbi::new(true, false, vec!["player.write".to_owned()]),
    ));
    let error = compile_update_with_abi(
        &initial,
        SourceId::new(3),
        "fn main() { return 3; }",
        changed_access,
    )
    .expect_err("method access change should fail");
    assert_eq!(
        error.kind,
        HotReloadErrorKind::ChangedMethodAccess {
            type_name: "Player".to_owned(),
            method: "grant_exp".to_owned(),
            old: AccessAbi::new(true, true, vec!["player.write".to_owned()]),
            new: AccessAbi::new(true, false, vec!["player.write".to_owned()]),
        }
    );
}

#[test]
fn abi_manifest_can_be_built_from_type_registry() {
    let player = TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
        .schema_hash(SchemaHash::new(0xfeed))
        .method(
            MethodDesc::new(HostMethodId::new(9), "grant_exp")
                .effects(MethodEffectSet::host_write())
                .access(
                    MethodAccess::new()
                        .reflect_callable(true)
                        .require_permission("player.write"),
                ),
        );
    let mut registry = TypeRegistry::new();
    registry.register(player);
    registry.register_function(
        FunctionDesc::new(FunctionId::new(11), "game.reward.grant")
            .effects(FunctionEffectSet::event_emit())
            .access(
                FunctionAccess::new()
                    .reflect_visible(true)
                    .require_permission("reward.grant"),
            ),
    );

    let abi = HotReloadAbi::from_registry(&registry);
    let initial =
        compile_initial_with_abi(SourceId::new(1), "fn main() { return 1; }", abi.clone())
            .expect("initial");

    compile_update_with_abi(&initial, SourceId::new(2), "fn main() { return 2; }", abi)
        .expect("unchanged registry ABI should be accepted");
}

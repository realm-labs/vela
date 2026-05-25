use vela_common::{FieldId, HostObjectId, HostTypeId};
use vela_engine::{
    EffectSet, Engine, FunctionAccess, HostRef, NativeCallContext, NativeFunctionDesc,
    NativeFunctionId, TypeHint, Value,
};
use vela_host::{HostPath, HostValue, MockStateAdapter, PatchOp, PatchTx};
use vela_macros::{script_context_function, script_function, script_host_function};
use vela_vm::{HostExecution, VmResult};

/// Grants a copied bonus amount.
#[script_function(
    id = 41,
    name = "game.grant_bonus",
    effect = "pure",
    reflect = true,
    permission = "bonus.read"
)]
fn grant_bonus(amount: i64, multiplier: i64) -> i64 {
    amount * multiplier
}

/// Sets a copied player level through PatchTx.
#[script_context_function(
    id = 42,
    name = "game.set_level",
    effect = "write_host",
    reflect = true,
    permission = "player.write"
)]
fn set_level(ctx: &mut NativeCallContext<'_, '_>, player: HostRef, level: i64) -> VmResult<bool> {
    ctx.charge_instructions(3)?;
    ctx.tx().set_path(
        HostPath::new(player).field(FieldId::new(1)),
        HostValue::Int(level),
        None,
    )?;
    Ok(ctx.has_permission("player.write"))
}

/// Sets a copied player score through host execution.
#[script_host_function(
    id = 43,
    name = "game.set_score",
    effect = "write_host",
    reflect = true,
    permission = "player.write"
)]
fn set_score(host: &mut HostExecution<'_>, player: HostRef, score: i64) -> VmResult<i64> {
    host.tx.set_path(
        HostPath::new(player).field(FieldId::new(2)),
        HostValue::Int(score),
        None,
    )?;
    Ok(score)
}

#[test]
fn script_function_generates_native_function_metadata() {
    assert_eq!(
        vela_native_function_desc_grant_bonus(),
        NativeFunctionDesc::new("game.grant_bonus", NativeFunctionId::new(41))
            .param("amount", TypeHint::Int)
            .param("multiplier", TypeHint::Int)
            .returns(TypeHint::Int)
            .effects(EffectSet::pure())
            .access(
                FunctionAccess::public()
                    .reflect_callable(true)
                    .require_permission("bonus.read"),
            )
            .docs("Grants a copied bonus amount."),
    );
}

#[test]
fn script_context_function_generates_native_function_metadata() {
    assert_eq!(
        vela_native_function_desc_set_level(),
        NativeFunctionDesc::new("game.set_level", NativeFunctionId::new(42))
            .param("player", TypeHint::Any)
            .param("level", TypeHint::Int)
            .returns(TypeHint::Bool)
            .effects(EffectSet::host_write())
            .access(
                FunctionAccess::public()
                    .reflect_callable(true)
                    .require_permission("player.write"),
            )
            .docs("Sets a copied player level through PatchTx."),
    );
}

#[test]
fn script_host_function_generates_native_function_metadata() {
    assert_eq!(
        vela_native_function_desc_set_score(),
        NativeFunctionDesc::new("game.set_score", NativeFunctionId::new(43))
            .param("player", TypeHint::Any)
            .param("score", TypeHint::Int)
            .returns(TypeHint::Int)
            .effects(EffectSet::host_write())
            .access(
                FunctionAccess::public()
                    .reflect_callable(true)
                    .require_permission("player.write"),
            )
            .docs("Sets a copied player score through host execution."),
    );
}

#[test]
fn script_function_registers_typed_native_with_engine() {
    let engine =
        vela_register_native_function_grant_bonus(Engine::builder().grant_permission("bonus.read"))
            .build()
            .expect("engine should build from macro native function");
    let root = unique_test_dir("script_function_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.lang");
    std::fs::write(
        &source,
        r#"
fn main() {
    return game.grant_bonus(6, 7);
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered native");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(42)),
    );
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn script_context_function_registers_typed_native_with_engine() {
    let engine = vela_register_context_native_function_set_level(
        Engine::builder().grant_permission("player.write"),
    )
    .build()
    .expect("engine should build from macro context native function");
    let root = unique_test_dir("script_context_function_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.lang");
    std::fs::write(
        &source,
        r#"
fn main(player) {
    return game.set_level(player, 9);
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered context native");
    let player = HostRef::new(HostTypeId::new(1001), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.into_vm().run_program_with_host(
            &program,
            "main",
            &[Value::HostRef(player)],
            &mut host,
        ),
        Ok(Value::Bool(true)),
    );
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(9)));
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn script_host_function_registers_typed_native_with_engine() {
    let engine = vela_register_host_native_function_set_score(
        Engine::builder().grant_permission("player.write"),
    )
    .build()
    .expect("engine should build from macro host native function");
    let root = unique_test_dir("script_host_function_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.lang");
    std::fs::write(
        &source,
        r#"
fn main(player) {
    return game.set_score(player, 12);
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered host native");
    let player = HostRef::new(HostTypeId::new(1001), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.into_vm().run_program_with_host(
            &program,
            "main",
            &[Value::HostRef(player)],
            &mut host,
        ),
        Ok(Value::Int(12)),
    );
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(12)));
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

fn unique_test_dir(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "vela_macros_{name}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before epoch")
            .as_nanos()
    ));
    path
}

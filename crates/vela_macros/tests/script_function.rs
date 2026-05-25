use vela_engine::{
    EffectSet, Engine, FunctionAccess, NativeFunctionDesc, NativeFunctionId, TypeHint, Value,
};
use vela_macros::script_function;

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

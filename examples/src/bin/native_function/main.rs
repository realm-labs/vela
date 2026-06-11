#![allow(clippy::result_large_err)]

use std::collections::BTreeMap;
use std::error::Error;

use vela_engine::prelude::*;
use vela_examples::example_file;
use vela_macros::{ScriptHost, script_context_function, script_function, script_methods};

fn main() -> Result<(), Box<dyn Error>> {
    let engine = vela_register_context_native_function_grant_level(
        vela_register_native_function_collection_bonus(vela_register_native_function_bonus_macro(
            Engine::builder()
                .capability(Capability::HostRead)
                .capability(Capability::HostWrite)
                .register_script_host::<Player>()
                .register_typed_native_fn::<(i64, i64), _>(
                    NativeFunctionDesc::new("game::bonus_manual", NativeFunctionId::new(10_001))
                        .param("amount", TypeHint::Int)
                        .param("multiplier", TypeHint::Int)
                        .returns(TypeHint::Int)
                        .effects(EffectSet::pure())
                        .access(FunctionAccess::public().reflect_callable(true)),
                    bonus_manual,
                ),
        )),
    )
    .build()?;
    let program = engine.compile_file(example_file("native_function", "main.vela"))?;
    let mut runtime = Runtime::new(engine, program);
    let mut player = Player { level: 1 };

    let output = runtime.call(
        "main",
        CallArgs::new().with_host_mut("player", &mut player),
        CallOptions::new(10_000, 1024 * 1024, 64),
    )?;

    println!(
        "native_function_result={:?} final_level={}",
        runtime.value_to_owned(&output)?,
        player.level
    );
    Ok(())
}

#[derive(Debug, ScriptHost)]
#[script(path = "examples::native_function::Player")]
struct Player {
    #[script(get, set, hint = "int")]
    level: i64,
}

#[script_methods]
impl Player {}

fn bonus_manual(amount: i64, multiplier: i64) -> i64 {
    amount * multiplier
}

#[script_function(name = "game::bonus_macro", effect = "pure", reflect = true)]
fn bonus_macro(amount: i64, extra: i64) -> i64 {
    amount + extra
}

#[script_function(name = "game::collection_bonus", effect = "pure", reflect = true)]
fn collection_bonus(scores: BTreeMap<String, i64>, tags: Vec<String>) -> i64 {
    scores.values().sum::<i64>() + i64::try_from(tags.len()).unwrap_or_default()
}

#[script_context_function(name = "game::grant_level", effect = "write_host", reflect = true)]
fn grant_level(
    ctx: &mut NativeCallContext<'_, '_>,
    player: HostRef,
    amount: i64,
) -> vela_vm::error::VmResult<i64> {
    let path = Player::vela_field_path_level(player);
    ctx.add_path(
        path.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(amount)),
        None,
    )?;
    match ctx.read_path(&path, None)? {
        HostValue::Scalar(vela_common::ScalarValue::I64(level)) => Ok(level),
        _ => Ok(0),
    }
}

use std::error::Error;

use vela_engine::prelude::*;
use vela_examples::example_file;
use vela_macros::script_function;

fn main() -> Result<(), Box<dyn Error>> {
    let engine = vela_register_native_function_bonus_macro(
        Engine::builder().register_typed_native_fn::<(i64, i64), _>(
            NativeFunctionDesc::new("game::bonus_manual", NativeFunctionId::new(10_001))
                .param("amount", TypeHint::Int)
                .param("multiplier", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::pure())
                .access(FunctionAccess::public().reflect_callable(true)),
            bonus_manual,
        ),
    )
    .build()?;
    let program = engine.compile_file(example_file("native_function", "main.vela"))?;
    let mut runtime = Runtime::new(engine, program);

    let output = runtime.call(
        "main",
        CallArgs::new(),
        CallOptions::new(10_000, 1024 * 1024, 64),
    )?;

    println!("native_function_result={:?}", output.value());
    Ok(())
}

fn bonus_manual(amount: i64, multiplier: i64) -> i64 {
    amount * multiplier
}

#[script_function(name = "game::bonus_macro", effect = "pure", reflect = true)]
fn bonus_macro(amount: i64, extra: i64) -> i64 {
    amount + extra
}

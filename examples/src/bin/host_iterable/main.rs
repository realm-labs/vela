use std::error::Error;

use vela_engine::prelude::*;

fn main() -> Result<(), Box<dyn Error>> {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::scores", NativeFunctionId::new(30_001))
                .returns(TypeHint::iterator())
                .effects(EffectSet::pure())
                .access(FunctionAccess::public().reflect_callable(true)),
            scores,
        )
        .build()?;
    let program = engine.compile_source(include_str!("main.vela"))?;
    let mut runtime = Runtime::new(engine, program);

    let output = runtime.call("main", CallArgs::new(), CallOptions::unbounded())?;
    let total: i64 = runtime.from_value(&output)?;

    assert_eq!(total, 20);
    println!("host_iterable total={total}");

    Ok(())
}

fn scores(args: &[OwnedValue]) -> vela_vm::error::VmResult<OwnedValue> {
    if !args.is_empty() {
        return Err(vela_vm::error::VmError::new(
            vela_vm::error::VmErrorKind::ArityMismatch {
                name: "game::scores".to_owned(),
                expected: 0,
                actual: args.len(),
            },
        ));
    }

    Ok(OwnedValue::iterator([
        OwnedValue::i64(2),
        OwnedValue::i64(3),
        OwnedValue::i64(5),
    ]))
}

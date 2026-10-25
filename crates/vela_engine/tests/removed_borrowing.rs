use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use vela_engine::args::ToScriptValueRef;
use vela_engine::engine::Engine;
use vela_engine::native::{EffectSet, NativeFunctionDesc, TypeHint};
use vela_engine::permission::Capability;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_vm::error::VmErrorKind;
use vela_vm::owned_value::OwnedValue;

#[test]
fn removed_host_source_cannot_reach_registration_or_dispatch() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine");
    for source in [
        "extern state borrowed: i64; fn main() { return borrowed; }",
        "fn main(value) { host::release(value); }",
        "fn main(value) { return host::try_release(value); }",
        "fn main(value: ArrayView<i64>) { return value[0]; }",
        "fn main(value: ArrayMut<i64>) { return value[0]; }",
        "fn main(value: MapView<String, i64>) { return value; }",
        "fn main(value: MapMut<String, i64>) { return value; }",
        "fn main(value: SetView<i64>) { return value; }",
        "fn main(value: SetMut<i64>) { return value; }",
        "fn main(value: PathProxy) { return value; }",
    ] {
        assert!(
            engine.compile_source(source).is_err(),
            "removed source compiled: {source}"
        );
    }
    assert!(
        engine
            .registry()
            .function_by_name("host::release")
            .is_none()
    );
    assert!(
        engine
            .registry()
            .function_by_name("host::try_release")
            .is_none()
    );
}

#[test]
fn reference_conversion_is_an_owned_copy_without_rust_writeback() {
    let source = vec![1_i64, 2];
    let copied = source.to_script_value_ref();
    let engine = Engine::builder().build().expect("engine");
    let program = engine
        .compile_source(
            "fn update(values: Array<i64>) { values[0] = 9; values.push(3); return values; }",
        )
        .expect("script");
    let mut runtime = Runtime::new(engine, program).expect("runtime");
    for args in [
        CallArgs::new().with(copied.clone()),
        CallArgs::new().with_value("values", copied),
    ] {
        let result = runtime
            .call("update", args, CallOptions::new(10_000, 1 << 20, 32))
            .expect("copy-in");
        assert_eq!(
            runtime.value_to_owned(&result),
            Ok(OwnedValue::Array(vec![
                9_i64.into(),
                2_i64.into(),
                3_i64.into()
            ]))
        );
        assert_eq!(source, [1, 2]);
    }
}

#[test]
fn context_natives_keep_execution_and_memory_accounting_without_host_resources() {
    for charge_memory in [false, true] {
        let engine = Engine::builder()
            .register_typed_context_host_native_fn::<(), _>(
                NativeFunctionDesc::new("test::charge", vela_def::FunctionId::new(0xD002))
                    .returns(TypeHint::i64()),
                move |context: &mut vela_engine::context::NativeCallContext<'_>| -> vela_vm::error::VmResult<i64> {
                    if charge_memory {
                        context.charge_memory_bytes(2 << 20)?;
                    } else {
                        context.charge_execution_units(10_000)?;
                    }
                    Ok(42)
                },
            )
            .build()
            .expect("engine");
        let program = engine
            .compile_source("fn main() { return test::charge(); }")
            .expect("script");
        let mut runtime = Runtime::new(engine, program).expect("runtime");
        let error = runtime
            .call(
                "main",
                CallArgs::new(),
                CallOptions::new(1_000, 1 << 20, 32),
            )
            .expect_err("native charge must exhaust shared call budget");
        let expected = if charge_memory {
            vela_vm::budget::ExecutionBudgetKind::MemoryBytes
        } else {
            vela_vm::budget::ExecutionBudgetKind::ExecutionUnits
        };
        assert!(
            matches!(error.kind(), VmErrorKind::BudgetExceeded { budget, .. } if budget == expected)
        );
        assert!(error.source_span.is_some());
    }
}

#[test]
fn independent_native_host_effects_still_gate_rust_invocation() {
    for (effects, capability) in [
        (EffectSet::host_read(), Capability::HostRead),
        (EffectSet::host_write(), Capability::HostWrite),
    ] {
        for granted in [false, true] {
            let calls = Arc::new(AtomicUsize::new(0));
            let invoked = Arc::clone(&calls);
            let mut builder = Engine::builder().register_native_fn(
                NativeFunctionDesc::new("test::guarded", vela_def::FunctionId::new(0xD001))
                    .returns(TypeHint::i64())
                    .effects(effects),
                move |_| {
                    invoked.fetch_add(1, Ordering::SeqCst);
                    Ok(OwnedValue::i64(42))
                },
            );
            if granted {
                builder = builder.capability(capability);
            }
            let engine = builder.build().expect("engine");
            let program = engine
                .compile_source("fn main() { return test::guarded(); }")
                .expect("script");
            let mut runtime = Runtime::new(engine, program).expect("runtime");
            let result = runtime.call(
                "main",
                CallArgs::new(),
                CallOptions::new(1_000, 1 << 20, 32),
            );
            if granted {
                let value = result.expect("granted native");
                assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::i64(42)));
                assert_eq!(calls.load(Ordering::SeqCst), 1);
            } else {
                assert!(
                    matches!(result.expect_err("permission required").kind(), VmErrorKind::PermissionDenied { capability: denied, .. } if denied == capability.as_str())
                );
                assert_eq!(calls.load(Ordering::SeqCst), 0);
            }
        }
    }
}

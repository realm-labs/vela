#![cfg(feature = "artifact-codec")]

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use rstest::rstest;
use vela_bytecode::PortableProgramArtifact;
use vela_def::FunctionId;
use vela_engine::engine::Engine;
use vela_engine::native::{EffectSet, NativeFunctionDesc, TypeHint};
use vela_engine::permission::Capability;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_vm::budget::ExecutionBudgetKind;
use vela_vm::error::VmErrorKind;
use vela_vm::owned_value::OwnedValue;

#[rstest]
#[case::nested_captures(
    include_str!("../../vela_bytecode/tests/fixtures/compiler/nested_captures.vela"),
    17
)]
#[case::scalar_loop(
    include_str!("../../vela_bytecode/tests/fixtures/compiler/scalar_loop.vela"),
    11_940
)]
#[case::typed_record_write(
    include_str!("../../vela_bytecode/tests/fixtures/compiler/typed_record_write.vela"),
    5
)]
#[case::named_default_method(
    include_str!("../../vela_bytecode/tests/fixtures/compiler/named_default_method.vela"),
    21
)]
fn source_and_decoded_portable_execution_match(#[case] source: &str, #[case] expected: i64) {
    let build_engine = Engine::builder().build().expect("compiler engine");
    let compiled = build_engine
        .compile_source(source)
        .expect("SF04 source compiles");
    let bytes = PortableProgramArtifact::from_compiled(compiled)
        .expect("portable-supported source")
        .encode()
        .expect("encode artifact");
    let source_program = build_engine
        .compile_source(source)
        .expect("source route compiles");
    let source_runtime = Runtime::new(build_engine, source_program).expect("source runtime");

    let receiving_engine = Engine::builder().build().expect("fresh receiving engine");
    let decoded = PortableProgramArtifact::decode(&bytes).expect("decode artifact");
    let linked = receiving_engine
        .link_portable_program(decoded.into_compiled())
        .expect("link in receiving engine without compiling source");
    let portable_runtime =
        Runtime::from_linked_artifact(receiving_engine, linked).expect("portable runtime");

    for (route, mut runtime) in [("source", source_runtime), ("decoded", portable_runtime)] {
        let error = runtime
            .call("main", CallArgs::new(), CallOptions::new(0, 1 << 20, 32))
            .expect_err("zero execution budget must fail on either route");
        assert_eq!(
            error.kind(),
            VmErrorKind::BudgetExceeded {
                budget: ExecutionBudgetKind::ExecutionUnits,
                limit: 0,
            },
            "{route}"
        );
        for _ in 0..2 {
            let value = runtime
                .call(
                    "main",
                    CallArgs::new(),
                    CallOptions::new(100_000, 1 << 20, 32),
                )
                .unwrap_or_else(|error| panic!("{route}: {error}"));
            assert_eq!(
                runtime.value_to_owned(&value),
                Ok(OwnedValue::i64(expected)),
                "{route}"
            );
        }
    }
}

fn effect_engine(calls: &Arc<AtomicUsize>, allowed: bool) -> Engine {
    let calls = Arc::clone(calls);
    let mut builder = Engine::builder().register_native_fn(
        NativeFunctionDesc::new("test::record", FunctionId::new(0x5F06))
            .param("value", TypeHint::i64())
            .returns(TypeHint::i64())
            .effects(EffectSet::host_write()),
        move |args| {
            assert_eq!(args, &[OwnedValue::i64(7)]);
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(OwnedValue::i64(8))
        },
    );
    if allowed {
        builder = builder.capability(Capability::HostWrite);
    }
    builder.build().expect("effect engine")
}

#[rstest]
#[case::allowed(true)]
#[case::denied(false)]
fn source_and_decoded_portable_preserve_native_effects(#[case] allowed: bool) {
    // Keep this one-call permission probe inline; the four SF04 inputs above stay unmodified.
    let source_calls = Arc::new(AtomicUsize::new(0));
    let engine = effect_engine(&source_calls, allowed);
    let source = "fn main() { return test::record(7); }";
    let compiled = engine
        .compile_source(source)
        .expect("native source compiles independently of runtime permission");
    let bytes = PortableProgramArtifact::from_compiled(compiled)
        .expect("native artifact")
        .encode()
        .expect("encode native artifact");
    let source_program = engine
        .compile_source(source)
        .expect("source route compiles");
    let source_runtime = Runtime::new(engine, source_program).expect("source runtime");
    let decoded_calls = Arc::new(AtomicUsize::new(0));
    let receiver = effect_engine(&decoded_calls, allowed);
    let decoded = PortableProgramArtifact::decode(&bytes).expect("decode native artifact");
    let linked = receiver
        .link_portable_program(decoded.into_compiled())
        .expect("receiving registry binds native identity");
    let decoded_runtime = Runtime::from_linked_artifact(receiver, linked).expect("decoded runtime");

    for (route, mut runtime, calls) in [
        ("source", source_runtime, source_calls),
        ("decoded", decoded_runtime, decoded_calls),
    ] {
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "{route}: linking has no host effects"
        );
        let result = runtime.call("main", CallArgs::new(), CallOptions::new(100, 1 << 20, 8));
        if allowed {
            let value = result.expect("host write granted");
            assert_eq!(
                runtime.value_to_owned(&value),
                Ok(OwnedValue::i64(8)),
                "{route}"
            );
            assert_eq!(
                calls.load(Ordering::SeqCst),
                1,
                "{route}: exactly one host write"
            );
        } else {
            assert_eq!(
                result.expect_err("host write denied").kind(),
                VmErrorKind::PermissionDenied {
                    native: "test::record".to_owned(),
                    capability: "host_write".to_owned(),
                },
                "{route}"
            );
            assert_eq!(
                calls.load(Ordering::SeqCst),
                0,
                "{route}: deny before host write"
            );
        }
    }
}

#[test]
fn decoded_portable_program_executes_without_source_compilation() {
    let build_engine = Engine::builder().build().expect("build compiler engine");
    let compiled = build_engine
        .compile_source("fn main() { return 40 + 2; }")
        .expect("compile source outside receiving engine");
    let bytes = PortableProgramArtifact::from_compiled(compiled)
        .expect("create portable artifact")
        .encode()
        .expect("encode portable artifact");

    let receiving_engine = Engine::builder().build().expect("build receiving engine");
    let decoded = PortableProgramArtifact::decode(&bytes).expect("decode portable artifact");
    let linked = receiving_engine
        .link_portable_program(decoded.into_compiled())
        .expect("bind stable identities");
    let mut runtime =
        Runtime::from_linked_artifact(receiving_engine, linked).expect("build portable runtime");
    let value = runtime
        .call(
            "main",
            CallArgs::from_positional([]),
            CallOptions::unbounded(),
        )
        .expect("execute portable bytecode");

    assert_eq!(
        runtime.value_to_owned(&value).expect("detach result"),
        OwnedValue::i64(42)
    );
}

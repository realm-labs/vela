use vela_engine::prelude::*;

#[test]
fn prelude_covers_the_ordinary_compile_and_call_path() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source("pub fn add(left: i64, right: i64) { return left + right; }")
        .expect("source should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

    let value = runtime
        .call(
            "add",
            CallArgs::from_positional([2_i64.into(), 3_i64.into()]),
            CallOptions::unbounded(),
        )
        .expect("call should succeed");

    assert_eq!(
        runtime.value_to_owned(&value),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(5)))
    );
}

#[test]
fn prelude_covers_typed_native_registration() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(i64, i64), _>(
            NativeFunctionDesc::new("math::add", NativeFunctionId::new(44))
                .param("left", TypeHint::i64())
                .param("right", TypeHint::i64())
                .returns(TypeHint::i64())
                .effects(EffectSet::pure())
                .access(FunctionAccess::public()),
            |left: i64, right: i64| left + right,
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source("pub fn main() { return math::add(4, 5); }")
        .expect("source should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

    let value = runtime
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect("call should succeed");

    assert_eq!(
        runtime.value_to_owned(&value),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(9)))
    );
}

#[test]
fn prelude_covers_service_patch_authoring() {
    let empty = PatchRevision::empty();
    let revision = empty
        .apply(PatchEdit::put(
            "rules/reward.vela",
            "pub fn reward() { return 1; }",
        ))
        .expect("patch should apply");
    let patch = ServicePatch::against(&revision)
        .put("rules/audit.vela", "pub fn audit() {}")
        .remove("rules/reward.vela");

    assert!(revision.sources().contains("rules/reward.vela"));
    assert!(matches!(patch, ServicePatch::Edit { .. }));
}

#[test]
fn prelude_covers_source_and_reload_results() {
    let engine = Engine::builder().build().expect("engine should build");
    let compile_error: EngineSourceError = engine
        .compile_file("missing-prelude-source.vela")
        .expect_err("missing source should report an engine source error");

    assert!(matches!(
        compile_error.kind,
        EngineSourceErrorKind::Io { .. }
    ));

    let reload_result: EngineHotReloadSourceResult<ProgramVersion> =
        engine.compile_hot_reload_initial_file("missing-prelude-reload.vela");
    let reload_error = reload_result.expect_err("missing reload source should report source error");

    assert!(matches!(
        reload_error.kind,
        EngineHotReloadSourceErrorKind::Source(EngineSourceError {
            kind: EngineSourceErrorKind::Io { .. },
        })
    ));
}

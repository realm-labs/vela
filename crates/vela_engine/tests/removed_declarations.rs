use vela_engine::engine::Engine;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_engine::source::EngineSourceErrorKind;
use vela_vm::owned_value::OwnedValue;

#[test]
fn removed_service_and_provider_declarations_fail_even_when_unused() {
    let engine = Engine::builder().build().expect("engine");
    for (attribute, source) in [
        (
            "service_impl",
            "#[service_impl(app::calculator)] impl Patch { fn value() { return 1; } }\n\
             pub fn main() { return 42; }",
        ),
        (
            "provider",
            "trait Calculator { fn value(self) -> i64; }\n\
             struct CalculatorImpl {}\n\
             #[provider(id = \"calculator\")]\n\
             impl Calculator for CalculatorImpl { fn value(self) -> i64 { return 1; } }\n\
             pub fn main() { return 42; }",
        ),
    ] {
        let error = engine
            .compile_source(source)
            .expect_err("removed declaration");
        let EngineSourceErrorKind::Frontend(error) = error.kind else {
            panic!("removed declarations must fail before backend compilation");
        };
        assert!(error.diagnostics().iter().any(|diagnostic| {
            diagnostic.code.as_deref() == Some("hir::removed_declaration")
                && diagnostic.message.contains(attribute)
                && diagnostic.span.is_some()
        }));
    }
}

#[test]
fn removed_service_dispatch_paths_fail_during_source_compilation() {
    let engine = Engine::builder().build().expect("engine");
    for path in ["service::base::value", "service::pinned::calculator::value"] {
        let error = engine
            .compile_source(&format!("pub fn main() {{ return {path}(); }}"))
            .expect_err("removed Service dispatch");
        let EngineSourceErrorKind::Frontend(error) = error.kind else {
            panic!("removed dispatch must fail before backend compilation");
        };
        assert!(error.diagnostics().iter().any(|diagnostic| {
            diagnostic.code.as_deref() == Some("hir::removed_service_call")
                && diagnostic.span.is_some()
        }));
    }
}

#[test]
fn ordinary_trait_implementation_still_compiles_and_runs() {
    let engine = Engine::builder().build().expect("engine");
    let program = engine
        .compile_source(
            "trait Calculator { fn value(self) -> i64; }\n\
             struct CalculatorImpl {}\n\
             impl Calculator for CalculatorImpl { fn value(self) -> i64 { return 42; } }\n\
             pub fn main() { return CalculatorImpl {}.value(); }",
        )
        .expect("ordinary trait implementation");
    let mut runtime = Runtime::new(engine, program).expect("runtime");
    let value = runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::new(1_000, 1 << 20, 64),
        )
        .expect("ordinary trait method");
    assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::i64(42)));
}

#[test]
fn removed_declaration_reload_does_not_replace_active_code() {
    let engine = Engine::builder().build().expect("engine");
    let program = engine
        .compile_source("pub fn main() { return 42; }")
        .expect("initial source");
    let mut runtime = Runtime::builder(engine, program)
        .expect("runtime image")
        .with_hot_reload()
        .expect("reload owner")
        .build()
        .expect("runtime");

    runtime
        .stage_reload(
            "#[service_impl(app::calculator)] impl Patch { fn value() { return 1; } }\n\
             pub fn main() { return 0; }",
        )
        .expect_err("removed declaration cannot be staged");
    assert!(runtime.activate_reload().expect("safe point").is_none());
    let value = runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::new(1_000, 1 << 20, 64),
        )
        .expect("active code remains callable");
    assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::i64(42)));
}

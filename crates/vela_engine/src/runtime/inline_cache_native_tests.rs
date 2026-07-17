use std::sync::{Arc, Barrier};

use vela_bytecode::{CacheSiteId, CacheSiteKind};
use vela_common::{ScalarValue, SourceId};
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;
use crate::native::{NativeFunctionDesc, NativeFunctionId, TypeHint};
use crate::runtime::{CallArgs, CallOptions, Runtime};

#[test]
fn accepted_hot_reload_clears_native_call_inline_caches() {
    let native_id = NativeFunctionId::new(91);
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::answer", native_id).returns(TypeHint::i64()),
            |_| Ok(OwnedValue::Scalar(ScalarValue::I64(41))),
        )
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(
            SourceId::new(1),
            r#"
fn main() {
    return game::answer();
}
"#,
        )
        .expect("initial source should compile");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    let initial_site = native_call_site(&runtime, "main");

    let first = runtime
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect("initial main should run");
    assert_eq!(
        runtime.value_to_owned(&first),
        Ok(OwnedValue::Scalar(ScalarValue::I64(41)))
    );
    assert_eq!(
        runtime
            .image
            .execution_data()
            .inline_caches()
            .native_call(initial_site)
            .expect("initial native call should populate inline cache")
            .native_id(),
        native_id
    );

    let update = runtime
        .compile_hot_reload_update_with_id(
            SourceId::new(2),
            r#"
fn main() {
    return game::answer() + 1;
}
"#,
        )
        .expect("runtime should compile native hot reload update")
        .expect("native call body update should be accepted");
    let report = runtime
        .apply_hot_update(update)
        .expect("native hot reload update should apply");
    assert!(report.accepted);

    let reloaded_site = native_call_site(&runtime, "main");
    assert!(
        runtime
            .image
            .execution_data()
            .inline_caches()
            .native_call(reloaded_site)
            .is_none()
    );

    let second = runtime
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect("reloaded main should run");
    assert_eq!(
        runtime.value_to_owned(&second),
        Ok(OwnedValue::Scalar(ScalarValue::I64(42)))
    );
    assert_eq!(
        runtime
            .image
            .execution_data()
            .inline_caches()
            .native_call(reloaded_site)
            .expect("reloaded native call should repopulate inline cache")
            .native_id(),
        native_id
    );
}

#[test]
fn concurrent_owned_runtimes_share_safe_first_native_cache_population() {
    const WORKERS: usize = 8;

    let native_id = NativeFunctionId::new(92);
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::shared_answer", native_id).returns(TypeHint::i64()),
            |_| Ok(OwnedValue::Scalar(ScalarValue::I64(42))),
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            "fn main() { return game::shared_answer(); }",
        )
        .expect("source should compile");
    let artifact = engine
        .link_compiled_program(program)
        .expect("program should link");
    let mut runtimes = (0..WORKERS)
        .map(|_| {
            Runtime::from_linked_artifact(engine.clone(), Arc::clone(&artifact))
                .expect("owned runtime should initialize")
        })
        .collect::<Vec<_>>();
    let site = native_call_site(&runtimes[0], "main");
    assert!(runtimes.iter().all(|runtime| Arc::ptr_eq(
        runtime.image.execution_data(),
        runtimes[0].image.execution_data()
    )));
    let barrier = Arc::new(Barrier::new(WORKERS));

    let workers = runtimes
        .drain(..)
        .map(|mut runtime| {
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                let value = runtime
                    .call("main", CallArgs::new(), CallOptions::unbounded())
                    .expect("concurrent native call should execute");
                assert_eq!(
                    runtime.value_to_owned(&value),
                    Ok(OwnedValue::Scalar(ScalarValue::I64(42)))
                );
                runtime
            })
        })
        .collect::<Vec<_>>();
    let runtimes = workers
        .into_iter()
        .map(|worker| worker.join().expect("worker should not panic"))
        .collect::<Vec<_>>();

    assert!(runtimes.iter().all(|runtime| {
        runtime
            .image
            .execution_data()
            .inline_caches()
            .native_call(site)
            .is_some_and(|entry| entry.native_id() == native_id)
    }));
}

fn native_call_site(runtime: &Runtime, function_name: &str) -> CacheSiteId {
    runtime
        .image
        .program_image()
        .function_by_name(function_name)
        .unwrap_or_else(|| panic!("{function_name} should exist"))
        .cache_sites
        .sites()
        .iter()
        .find(|site| site.kind == CacheSiteKind::NativeCall)
        .unwrap_or_else(|| panic!("{function_name} should have a native call site"))
        .id
}

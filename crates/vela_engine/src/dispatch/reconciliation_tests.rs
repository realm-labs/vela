use super::*;

#[export(path = "host::game::panic_async")]
pub async fn panic_async(_value: i64) -> VmResult<i64> {
    panic!("intentional replacement unwind proof")
}

#[test]
fn pending_actors_overlap_and_keep_vela_state_isolated() {
    let slots = vec![vela_replaceable_slot_replaceable_outer_async()];
    let engine = crate::engine::Engine::builder()
        .register_exports(vela_export_bundle_pause_once())
        .register_replaceable_slots(slots.clone())
        .build()
        .expect("engine");
    let program = engine
        .compile_source(
            r#"
state calls: i64 = 0;

#[override(host::game::outer_async)]
pub async fn outer(value: i64) -> i64 {
    let value = host::game::pause_once(value).await;
    calls += 1;
    return calls;
}
"#,
        )
        .expect("stateful async override program");
    let staging_runtime = dispatch_runtime(engine, program);
    let controller = DispatchController::new(slots).expect("controller");
    let candidate = controller
        .stage_current(&staging_runtime)
        .expect("override stage");
    controller.activate(candidate).expect("activate candidate");
    let mut first = ActorContext::for_turn(DispatchRoot::pin(&controller), &staging_runtime);
    let mut second = ActorContext::for_turn(DispatchRoot::pin(&controller), &staging_runtime);

    let first_result = {
        let mut first_call = std::pin::pin!(replaceable_outer_async(&mut first, 0));
        let mut task = Context::from_waker(Waker::noop());
        assert!(matches!(first_call.as_mut().poll(&mut task), Poll::Pending));
        assert_eq!(
            ready(replaceable_outer_async(&mut second, 0)),
            Ok(1),
            "one pending Actor must not block another Actor using the same override generation"
        );
        loop {
            if let Poll::Ready(result) = first_call.as_mut().poll(&mut task) {
                break result;
            }
        }
    };
    assert_eq!(first_result, Ok(1));
    assert_eq!(
        ready(replaceable_outer_async(&mut second, 0)),
        Ok(2),
        "each Actor Runtime must retain its own persistent Vela state"
    );
}

#[test]
fn panic_and_unpolled_drop_release_actor_turn_authority() {
    let slots = vec![vela_replaceable_slot_replaceable_outer_async()];
    let engine = crate::engine::Engine::builder()
        .register_exports(vela_export_bundle_panic_async())
        .register_replaceable_slots(slots.clone())
        .build()
        .expect("engine");
    let program = engine
        .compile_source(
            r#"
#[override(host::game::outer_async)]
pub async fn outer(value: i64) -> i64 {
    return host::game::panic_async(value).await;
}

pub fn healthy() -> i64 { return 7; }
"#,
        )
        .expect("panicking async override program");
    let staging_runtime = dispatch_runtime(engine, program);
    let controller = DispatchController::new(slots).expect("controller");
    let candidate = controller
        .stage_current(&staging_runtime)
        .expect("override stage");
    controller.activate(candidate).expect("activate candidate");
    let mut actor = ActorContext::for_turn(DispatchRoot::pin(&controller), &staging_runtime);

    drop(replaceable_outer_async(&mut actor, 0));

    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ready(replaceable_outer_async(&mut actor, 0))
    }));
    assert!(
        unwind.is_err(),
        "the native panic must cross the scoped call future"
    );

    let healthy = actor
        .runtime
        .call("healthy", CallArgs::new(), CallOptions::unbounded())
        .expect("Runtime remains callable after unwind");
    let healthy = actor
        .runtime
        .value_to_owned(&healthy)
        .expect("healthy result materializes");
    assert_eq!(i64::from_script_arg(&healthy), Ok(7));
}

use super::*;

#[test]
fn explicit_trait_impl_exports_install_ufcs_method_thunks() {
    let mut runtime = host_export_runtime(
        "fn main(player: Player) { player.take_damage(3); return player.is_alive(); }",
    );
    let mut player = Player { level: 5 };

    let result = runtime
        .call(
            "main",
            CallArgs::new().with_host_mut("player", &mut player),
            CallOptions::unbounded(),
        )
        .expect("explicit trait implementation exports should execute");

    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
    assert_eq!(player.level, 2);
}

#[test]
fn declaration_only_external_trait_adapter_calls_existing_impl() {
    let engine = Engine::builder()
        .capability(Capability::HostRead)
        .capability(Capability::HostWrite)
        .register_type::<ExternalNpc>()
        .register_exports(ExternalNpc::vela_inherent_exports())
        .register_exports(VelaExternalExternalNpcExternalDamageExports::vela_exports())
        .build()
        .expect("declaration-only adapter should register");
    let program = engine
        .compile_source(
            "fn main(npc: Npc) { npc.hit(3); return npc.active() && npc.current_hp() == 2; }",
        )
        .expect("external trait methods should compile as ordinary methods");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let mut npc = ExternalNpc { hp: 5 };

    let result = runtime
        .call(
            "main",
            CallArgs::new().with_host_mut("npc", &mut npc),
            CallOptions::unbounded(),
        )
        .expect("generated UFCS thunks should call the existing trait impl");

    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
    assert_eq!(npc.hp, 2);
}

#[test]
fn value_only_async_export_uses_ordinary_await_syntax() {
    let mut runtime =
        host_export_runtime("async fn main() { return game::double_async(6).await; }");
    let mut future =
        Box::pin(runtime.call_async("main", CallArgs::new(), CallOptions::unbounded()));
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    let value = loop {
        match std::future::Future::poll(future.as_mut(), &mut context) {
            std::task::Poll::Ready(value) => break value.expect("async export should complete"),
            std::task::Poll::Pending => continue,
        }
    };
    drop(future);

    assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::i64(12)));
}

#[test]
fn borrowed_return_requires_explicit_release_before_await() {
    let mut runtime = host_export_runtime(
        "async fn implicit(service: PlayerService) { let player = service.player_mut(); player.increment(2); game::double_async(3).await; return game::touch_service(service); } \
         async fn explicit(service: PlayerService) { let player = service.player_mut(); player.increment(2); host::release(player); game::double_async(3).await; return game::touch_service(service); }",
    );
    let mut service = PlayerService {
        player: Player { level: 5 },
        touches: 0,
    };
    let mut future = Box::pin(runtime.call_async(
        "implicit",
        CallArgs::new().with_host_mut("service", &mut service),
        CallOptions::unbounded(),
    ));
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    let error = loop {
        match std::future::Future::poll(future.as_mut(), &mut context) {
            std::task::Poll::Ready(value) => {
                break value.expect_err("an unreleased child must block suspension");
            }
            std::task::Poll::Pending => continue,
        }
    };
    drop(future);
    assert!(matches!(
        error.kind(),
        VmErrorKind::Host(vela_host::error::HostErrorKind::UnreleasedScopedResourcesAtAwait { .. })
    ));
    assert!(
        error
            .to_diagnostic()
            .message
            .contains("host::release(value)")
    );

    let mut future = Box::pin(runtime.call_async(
        "explicit",
        CallArgs::new().with_host_mut("service", &mut service),
        CallOptions::unbounded(),
    ));
    let value = loop {
        match std::future::Future::poll(future.as_mut(), &mut context) {
            std::task::Poll::Ready(value) => {
                break value.expect("explicit release should permit suspension");
            }
            std::task::Poll::Pending => continue,
        }
    };
    drop(future);

    assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::i64(1)));
    assert_eq!(service.player.level, 9);
    assert_eq!(service.touches, 1);
}

#[test]
fn borrowed_return_cannot_cross_async_suspend() {
    let mut runtime = host_export_runtime(
        "async fn main(service: PlayerService, other: Player) { let player = service.player_mut(); game::transfer_async(player, other, 2).await; return game::touch_service(service); }",
    );
    let mut service = PlayerService {
        player: Player { level: 5 },
        touches: 0,
    };
    let mut other = Player { level: 3 };
    let mut future = Box::pin(
        runtime.call_async(
            "main",
            CallArgs::new()
                .with_host_mut("service", &mut service)
                .with_host_mut("other", &mut other),
            CallOptions::unbounded(),
        ),
    );
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    let error = loop {
        match std::future::Future::poll(future.as_mut(), &mut context) {
            std::task::Poll::Ready(value) => {
                break value.expect_err("a live scoped child cannot cross suspension");
            }
            std::task::Poll::Pending => continue,
        }
    };
    drop(future);

    assert!(matches!(
        error.kind(),
        VmErrorKind::Host(vela_host::error::HostErrorKind::UnreleasedScopedResourcesAtAwait { .. })
    ));
    assert_eq!(service.player.level, 5);
    assert_eq!(other.level, 3);
    assert_eq!(service.touches, 0);
}

#[test]
fn dead_scoped_locals_and_ready_or_pending_targets_obey_the_same_await_gate() {
    let mut runtime = host_export_runtime(
        "async fn dead(service: PlayerService) { { let player = service.player_mut(); player.increment(1); } game::double_async(1).await; } \
         async fn pending(service: PlayerService, other: Player) { let player = service.player_mut(); player.increment(1); game::hold_player_async(other).await; }",
    );
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);

    let mut service = PlayerService {
        player: Player { level: 5 },
        touches: 0,
    };
    let mut dead = Box::pin(runtime.call_async(
        "dead",
        CallArgs::new().with_host_mut("service", &mut service),
        CallOptions::unbounded(),
    ));
    let dead_error = match std::future::Future::poll(dead.as_mut(), &mut context) {
        std::task::Poll::Ready(value) => {
            value.expect_err("a dead local must remain in the active resource table")
        }
        std::task::Poll::Pending => panic!("await gate must run before polling a ready target"),
    };
    drop(dead);
    assert!(matches!(
        dead_error.kind(),
        VmErrorKind::Host(vela_host::error::HostErrorKind::UnreleasedScopedResourcesAtAwait { .. })
    ));

    let mut other = Player { level: 3 };
    let mut pending = Box::pin(
        runtime.call_async(
            "pending",
            CallArgs::new()
                .with_host_mut("service", &mut service)
                .with_host_mut("other", &mut other),
            CallOptions::unbounded(),
        ),
    );
    let pending_error = match std::future::Future::poll(pending.as_mut(), &mut context) {
        std::task::Poll::Ready(value) => {
            value.expect_err("the await gate must reject before polling a pending target")
        }
        std::task::Poll::Pending => panic!("pending target must not start with a live resource"),
    };
    drop(pending);
    assert!(matches!(
        pending_error.kind(),
        VmErrorKind::Host(vela_host::error::HostErrorKind::UnreleasedScopedResourcesAtAwait { .. })
    ));
    assert_eq!(service.player.level, 7);
    assert_eq!(other.level, 3);
}

#[test]
fn async_host_function_exports_hold_all_leases_to_completion() {
    let mut runtime = host_export_runtime(
        "async fn main(first: Player, second: Player) { return game::transfer_async(first, second, 3).await; }",
    );
    let mut first = Player { level: 10 };
    let mut second = Player { level: 4 };
    let mut future = Box::pin(
        runtime.call_async(
            "main",
            CallArgs::new()
                .with_host_mut("first", &mut first)
                .with_host_mut("second", &mut second),
            CallOptions::unbounded(),
        ),
    );
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    let value = loop {
        match std::future::Future::poll(future.as_mut(), &mut context) {
            std::task::Poll::Ready(value) => {
                break value.expect("async host export should complete");
            }
            std::task::Poll::Pending => continue,
        }
    };
    drop(future);

    assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::i64(14)));
    assert_eq!((first.level, second.level), (7, 7));
}

#[test]
fn async_host_function_exports_preflight_aliases() {
    let mut runtime = host_export_runtime(
        "async fn main(player: Player) { return game::transfer_async(player, player, 3).await; }",
    );
    let mut player = Player { level: 10 };
    let mut future = Box::pin(runtime.call_async(
        "main",
        CallArgs::new().with_host_mut("player", &mut player),
        CallOptions::unbounded(),
    ));
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    let error = loop {
        match std::future::Future::poll(future.as_mut(), &mut context) {
            std::task::Poll::Ready(result) => {
                break result
                    .expect_err("aliased async mutable parameters must fail before invocation");
            }
            std::task::Poll::Pending => continue,
        }
    };
    drop(future);

    assert_eq!(
        error.kind(),
        VmErrorKind::AliasedMutableHostArguments {
            callable: "game::transfer_async".to_owned(),
            first_parameter: "first".to_owned(),
            second_parameter: "second".to_owned(),
        }
    );
    assert_eq!(player.level, 10);
}

#[test]
fn dropping_async_host_function_releases_retained_lease() {
    let mut runtime = host_export_runtime(
        "async fn main(player: Player) { game::hold_player_async(player).await; } fn after(player: Player) { game::grant_exp(player, 1); return player.current_level(); }",
    );
    let mut player = Player { level: 3 };
    let mut future = Box::pin(runtime.call_async(
        "main",
        CallArgs::new().with_host_mut("player", &mut player),
        CallOptions::unbounded(),
    ));
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    assert!(matches!(
        std::future::Future::poll(future.as_mut(), &mut context),
        std::task::Poll::Pending
    ));
    drop(future);

    let value = runtime
        .call(
            "after",
            CallArgs::new().with_host_mut("player", &mut player),
            CallOptions::unbounded(),
        )
        .expect("dropping the future must release all retained host leases");
    assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::i64(4)));
    assert_eq!(player.level, 4);
}

#[test]
fn async_method_exports_hold_receiver_and_parameter_leases_to_completion() {
    let mut runtime = host_export_runtime(
        "async fn main(first: Player, second: Player) { first.increment_async(2).await; return first.absorb_async(second).await; }",
    );
    let mut first = Player { level: 3 };
    let mut second = Player { level: 4 };
    let mut future = Box::pin(
        runtime.call_async(
            "main",
            CallArgs::new()
                .with_host_mut("first", &mut first)
                .with_host_mut("second", &mut second),
            CallOptions::unbounded(),
        ),
    );
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    let value = loop {
        match std::future::Future::poll(future.as_mut(), &mut context) {
            std::task::Poll::Ready(value) => {
                break value.expect("async method exports should complete");
            }
            std::task::Poll::Pending => continue,
        }
    };
    drop(future);

    assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::i64(9)));
    assert_eq!((first.level, second.level), (9, 0));
}

#[test]
fn dropping_async_method_call_releases_retained_receiver_lease() {
    let mut runtime = host_export_runtime(
        "async fn main(player: Player) { player.hold_async().await; } fn after(player: Player) { player.increment(1); return player.current_level(); }",
    );
    let mut player = Player { level: 3 };
    let mut future = Box::pin(runtime.call_async(
        "main",
        CallArgs::new().with_host_mut("player", &mut player),
        CallOptions::unbounded(),
    ));
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    assert!(matches!(
        std::future::Future::poll(future.as_mut(), &mut context),
        std::task::Poll::Pending
    ));
    drop(future);

    let value = runtime
        .call(
            "after",
            CallArgs::new().with_host_mut("player", &mut player),
            CallOptions::unbounded(),
        )
        .expect("dropping the future must release the retained receiver lease");
    assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::i64(4)));
    assert_eq!(player.level, 4);
}

#[test]
fn async_context_method_retains_receiver_lease_and_runtime_authority() {
    let mut runtime = host_export_runtime(
        "async fn main(player: Player) { return player.context_increment_async(3).await; }",
    );
    let mut player = Player { level: 4 };
    let mut future = Box::pin(runtime.call_async(
        "main",
        CallArgs::new().with_host_mut("player", &mut player),
        CallOptions::unbounded(),
    ));
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    let value = loop {
        match std::future::Future::poll(future.as_mut(), &mut context) {
            std::task::Poll::Ready(value) => {
                break value.expect("async context method should complete");
            }
            std::task::Poll::Pending => continue,
        }
    };
    drop(future);

    assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::i64(7)));
    assert_eq!(player.level, 7);
}

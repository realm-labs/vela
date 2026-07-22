use super::*;
use vela_bytecode::compiler::CompiledProgram;
use vela_vm::budget::ExecutionBudget;
use vela_vm::owned_value::OwnedValue;

fn run_future<F: std::future::Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        if let std::task::Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
    }
}

fn run_linked_program_with_host(
    engine: &Engine,
    program: CompiledProgram,
    args: &[OwnedValue],
    host: &mut HostExecution<'_>,
) -> VmResult<OwnedValue> {
    let vm = engine.into_vm_for_program(program.bytecode());
    let linked = engine
        .link_compiled_program(program)
        .expect("script method metadata program should link");
    let mut budget = ExecutionBudget::unbounded();
    vm.run_linked_program_with_host_budget_and_caches(
        &linked,
        "main",
        args,
        host,
        &mut budget,
        None,
    )
}

#[test]
fn script_macros_feed_engine_builder_registration() {
    let desc =
        <Player as vela_engine::schema::ScriptHostMethodMetadata>::script_host_method_descs()
            .into_iter()
            .find(|desc| desc.id == method_id("grant_exp"))
            .expect("method descriptor");
    let engine = Engine::builder()
        .register_host_type::<Player>()
        .capability(Capability::HostWrite)
        .register_native_method_fn(desc, |_, _, _| Ok(OwnedValue::Unit))
        .build()
        .expect("engine should build from macro metadata");

    let registry = engine.registry();
    let player = registry.type_by_name("Player").expect("registered player");
    assert_eq!(player.fields.len(), 1);
    assert_eq!(player.methods.len(), 1);
    assert_eq!(player.methods[0].name, "grant_exp");
    assert!(player.methods[0].effects.writes_host);
    assert!(player.methods[0].access.required_permissions().is_empty());
}

#[test]
fn script_methods_generate_callable_native_registration() {
    let engine = Player::vela_register_native_method_fns(
        Engine::builder()
            .register_host_type::<Player>()
            .capability(Capability::HostRead)
            .capability(Capability::HostWrite),
    )
    .build()
    .expect("engine should build from macro callable methods");
    let player = HostRef::new(Player::vela_host_type_id(), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        state_values: None,
    };

    assert_eq!(
        engine.call_native_method(
            method_id("grant_score"),
            &HostPath::new(player),
            &[OwnedValue::Scalar(vela_common::ScalarValue::I64(13))],
            &mut host,
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(13))),
    );
}

#[test]
fn script_methods_feed_stable_engine_registration_api() {
    let generated_schema = Player::vela_host_type_desc();
    let generated_methods = Player::vela_native_method_descs();
    let engine = Engine::builder()
        .register_script_host::<Player>()
        .capability(Capability::HostWrite)
        .build()
        .expect("engine should build from macro host methods");
    let registry = engine.registry();
    let player_type = registry
        .type_by_name("Player")
        .expect("registered player type");
    assert_eq!(player_type.key, generated_schema.key);
    assert_eq!(player_type.kind, generated_schema.kind);
    assert_eq!(player_type.schema_hash, generated_schema.schema_hash);
    assert_eq!(player_type.host_type_id, generated_schema.host_type_id);
    assert_eq!(player_type.fields, generated_schema.fields);
    assert_eq!(player_type.attrs, generated_schema.attrs);
    assert_eq!(player_type.methods.len(), generated_methods.len());
    for (registered, generated) in player_type.methods.iter().zip(generated_methods.iter()) {
        assert_registered_method_matches_native_desc(registered, generated);
    }
    assert_eq!(player_type.methods[0].name, "grant_exp");
    assert_eq!(player_type.methods[3].name, "sum_score");
    assert_eq!(player_type.methods[4].name, "sum6_score");
    let type_bindings = engine.type_bindings();
    let binding = type_bindings
        .get_for::<Player>()
        .expect("generated host methods should compose into one TypeBinding");
    assert_eq!(binding.key, generated_schema.key);
    assert_eq!(
        registry
            .type_binding_for_key(&generated_schema.key)
            .expect("reflection should use the same sealed binding"),
        binding
    );

    let player = HostRef::new(Player::vela_host_type_id(), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        state_values: None,
    };

    assert_eq!(
        engine.call_native_method(
            method_id("sum_score"),
            &HostPath::new(player),
            &[
                OwnedValue::Scalar(vela_common::ScalarValue::I64(1)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(3)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(4)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(5)),
            ],
            &mut host,
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(15))),
    );

    assert_eq!(
        engine.call_native_method(
            method_id("sum6_score"),
            &HostPath::new(player),
            &[
                OwnedValue::Scalar(vela_common::ScalarValue::I64(1)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(3)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(4)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(5)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(6)),
            ],
            &mut host,
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(21))),
    );
}

#[test]
fn script_methods_register_async_shared_and_mutable_direct_receivers() {
    let engine = Engine::builder()
        .register_script_host::<DirectCounter>()
        .register_script_host::<DirectPeer>()
        .register_script_host::<DirectConfig>()
        .capability(Capability::HostRead)
        .capability(Capability::HostWrite)
        .reflection_permissions(vela_reflect::permissions::ReflectPermissionSet::all())
        .build()
        .expect("engine should register direct async methods");
    assert!(engine.type_bindings().get_for::<DirectCounter>().is_some());
    assert!(engine.type_bindings().get_for::<DirectPeer>().is_some());
    assert!(engine.type_bindings().get_for::<DirectConfig>().is_some());
    let program = engine
        .compile_source(
            r#"
async fn main(counter: DirectCounter) {
    counter.add_async(4).await;
    reflect::call(counter, "add_async", 2).await;
    return counter.read_async().await;
}

async fn wait(counter: DirectCounter) {
    return counter.wait_with_context().await;
}

async fn panic_context(counter: DirectCounter) {
    return counter.panic_with_context().await;
}

async fn read_async_entry(counter: DirectCounter) {
    return counter.read_async().await;
}

async fn shared_alias(counter: DirectCounter) {
    return counter.read_shared_alias(counter, counter).await;
}

async fn wait_shared(counter: DirectCounter) {
    return counter.wait_shared().await;
}

fn read(counter: DirectCounter) {
    return counter.total;
}

fn hook(counter: DirectCounter) {
    counter.total += 10;
}

async fn reenter(counter: DirectCounter) {
    return counter.add_with_hook(counter, 4).await;
}

fn raw_read(counter: DirectCounter) { return counter.total; }

async fn typed_leases(
    counter: DirectCounter,
    peer: DirectPeer,
    config: DirectConfig,
) {
    return counter.update_with(peer, config, 4).await;
}

async fn alias_conflict(counter: DirectCounter) {
    return counter.merge(counter).await;
}
"#,
        )
        .expect("direct async method source should compile");
    let mut runtime =
        vela_engine::runtime::Runtime::new(engine, program).expect("runtime should initialize");
    let mut counter = DirectCounter { total: 3 };
    let mut reentry_counter = DirectCounter { total: 3 };

    let mut shared_alias_counter = DirectCounter { total: 6 };
    let shared_alias = run_future(runtime.call_async(
        "shared_alias",
        vela_engine::runtime::CallArgs::new().with_host_mut("counter", &mut shared_alias_counter),
        vela_engine::runtime::CallOptions::unbounded(),
    ))
    .expect("mutable-origin shared aliases should coexist across pending reentry");
    assert_eq!(
        runtime.value_to_owned(&shared_alias),
        Ok(OwnedValue::i64(12))
    );
    let exclusive_after_shared = run_future(runtime.call_async(
        "main",
        vela_engine::runtime::CallArgs::new().with_host_mut("counter", &mut shared_alias_counter),
        vela_engine::runtime::CallOptions::unbounded(),
    ))
    .expect("completed shared aliases should restore exclusive access");
    assert_eq!(
        runtime.value_to_owned(&exclusive_after_shared),
        Ok(OwnedValue::i64(12))
    );

    let mut cancelled_shared = std::boxed::Box::pin(runtime.call_async(
        "wait_shared",
        vela_engine::runtime::CallArgs::new().with_host_mut("counter", &mut shared_alias_counter),
        vela_engine::runtime::CallOptions::unbounded(),
    ));
    let mut shared_context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(matches!(
        cancelled_shared.as_mut().poll(&mut shared_context),
        std::task::Poll::Pending
    ));
    drop(cancelled_shared);
    let read_after_shared_cancel = runtime
        .call(
            "read",
            vela_engine::runtime::CallArgs::new()
                .with_host_mut("counter", &mut shared_alias_counter),
            vela_engine::runtime::CallOptions::unbounded(),
        )
        .expect("cancelling shared leases should restore the available state");
    assert_eq!(
        runtime.value_to_owned(&read_after_shared_cancel),
        Ok(OwnedValue::i64(12))
    );

    let reentered = run_future(runtime.call_async(
        "reenter",
        vela_engine::runtime::CallArgs::new().with_host_mut("counter", &mut reentry_counter),
        vela_engine::runtime::CallOptions::unbounded(),
    ))
    .expect("direct async method should reborrow its receiver into child Vela");
    assert_eq!(runtime.value_to_owned(&reentered), Ok(OwnedValue::i64(18)));
    assert_eq!(reentry_counter.total, 18);

    let mut lease_counter = DirectCounter { total: 1 };
    let mut peer = DirectPeer { total: 2 };
    let config = DirectConfig { bonus: 3 };
    let leased = run_future(
        runtime.call_async(
            "typed_leases",
            vela_engine::runtime::CallArgs::new()
                .with_host_mut("counter", &mut lease_counter)
                .with_host_mut("peer", &mut peer)
                .with_host_ref("config", &config),
            vela_engine::runtime::CallOptions::unbounded(),
        ),
    )
    .expect("typed host reference parameters should acquire atomic leases");
    assert_eq!(runtime.value_to_owned(&leased), Ok(OwnedValue::i64(10)));
    assert_eq!(lease_counter.total, 10);
    assert_eq!(peer.total, 6);

    let mut aliased = DirectCounter { total: 5 };
    let alias_result = run_future(runtime.call_async(
        "alias_conflict",
        vela_engine::runtime::CallArgs::new().with_host_mut("counter", &mut aliased),
        vela_engine::runtime::CallOptions::unbounded(),
    ));
    let alias_error = alias_result.expect_err("aliased exclusive leases should conflict");
    assert!(matches!(
        alias_error.kind(),
        vela_vm::error::VmErrorKind::Host(vela_host::error::HostErrorKind::HostObjectBusy { .. })
    ));
    let alias_read = runtime
        .call(
            "read",
            vela_engine::runtime::CallArgs::new().with_host_ref("counter", &aliased),
            vela_engine::runtime::CallOptions::unbounded(),
        )
        .expect("an atomic lease conflict should restore the receiver binding");
    assert_eq!(runtime.value_to_owned(&alias_read), Ok(OwnedValue::i64(5)));

    let result = run_future(runtime.call_async(
        "main",
        vela_engine::runtime::CallArgs::new().with_host_mut("counter", &mut counter),
        vela_engine::runtime::CallOptions::unbounded(),
    ))
    .expect("direct async methods should execute");

    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(9)));
    assert_eq!(counter.total, 9);

    let result = run_future(runtime.call_async(
        "read_async_entry",
        vela_engine::runtime::CallArgs::new().with_host_ref("counter", &counter),
        vela_engine::runtime::CallOptions::unbounded(),
    ))
    .expect("shared direct host binding should support a shared async lease");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(9)));

    let mut cancelled = std::boxed::Box::pin(runtime.call_async(
        "wait",
        vela_engine::runtime::CallArgs::new().with_host_mut("counter", &mut counter),
        vela_engine::runtime::CallOptions::unbounded(),
    ));
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(matches!(
        cancelled.as_mut().poll(&mut context),
        std::task::Poll::Pending
    ));
    drop(cancelled);

    let mut panicking = std::boxed::Box::pin(runtime.call_async(
        "panic_context",
        vela_engine::runtime::CallArgs::new().with_host_mut("counter", &mut counter),
        vela_engine::runtime::CallOptions::unbounded(),
    ));
    let panic_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = panicking.as_mut().poll(&mut context);
    }));
    assert!(panic_result.is_err());
    drop(panicking);

    let result = runtime
        .call(
            "read",
            vela_engine::runtime::CallArgs::new().with_host_ref("counter", &counter),
            vela_engine::runtime::CallOptions::unbounded(),
        )
        .expect("dropping a pending direct method should release Runtime and its lease");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(9)));
}

fn assert_registered_method_matches_native_desc(
    registered: &vela_reflect::registry::MethodDesc,
    generated: &NativeMethodDesc,
) {
    assert_eq!(registered.id, generated.id);
    assert_eq!(registered.name, generated.name);
    assert_eq!(
        registered.return_type.as_deref(),
        Some(type_hint_name(&generated.returns).as_str())
    );
    assert_eq!(
        registered.effects.reads_host,
        generated.effects.reads_host()
    );
    assert_eq!(
        registered.effects.writes_host,
        generated.effects.writes_host()
    );
    assert_eq!(
        registered.effects.emits_events,
        generated.effects.emits_events()
    );
    assert_eq!(registered.access.public, generated.access.public);
    assert_eq!(
        registered.access.reflect_callable,
        generated.access.reflect_callable
    );
    assert_eq!(registered.docs, generated.docs);
    assert_eq!(registered.attrs, generated.attrs);
    assert_eq!(registered.source_span, generated.source_span);
    assert_eq!(registered.params.len(), generated.params.len());
    for (registered_param, generated_param) in registered.params.iter().zip(generated.params.iter())
    {
        assert_eq!(registered_param.name, generated_param.name);
        assert_eq!(
            registered_param.type_hint.as_deref(),
            Some(type_hint_name(&generated_param.hint).as_str())
        );
        assert!(!registered_param.has_default);
    }
}

fn type_hint_name(hint: &TypeHint) -> String {
    match hint {
        TypeHint::Any => "Any".to_owned(),
        TypeHint::Primitive(tag) => tag.name().to_owned(),
        TypeHint::Array => "Array".to_owned(),
        TypeHint::ArrayOf(element) => format!("Array<{}>", type_hint_name(element)),
        TypeHint::Map => "Map".to_owned(),
        TypeHint::MapOf { key, value } => {
            format!("Map<{}, {}>", type_hint_name(key), type_hint_name(value))
        }
        TypeHint::Set => "Set".to_owned(),
        TypeHint::SetOf(element) => format!("Set<{}>", type_hint_name(element)),
        TypeHint::TupleOf(elements) => format!(
            "({})",
            elements
                .iter()
                .map(type_hint_name)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeHint::PathProxy => "path_proxy".to_owned(),
        TypeHint::Record(key) | TypeHint::Enum(key) | TypeHint::Host(key) => key.name.clone(),
        TypeHint::Trait(name) => name.clone(),
        TypeHint::Function => "Function".to_owned(),
        TypeHint::Iterator => "Iterator".to_owned(),
        TypeHint::IteratorOf(item) => format!("Iterator<{}>", type_hint_name(item)),
        TypeHint::OptionOf(payload) => format!("Option<{}>", type_hint_name(payload)),
        TypeHint::ResultOf { ok, err } => {
            format!("Result<{}, {}>", type_hint_name(ok), type_hint_name(err))
        }
    }
}

#[test]
fn script_methods_generate_callable_result_native_registration() {
    let engine = Player::vela_register_native_method_fns(
        Engine::builder()
            .register_host_type::<Player>()
            .capability(Capability::HostRead)
            .capability(Capability::HostWrite),
    )
    .build()
    .expect("engine should build from macro callable methods");
    let player = HostRef::new(Player::vela_host_type_id(), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        state_values: None,
    };

    assert_eq!(
        engine.call_native_method(
            method_id("checked_preview"),
            &HostPath::new(player),
            &[OwnedValue::Bool(true)],
            &mut host,
        ),
        Ok(OwnedValue::Enum {
            enum_name: "Result".to_owned(),
            variant: "Ok".to_owned(),
            fields: [(
                "0".to_owned(),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(17))
            )]
            .into(),
        }),
    );
    assert_eq!(
        engine.call_native_method(
            method_id("checked_preview"),
            &HostPath::new(player),
            &[OwnedValue::Bool(false)],
            &mut host,
        ),
        Ok(OwnedValue::Enum {
            enum_name: "Result".to_owned(),
            variant: "Err".to_owned(),
            fields: [("0".to_owned(), OwnedValue::String("blocked".to_owned()))].into(),
        }),
    );
}

#[test]
fn script_methods_generate_callable_option_native_registration() {
    let engine = Player::vela_register_native_method_fns(
        Engine::builder()
            .register_host_type::<Player>()
            .capability(Capability::HostRead)
            .capability(Capability::HostWrite),
    )
    .build()
    .expect("engine should build from macro callable methods");
    let player = HostRef::new(Player::vela_host_type_id(), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        state_values: None,
    };

    assert_eq!(
        engine.call_native_method(
            method_id("preview_bonus"),
            &HostPath::new(player),
            &[OwnedValue::Enum {
                enum_name: "Option".to_owned(),
                variant: "None".to_owned(),
                fields: [].into(),
            }],
            &mut host,
        ),
        Ok(OwnedValue::Enum {
            enum_name: "Option".to_owned(),
            variant: "None".to_owned(),
            fields: [].into(),
        }),
    );
    assert_eq!(
        engine.call_native_method(
            method_id("preview_bonus"),
            &HostPath::new(player),
            &[OwnedValue::Enum {
                enum_name: "Option".to_owned(),
                variant: "Some".to_owned(),
                fields: [(
                    "0".to_owned(),
                    OwnedValue::Scalar(vela_common::ScalarValue::I64(4))
                )]
                .into(),
            }],
            &mut host,
        ),
        Ok(OwnedValue::Enum {
            enum_name: "Option".to_owned(),
            variant: "Some".to_owned(),
            fields: [(
                "0".to_owned(),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(5))
            )]
            .into(),
        }),
    );
}

#[test]
fn script_method_metadata_compiles_to_host_access_calls() {
    let engine = Engine::builder()
        .register_host_type::<Player>()
        .register_host_method_metadata::<Player>()
        .build()
        .expect("engine should build from macro metadata");
    let program = compile_source!(
        engine,
        r#"
fn main(player: Player) {
    player.grant_exp(5);
    return 1;
}
"#,
        "compile source"
    );
    let player = HostRef::new(Player::vela_host_type_id(), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        state_values: None,
    };

    assert_eq!(
        run_linked_program_with_host(&engine, program, &[OwnedValue::HostRef(player)], &mut host),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1))),
    );
}

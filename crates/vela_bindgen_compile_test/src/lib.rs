//! Compile-only proof that the official generator targets the public binding API.

mod model;

pub use model::Player;

include!(concat!(env!("OUT_DIR"), "/vela_bindings.rs"));

#[vela_macros::export(path = "test::reenter_player")]
pub fn reenter_player(
    context: &mut vela_engine::context::NativeCallContext<'_, '_>,
    player: &mut Player,
    amount: i64,
) -> vela_engine::binding::VmResult<i64> {
    let before = player.level;
    let mut package = vela_bindings::bind_active(context)?;
    let mut module = package.dev_vela_anonymous_root_module();
    let nested = module.raise(player, amount)?;
    player.level += 1;
    Ok(before + nested + player.level)
}

#[vela_macros::export(path = "test::reject_unrelated")]
pub fn reject_unrelated(
    context: &mut vela_engine::context::NativeCallContext<'_, '_>,
    _player: &mut Player,
) -> vela_engine::binding::VmResult<i64> {
    let mut unrelated = Player { level: 99 };
    let mut package = vela_bindings::bind_active(context)?;
    let mut module = package.dev_vela_anonymous_root_module();
    module.raise(&mut unrelated, 1)
}

#[vela_macros::export(path = "test::deny_effect_expansion")]
pub fn deny_effect_expansion(
    context: &mut vela_engine::context::NativeCallContext<'_, '_>,
) -> vela_engine::binding::VmResult<i64> {
    let mut package = vela_bindings::bind_active(context)?;
    let mut module = package.dev_vela_anonymous_root_module();
    module.random_value()
}

#[vela_macros::export(path = "test::deny_context_random")]
pub fn deny_context_random(
    context: &mut vela_engine::context::NativeCallContext<'_, '_>,
) -> vela_engine::binding::VmResult<i64> {
    context.require_capability(vela_engine::permission::Capability::Random)?;
    Ok(1)
}

pub fn call_generated_add(runtime: &mut vela_engine::runtime::Runtime) -> Result<i64, String> {
    let mut package = vela_bindings::bind(runtime).map_err(|error| error.to_string())?;
    let mut module = package.dev_vela_anonymous_root_module();
    module.add(20, 22).map_err(|error| error.to_string())
}

pub fn compile_generated_async_call(runtime: &mut vela_engine::runtime::Runtime) {
    let Ok(mut package) = vela_bindings::bind(runtime) else {
        return;
    };
    let mut module = package.dev_vela_anonymous_root_module();
    let _future = module.label(42);
}

pub async fn call_generated_label(
    runtime: &mut vela_engine::runtime::Runtime,
) -> Result<String, String> {
    let mut package = vela_bindings::bind(runtime).map_err(|error| error.to_string())?;
    let mut module = package.dev_vela_anonymous_root_module();
    module.label(42).await.map_err(|error| error.to_string())
}

pub type ConversionResults = (i64, i64, i64, Option<i64>, Result<i64, String>);

pub fn call_generated_conversions(
    runtime: &mut vela_engine::runtime::Runtime,
) -> Result<ConversionResults, String> {
    let mut package = vela_bindings::bind(runtime).map_err(|error| error.to_string())?;
    let mut module = package.dev_vela_anonymous_root_module();
    let text = module.text_len("vela").map_err(|error| error.to_string())?;
    let bytes = module
        .bytes_len(b"vela")
        .map_err(|error| error.to_string())?;
    let first = module
        .first(vec![7, 8])
        .map_err(|error| error.to_string())?;
    let maybe = module.maybe(Some(9)).map_err(|error| error.to_string())?;
    let outcome = module.outcome(Ok(10)).map_err(|error| error.to_string())?;
    Ok((text, bytes, first, maybe, outcome))
}

pub type ModelResults = (
    vela_bindings::types::Point,
    i64,
    vela_bindings::types::Choice,
);

pub fn call_generated_models(
    runtime: &mut vela_engine::runtime::Runtime,
) -> Result<ModelResults, String> {
    let mut package = vela_bindings::bind(runtime).map_err(|error| error.to_string())?;
    let mut module = package.dev_vela_anonymous_root_module();
    let point = module
        .shift(vela_bindings::types::Point { x: 1, y: 2 })
        .map_err(|error| error.to_string())?;
    let sum = module
        .sum(point.clone())
        .map_err(|error| error.to_string())?;
    let choice = module
        .echo_choice(vela_bindings::types::Choice::Named { value: 7 })
        .map_err(|error| error.to_string())?;
    Ok((point, sum, choice))
}

pub fn call_generated_active(
    context: &mut vela_engine::context::NativeCallContext<'_, '_>,
    left: i64,
    right: i64,
) -> vela_engine::binding::VmResult<i64> {
    let mut package = vela_bindings::bind_active(context)?;
    let mut module = package.dev_vela_anonymous_root_module();
    module.add(left, right)
}

pub fn call_generated_host(
    runtime: &mut vela_engine::runtime::Runtime,
    player: &mut Player,
) -> Result<(i64, i64), String> {
    let mut package = vela_bindings::bind(runtime).map_err(|error| error.to_string())?;
    let mut module = package.dev_vela_anonymous_root_module();
    let raised = module.raise(player, 2).map_err(|error| error.to_string())?;
    let read = module
        .read_level(player)
        .map_err(|error| error.to_string())?;
    Ok((raised, read))
}

pub async fn call_generated_host_async(
    runtime: &mut vela_engine::runtime::Runtime,
    player: &mut Player,
) -> Result<i64, String> {
    let mut package = vela_bindings::bind(runtime).map_err(|error| error.to_string())?;
    let mut module = package.dev_vela_anonymous_root_module();
    module
        .raise_async(player, 3)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll, Waker};

    fn ready<T>(future: impl Future<Output = T>) -> T {
        let mut future = std::pin::pin!(future);
        let mut context = Context::from_waker(Waker::noop());
        match Pin::new(&mut future).poll(&mut context) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("test future unexpectedly suspended"),
        }
    }

    #[test]
    fn generated_binding_executes_without_runtime_names_or_manual_values() {
        assert_eq!(
            super::vela_callable_contract_reenter_player().effects,
            vela_engine::native::EffectSet::host_write()
        );
        let engine = vela_engine::engine::Engine::builder()
            .capability(vela_engine::permission::Capability::HostRead)
            .capability(vela_engine::permission::Capability::HostWrite)
            .register_type::<super::Player>()
            .register_exports(super::vela_export_bundle_reenter_player())
            .register_exports(super::vela_export_bundle_reject_unrelated())
            .register_exports(super::vela_export_bundle_deny_effect_expansion())
            .register_exports(super::vela_export_bundle_deny_context_random())
            .with_controlled_random(7)
            .capability(vela_engine::permission::Capability::Random)
            .build()
            .expect("engine");
        let program = engine
            .compile_source(include_str!("../script.vela"))
            .expect("program");
        let mut runtime = vela_engine::runtime::Runtime::new(engine, program).expect("runtime");

        assert_eq!(super::call_generated_add(&mut runtime), Ok(42));
        assert_eq!(
            super::call_generated_conversions(&mut runtime),
            Ok((4, 4, 7, Some(9), Ok(10)))
        );
        assert_eq!(
            ready(super::call_generated_label(&mut runtime)),
            Ok("value".to_owned())
        );
        assert_eq!(
            super::call_generated_models(&mut runtime),
            Ok((
                super::vela_bindings::types::Point { x: 2, y: 3 },
                5,
                super::vela_bindings::types::Choice::Named { value: 7 },
            ))
        );
        let mut player = super::Player { level: 10 };
        assert_eq!(
            super::call_generated_host(&mut runtime, &mut player),
            Ok((12, 12))
        );
        assert_eq!(player.level, 12);
        {
            let dropped = super::call_generated_host_async(&mut runtime, &mut player);
            require_send(&dropped);
            drop(dropped);
        }
        assert_eq!(player.level, 12);
        assert_eq!(
            ready(super::call_generated_host_async(&mut runtime, &mut player)),
            Ok(20)
        );
        assert_eq!(player.level, 20);
        let round_trip = runtime
            .call(
                "round_trip",
                vela_engine::runtime::CallArgs::new()
                    .with_host_mut("player", &mut player)
                    .with_value("amount", 3_i64),
                vela_engine::runtime::CallOptions::new(100_000, 1024 * 1024, 32),
            )
            .expect("round trip");
        let owned = runtime
            .value_to_owned(&round_trip)
            .expect("owned round trip");
        assert_eq!(
            <i64 as vela_engine::args::FromScriptArg>::from_script_arg(&owned),
            Ok(67)
        );
        assert_eq!(player.level, 24);
        let unrelated_error = runtime
            .call(
                "round_trip_unrelated",
                vela_engine::runtime::CallArgs::new().with_host_mut("player", &mut player),
                vela_engine::runtime::CallOptions::new(100_000, 1024 * 1024, 32),
            )
            .expect_err("unrelated active reborrow must fail");
        assert!(matches!(
            unrelated_error.kind(),
            vela_engine::binding::VmErrorKind::TypeMismatch {
                operation: "generated active host argument lacks live lease provenance"
            }
        ));
        let effect_error = runtime
            .call(
                "effect_ceiling_denied",
                vela_engine::runtime::CallArgs::new(),
                vela_engine::runtime::CallOptions::new(100_000, 1024 * 1024, 32),
            )
            .expect_err("nested random effect must exceed a pure Rust ceiling");
        assert!(
            matches!(
            effect_error.kind(),
            vela_engine::binding::VmErrorKind::PermissionDenied { native, capability }
                if native == "random_value" && capability == "random"
            ),
            "unexpected effect-ceiling error: {effect_error:?}"
        );
        let context_error = runtime
            .call(
                "context_effect_ceiling_denied",
                vela_engine::runtime::CallArgs::new(),
                vela_engine::runtime::CallOptions::new(100_000, 1024 * 1024, 32),
            )
            .expect_err("context random operation must exceed a pure Rust ceiling");
        assert!(
            matches!(
                context_error.kind(),
                vela_engine::binding::VmErrorKind::PermissionDenied { native, capability }
                    if native == "NativeCallContext operation" && capability == "random"
            ),
            "unexpected context effect-ceiling error: {context_error:?}"
        );
    }

    fn require_send<T: Send>(_: &T) {}

    #[test]
    fn generated_binding_re_resolves_after_compatible_body_reload() {
        let engine = vela_engine::engine::Engine::builder()
            .capability(vela_engine::permission::Capability::HostRead)
            .capability(vela_engine::permission::Capability::HostWrite)
            .register_type::<super::Player>()
            .register_exports(super::vela_export_bundle_reenter_player())
            .register_exports(super::vela_export_bundle_reject_unrelated())
            .register_exports(super::vela_export_bundle_deny_effect_expansion())
            .register_exports(super::vela_export_bundle_deny_context_random())
            .with_controlled_random(7)
            .capability(vela_engine::permission::Capability::Random)
            .build()
            .expect("engine");
        let initial = engine
            .compile_hot_reload_initial(include_str!("../script.vela"))
            .expect("initial program");
        let mut runtime = vela_engine::runtime::Runtime::from_hot_reload_version(engine, initial)
            .expect("runtime");

        assert_eq!(super::call_generated_add(&mut runtime), Ok(42));
        runtime
            .stage_hot_reload_update(
                &include_str!("../script.vela")
                    .replace("return left + right;", "return left + right + 1;"),
            )
            .expect("compatible update should compile")
            .expect("compatible update should stage");
        runtime
            .check_reload()
            .expect("compatible update should publish");

        assert_eq!(super::call_generated_add(&mut runtime), Ok(43));
    }
}

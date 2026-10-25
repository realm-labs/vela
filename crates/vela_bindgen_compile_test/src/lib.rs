//! Compile-only proof that the official generator targets the public binding API.

include!(concat!(env!("OUT_DIR"), "/vela_bindings.rs"));

#[vela_macros::export(path = "test::deny_effect_expansion")]
pub fn deny_effect_expansion(
    context: &mut vela_engine::context::NativeCallContext<'_>,
) -> vela_engine::binding::VmResult<i64> {
    let mut package = vela_bindings::bind_active(context)?;
    let mut module = package.dev_vela_anonymous_root_module();
    module.random_value()
}

#[vela_macros::export(path = "test::deny_context_random")]
pub fn deny_context_random(
    context: &mut vela_engine::context::NativeCallContext<'_>,
) -> vela_engine::binding::VmResult<i64> {
    context.require_capability(vela_engine::permission::Capability::Random)?;
    Ok(1)
}

pub fn call_generated_add(runtime: &mut vela_engine::runtime::Runtime) -> Result<i64, String> {
    let mut package = vela_bindings::bind(runtime).map_err(|error| error.to_string())?;
    let mut module = package.dev_vela_anonymous_root_module();
    module.add(20, 22).map_err(|error| error.to_string())
}

pub fn call_generated_label(runtime: &mut vela_engine::runtime::Runtime) -> Result<String, String> {
    let mut package = vela_bindings::bind(runtime).map_err(|error| error.to_string())?;
    let mut module = package.dev_vela_anonymous_root_module();
    module.label(42).map_err(|error| error.to_string())
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
    context: &mut vela_engine::context::NativeCallContext<'_>,
    left: i64,
    right: i64,
) -> vela_engine::binding::VmResult<i64> {
    let mut package = vela_bindings::bind_active(context)?;
    let mut module = package.dev_vela_anonymous_root_module();
    module.add(left, right)
}

#[cfg(test)]
mod tests {

    #[test]
    fn generated_binding_executes_without_runtime_names_or_manual_values() {
        let engine = vela_engine::engine::Engine::builder()
            .install_registration(super::vela_function_deny_effect_expansion())
            .install_registration(super::vela_function_deny_context_random())
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
            super::call_generated_label(&mut runtime),
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

    #[test]
    fn generated_binding_re_resolves_after_compatible_body_reload() {
        let engine = vela_engine::engine::Engine::builder()
            .install_registration(super::vela_function_deny_effect_expansion())
            .install_registration(super::vela_function_deny_context_random())
            .with_controlled_random(7)
            .capability(vela_engine::permission::Capability::Random)
            .build()
            .expect("engine");
        let program = engine
            .compile_source(include_str!("../script.vela"))
            .expect("initial program");
        let mut runtime = vela_engine::runtime::Runtime::builder(engine, program)
            .expect("runtime image")
            .with_hot_reload()
            .expect("hot reload")
            .build()
            .expect("runtime");

        assert_eq!(super::call_generated_add(&mut runtime), Ok(42));
        let updated_source = include_str!("../script.vela")
            .replace("return left + right;", "return left + right + 1;");
        runtime
            .stage_reload(updated_source.as_str())
            .expect("compatible update should stage");
        runtime
            .activate_reload()
            .expect("compatible update should publish");

        assert_eq!(super::call_generated_add(&mut runtime), Ok(43));
    }
}

//! Compile-only proof that the official generator targets the public binding API.

include!(concat!(env!("OUT_DIR"), "/vela_bindings.rs"));

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

pub fn call_generated_active(
    context: &mut vela_engine::context::NativeCallContext<'_, '_>,
    left: i64,
    right: i64,
) -> vela_engine::binding::VmResult<i64> {
    let mut package = vela_bindings::bind_active(context)?;
    let mut module = package.dev_vela_anonymous_root_module();
    module.add(left, right)
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
        let engine = vela_engine::engine::Engine::builder()
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
    }

    #[test]
    fn generated_binding_re_resolves_after_compatible_body_reload() {
        let engine = vela_engine::engine::Engine::builder()
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

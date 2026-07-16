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
    }
}

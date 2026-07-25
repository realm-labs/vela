use vela_engine::engine::Engine;

pub(crate) fn schema_json(engine: &Engine) -> Result<String, Box<dyn std::error::Error>> {
    engine
        .tooling_schema_artifact()?
        .to_json()
        .map_err(|error| error.message().to_owned().into())
}

#[cfg(test)]
mod tests {
    use vela_common::HostConstructionLifetime;
    use vela_def::FunctionId;
    use vela_engine::engine::Engine;
    use vela_engine::native::{EffectSet, NativeFunctionDesc, TypeHint};
    use vela_macros::{ScriptHost, service, service_set};
    use vela_vm::error::VmResult;

    use super::schema_json;

    #[derive(ScriptHost)]
    #[script(path = "cli_test::Observed")]
    pub struct Observed {
        #[script(get)]
        value: i64,
    }

    #[vela_macros::script_methods]
    impl Observed {}

    #[service(path = "cli_test::handler")]
    pub trait HandlerService: Send + Sync {
        fn handle(&self, value: i64) -> i64;
        fn inspect(&self, value: &Observed) -> i64;
        fn identity<'a>(&self, value: &'a Observed) -> &'a Observed;
    }

    pub struct RustHandlerService;

    impl HandlerService for RustHandlerService {
        fn handle(&self, value: i64) -> i64 {
            value
        }

        fn inspect(&self, value: &Observed) -> i64 {
            value.value
        }

        fn identity<'a>(&self, value: &'a Observed) -> &'a Observed {
            value
        }
    }

    #[service_set(context = ())]
    pub struct CliServices {
        #[vela::default(RustHandlerService)]
        pub handler: dyn HandlerService,
    }

    #[test]
    fn cli_schema_json_projects_registered_service_metadata() {
        assert_eq!(RustHandlerService.handle(7), 7);
        let observed = Observed { value: 9 };
        assert_eq!(RustHandlerService.inspect(&observed), 9);
        assert!(std::ptr::eq(
            RustHandlerService.identity(&observed),
            &observed,
        ));
        let constructor = NativeFunctionDesc::new("Observed::new", FunctionId::new(0xdead))
            .returns(TypeHint::Host(Observed::vela_host_type_desc().key))
            .effects(EffectSet::pure());
        let engine = CliServices::register_types(Engine::builder().register_rust_type::<Observed>(
            Observed::vela_type_binding().host_constructor_fn(
                HostConstructionLifetime::CallScoped,
                constructor,
                |_args, _host| -> VmResult<Observed> { Ok(Observed { value: 11 }) },
            ),
        ))
        .build()
        .expect("service engine");
        let json = schema_json(&engine).expect("schema JSON");

        assert!(json.contains(r#""serviceSet""#));
        assert!(json.contains(r#""path": "cli_test::handler""#));
        assert!(json.contains(r#""name": "handle""#));
        assert!(json.contains(r#""typeBindings""#));
        assert!(json.contains(r#""mode": "storage_directed_shared""#));
        assert!(json.contains(r#""injected""#));
        assert!(json.contains(r#""constructible""#));
        assert!(json.contains(r#""produced_borrow""#));
    }
}

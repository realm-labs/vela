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
    use vela_macros::ScriptHost;
    use vela_vm::error::VmResult;

    use super::schema_json;

    #[derive(ScriptHost)]
    #[vela(path = "cli_test::Observed")]
    pub struct Observed {
        #[vela(get)]
        value: i64,
    }

    #[test]
    fn cli_schema_json_projects_registered_host_metadata_without_services() {
        let constructor = NativeFunctionDesc::new("Observed::new", FunctionId::new(0xdead))
            .returns(TypeHint::Host(Observed::vela_host_type_desc().key))
            .effects(EffectSet::pure());
        let engine = Engine::builder()
            .install_type_binding::<Observed>(Observed::vela_type_binding().host_constructor_fn(
                HostConstructionLifetime::CallScoped,
                constructor,
                |_args, _host| -> VmResult<Observed> { Ok(Observed { value: 11 }) },
            ))
            .build()
            .expect("engine");
        let json = schema_json(&engine).expect("schema JSON");
        let artifact =
            vela_language_service::SchemaArtifact::from_json(&json).expect("current schema format");

        assert!(json.contains(r#""formatVersion": 3"#));
        assert!(!json.contains(r#""serviceSet""#));
        assert!(json.contains(r#""typeBindings""#));
        let facts = artifact.to_registry_facts();
        let registered = engine.tooling_registry_facts().expect("registry facts");
        assert_eq!(
            facts.type_fact("Observed"),
            registered.type_fact("Observed")
        );
        assert_eq!(
            facts.field_access_fact("Observed", "value"),
            registered.field_access_fact("Observed", "value")
        );
        let binding = facts.type_binding_fact("Observed").expect("host binding");
        assert_eq!(Some(binding), registered.type_binding_fact("Observed"));
        assert!(
            binding
                .capabilities
                .contains(vela_common::ReceiverCapability::Construct)
        );
        assert_eq!(binding.constructor_ids, [FunctionId::new(0xdead)]);
        assert_eq!(
            binding.host_constructors,
            [vela_common::HostConstructorBinding::new(
                FunctionId::new(0xdead),
                HostConstructionLifetime::CallScoped,
            )]
        );
    }
}

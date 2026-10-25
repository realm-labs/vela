use vela_engine::engine::Engine;

pub(crate) fn schema_json(engine: &Engine) -> Result<String, Box<dyn std::error::Error>> {
    engine
        .tooling_schema_artifact()?
        .to_json()
        .map_err(|error| error.message().to_owned().into())
}

#[cfg(test)]
mod tests {
    use vela_engine::engine::Engine;
    use vela_macros::Value;

    use super::schema_json;

    #[derive(Value)]
    #[vela(path = "cli_test::Observed")]
    pub struct Observed {
        value: i64,
    }

    #[test]
    fn cli_schema_json_projects_registered_value_metadata_without_host_surface() {
        let engine = Engine::builder()
            .install_type_binding::<Observed>(Observed::vela_type_binding())
            .build()
            .expect("engine");
        let json = schema_json(&engine).expect("schema JSON");
        let artifact =
            vela_language_service::SchemaArtifact::from_json(&json).expect("current schema format");
        assert!(json.contains(r#""formatVersion": 4"#));
        for removed in [
            "serviceSet",
            "hostConstructors",
            "constructorIds",
            "collectionView",
            "scoped_resource",
            "capabilities",
            "storage",
        ] {
            assert!(!json.contains(removed), "{json}");
        }
        let facts = artifact.to_registry_facts();
        let registered = engine.tooling_registry_facts().expect("registry facts");
        assert_eq!(
            facts.type_fact("cli_test::Observed"),
            registered.type_fact("cli_test::Observed")
        );
        assert_eq!(
            facts.field_access_fact("cli_test::Observed", "value"),
            registered.field_access_fact("cli_test::Observed", "value")
        );
        let binding = facts
            .type_binding_fact("cli_test::Observed")
            .expect("value binding");
        assert_eq!(
            Some(binding),
            registered.type_binding_fact("cli_test::Observed")
        );
    }
}

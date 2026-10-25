use super::Engine;

impl Engine {
    /// Copies the sealed compiler registry into the metadata model shared by
    /// CLI schema export and native language tooling.
    pub fn tooling_registry_facts(
        &self,
    ) -> Result<vela_analysis::registry::RegistryFacts, vela_registry::RegistryDeclarationSlotError>
    {
        vela_analysis::registry::RegistryFacts::from_compile_view(self.compiler_registry())
    }

    /// Projects this engine's sealed compiler registry into the
    /// stable metadata artifact consumed by offline compilers and language
    /// tooling.
    ///
    /// The artifact describes the ABI that scripts may compile against. It
    /// does not contain executable bytecode and must still be paired with a
    /// separately validated program artifact before deployment.
    #[cfg(feature = "schema-artifact")]
    pub fn tooling_schema_artifact(
        &self,
    ) -> Result<vela_language_service::SchemaArtifact, vela_registry::RegistryDeclarationSlotError>
    {
        let facts = self.tooling_registry_facts()?;
        Ok(vela_language_service::SchemaArtifact::from_registry_facts(
            &facts,
        ))
    }
}

#[cfg(all(test, feature = "schema-artifact"))]
mod tests {
    use crate::engine::Engine;
    use crate::native::{EffectSet, NativeFunctionDesc, NativeFunctionId, TypeHint};
    use crate::permission::Capability;
    use vela_vm::owned_value::OwnedValue;

    #[test]
    fn tooling_schema_round_trips_callable_effects_without_execution_effect_ceiling() {
        let engine = Engine::builder()
            .capability(Capability::IoRead)
            .register_native_fn(
                NativeFunctionDesc::new("test::read", NativeFunctionId::new(0xC301))
                    .returns(TypeHint::unit())
                    .effects(EffectSet::io_read()),
                |_| Ok(OwnedValue::Unit),
            )
            .build()
            .expect("engine");
        let facts = engine.tooling_registry_facts().expect("registry facts");
        assert!(facts.execution_effect_ceiling().is_none());
        let artifact = engine.tooling_schema_artifact().expect("tooling schema");
        let json = artifact.to_json().expect("encode tooling schema");
        assert!(!json.contains("executionEffectCeiling"));
        let decoded =
            vela_language_service::SchemaArtifact::from_json(&json).expect("decode tooling schema");
        let decoded_facts = decoded.to_registry_facts();
        assert!(decoded_facts.execution_effect_ceiling().is_none());
        let effects = decoded_facts
            .function_effect_fact("test::read")
            .expect("retained callable effects");
        assert!(effects.reads_io);
        assert!(!effects.writes_io);
        assert_eq!(Some(effects), facts.function_effect_fact("test::read"));
        assert_eq!(
            decoded_facts.function_signature_fact("test::read"),
            facts.function_signature_fact("test::read")
        );

        let legacy = json.replacen(
            "\"facts\": {",
            "\"facts\": {\"executionEffectCeiling\": {},",
            1,
        );
        let error = vela_language_service::SchemaArtifact::from_json(&legacy)
            .expect_err("removed execution ceiling must not be silently defaulted");
        assert!(
            error
                .message()
                .contains("unknown field `executionEffectCeiling`")
        );
    }
}

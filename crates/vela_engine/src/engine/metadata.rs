use super::Engine;

impl Engine {
    /// Copies the sealed compiler registry into the metadata model shared by
    /// CLI schema export and native language tooling.
    pub fn tooling_registry_facts(
        &self,
    ) -> Result<vela_analysis::registry::RegistryFacts, vela_registry::RegistryDeclarationSlotError>
    {
        let mut facts =
            vela_analysis::registry::RegistryFacts::from_compile_view(self.compiler_registry())?;
        facts.set_execution_effect_ceiling(capability_effect_ceiling(self.capabilities()));
        Ok(facts)
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

fn capability_effect_ceiling(
    capabilities: vela_common::CapabilitySet,
) -> vela_analysis::registry::RegistryEffectFact {
    use vela_common::Capability;

    vela_analysis::registry::RegistryEffectFact {
        reads_host: capabilities.contains(Capability::HostRead)
            || capabilities.contains(Capability::HostWrite),
        writes_host: capabilities.contains(Capability::HostWrite),
        emits_events: capabilities.contains(Capability::EventEmit),
        reads_time: capabilities.contains(Capability::Time),
        uses_random: capabilities.contains(Capability::Random),
        reads_io: capabilities.contains(Capability::IoRead),
        writes_io: capabilities.contains(Capability::IoWrite),
        reads_reflection: capabilities.contains(Capability::ReflectionRead),
        writes_reflection: capabilities.contains(Capability::ReflectionWrite),
        calls_reflection: capabilities.contains(Capability::ReflectionCall),
        spawns_tasks: capabilities.contains(Capability::TaskSpawn),
    }
}

#[cfg(all(test, feature = "schema-artifact"))]
mod tests {
    use crate::engine::Engine;
    use crate::permission::Capability;

    #[test]
    fn tooling_schema_round_trips_the_engine_execution_effect_ceiling() {
        let engine = Engine::builder()
            .capability(Capability::TaskSpawn)
            .capability(Capability::IoRead)
            .build()
            .expect("engine");
        let artifact = engine.tooling_schema_artifact().expect("tooling schema");
        let json = artifact.to_json().expect("encode tooling schema");
        assert!(json.contains("executionEffectCeiling"));
        let decoded =
            vela_language_service::SchemaArtifact::from_json(&json).expect("decode tooling schema");
        let facts = decoded.to_registry_facts();
        let ceiling = facts
            .execution_effect_ceiling()
            .expect("execution effect ceiling");

        assert!(ceiling.spawns_tasks);
        assert!(ceiling.reads_io);
        assert!(!ceiling.writes_io);
    }
}

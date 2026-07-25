use super::Engine;

impl Engine {
    #[must_use]
    pub fn service_set_schema(&self) -> Option<&crate::service::ServiceSetSchema> {
        self.service_set_schema.as_deref()
    }

    /// Copies the sealed compiler registry into the metadata model shared by
    /// CLI schema export and native language tooling.
    pub fn tooling_registry_facts(
        &self,
    ) -> Result<vela_analysis::registry::RegistryFacts, vela_registry::RegistryDeclarationSlotError>
    {
        vela_analysis::registry::RegistryFacts::from_compile_view(self.compiler_registry())
    }
}

use super::Compiler;
use super::schema_defaults::ConstructorShape;

impl<'ast, 'registry> Compiler<'ast, 'registry> {
    pub(super) fn record_constructor_shape(&self, type_name: &str) -> Option<ConstructorShape> {
        self.facts.schema_defaults.record(type_name).cloned()
    }

    pub(super) fn enum_constructor_shape(
        &self,
        type_name: &str,
        variant: &str,
    ) -> Option<ConstructorShape> {
        self.facts
            .schema_defaults
            .enum_variant(type_name, variant)
            .cloned()
    }
}

//! Schema declaration locations and projection into a current source table.
use std::collections::BTreeMap;

use vela_common::Span;

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct SchemaSourceLocations {
    pub(super) types: BTreeMap<String, Span>,
    pub(super) traits: BTreeMap<String, Span>,
    pub(super) modules: BTreeMap<String, Span>,
    pub(super) fields: BTreeMap<(String, String), Span>,
    pub(super) variants: BTreeMap<(String, String), Span>,
    pub(super) methods: BTreeMap<(String, String), Span>,
    pub(super) trait_methods: BTreeMap<(String, String), Span>,
    pub(super) functions: BTreeMap<String, Span>,
}

impl SchemaSourceLocations {
    pub(crate) fn source_ids(&self) -> std::collections::BTreeSet<vela_common::SourceId> {
        self.types
            .values()
            .chain(self.traits.values())
            .chain(self.modules.values())
            .chain(self.fields.values())
            .chain(self.variants.values())
            .chain(self.methods.values())
            .chain(self.trait_methods.values())
            .chain(self.functions.values())
            .map(|span| span.source)
            .collect()
    }

    pub(crate) fn map_spans(&self, mut map: impl FnMut(Span) -> Option<Span>) -> Self {
        fn mapped<K: Clone + Ord>(
            entries: &BTreeMap<K, Span>,
            map: &mut impl FnMut(Span) -> Option<Span>,
        ) -> BTreeMap<K, Span> {
            entries
                .iter()
                .filter_map(|(key, span)| map(*span).map(|span| (key.clone(), span)))
                .collect()
        }
        Self {
            types: mapped(&self.types, &mut map),
            traits: mapped(&self.traits, &mut map),
            modules: mapped(&self.modules, &mut map),
            fields: mapped(&self.fields, &mut map),
            variants: mapped(&self.variants, &mut map),
            methods: mapped(&self.methods, &mut map),
            trait_methods: mapped(&self.trait_methods, &mut map),
            functions: mapped(&self.functions, &mut map),
        }
    }

    #[must_use]
    pub fn type_span(&self, name: &str) -> Option<Span> {
        self.types.get(name).copied()
    }

    #[must_use]
    pub fn trait_span(&self, name: &str) -> Option<Span> {
        self.traits.get(name).copied()
    }

    #[must_use]
    pub fn module_span(&self, name: &str) -> Option<Span> {
        self.modules.get(name).copied()
    }

    #[must_use]
    pub fn field_span(&self, owner: &str, name: &str) -> Option<Span> {
        self.fields
            .get(&(owner.to_owned(), name.to_owned()))
            .copied()
    }

    #[must_use]
    pub fn variant_span(&self, owner: &str, name: &str) -> Option<Span> {
        self.variants
            .get(&(owner.to_owned(), name.to_owned()))
            .copied()
    }

    #[must_use]
    pub fn method_span(&self, owner: &str, name: &str) -> Option<Span> {
        self.methods
            .get(&(owner.to_owned(), name.to_owned()))
            .copied()
    }

    #[must_use]
    pub fn trait_method_span(&self, owner: &str, name: &str) -> Option<Span> {
        self.trait_methods
            .get(&(owner.to_owned(), name.to_owned()))
            .copied()
    }

    #[must_use]
    pub fn function_span(&self, name: &str) -> Option<Span> {
        // Callers resolve the canonical function identity before looking up its
        // source. A metadata-only function must not borrow another owner's span.
        self.functions.get(name).copied()
    }
}

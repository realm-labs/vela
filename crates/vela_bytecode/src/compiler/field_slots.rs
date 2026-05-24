use std::collections::BTreeMap;

use vela_hir::{EnumVariantFieldsHint, HirDeclId, ModuleGraph};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ScriptFieldSlots {
    record_slots: BTreeMap<(String, String), usize>,
    enum_slots: BTreeMap<(String, String, String), usize>,
}

impl ScriptFieldSlots {
    pub(super) fn from_graph(
        graph: &ModuleGraph,
        type_symbols: &BTreeMap<HirDeclId, String>,
    ) -> Self {
        let mut record_slots = BTreeMap::new();
        let mut enum_slots = BTreeMap::new();

        for (declaration, type_name) in type_symbols {
            if let Some(shape) = graph.struct_shape(*declaration) {
                for (field, slot) in
                    sorted_slots(shape.fields.iter().map(|field| field.name.clone()))
                {
                    record_slots.insert((type_name.clone(), field), slot);
                }
            }

            if let Some(shape) = graph.enum_shape(*declaration) {
                for variant in &shape.variants {
                    for (field, slot) in enum_variant_slots(&variant.fields) {
                        enum_slots.insert((type_name.clone(), variant.name.clone(), field), slot);
                    }
                }
            }
        }

        Self {
            record_slots,
            enum_slots,
        }
    }

    pub(super) fn record(&self, type_name: &str, field: &str) -> Option<usize> {
        self.record_slots
            .get(&(type_name.to_owned(), field.to_owned()))
            .copied()
    }

    pub(super) fn enum_variant(
        &self,
        type_name: &str,
        variant: &str,
        field: &str,
    ) -> Option<usize> {
        self.enum_slots
            .get(&(type_name.to_owned(), variant.to_owned(), field.to_owned()))
            .copied()
    }
}

fn enum_variant_slots(fields: &EnumVariantFieldsHint) -> Vec<(String, usize)> {
    match fields {
        EnumVariantFieldsHint::Unit => Vec::new(),
        EnumVariantFieldsHint::Tuple(fields) => sorted_slots(
            fields
                .iter()
                .enumerate()
                .map(|(index, _)| index.to_string()),
        ),
        EnumVariantFieldsHint::Record(fields) => {
            sorted_slots(fields.iter().map(|field| field.name.clone()))
        }
    }
}

fn sorted_slots(fields: impl IntoIterator<Item = String>) -> Vec<(String, usize)> {
    let mut fields = fields.into_iter().collect::<Vec<_>>();
    fields.sort_unstable();
    fields
        .into_iter()
        .enumerate()
        .map(|(slot, field)| (field, slot))
        .collect()
}

use std::collections::BTreeMap;

use vela_hir::{HirDeclId, ModuleGraph};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ScriptFieldSlots {
    slots: BTreeMap<(String, String), usize>,
}

impl ScriptFieldSlots {
    pub(super) fn from_graph(
        graph: &ModuleGraph,
        type_symbols: &BTreeMap<HirDeclId, String>,
    ) -> Self {
        let slots = type_symbols
            .iter()
            .filter_map(|(declaration, type_name)| {
                let shape = graph.struct_shape(*declaration)?;
                let mut fields = shape
                    .fields
                    .iter()
                    .map(|field| field.name.as_str())
                    .collect::<Vec<_>>();
                fields.sort_unstable();
                Some(
                    fields
                        .into_iter()
                        .enumerate()
                        .map(|(slot, field)| ((type_name.clone(), field.to_owned()), slot))
                        .collect::<Vec<_>>(),
                )
            })
            .flatten()
            .collect();
        Self { slots }
    }

    pub(super) fn get(&self, type_name: &str, field: &str) -> Option<usize> {
        self.slots
            .get(&(type_name.to_owned(), field.to_owned()))
            .copied()
    }
}

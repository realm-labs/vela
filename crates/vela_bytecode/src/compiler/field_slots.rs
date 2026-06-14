use std::collections::BTreeMap;

use vela_common::ShapeId;
use vela_hir::ids::HirDeclId;
use vela_hir::module_graph::ModuleGraph;
use vela_hir::type_hint::{EnumVariantFieldsHint, HirTypeHint};

use super::script_types::{ScriptTypeFact, type_hint_script_type};
use super::value_types::{RuntimeTypeFact, type_hint_value_type};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ScriptFieldSlots {
    record_slots: BTreeMap<(String, String), usize>,
    record_field_facts: BTreeMap<(String, String), ScriptTypeFact>,
    record_field_value_types: BTreeMap<(String, String), RuntimeTypeFact>,
    enum_slots: BTreeMap<(String, String, String), usize>,
    enum_field_facts: BTreeMap<(String, String, String), ScriptTypeFact>,
    enum_field_value_types: BTreeMap<(String, String, String), RuntimeTypeFact>,
}

impl ScriptFieldSlots {
    pub(super) fn from_graph(
        graph: &ModuleGraph,
        type_symbols: &BTreeMap<HirDeclId, String>,
    ) -> Self {
        let mut record_slots = BTreeMap::new();
        let mut record_field_facts = BTreeMap::new();
        let mut record_field_value_types = BTreeMap::new();
        let mut enum_slots = BTreeMap::new();
        let mut enum_field_facts = BTreeMap::new();
        let mut enum_field_value_types = BTreeMap::new();
        let type_names = type_symbols.values().collect::<Vec<_>>();

        for (declaration, type_name) in type_symbols {
            if let Some(shape) = graph.struct_shape(*declaration) {
                for (field, slot) in
                    sorted_slots(shape.fields.iter().map(|field| field.name.clone()))
                {
                    record_slots.insert((type_name.clone(), field), slot);
                }
                for field in &shape.fields {
                    if let Some(hint) = field.type_hint.as_ref() {
                        if let Some(type_name_hint) =
                            type_hint_script_type(hint, type_names.clone())
                        {
                            record_field_facts.insert(
                                (type_name.clone(), field.name.clone()),
                                ScriptTypeFact::new(type_name_hint),
                            );
                        }
                        if let Some(value_type) = type_hint_value_type(hint) {
                            record_field_value_types
                                .insert((type_name.clone(), field.name.clone()), value_type);
                        }
                    }
                }
            }

            if let Some(shape) = graph.enum_shape(*declaration) {
                for variant in &shape.variants {
                    for (field, slot) in enum_variant_slots(&variant.fields) {
                        enum_slots.insert(
                            (type_name.clone(), variant.name.clone(), field.clone()),
                            slot,
                        );
                    }
                    for (field, hint) in enum_variant_field_hints(&variant.fields) {
                        if let Some(type_name_hint) =
                            hint.and_then(|hint| type_hint_script_type(hint, type_names.clone()))
                        {
                            enum_field_facts.insert(
                                (type_name.clone(), variant.name.clone(), field.clone()),
                                ScriptTypeFact::new(type_name_hint),
                            );
                        }
                        if let Some(value_type) = hint.and_then(type_hint_value_type) {
                            enum_field_value_types.insert(
                                (type_name.clone(), variant.name.clone(), field),
                                value_type,
                            );
                        }
                    }
                }
            }
        }

        Self {
            record_slots,
            record_field_facts,
            record_field_value_types,
            enum_slots,
            enum_field_facts,
            enum_field_value_types,
        }
    }

    pub(super) fn record(&self, type_name: &str, field: &str) -> Option<usize> {
        let type_name = self.resolve_record_type_name(type_name)?;
        self.record_slots
            .get(&(type_name, field.to_owned()))
            .copied()
    }

    pub(super) fn record_shape_id(&self, type_name: &str) -> Option<(String, ShapeId)> {
        let type_name = self.resolve_record_type_name(type_name)?;
        let fields = self
            .record_slots
            .keys()
            .filter_map(|(owner, field)| (owner == &type_name).then_some(field.as_str()));
        Some((
            type_name.clone(),
            vela_common::script_shape_id(&type_name, fields),
        ))
    }

    pub(super) fn record_field_value_type(
        &self,
        type_name: &str,
        field: &str,
    ) -> Option<RuntimeTypeFact> {
        let type_name = self.resolve_record_type_name(type_name)?;
        self.record_field_value_types
            .get(&(type_name, field.to_owned()))
            .cloned()
    }

    pub(super) fn record_field_fact(&self, type_name: &str, field: &str) -> Option<ScriptTypeFact> {
        let type_name = self.resolve_record_type_name(type_name)?;
        self.record_field_facts
            .get(&(type_name, field.to_owned()))
            .cloned()
    }

    pub(super) fn record_fields(
        &self,
        type_name: &str,
    ) -> Vec<(String, Option<ScriptTypeFact>, Option<RuntimeTypeFact>)> {
        let Some(type_name) = self.resolve_record_type_name(type_name) else {
            return Vec::new();
        };
        let fields = self
            .record_slots
            .keys()
            .filter_map(|(owner, field)| (owner == &type_name).then_some(field.clone()))
            .collect::<Vec<_>>();
        fields
            .into_iter()
            .map(move |field| {
                (
                    field.clone(),
                    self.record_field_facts
                        .get(&(type_name.clone(), field.clone()))
                        .cloned(),
                    self.record_field_value_types
                        .get(&(type_name.clone(), field.clone()))
                        .cloned(),
                )
            })
            .collect()
    }

    fn resolve_record_type_name(&self, type_name: &str) -> Option<String> {
        if self
            .record_slots
            .keys()
            .any(|(owner, _)| owner == type_name)
        {
            return Some(type_name.to_owned());
        }
        let suffix = format!("::{type_name}");
        let mut matches = self
            .record_slots
            .keys()
            .filter_map(|(owner, _)| owner.ends_with(&suffix).then_some(owner.clone()))
            .collect::<Vec<_>>();
        matches.sort();
        matches.dedup();
        match matches.as_slice() {
            [only] => Some(only.clone()),
            _ => None,
        }
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

    pub(super) fn enum_variant_field_fact(
        &self,
        type_name: &str,
        variant: &str,
        field: &str,
    ) -> Option<ScriptTypeFact> {
        self.enum_field_facts
            .get(&(type_name.to_owned(), variant.to_owned(), field.to_owned()))
            .cloned()
    }

    pub(super) fn enum_variant_field_value_type(
        &self,
        type_name: &str,
        variant: &str,
        field: &str,
    ) -> Option<RuntimeTypeFact> {
        self.enum_field_value_types
            .get(&(type_name.to_owned(), variant.to_owned(), field.to_owned()))
            .cloned()
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

fn enum_variant_field_hints(fields: &EnumVariantFieldsHint) -> Vec<(String, Option<&HirTypeHint>)> {
    match fields {
        EnumVariantFieldsHint::Unit => Vec::new(),
        EnumVariantFieldsHint::Tuple(fields) => fields
            .iter()
            .enumerate()
            .map(|(index, field)| (index.to_string(), field.type_hint.as_ref()))
            .collect(),
        EnumVariantFieldsHint::Record(fields) => fields
            .iter()
            .map(|field| (field.name.clone(), field.type_hint.as_ref()))
            .collect(),
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

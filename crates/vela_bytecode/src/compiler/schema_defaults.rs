use std::collections::BTreeMap;

use vela_hir::{HirDeclId, ModuleGraph, ModuleId};
use vela_syntax::{EnumVariantFields, Expr, ItemKind, SourceFile};

use crate::Constant;

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct ScriptSchemaDefaults {
    record_defaults: BTreeMap<String, Vec<SchemaFieldDefault>>,
    enum_defaults: BTreeMap<(String, String), Vec<SchemaFieldDefault>>,
}

impl ScriptSchemaDefaults {
    pub(super) fn merge(&mut self, other: Self) {
        self.record_defaults.extend(other.record_defaults);
        self.enum_defaults.extend(other.enum_defaults);
    }

    pub(super) fn record(&self, type_name: &str) -> Option<&[SchemaFieldDefault]> {
        self.record_defaults
            .get(type_name)
            .map(std::vec::Vec::as_slice)
    }

    pub(super) fn enum_variant(
        &self,
        type_name: &str,
        variant: &str,
    ) -> Option<&[SchemaFieldDefault]> {
        self.enum_defaults
            .get(&(type_name.to_owned(), variant.to_owned()))
            .map(std::vec::Vec::as_slice)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct SchemaFieldDefault {
    pub(super) name: String,
    pub(super) value: Expr,
    pub(super) constants: BTreeMap<String, Constant>,
}

pub(super) fn source_schema_defaults(
    parsed: &SourceFile,
    graph: &ModuleGraph,
    module: ModuleId,
    type_symbols: &BTreeMap<HirDeclId, String>,
    constants: BTreeMap<String, Constant>,
) -> ScriptSchemaDefaults {
    let mut defaults = ScriptSchemaDefaults::default();
    let Some(declarations) = graph.module(module) else {
        return defaults;
    };

    for item in &parsed.items {
        match &item.kind {
            ItemKind::Struct(record) => {
                let Some(type_name) = declarations
                    .get(&record.name)
                    .and_then(|declaration| type_symbols.get(&declaration))
                    .cloned()
                else {
                    continue;
                };
                let field_defaults = record
                    .fields
                    .iter()
                    .filter_map(|field| {
                        let value = field.default_value.clone()?;
                        Some(SchemaFieldDefault {
                            name: field.name.clone(),
                            value,
                            constants: constants.clone(),
                        })
                    })
                    .collect::<Vec<_>>();
                if !field_defaults.is_empty() {
                    defaults.record_defaults.insert(type_name, field_defaults);
                }
            }
            ItemKind::Enum(enumeration) => {
                let Some(type_name) = declarations
                    .get(&enumeration.name)
                    .and_then(|declaration| type_symbols.get(&declaration))
                    .cloned()
                else {
                    continue;
                };
                for variant in &enumeration.variants {
                    let field_defaults =
                        enum_variant_field_defaults(&variant.fields, constants.clone());
                    if !field_defaults.is_empty() {
                        defaults
                            .enum_defaults
                            .insert((type_name.clone(), variant.name.clone()), field_defaults);
                    }
                }
            }
            _ => {}
        }
    }

    defaults
}

fn enum_variant_field_defaults(
    fields: &EnumVariantFields,
    constants: BTreeMap<String, Constant>,
) -> Vec<SchemaFieldDefault> {
    match fields {
        EnumVariantFields::Unit => Vec::new(),
        EnumVariantFields::Tuple(fields) => fields
            .iter()
            .enumerate()
            .filter_map(|(index, field)| {
                let value = field.default_value.clone()?;
                Some(SchemaFieldDefault {
                    name: index.to_string(),
                    value,
                    constants: constants.clone(),
                })
            })
            .collect(),
        EnumVariantFields::Record(fields) => fields
            .iter()
            .filter_map(|field| {
                let value = field.default_value.clone()?;
                Some(SchemaFieldDefault {
                    name: field.name.clone(),
                    value,
                    constants: constants.clone(),
                })
            })
            .collect(),
    }
}

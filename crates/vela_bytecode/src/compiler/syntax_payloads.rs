use std::collections::BTreeMap;

use vela_common::SourceId;
use vela_syntax::Parse as SyntaxParse;
use vela_syntax::ast::{
    EnumVariantFields, Expr, ItemKind, SourceFile, SyntaxExpression, SyntaxSourceFile,
};

use super::schema_defaults::{SchemaDefaultPayloads, SchemaDefaultValue};

pub(super) fn const_value_payloads(
    parsed: &SyntaxParse<SyntaxSourceFile>,
) -> BTreeMap<String, SyntaxExpression> {
    let mut payloads = BTreeMap::new();
    for item in parsed.tree().consts() {
        let Some(name) = item.name_text() else {
            continue;
        };
        let Some(value) = item.value() else {
            continue;
        };
        payloads.entry(name).or_insert(value);
    }
    payloads
}

pub(super) fn schema_default_payloads(
    source: SourceId,
    parsed: &SyntaxParse<SyntaxSourceFile>,
    legacy: &SourceFile,
) -> SchemaDefaultPayloads {
    let legacy = legacy_schema_default_payloads(legacy);
    let mut payloads = SchemaDefaultPayloads::default();
    for item in parsed.tree().structs() {
        let Some(type_name) = item.name_text() else {
            continue;
        };
        let Some(fields) = item.field_list() else {
            continue;
        };
        for field in fields.fields() {
            let Some(field_name) = field.name_text() else {
                continue;
            };
            let Some(value) = field.default_value() else {
                continue;
            };
            let Some(legacy_value) = legacy.struct_field(&type_name, &field_name) else {
                continue;
            };
            payloads.insert_struct_field(
                type_name.clone(),
                field_name,
                SchemaDefaultValue::new(source, value, legacy_value),
            );
        }
    }

    for item in parsed.tree().enums() {
        let Some(type_name) = item.name_text() else {
            continue;
        };
        let Some(variants) = item.variant_list() else {
            continue;
        };
        for variant in variants.variants() {
            let Some(variant_name) = variant.name_text() else {
                continue;
            };
            if let Some(fields) = variant.tuple_field_list() {
                for (index, field) in fields.params().enumerate() {
                    let Some(value) = field.default_value() else {
                        continue;
                    };
                    let Some(legacy_value) =
                        legacy.enum_tuple_field(&type_name, &variant_name, index)
                    else {
                        continue;
                    };
                    payloads.insert_enum_tuple_field(
                        type_name.clone(),
                        variant_name.clone(),
                        index,
                        SchemaDefaultValue::new(source, value, legacy_value),
                    );
                }
            }
            if let Some(fields) = variant.record_field_list() {
                for field in fields.fields() {
                    let Some(field_name) = field.name_text() else {
                        continue;
                    };
                    let Some(value) = field.default_value() else {
                        continue;
                    };
                    let Some(legacy_value) =
                        legacy.enum_record_field(&type_name, &variant_name, &field_name)
                    else {
                        continue;
                    };
                    payloads.insert_enum_record_field(
                        type_name.clone(),
                        variant_name.clone(),
                        field_name,
                        SchemaDefaultValue::new(source, value, legacy_value),
                    );
                }
            }
        }
    }

    payloads
}

#[derive(Default)]
struct LegacySchemaDefaultPayloads {
    struct_fields: BTreeMap<(String, String), Expr>,
    enum_tuple_fields: BTreeMap<(String, String, usize), Expr>,
    enum_record_fields: BTreeMap<(String, String, String), Expr>,
}

impl LegacySchemaDefaultPayloads {
    fn struct_field(&self, type_name: &str, field_name: &str) -> Option<Expr> {
        self.struct_fields
            .get(&(type_name.to_owned(), field_name.to_owned()))
            .cloned()
    }

    fn enum_tuple_field(&self, type_name: &str, variant_name: &str, index: usize) -> Option<Expr> {
        self.enum_tuple_fields
            .get(&(type_name.to_owned(), variant_name.to_owned(), index))
            .cloned()
    }

    fn enum_record_field(
        &self,
        type_name: &str,
        variant_name: &str,
        field_name: &str,
    ) -> Option<Expr> {
        self.enum_record_fields
            .get(&(
                type_name.to_owned(),
                variant_name.to_owned(),
                field_name.to_owned(),
            ))
            .cloned()
    }
}

fn legacy_schema_default_payloads(parsed: &SourceFile) -> LegacySchemaDefaultPayloads {
    let mut payloads = LegacySchemaDefaultPayloads::default();
    for item in &parsed.items {
        match &item.kind {
            ItemKind::Struct(record) => {
                for field in &record.fields {
                    if let Some(default_value) = field.default_value.clone() {
                        payloads
                            .struct_fields
                            .insert((record.name.clone(), field.name.clone()), default_value);
                    }
                }
            }
            ItemKind::Enum(enumeration) => {
                for variant in &enumeration.variants {
                    match &variant.fields {
                        EnumVariantFields::Unit => {}
                        EnumVariantFields::Tuple(fields) => {
                            for (index, field) in fields.iter().enumerate() {
                                if let Some(default_value) = field.default_value.clone() {
                                    payloads.enum_tuple_fields.insert(
                                        (enumeration.name.clone(), variant.name.clone(), index),
                                        default_value,
                                    );
                                }
                            }
                        }
                        EnumVariantFields::Record(fields) => {
                            for field in fields {
                                if let Some(default_value) = field.default_value.clone() {
                                    payloads.enum_record_fields.insert(
                                        (
                                            enumeration.name.clone(),
                                            variant.name.clone(),
                                            field.name.clone(),
                                        ),
                                        default_value,
                                    );
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    payloads
}

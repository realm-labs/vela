use std::collections::BTreeMap;

use vela_syntax::ast::{EnumVariantFields, Expr, FunctionItem, ItemKind, SourceFile};

use super::schema_defaults::SchemaDefaultPayloads;

pub(super) fn const_value_payloads(parsed: &SourceFile) -> BTreeMap<&str, &Expr> {
    let mut payloads = BTreeMap::new();
    for (name, value) in parsed.items.iter().filter_map(|item| match &item.kind {
        ItemKind::Const(item) => Some((item.name.as_str(), &item.value)),
        _ => None,
    }) {
        payloads.entry(name).or_insert(value);
    }
    payloads
}

pub(super) fn function_body_payloads(parsed: &SourceFile) -> BTreeMap<&str, &FunctionItem> {
    let mut payloads = BTreeMap::new();
    for (name, function) in parsed.items.iter().filter_map(|item| match &item.kind {
        ItemKind::Function(function) => Some((function.name.as_str(), function)),
        _ => None,
    }) {
        payloads.entry(name).or_insert(function);
    }
    payloads
}

pub(super) fn schema_default_payloads(parsed: &SourceFile) -> SchemaDefaultPayloads {
    let mut payloads = SchemaDefaultPayloads::default();
    for item in &parsed.items {
        match &item.kind {
            ItemKind::Struct(record) => {
                for field in &record.fields {
                    if let Some(default_value) = field.default_value.clone() {
                        payloads.insert_struct_field(
                            record.name.clone(),
                            field.name.clone(),
                            default_value,
                        );
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
                                    payloads.insert_enum_tuple_field(
                                        enumeration.name.clone(),
                                        variant.name.clone(),
                                        index,
                                        default_value,
                                    );
                                }
                            }
                        }
                        EnumVariantFields::Record(fields) => {
                            for field in fields {
                                if let Some(default_value) = field.default_value.clone() {
                                    payloads.insert_enum_record_field(
                                        enumeration.name.clone(),
                                        variant.name.clone(),
                                        field.name.clone(),
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

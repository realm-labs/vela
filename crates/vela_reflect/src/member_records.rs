use std::collections::BTreeMap;

use vela_host::HostValue;

use crate::{
    DeclOrigin, FieldDesc, MethodDesc, TraitDesc, TraitMethodDesc, VariantDesc,
    metadata::{attrs_value, docs_value, span_value},
};

pub(crate) fn method_record_with_owner(type_name: &str, method: &MethodDesc) -> HostValue {
    let mut fields = BTreeMap::new();
    fields.insert("owner".to_owned(), HostValue::String(type_name.to_owned()));
    fields.extend(method_record_fields(method));
    method_record_from_fields(fields)
}

fn method_record_fields(method: &MethodDesc) -> BTreeMap<String, HostValue> {
    let mut fields = BTreeMap::new();
    fields.insert("id".to_owned(), HostValue::Int(i64::from(method.id.get())));
    fields.insert("name".to_owned(), HostValue::String(method.name.clone()));
    fields.insert("origin".to_owned(), origin_value(method.origin));
    fields.insert(
        "params".to_owned(),
        HostValue::Array(method.params.iter().map(method_param_record).collect()),
    );
    fields.insert(
        "return".to_owned(),
        method
            .return_type
            .as_ref()
            .map_or(HostValue::Null, |return_type| {
                HostValue::String(return_type.clone())
            }),
    );
    fields.insert(
        "returns".to_owned(),
        method
            .return_type
            .as_ref()
            .map_or(HostValue::Null, |return_type| {
                HostValue::String(return_type.clone())
            }),
    );
    fields.insert("effects".to_owned(), method_effects_record(method));
    fields.insert("access".to_owned(), method_access_record(method));
    fields.insert("docs".to_owned(), docs_value(method.docs.as_deref()));
    fields.insert("attrs".to_owned(), attrs_value(&method.attrs));
    fields.insert("source_span".to_owned(), span_value(method.source_span));
    fields
}

fn method_record_from_fields(fields: BTreeMap<String, HostValue>) -> HostValue {
    HostValue::Record {
        type_name: "ReflectMethod".to_owned(),
        fields,
    }
}

fn method_param_record(param: &crate::MethodParamDesc) -> HostValue {
    let mut fields = BTreeMap::new();
    fields.insert("name".to_owned(), HostValue::String(param.name.clone()));
    fields.insert(
        "type".to_owned(),
        param
            .type_hint
            .as_ref()
            .map_or(HostValue::Null, |hint| HostValue::String(hint.clone())),
    );
    fields.insert("defaulted".to_owned(), HostValue::Bool(param.has_default));
    HostValue::Record {
        type_name: "ReflectParam".to_owned(),
        fields,
    }
}

fn method_effects_record(method: &MethodDesc) -> HostValue {
    HostValue::Record {
        type_name: "ReflectEffectSet".to_owned(),
        fields: BTreeMap::from([
            (
                "reads_host".to_owned(),
                HostValue::Bool(method.effects.reads_host),
            ),
            (
                "writes_host".to_owned(),
                HostValue::Bool(method.effects.writes_host),
            ),
            (
                "emits_events".to_owned(),
                HostValue::Bool(method.effects.emits_events),
            ),
        ]),
    }
}

fn method_access_record(method: &MethodDesc) -> HostValue {
    HostValue::Record {
        type_name: "ReflectMethodAccess".to_owned(),
        fields: BTreeMap::from([
            ("public".to_owned(), HostValue::Bool(method.access.public)),
            (
                "reflect_callable".to_owned(),
                HostValue::Bool(method.access.reflect_callable),
            ),
            (
                "required_permissions".to_owned(),
                HostValue::Array(
                    method
                        .access
                        .required_permissions()
                        .iter()
                        .map(|permission| HostValue::String(permission.clone()))
                        .collect(),
                ),
            ),
        ]),
    }
}

pub(crate) fn trait_record(trait_desc: &TraitDesc) -> HostValue {
    let mut fields = BTreeMap::new();
    fields.insert(
        "id".to_owned(),
        HostValue::Int(i64::from(trait_desc.id.get())),
    );
    fields.insert(
        "name".to_owned(),
        HostValue::String(trait_desc.name.clone()),
    );
    fields.insert(
        "methods".to_owned(),
        HostValue::Array(
            trait_desc
                .methods
                .iter()
                .map(|method| trait_method_record(&trait_desc.name, method))
                .collect(),
        ),
    );
    fields.insert("origin".to_owned(), origin_value(trait_desc.origin));
    fields.insert("docs".to_owned(), docs_value(trait_desc.docs.as_deref()));
    fields.insert("attrs".to_owned(), attrs_value(&trait_desc.attrs));
    fields.insert("source_span".to_owned(), span_value(trait_desc.source_span));
    HostValue::Record {
        type_name: "ReflectTrait".to_owned(),
        fields,
    }
}

fn origin_value(origin: DeclOrigin) -> HostValue {
    HostValue::String(origin.as_str().to_owned())
}

fn trait_method_record(owner: &str, method: &TraitMethodDesc) -> HostValue {
    let mut fields = BTreeMap::new();
    fields.insert("id".to_owned(), HostValue::Int(i64::from(method.id.get())));
    fields.insert("name".to_owned(), HostValue::String(method.name.clone()));
    fields.insert("owner".to_owned(), HostValue::String(owner.to_owned()));
    fields.insert("origin".to_owned(), origin_value(method.origin));
    fields.insert(
        "params".to_owned(),
        HostValue::Array(method.params.iter().map(method_param_record).collect()),
    );
    fields.insert(
        "return".to_owned(),
        method
            .return_type
            .as_ref()
            .map_or(HostValue::Null, |return_type| {
                HostValue::String(return_type.clone())
            }),
    );
    fields.insert(
        "returns".to_owned(),
        method
            .return_type
            .as_ref()
            .map_or(HostValue::Null, |return_type| {
                HostValue::String(return_type.clone())
            }),
    );
    fields.insert("defaulted".to_owned(), HostValue::Bool(method.has_default));
    fields.insert("docs".to_owned(), docs_value(method.docs.as_deref()));
    fields.insert("attrs".to_owned(), attrs_value(&method.attrs));
    fields.insert("source_span".to_owned(), span_value(method.source_span));
    HostValue::Record {
        type_name: "ReflectTraitMethod".to_owned(),
        fields,
    }
}

pub(crate) fn variant_record_with_owner(type_name: &str, variant: &VariantDesc) -> HostValue {
    variant_record_with_owner_and_fields(type_name, variant, variant.fields.iter())
}

pub(crate) fn variant_record_with_owner_and_fields<'a>(
    type_name: &str,
    variant: &VariantDesc,
    variant_fields: impl IntoIterator<Item = &'a FieldDesc>,
) -> HostValue {
    let mut fields = variant_record_fields(variant, variant_fields);
    fields.insert("owner".to_owned(), HostValue::String(type_name.to_owned()));
    variant_record_from_fields(fields)
}

fn variant_record_fields<'a>(
    variant: &VariantDesc,
    variant_fields: impl IntoIterator<Item = &'a FieldDesc>,
) -> BTreeMap<String, HostValue> {
    let mut fields = BTreeMap::new();
    fields.insert("id".to_owned(), HostValue::Int(i64::from(variant.id.get())));
    fields.insert("name".to_owned(), HostValue::String(variant.name.clone()));
    fields.insert("origin".to_owned(), origin_value(variant.origin));
    fields.insert(
        "fields".to_owned(),
        HostValue::Array(variant_fields.into_iter().map(field_record).collect()),
    );
    fields.insert("docs".to_owned(), docs_value(variant.docs.as_deref()));
    fields.insert("attrs".to_owned(), attrs_value(&variant.attrs));
    fields.insert("source_span".to_owned(), span_value(variant.source_span));
    fields
}

fn variant_record_from_fields(fields: BTreeMap<String, HostValue>) -> HostValue {
    HostValue::Record {
        type_name: "ReflectVariant".to_owned(),
        fields,
    }
}

fn field_record(field: &FieldDesc) -> HostValue {
    field_record_from_fields(field_record_fields(field))
}

pub(crate) fn field_record_with_owner(type_name: &str, field: &FieldDesc) -> HostValue {
    let mut fields = BTreeMap::new();
    fields.insert("owner".to_owned(), HostValue::String(type_name.to_owned()));
    fields.extend(field_record_fields(field));
    field_record_from_fields(fields)
}

fn field_record_fields(field: &FieldDesc) -> BTreeMap<String, HostValue> {
    let mut fields = BTreeMap::new();
    fields.insert("id".to_owned(), HostValue::Int(i64::from(field.id.get())));
    fields.insert("name".to_owned(), HostValue::String(field.name.clone()));
    fields.insert("origin".to_owned(), origin_value(field.origin));
    fields.insert(
        "type".to_owned(),
        field
            .type_hint
            .as_ref()
            .filter(|hint| !hint.is_empty())
            .map_or(HostValue::Null, |hint| HostValue::String(hint.clone())),
    );
    fields.insert("writable".to_owned(), HostValue::Bool(field.writable));
    fields.insert("defaulted".to_owned(), HostValue::Bool(field.has_default));
    fields.insert("access".to_owned(), field_access_record(field));
    fields.insert("docs".to_owned(), docs_value(field.docs.as_deref()));
    fields.insert("attrs".to_owned(), attrs_value(&field.attrs));
    fields.insert("source_span".to_owned(), span_value(field.source_span));
    fields
}

fn field_record_from_fields(fields: BTreeMap<String, HostValue>) -> HostValue {
    HostValue::Record {
        type_name: "ReflectField".to_owned(),
        fields,
    }
}

fn field_access_record(field: &FieldDesc) -> HostValue {
    HostValue::Record {
        type_name: "ReflectFieldAccess".to_owned(),
        fields: BTreeMap::from([
            (
                "readable".to_owned(),
                HostValue::Bool(field.access.readable),
            ),
            (
                "writable".to_owned(),
                HostValue::Bool(field.access.writable),
            ),
            (
                "reflect_readable".to_owned(),
                HostValue::Bool(field.access.reflect_readable),
            ),
            (
                "reflect_writable".to_owned(),
                HostValue::Bool(field.access.reflect_writable),
            ),
            (
                "required_permissions".to_owned(),
                HostValue::Array(
                    field
                        .access
                        .required_permissions()
                        .iter()
                        .map(|permission| HostValue::String(permission.clone()))
                        .collect(),
                ),
            ),
        ]),
    }
}

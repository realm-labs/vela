use std::collections::BTreeMap;

use vela_host::value::HostValue;

use crate::{
    access::MethodEffectSet,
    metadata::{
        array, attrs_value, bool_value, docs_value, host, int_value, null_value, record,
        span_value, string,
    },
    modules::DeclOrigin,
    registry::{FieldDesc, MethodDesc, TraitDesc, TraitMethodDesc, VariantDesc},
    value::ReflectValue,
};

type ReflectFields = BTreeMap<String, ReflectValue>;

pub(crate) fn method_record_with_owner(type_name: &str, method: &MethodDesc) -> ReflectValue {
    let mut fields = BTreeMap::new();
    fields.insert("owner".to_owned(), string(type_name));
    fields.extend(method_record_fields(method));
    method_record_from_fields(fields)
}

fn method_record_fields(method: &MethodDesc) -> ReflectFields {
    let mut fields = BTreeMap::new();
    fields.insert("id".to_owned(), id_value(method.id.get()));
    fields.insert("name".to_owned(), string(method.name.clone()));
    fields.insert("origin".to_owned(), origin_value(method.origin));
    fields.insert(
        "params".to_owned(),
        array(method.params.iter().map(method_param_record)),
    );
    fields.insert(
        "return".to_owned(),
        optional_string(method.return_type.as_ref()),
    );
    fields.insert(
        "returns".to_owned(),
        optional_string(method.return_type.as_ref()),
    );
    fields.insert("effects".to_owned(), method_effects_record(method));
    fields.insert("access".to_owned(), method_access_record(method));
    fields.insert("docs".to_owned(), docs_value(method.docs.as_deref()));
    fields.insert("attrs".to_owned(), attrs_value(&method.attrs));
    fields.insert("source_span".to_owned(), span_value(method.source_span));
    fields
}

fn method_record_from_fields(fields: ReflectFields) -> ReflectValue {
    record("ReflectMethod", fields)
}

fn method_param_record(param: &crate::registry::MethodParamDesc) -> ReflectValue {
    record(
        "ReflectParam",
        BTreeMap::from([
            ("name".to_owned(), string(param.name.clone())),
            ("type".to_owned(), optional_string(param.type_hint.as_ref())),
            ("defaulted".to_owned(), bool_value(param.has_default)),
        ]),
    )
}

fn method_effects_record(method: &MethodDesc) -> ReflectValue {
    effect_set_record(&method.effects)
}

fn method_access_record(method: &MethodDesc) -> ReflectValue {
    record(
        "ReflectMethodAccess",
        BTreeMap::from([
            ("public".to_owned(), bool_value(method.access.public)),
            (
                "reflect_callable".to_owned(),
                bool_value(method.access.reflect_callable),
            ),
            (
                "required_permissions".to_owned(),
                string_array(method.access.required_permissions()),
            ),
        ]),
    )
}

pub(crate) fn trait_record(trait_desc: &TraitDesc) -> ReflectValue {
    record(
        "ReflectTrait",
        BTreeMap::from([
            ("id".to_owned(), id_value(trait_desc.id.get())),
            ("name".to_owned(), string(trait_desc.name.clone())),
            (
                "methods".to_owned(),
                array(
                    trait_desc
                        .methods
                        .iter()
                        .map(|method| trait_method_record(&trait_desc.name, method)),
                ),
            ),
            ("origin".to_owned(), origin_value(trait_desc.origin)),
            ("docs".to_owned(), docs_value(trait_desc.docs.as_deref())),
            ("attrs".to_owned(), attrs_value(&trait_desc.attrs)),
            ("source_span".to_owned(), span_value(trait_desc.source_span)),
        ]),
    )
}

fn origin_value(origin: DeclOrigin) -> ReflectValue {
    string(origin.as_str())
}

fn trait_method_record(owner: &str, method: &TraitMethodDesc) -> ReflectValue {
    record(
        "ReflectTraitMethod",
        BTreeMap::from([
            ("id".to_owned(), id_value(method.id.get())),
            ("name".to_owned(), string(method.name.clone())),
            ("owner".to_owned(), string(owner)),
            ("origin".to_owned(), origin_value(method.origin)),
            (
                "params".to_owned(),
                array(method.params.iter().map(method_param_record)),
            ),
            (
                "return".to_owned(),
                optional_string(method.return_type.as_ref()),
            ),
            (
                "returns".to_owned(),
                optional_string(method.return_type.as_ref()),
            ),
            ("defaulted".to_owned(), bool_value(method.has_default)),
            ("docs".to_owned(), docs_value(method.docs.as_deref())),
            ("attrs".to_owned(), attrs_value(&method.attrs)),
            ("source_span".to_owned(), span_value(method.source_span)),
        ]),
    )
}

pub(crate) fn variant_record_with_owner(type_name: &str, variant: &VariantDesc) -> ReflectValue {
    variant_record_with_owner_and_fields(type_name, variant, variant.fields.iter())
}

pub(crate) fn variant_record_with_owner_and_fields<'a>(
    type_name: &str,
    variant: &VariantDesc,
    variant_fields: impl IntoIterator<Item = &'a FieldDesc>,
) -> ReflectValue {
    let mut fields = variant_record_fields(type_name, variant, variant_fields);
    fields.insert("owner".to_owned(), string(type_name));
    variant_record_from_fields(fields)
}

fn variant_record_fields<'a>(
    type_name: &str,
    variant: &VariantDesc,
    variant_fields: impl IntoIterator<Item = &'a FieldDesc>,
) -> ReflectFields {
    let field_owner = format!("{type_name}::{}", variant.name);
    let mut fields = BTreeMap::new();
    fields.insert("id".to_owned(), id_value(variant.id.get()));
    fields.insert("name".to_owned(), string(variant.name.clone()));
    fields.insert("origin".to_owned(), origin_value(variant.origin));
    fields.insert(
        "fields".to_owned(),
        array(
            variant_fields
                .into_iter()
                .map(|field| field_record_with_owner(&field_owner, field)),
        ),
    );
    fields.insert("docs".to_owned(), docs_value(variant.docs.as_deref()));
    fields.insert("attrs".to_owned(), attrs_value(&variant.attrs));
    fields.insert("source_span".to_owned(), span_value(variant.source_span));
    fields
}

fn variant_record_from_fields(fields: ReflectFields) -> ReflectValue {
    record("ReflectVariant", fields)
}

pub(crate) fn field_record_with_owner(type_name: &str, field: &FieldDesc) -> ReflectValue {
    let mut fields = BTreeMap::new();
    fields.insert("owner".to_owned(), string(type_name));
    fields.extend(field_record_fields(field));
    field_record_from_fields(fields)
}

fn field_record_fields(field: &FieldDesc) -> ReflectFields {
    let mut fields = BTreeMap::new();
    fields.insert("id".to_owned(), id_value(field.id.get()));
    fields.insert("name".to_owned(), string(field.name.clone()));
    fields.insert("origin".to_owned(), origin_value(field.origin));
    fields.insert(
        "type".to_owned(),
        field
            .type_hint
            .as_ref()
            .filter(|hint| !hint.is_empty())
            .map_or_else(null_value, string),
    );
    fields.insert("writable".to_owned(), bool_value(field.writable));
    fields.insert("defaulted".to_owned(), bool_value(field.has_default));
    fields.insert("access".to_owned(), field_access_record(field));
    fields.insert("docs".to_owned(), docs_value(field.docs.as_deref()));
    fields.insert("attrs".to_owned(), attrs_value(&field.attrs));
    fields.insert("source_span".to_owned(), span_value(field.source_span));
    fields
}

fn id_value(id: u64) -> ReflectValue {
    // TODO(reflect): stable IDs are u64, but reflection currently exposes IDs
    // through signed script ints. Replace this lossy saturation with a deliberate
    // unsigned/ID value surface before treating reflect::id() as a stable public
    // identity API.
    int_value(i64::try_from(id).unwrap_or(i64::MAX))
}

fn field_record_from_fields(fields: ReflectFields) -> ReflectValue {
    record("ReflectField", fields)
}

fn field_access_record(field: &FieldDesc) -> ReflectValue {
    record(
        "ReflectFieldAccess",
        BTreeMap::from([
            ("readable".to_owned(), bool_value(field.access.readable)),
            ("writable".to_owned(), bool_value(field.access.writable)),
            (
                "reflect_readable".to_owned(),
                bool_value(field.access.reflect_readable),
            ),
            (
                "reflect_writable".to_owned(),
                bool_value(field.access.reflect_writable),
            ),
            (
                "required_permissions".to_owned(),
                string_array(field.access.required_permissions()),
            ),
        ]),
    )
}

fn effect_set_record(effects: &MethodEffectSet) -> ReflectValue {
    record(
        "ReflectEffectSet",
        BTreeMap::from([
            ("reads_host".to_owned(), bool_value(effects.reads_host)),
            ("writes_host".to_owned(), bool_value(effects.writes_host)),
            ("emits_events".to_owned(), bool_value(effects.emits_events)),
            ("reads_time".to_owned(), bool_value(effects.reads_time)),
            ("uses_random".to_owned(), bool_value(effects.uses_random)),
            (
                "reads_reflection".to_owned(),
                bool_value(effects.reads_reflection),
            ),
            (
                "writes_reflection".to_owned(),
                bool_value(effects.writes_reflection),
            ),
            (
                "calls_reflection".to_owned(),
                bool_value(effects.calls_reflection),
            ),
        ]),
    )
}

fn string_array(values: &[String]) -> ReflectValue {
    array(values.iter().map(|value| string(value.clone())))
}

fn optional_string(value: Option<&String>) -> ReflectValue {
    value.map_or_else(null_value, |value| host(HostValue::String(value.clone())))
}

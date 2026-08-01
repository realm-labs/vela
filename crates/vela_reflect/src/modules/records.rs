use std::collections::BTreeMap;

use crate::{
    metadata::{
        array, attrs_value, bool_value, docs_value, int_value, optional_string,
        optional_type_hint_desc, record, span_value, string,
    },
    value::ReflectValue,
};

use super::{DeclOrigin, FunctionDesc, FunctionParamDesc, ModuleDesc, StateDesc};

pub(super) fn module_record(desc: &ModuleDesc) -> ReflectValue {
    module_record_with_exports(desc, desc.exports.iter().map(|export| export.name.clone()))
}

pub(super) fn module_record_with_exports(
    desc: &ModuleDesc,
    exports: impl IntoIterator<Item = String>,
) -> ReflectValue {
    record(
        "ReflectModule",
        BTreeMap::from([
            ("name".to_owned(), string(desc.name.clone())),
            ("origin".to_owned(), origin_value(desc.origin)),
            ("exports".to_owned(), array(exports.into_iter().map(string))),
            ("docs".to_owned(), docs_value(desc.docs.as_deref())),
            ("attrs".to_owned(), attrs_value(&desc.attrs)),
            ("source_span".to_owned(), span_value(desc.source_span)),
        ]),
    )
}

pub(super) fn function_record(desc: &FunctionDesc) -> ReflectValue {
    record(
        "ReflectFunction",
        BTreeMap::from([
            (
                "id".to_owned(),
                // TODO(reflect): stable IDs are u64, but reflection currently exposes IDs
                // through signed script ints. Replace this lossy saturation with a deliberate
                // unsigned/ID value surface before treating reflect::id() as a stable public
                // identity API.
                int_value(i64::try_from(desc.id.get()).unwrap_or(i64::MAX)),
            ),
            ("name".to_owned(), string(desc.name.clone())),
            ("module".to_owned(), optional_string(desc.module.as_deref())),
            ("public".to_owned(), bool_value(desc.public)),
            ("is_async".to_owned(), bool_value(desc.asyncness.is_async())),
            ("effects".to_owned(), function_effects_record(desc)),
            ("access".to_owned(), function_access_record(desc)),
            ("origin".to_owned(), origin_value(desc.origin)),
            (
                "return".to_owned(),
                optional_string(desc.return_type.as_deref()),
            ),
            (
                "return_desc".to_owned(),
                optional_type_hint_desc(desc.return_type.as_deref()),
            ),
            (
                "returns".to_owned(),
                optional_string(desc.return_type.as_deref()),
            ),
            (
                "returns_desc".to_owned(),
                optional_type_hint_desc(desc.return_type.as_deref()),
            ),
            (
                "params".to_owned(),
                array(desc.params.iter().map(param_record)),
            ),
            ("docs".to_owned(), docs_value(desc.docs.as_deref())),
            ("attrs".to_owned(), attrs_value(&desc.attrs)),
            ("source_span".to_owned(), span_value(desc.source_span)),
        ]),
    )
}

pub(super) fn state_record(desc: &StateDesc) -> ReflectValue {
    record(
        "ReflectState",
        BTreeMap::from([
            (
                "id".to_owned(),
                int_value(i64::try_from(desc.id.get()).unwrap_or(i64::MAX)),
            ),
            ("name".to_owned(), string(desc.name.clone())),
            ("module".to_owned(), optional_string(desc.module.as_deref())),
            ("public".to_owned(), bool_value(desc.public)),
            ("storage".to_owned(), string(desc.storage.as_str())),
            ("type".to_owned(), string(desc.type_contract.clone())),
            (
                "type_desc".to_owned(),
                optional_type_hint_desc(Some(&desc.type_contract)),
            ),
            (
                "has_initializer".to_owned(),
                bool_value(desc.has_initializer),
            ),
            ("origin".to_owned(), origin_value(desc.origin)),
            ("source_span".to_owned(), span_value(desc.source_span)),
        ]),
    )
}

fn origin_value(origin: DeclOrigin) -> ReflectValue {
    string(origin.as_str())
}

fn function_effects_record(desc: &FunctionDesc) -> ReflectValue {
    record(
        "ReflectEffectSet",
        BTreeMap::from([
            ("reads_host".to_owned(), bool_value(desc.effects.reads_host)),
            (
                "writes_host".to_owned(),
                bool_value(desc.effects.writes_host),
            ),
            (
                "emits_events".to_owned(),
                bool_value(desc.effects.emits_events),
            ),
            ("reads_time".to_owned(), bool_value(desc.effects.reads_time)),
            (
                "uses_random".to_owned(),
                bool_value(desc.effects.uses_random),
            ),
            ("reads_io".to_owned(), bool_value(desc.effects.reads_io)),
            ("writes_io".to_owned(), bool_value(desc.effects.writes_io)),
            (
                "reads_reflection".to_owned(),
                bool_value(desc.effects.reads_reflection),
            ),
            (
                "writes_reflection".to_owned(),
                bool_value(desc.effects.writes_reflection),
            ),
            (
                "calls_reflection".to_owned(),
                bool_value(desc.effects.calls_reflection),
            ),
            (
                "spawns_tasks".to_owned(),
                bool_value(desc.effects.spawns_tasks),
            ),
        ]),
    )
}

fn function_access_record(desc: &FunctionDesc) -> ReflectValue {
    record(
        "ReflectFunctionAccess",
        BTreeMap::from([
            ("public".to_owned(), bool_value(desc.access.public)),
            (
                "reflect_visible".to_owned(),
                bool_value(desc.access.reflect_visible),
            ),
            (
                "reflect_callable".to_owned(),
                bool_value(desc.access.reflect_callable),
            ),
            (
                "required_permissions".to_owned(),
                array(
                    desc.access
                        .required_permissions()
                        .iter()
                        .map(|permission| string(permission.clone())),
                ),
            ),
        ]),
    )
}

fn param_record(param: &FunctionParamDesc) -> ReflectValue {
    record(
        "ReflectParam",
        BTreeMap::from([
            ("name".to_owned(), string(param.name.clone())),
            (
                "type".to_owned(),
                optional_string(param.type_hint.as_deref()),
            ),
            (
                "type_desc".to_owned(),
                optional_type_hint_desc(param.type_hint.as_deref()),
            ),
            ("defaulted".to_owned(), bool_value(param.has_default)),
        ]),
    )
}

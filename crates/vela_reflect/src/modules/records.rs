use std::collections::BTreeMap;

use vela_host::value::HostValue;

use super::{DeclOrigin, FunctionDesc, FunctionParamDesc, ModuleDesc};
use crate::{
    metadata::{attrs_value, docs_value, span_value},
    value::ReflectValue,
};

pub(super) fn module_record(desc: &ModuleDesc) -> ReflectValue {
    ReflectValue::Host(module_record_host(desc))
}

pub(super) fn module_record_host(desc: &ModuleDesc) -> HostValue {
    module_record_host_with_exports(desc, desc.exports.iter().map(|export| export.name.clone()))
}

pub(super) fn module_record_with_exports(
    desc: &ModuleDesc,
    exports: impl IntoIterator<Item = String>,
) -> ReflectValue {
    ReflectValue::Host(module_record_host_with_exports(desc, exports))
}

pub(super) fn module_record_host_with_exports(
    desc: &ModuleDesc,
    exports: impl IntoIterator<Item = String>,
) -> HostValue {
    let mut fields = BTreeMap::new();
    fields.insert("name".to_owned(), HostValue::String(desc.name.clone()));
    fields.insert("origin".to_owned(), origin_value(desc.origin));
    fields.insert(
        "exports".to_owned(),
        HostValue::Array(exports.into_iter().map(HostValue::String).collect()),
    );
    fields.insert("docs".to_owned(), docs_value(desc.docs.as_deref()));
    fields.insert("attrs".to_owned(), attrs_value(&desc.attrs));
    fields.insert("source_span".to_owned(), span_value(desc.source_span));
    HostValue::Record {
        type_name: "ReflectModule".to_owned(),
        fields,
    }
}

pub(super) fn function_record(desc: &FunctionDesc) -> ReflectValue {
    ReflectValue::Host(function_record_host(desc))
}

pub(super) fn function_record_host(desc: &FunctionDesc) -> HostValue {
    let mut fields = BTreeMap::new();
    fields.insert(
        "id".to_owned(),
        // TODO(reflect): stable IDs are u64, but reflection currently exposes IDs
        // through signed script ints. Replace this lossy saturation with a deliberate
        // unsigned/ID value surface before treating reflect::id() as a stable public
        // identity API.
        HostValue::Int(i64::try_from(desc.id.get()).unwrap_or(i64::MAX)),
    );
    fields.insert("name".to_owned(), HostValue::String(desc.name.clone()));
    fields.insert(
        "module".to_owned(),
        desc.module
            .as_ref()
            .map_or(HostValue::Null, |module| HostValue::String(module.clone())),
    );
    fields.insert("public".to_owned(), HostValue::Bool(desc.public));
    fields.insert("effects".to_owned(), function_effects_record(desc));
    fields.insert("access".to_owned(), function_access_record(desc));
    fields.insert("origin".to_owned(), origin_value(desc.origin));
    fields.insert(
        "return".to_owned(),
        desc.return_type
            .as_ref()
            .map_or(HostValue::Null, |return_type| {
                HostValue::String(return_type.clone())
            }),
    );
    fields.insert(
        "params".to_owned(),
        HostValue::Array(desc.params.iter().map(param_record).collect()),
    );
    fields.insert("docs".to_owned(), docs_value(desc.docs.as_deref()));
    fields.insert("attrs".to_owned(), attrs_value(&desc.attrs));
    fields.insert("source_span".to_owned(), span_value(desc.source_span));
    HostValue::Record {
        type_name: "ReflectFunction".to_owned(),
        fields,
    }
}

fn origin_value(origin: DeclOrigin) -> HostValue {
    HostValue::String(origin.as_str().to_owned())
}

fn function_effects_record(desc: &FunctionDesc) -> HostValue {
    HostValue::Record {
        type_name: "ReflectEffectSet".to_owned(),
        fields: BTreeMap::from([
            (
                "reads_host".to_owned(),
                HostValue::Bool(desc.effects.reads_host),
            ),
            (
                "writes_host".to_owned(),
                HostValue::Bool(desc.effects.writes_host),
            ),
            (
                "emits_events".to_owned(),
                HostValue::Bool(desc.effects.emits_events),
            ),
            (
                "reads_time".to_owned(),
                HostValue::Bool(desc.effects.reads_time),
            ),
            (
                "uses_random".to_owned(),
                HostValue::Bool(desc.effects.uses_random),
            ),
            (
                "reads_reflection".to_owned(),
                HostValue::Bool(desc.effects.reads_reflection),
            ),
            (
                "writes_reflection".to_owned(),
                HostValue::Bool(desc.effects.writes_reflection),
            ),
            (
                "calls_reflection".to_owned(),
                HostValue::Bool(desc.effects.calls_reflection),
            ),
        ]),
    }
}

fn function_access_record(desc: &FunctionDesc) -> HostValue {
    HostValue::Record {
        type_name: "ReflectFunctionAccess".to_owned(),
        fields: BTreeMap::from([
            ("public".to_owned(), HostValue::Bool(desc.access.public)),
            (
                "reflect_visible".to_owned(),
                HostValue::Bool(desc.access.reflect_visible),
            ),
            (
                "reflect_callable".to_owned(),
                HostValue::Bool(desc.access.reflect_callable),
            ),
            (
                "required_permissions".to_owned(),
                HostValue::Array(
                    desc.access
                        .required_permissions()
                        .iter()
                        .map(|permission| HostValue::String(permission.clone()))
                        .collect(),
                ),
            ),
        ]),
    }
}

fn param_record(param: &FunctionParamDesc) -> HostValue {
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

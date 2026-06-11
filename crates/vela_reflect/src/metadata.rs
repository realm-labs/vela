use std::collections::BTreeMap;

use vela_common::Span;
use vela_host::value::HostValue;

use crate::{registry::AttrMap, value::ReflectValue};

pub(crate) fn host(value: HostValue) -> ReflectValue {
    ReflectValue::Host(value)
}

pub(crate) fn string(value: impl Into<String>) -> ReflectValue {
    host(HostValue::String(value.into()))
}

pub(crate) fn bool_value(value: bool) -> ReflectValue {
    host(HostValue::Bool(value))
}

pub(crate) fn int_value(value: i64) -> ReflectValue {
    host(HostValue::Scalar(vela_common::ScalarValue::I64(value)))
}

pub(crate) fn null_value() -> ReflectValue {
    host(HostValue::Null)
}

pub(crate) fn array(values: impl IntoIterator<Item = ReflectValue>) -> ReflectValue {
    ReflectValue::Array(values.into_iter().collect())
}

pub(crate) fn record(
    type_name: impl Into<String>,
    fields: BTreeMap<String, ReflectValue>,
) -> ReflectValue {
    ReflectValue::ScriptRecord {
        type_name: type_name.into(),
        fields,
    }
}

pub(crate) fn attrs_value(attrs: &AttrMap) -> ReflectValue {
    ReflectValue::Record(
        attrs
            .iter()
            .map(|(key, value)| (key.to_owned(), string(value.to_owned())))
            .collect::<BTreeMap<_, _>>(),
    )
}

pub(crate) fn docs_value(docs: Option<&str>) -> ReflectValue {
    docs.map_or_else(null_value, string)
}

pub(crate) fn span_value(span: Option<Span>) -> ReflectValue {
    span.map_or_else(null_value, |span| {
        record(
            "ReflectSourceSpan",
            BTreeMap::from([
                ("source".to_owned(), int_value(i64::from(span.source.get()))),
                ("start".to_owned(), int_value(i64::from(span.start))),
                ("end".to_owned(), int_value(i64::from(span.end))),
            ]),
        )
    })
}

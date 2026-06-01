use std::collections::BTreeMap;

use vela_common::Span;
use vela_host::value::HostValue;

use crate::registry::AttrMap;

pub(crate) fn attrs_value(attrs: &AttrMap) -> HostValue {
    HostValue::Map(
        attrs
            .iter()
            .map(|(key, value)| (key.to_owned(), HostValue::String(value.to_owned())))
            .collect::<BTreeMap<_, _>>(),
    )
}

pub(crate) fn docs_value(docs: Option<&str>) -> HostValue {
    docs.map_or(HostValue::Null, |docs| HostValue::String(docs.to_owned()))
}

pub(crate) fn span_value(span: Option<Span>) -> HostValue {
    span.map_or(HostValue::Null, |span| HostValue::Record {
        type_name: "ReflectSourceSpan".to_owned(),
        fields: BTreeMap::from([
            (
                "source".to_owned(),
                HostValue::Int(i64::from(span.source.get())),
            ),
            ("start".to_owned(), HostValue::Int(i64::from(span.start))),
            ("end".to_owned(), HostValue::Int(i64::from(span.end))),
        ]),
    })
}

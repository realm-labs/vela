use std::collections::BTreeMap;

use vela_host::HostValue;

use crate::AttrMap;

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

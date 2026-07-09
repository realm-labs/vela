use std::collections::BTreeMap;

use vela_common::{PrimitiveTag, Span};
use vela_host::value::HostValue;
use vela_registry::TypeHintDef;

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

pub(crate) fn option_some(value: ReflectValue) -> ReflectValue {
    ReflectValue::ScriptEnum {
        enum_name: "Option".to_owned(),
        variant: "Some".to_owned(),
        fields: BTreeMap::from([("0".to_owned(), value)]),
    }
}

pub(crate) fn option_none() -> ReflectValue {
    ReflectValue::ScriptEnum {
        enum_name: "Option".to_owned(),
        variant: "None".to_owned(),
        fields: BTreeMap::new(),
    }
}

pub(crate) fn optional_string(value: Option<&str>) -> ReflectValue {
    value.map_or_else(option_none, |value| option_some(string(value)))
}

pub(crate) fn optional_type_hint_desc(value: Option<&str>) -> ReflectValue {
    value
        .filter(|value| !value.is_empty())
        .and_then(TypeHintDef::parse)
        .map_or_else(option_none, |hint| option_some(type_hint_desc(&hint)))
}

fn type_hint_desc(hint: &TypeHintDef) -> ReflectValue {
    let display = hint.display();
    record(
        "ReflectTypeHint",
        BTreeMap::from([
            ("display".to_owned(), string(display.clone())),
            ("kind".to_owned(), string(type_hint_kind(hint, &display))),
            (
                "name".to_owned(),
                optional_string(type_hint_name(hint, &display)),
            ),
            (
                "args".to_owned(),
                array(hint.args.iter().map(type_hint_desc)),
            ),
        ]),
    )
}

fn type_hint_kind(hint: &TypeHintDef, display: &str) -> &'static str {
    if hint.path.as_slice() == ["()"] {
        return if hint.args.is_empty() {
            "unit"
        } else {
            "tuple"
        };
    }
    if PrimitiveTag::from_name(display).is_some() || matches!(display, "String" | "Bytes") {
        return "primitive";
    }
    if let [name] = hint.path.as_slice() {
        return match name.as_str() {
            "Option" | "Result" => "enum",
            "Array" | "Map" | "Set" | "Iterator" => "container",
            "Function" | "Closure" | "Any" => "builtin",
            _ => "named",
        };
    }
    "named"
}

fn type_hint_name<'a>(hint: &'a TypeHintDef, display: &'a str) -> Option<&'a str> {
    if hint.path.as_slice() == ["()"] {
        return hint.args.is_empty().then_some("()");
    }
    if let [name] = hint.path.as_slice() {
        return Some(name);
    }
    Some(display)
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
    optional_string(docs)
}

pub(crate) fn span_value(span: Option<Span>) -> ReflectValue {
    span.map_or_else(option_none, |span| {
        option_some(record(
            "ReflectSourceSpan",
            BTreeMap::from([
                ("source".to_owned(), int_value(i64::from(span.source.get()))),
                ("start".to_owned(), int_value(i64::from(span.start))),
                ("end".to_owned(), int_value(i64::from(span.end))),
            ]),
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host_string(value: &str) -> ReflectValue {
        ReflectValue::Host(HostValue::String(value.to_owned()))
    }

    fn some_record(value: ReflectValue) -> ReflectValue {
        option_some(value)
    }

    #[test]
    fn type_hint_descriptors_preserve_unit_option_and_tuple_structure() {
        assert_eq!(
            optional_type_hint_desc(Some("()")),
            some_record(record(
                "ReflectTypeHint",
                BTreeMap::from([
                    ("display".to_owned(), host_string("()")),
                    ("kind".to_owned(), host_string("unit")),
                    ("name".to_owned(), optional_string(Some("()"))),
                    ("args".to_owned(), ReflectValue::Array(vec![])),
                ]),
            ))
        );

        let descriptor = optional_type_hint_desc(Some("Option<(String, String)>"));
        let ReflectValue::ScriptEnum {
            enum_name,
            variant,
            fields,
        } = descriptor
        else {
            panic!("type descriptor should be Option::Some");
        };
        assert_eq!(enum_name, "Option");
        assert_eq!(variant, "Some");
        let Some(ReflectValue::ScriptRecord {
            type_name,
            fields: option,
        }) = fields.get("0")
        else {
            panic!("type descriptor payload should be a record");
        };
        assert_eq!(type_name, "ReflectTypeHint");
        assert_eq!(
            option.get("display"),
            Some(&host_string("Option<(String, String)>"))
        );
        assert_eq!(option.get("kind"), Some(&host_string("enum")));
        let Some(ReflectValue::Array(option_args)) = option.get("args") else {
            panic!("option descriptor args should be an array");
        };
        assert_eq!(option_args.len(), 1);
        let ReflectValue::ScriptRecord {
            type_name,
            fields: tuple,
        } = &option_args[0]
        else {
            panic!("option payload descriptor should be a record");
        };
        assert_eq!(type_name, "ReflectTypeHint");
        assert_eq!(tuple.get("display"), Some(&host_string("(String, String)")));
        assert_eq!(tuple.get("kind"), Some(&host_string("tuple")));
        assert_eq!(tuple.get("name"), Some(&option_none()));
        let Some(ReflectValue::Array(tuple_args)) = tuple.get("args") else {
            panic!("tuple descriptor args should be an array");
        };
        assert_eq!(tuple_args.len(), 2);
        for element in tuple_args {
            let ReflectValue::ScriptRecord { fields, .. } = element else {
                panic!("tuple element descriptor should be a record");
            };
            assert_eq!(fields.get("kind"), Some(&host_string("primitive")));
            assert_eq!(fields.get("name"), Some(&optional_string(Some("String"))));
        }
    }

    #[test]
    fn type_hint_descriptors_keep_absence_as_option_none() {
        assert_eq!(optional_type_hint_desc(None), option_none());
        assert_eq!(optional_type_hint_desc(Some("")), option_none());
        assert_eq!(optional_type_hint_desc(Some("(i64,)")), option_none());
    }
}

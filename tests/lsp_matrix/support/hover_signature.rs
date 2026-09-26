//! Marker-only coordinate and authored result oracles. No provider or production
//! line index participates in the expected values.
use super::{Document, Point, Spec, load};
use serde_json::{Value, json};

pub(crate) fn spec(crlf: bool, shifted: bool) -> Spec {
    let mut spec = load("hover-signature-coordinates");
    if shifted {
        let prefix = spec.oracle["prefix"].as_str().expect("Unicode prefix");
        spec.files.insert(
            "scripts/main.vela".into(),
            format!("{prefix}{}", spec.files["scripts/main.vela"]),
        );
    }
    if crlf {
        for text in spec.files.values_mut() {
            *text = text.replace('\n', "\r\n");
        }
    }
    spec
}

fn point(document: &Document, point: Point, utf16: bool) -> Value {
    let character = if utf16 {
        point.character
    } else {
        point.byte
            - document.text[..point.byte]
                .rfind('\n')
                .map_or(0, |index| index + 1)
    };
    json!({"line":point.line,"character":character})
}

pub(crate) fn position(document: &Document, marker: &str, utf16: bool, offset: usize) -> Value {
    let mut value = point(document, document.markers[marker].start, utf16);
    value["character"] = json!(
        value["character"].as_u64().expect("column") + u64::try_from(offset).expect("offset")
    );
    value
}

pub(crate) fn hover_result(document: &Document, query: &Value, utf16: bool) -> Value {
    let result = &query["result"];
    if result.is_null() {
        return Value::Null;
    }
    let marker = document.markers[query["marker"].as_str().expect("hover marker")];
    let range =
        json!({"start":point(document,marker.start,utf16),"end":point(document,marker.end,utf16)});
    if utf16 {
        json!({"contents":{"kind":"markdown","value":result["markdown"]},"range":range})
    } else {
        json!({"label":result["label"],"kind":result["kind"],"detail":result["detail"],"docs":result["docs"],"range":range})
    }
}

pub(crate) fn signature_result(query: &Value, protocol: bool) -> Value {
    let result = &query["result"];
    if result.is_null() || !protocol {
        return result.clone();
    }
    let signatures = result["signatures"]
        .as_array()
        .expect("signatures")
        .iter()
        .map(|signature| {
            let parameters = signature["parameters"]
                .as_array()
                .expect("parameters")
                .iter()
                .map(|parameter| json!({"label":parameter["label"]}))
                .collect::<Vec<_>>();
            json!({"label":signature["label"],"parameters":parameters})
        })
        .collect::<Vec<_>>();
    json!({"activeSignature":result["activeSignature"],"activeParameter":result["activeParameter"],"signatures":signatures})
}

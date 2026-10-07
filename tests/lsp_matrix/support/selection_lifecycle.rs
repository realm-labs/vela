//! Literal CST, selection and diagnostic expectations for source lifecycles.
use super::{Action, FixtureWorkspace, Point, Spec, load};
use serde_json::{Value, json};

pub(crate) use super::selection::{document, expected, positions};
pub(crate) use super::selection_recovery::assert_cst;

pub(crate) fn spec(crlf: bool, shifted: bool) -> Spec {
    let mut spec = load("selection-lifecycle");
    let transform = |text: &str| {
        let text = if shifted {
            text.replacen(
                "[[file:start]]",
                "[[file:start]]// shifted 中😀\n/* second 😀 */\n",
                1,
            )
        } else {
            text.to_owned()
        };
        if crlf {
            text.replace('\n', "\r\n")
        } else {
            text
        }
    };
    for text in spec.files.values_mut() {
        *text = transform(text);
    }
    for variant in spec.oracle["variants"]
        .as_object_mut()
        .expect("variants")
        .values_mut()
    {
        variant["source"] = json!(transform(variant["source"].as_str().expect("source")));
    }
    assert_eq!(
        spec.oracle["variants"].as_object().expect("variants").len(),
        11
    );
    assert_eq!(spec.oracle["phases"].as_array().expect("phases").len(), 43);
    spec
}

pub(crate) fn actions(spec: &Spec, phase: &Value) -> Vec<Action> {
    phase["actions"]
        .as_array()
        .expect("actions")
        .iter()
        .map(|row| Action {
            op: row["op"].as_str().expect("op").to_owned(),
            file: row["file"].as_str().expect("file").to_owned(),
            source: row["variant"].as_str().map(|id| {
                spec.oracle["variants"][id]["source"]
                    .as_str()
                    .expect("source")
                    .to_owned()
            }),
        })
        .collect()
}

pub(crate) fn fresh(spec: &Spec, phase: &Value) -> FixtureWorkspace {
    let mut model = spec.clone();
    model.files.clear();
    for (file, id) in phase["disk"].as_object().expect("disk") {
        if let Some(id) = id.as_str() {
            model.files.insert(
                file.clone(),
                spec.oracle["variants"][id]["source"]
                    .as_str()
                    .expect("source")
                    .to_owned(),
            );
        }
    }
    let mut fixture = FixtureWorkspace::new(&model).expect("literal disk state");
    for (file, id) in phase["open"].as_object().expect("overlays") {
        fixture.open.insert(
            file.clone(),
            document(&spec.oracle["variants"][id.as_str().expect("variant")]),
        );
    }
    fixture
}

pub(crate) fn assert_state(fixture: &FixtureWorkspace, spec: &Spec, phase: &Value) {
    let expected = fresh(spec, phase);
    assert_eq!(fixture.disk, expected.disk, "whole disk {}", phase["id"]);
    assert_eq!(fixture.open, expected.open, "whole overlay {}", phase["id"]);
    for file in phase["views"].as_object().expect("views").keys() {
        assert_eq!(fixture.document(file), expected.document(file));
    }
}

pub(crate) fn syntax_diagnostics(variant: &Value, protocol: bool) -> Value {
    let doc = document(variant);
    let point = |p: Point| json!({"line":p.line,"character":if protocol { p.character } else { super::selection::byte_column(&doc, p) }});
    Value::Array(variant["diagnostics"].as_array().expect("exact diagnostic set").iter().map(|row| {
        let span = doc.markers[row["range"].as_str().expect("diagnostic marker")];
        json!({"code":row["code"],"message":row["message"],"severity":1,"source":"vela","range":{"start":point(span.start),"end":point(span.end)}})
    }).collect())
}

pub(crate) fn queries(variant: Option<&Value>, protocol: bool) -> (Value, Value) {
    variant.map_or_else(|| (json!([{"line":0,"character":0},{"line":0,"character":0}]),
        json!([{"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":0}}},{"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":0}}}])), |variant| {
        let doc = document(variant);
        (positions(&doc, &variant["queries"], protocol), expected(&doc, &variant["queries"], protocol))
    })
}

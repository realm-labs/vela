//! Independent source and overlay state oracles for complete outline trees.
use super::{Action, Document, FixtureWorkspace, Spec, load, parse_markers};
use serde_json::Value;

pub(crate) fn spec(crlf: bool) -> Spec {
    let mut spec = load("document-symbol-lifecycle");
    if crlf {
        for source in spec.files.values_mut() {
            *source = source.replace('\n', "\r\n");
        }
        for variant in spec.oracle["variants"]
            .as_object_mut()
            .expect("variants")
            .values_mut()
        {
            variant["source"] = Value::String(
                variant["source"]
                    .as_str()
                    .expect("source")
                    .replace('\n', "\r\n"),
            );
        }
    }
    let document = document(&spec, "main-base");
    let name = document.markers["main-fn-name"];
    assert_eq!(
        (name.start.line, name.start.character, name.end.character),
        (1, 13, 16)
    );
    assert_eq!(
        name.start.byte - document.text[..name.start.byte].rfind('\n').expect("line") - 1,
        17
    );
    spec
}

pub(crate) fn document(spec: &Spec, variant: &str) -> Document {
    parse_markers(
        spec.oracle["variants"][variant]["source"]
            .as_str()
            .expect("authored source"),
    )
    .expect("marked document")
}

pub(crate) fn actions(spec: &Spec, phase: &Value) -> Vec<Action> {
    phase["actions"]
        .as_array()
        .expect("actions")
        .iter()
        .map(|a| Action {
            op: a["op"].as_str().expect("operation").to_owned(),
            file: a["file"].as_str().expect("file").to_owned(),
            source: a["variant"].as_str().map(|v| {
                spec.oracle["variants"][v]["source"]
                    .as_str()
                    .expect("source")
                    .to_owned()
            }),
        })
        .collect()
}

pub(crate) fn fresh_fixture(spec: &Spec, phase: &Value) -> FixtureWorkspace {
    let mut current = spec.clone();
    for (file, variant) in phase["disk"].as_object().expect("disk") {
        if let Some(variant) = variant.as_str() {
            current.files.insert(
                file.clone(),
                spec.oracle["variants"][variant]["source"]
                    .as_str()
                    .expect("source")
                    .to_owned(),
            );
        } else {
            current.files.remove(file);
        }
    }
    let mut fixture = FixtureWorkspace::new(&current).expect("fresh fixture");
    for (file, variant) in phase["open"].as_object().expect("overlays") {
        fixture.open.insert(
            file.clone(),
            document(spec, variant.as_str().expect("variant")),
        );
    }
    fixture
}

pub(crate) fn assert_state(fixture: &FixtureWorkspace, spec: &Spec, phase: &Value) {
    let fresh = fresh_fixture(spec, phase);
    for file in phase["views"].as_object().expect("views").keys() {
        assert_eq!(
            fixture.disk.get(file),
            fresh.disk.get(file),
            "disk {}: {file}",
            phase["id"]
        );
        assert_eq!(
            fixture.open.get(file),
            fresh.open.get(file),
            "overlay {}: {file}",
            phase["id"]
        );
        assert_eq!(
            fixture.document(file),
            fresh.document(file),
            "effective {}: {file}",
            phase["id"]
        );
    }
}

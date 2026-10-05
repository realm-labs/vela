//! Independent complete source state and coordinate oracles; no provider calls.
use super::{Document, FixtureWorkspace, Spec, load, parse_markers};
use serde_json::Value;

pub(crate) fn spec(crlf: bool, shifted: bool) -> Spec {
    let mut spec = load("workspace-symbol-sources");
    let transform = |source: &str| {
        let text = if shifted {
            source.replace(
                "[[file:start]]",
                "[[file:start]]// shifted 中😀\n/* extra 😀 */\n",
            )
        } else {
            source.to_owned()
        };
        if crlf {
            text.replace('\n', "\r\n")
        } else {
            text
        }
    };
    for source in spec.files.values_mut() {
        *source = transform(source);
    }
    for source in spec.oracle["variants"]
        .as_object_mut()
        .expect("literal variants")
        .values_mut()
    {
        *source = Value::String(transform(source.as_str().expect("marked source")));
    }
    for (id, source) in spec.oracle["variants"].as_object().expect("variants") {
        let doc = parse_markers(source.as_str().expect("source")).expect("markers");
        let name = doc.markers["decl-name"];
        let constant = id == "helper";
        let main = ["main", "dirty", "disk"].contains(&id.as_str());
        assert_eq!(
            (name.start.line, name.start.character),
            (
                usize::from(main) + usize::from(shifted) * 2,
                if constant { 18 } else { 15 }
            )
        );
        assert_eq!(
            name.start.byte - doc.text[..name.start.byte].rfind('\n').map_or(0, |i| i + 1),
            if constant { 22 } else { 19 }
        );
        assert_eq!(
            (doc.markers["file"].start.byte, doc.markers["file"].end.byte),
            (0, doc.text.len())
        );
    }
    assert_eq!(
        spec.oracle["sequences"]["workspace"]
            .as_array()
            .expect("workspace phases")
            .len(),
        14
    );
    assert_eq!(
        spec.oracle["sequences"]["scratch"]
            .as_array()
            .expect("scratch phases")
            .len(),
        5
    );
    spec
}

pub(crate) fn document(spec: &Spec, variant: &str) -> Document {
    parse_markers(spec.oracle["variants"][variant].as_str().expect("variant"))
        .expect("authored markers")
}

pub(crate) fn fixture(spec: &Spec, phase: &Value) -> FixtureWorkspace {
    let mut fixture = FixtureWorkspace {
        disk: Default::default(),
        open: Default::default(),
    };
    for (file, variant) in phase["disk"].as_object().expect("physical disk state") {
        fixture.disk.insert(
            file.clone(),
            document(spec, variant.as_str().expect("variant")),
        );
    }
    for (file, variant) in phase["open"].as_object().expect("overlay state") {
        fixture.open.insert(
            file.clone(),
            document(spec, variant.as_str().expect("variant")),
        );
    }
    assert!(!fixture.disk.contains_key("outside/scratch.vela"));
    fixture
}

pub(crate) fn effective(fixture: &FixtureWorkspace) -> FixtureWorkspace {
    let mut effective = fixture.clone();
    for (file, doc) in &fixture.open {
        effective.disk.insert(file.clone(), doc.clone());
    }
    effective
}

pub(crate) fn owned_files(phase: &Value) -> std::collections::BTreeSet<String> {
    phase["workspace"]["symbols"]
        .as_array()
        .expect("complete rows")
        .iter()
        .map(|row| row["file"].as_str().expect("owned file").to_owned())
        .collect()
}

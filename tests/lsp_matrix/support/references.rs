use super::{FixtureWorkspace, Spec, load};
use serde_json::Value;

pub(crate) fn spec(crlf: bool) -> Spec {
    let mut spec = load("reference-rename-coordinates");
    if crlf {
        for text in spec.files.values_mut() {
            *text = text.replace('\n', "\r\n");
        }
    }
    spec
}

pub(crate) fn sites(spec: &Spec, query: &Value) -> Vec<Value> {
    query["group"].as_str().map_or_else(Vec::new, |group| {
        spec.oracle["groups"][group]["sites"]
            .as_array()
            .expect("reference sites")
            .clone()
    })
}

pub(crate) fn renamed(spec: &Spec, group: &str, new_name: &str) -> Spec {
    let mut result = spec.clone();
    let definition = &spec.oracle["groups"][group];
    let old_name = definition["name"].as_str().expect("old name");
    for site in definition["sites"].as_array().expect("sites") {
        let file = site["file"].as_str().expect("file");
        let marker = site["marker"].as_str().expect("marker");
        let old = format!("[[{marker}:start]]{old_name}[[{marker}:end]]");
        let new = format!("[[{marker}:start]]{new_name}[[{marker}:end]]");
        let source = result.files.get_mut(file).expect("source");
        assert_eq!(source.matches(&old).count(), 1, "independent marked edit");
        *source = source.replace(&old, &new);
    }
    result.oracle["groups"][group]["name"] = new_name.into();
    if let Some(qualified) = definition["qualified"].as_str() {
        let (owner, _) = qualified.rsplit_once("::").expect("qualified owner");
        result.oracle["groups"][group]["qualified"] = format!("{owner}::{new_name}").into();
    }
    result
}

pub(crate) fn assert_parsed(fixture: &FixtureWorkspace) {
    for (file, document) in &fixture.disk {
        if file.ends_with(".vela") {
            assert!(
                vela_syntax::parse::parse_source(&document.text)
                    .diagnostics()
                    .is_empty(),
                "{file}"
            );
        }
    }
}

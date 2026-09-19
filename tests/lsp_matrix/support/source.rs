use super::{Action, FixtureWorkspace, Spec, parse_markers, safe_file};
use serde_json::Value;

pub(crate) fn source_lifecycle_action(phase: &Value, crlf: bool) -> Option<Action> {
    if phase["action"].is_null() {
        return None;
    }
    let mut action: Action =
        serde_json::from_value(phase["action"].clone()).expect("source action");
    safe_file(&action.file).expect("fixture-relative action");
    assert!(action.file.ends_with(".vela"));
    if let Some(source) = &mut action.source {
        *source = source.replace('\n', if crlf { "\r\n" } else { "\n" });
    }
    Some(action)
}

pub(crate) fn assert_source_overlay_state(
    actual: &FixtureWorkspace,
    expected: &FixtureWorkspace,
    phase: &Value,
    crlf: bool,
) {
    for file in phase["files"].as_object().expect("source files").keys() {
        assert_eq!(
            actual.document(file),
            expected.document(file),
            "effective source: {phase}"
        );
    }
    let file = phase["overlayFile"].as_str().expect("overlay file");
    for (label, document) in [
        ("diskSource", actual.disk.get(file)),
        ("openSource", actual.open.get(file)),
    ] {
        let expected = phase[label].as_str().map(|source| {
            parse_markers(&source.replace('\n', if crlf { "\r\n" } else { "\n" }))
                .expect("independent source oracle")
        });
        assert_eq!(document, expected.as_ref(), "{label}: {phase}");
    }
}

/// Phase sources and their marker locations are authored independently of the
/// service. Each phase describes a full replacement of the listed files.
pub(crate) fn source_lifecycle_spec(spec: &Spec, phase: &Value, crlf: bool) -> Spec {
    let mut current = spec.clone();
    for (file, source) in phase["files"].as_object().expect("source files") {
        safe_file(file).expect("fixture-relative source path");
        assert!(file.ends_with(".vela"), "source-only transition");
        if let Some(source) = source.as_str() {
            current.files.insert(
                file.clone(),
                source.replace('\n', if crlf { "\r\n" } else { "\n" }),
            );
        } else {
            assert!(source.is_null(), "source text or deletion");
            current.files.remove(file);
        }
    }
    current.oracle["queries"] = phase["queries"].clone();
    current
}

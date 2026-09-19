use super::{Spec, safe_file};
use serde_json::Value;

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

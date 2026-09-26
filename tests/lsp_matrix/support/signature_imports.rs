//! Authored package ownership and lifecycle signature oracles. No production
//! provider, type conversion, line index or package resolver supplies expectations.
use super::{Action, FixtureWorkspace, Spec, hover_signature, load};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

pub(crate) fn spec(crlf: bool) -> Spec {
    let mut spec = load("signature-s8");
    if crlf {
        for text in spec.files.values_mut() {
            *text = text.replace('\n', "\r\n");
        }
        for phase in spec.oracle["phases"].as_array_mut().expect("phases") {
            for action in phase["actions"].as_array_mut().expect("actions") {
                if let Some(source) = action["source"].as_str() {
                    action["source"] = json!(source.replace('\n', "\r\n"));
                }
            }
        }
    }
    spec
}

pub(crate) fn actions(phase: &Value) -> Vec<Action> {
    serde_json::from_value(phase["actions"].clone()).expect("authored actions")
}

pub(crate) fn cases(spec: &Spec, phase: &Value) -> Vec<Value> {
    spec.oracle["queries"]
        .as_array()
        .expect("queries")
        .iter()
        .map(|base| {
            let mut query = base.clone();
            if let Some(overrides) =
                phase["overrides"][base["id"].as_str().expect("id")].as_object()
            {
                query
                    .as_object_mut()
                    .expect("query")
                    .extend(overrides.clone());
            }
            let signature = &spec.oracle["signatures"][query["signature"].as_str().unwrap_or("")];
            query["result"] = if query["signature"].is_null() {
                Value::Null
            } else {
                assert!(signature.is_object(), "missing authored signature: {query}");
                json!({"activeSignature":0,"activeParameter":query["active"],"signatures":[
                    {"label":signature["label"],"parameters":signature["parameters"]}
                ]})
            };
            query["owner"] = signature["owner"].clone();
            query["named"] = signature["named"].clone();
            if query.get("definition").is_none() {
                query["definition"] = signature["target"].clone();
            }
            query
        })
        .collect()
}

pub(crate) fn definition(
    fixture: &FixtureWorkspace,
    case: &Value,
    utf16: bool,
    uri: impl Fn(&str) -> String,
) -> Value {
    let target = &case["definition"];
    if target.is_null() {
        return Value::Null;
    }
    let file = target["file"].as_str().expect("target file");
    let source = fixture.document(file).expect("target document");
    let marker_name = target["marker"].as_str().expect("target marker");
    let marker = source.markers[marker_name];
    let start = hover_signature::position(source, marker_name, utf16, 0);
    let end_column = if utf16 {
        marker.end.character
    } else {
        marker.end.byte
            - source.text[..marker.end.byte]
                .rfind('\n')
                .map_or(0, |i| i + 1)
    };
    json!({"uri":uri(file),"range":{"start":start,"end":{"line":marker.end.line,"character":end_column}}})
}

/// Every root is freshly and atomically created below temp; Drop owns only it.
pub(crate) struct Layout {
    pub root: PathBuf,
}

impl Layout {
    pub fn new(fixture: &FixtureWorkspace) -> Self {
        let root = std::env::temp_dir().join(format!(
            "vela-signature-s8-中 % {}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fixture.materialize(&root).expect("isolated fixture root");
        Self { root }
    }

    pub fn apply(&self, fixture: &FixtureWorkspace, action: &Action) {
        let path = self.root.join(&action.file);
        match action.op.as_str() {
            "write" => {
                fs::create_dir_all(path.parent().expect("parent")).expect("fixture directory");
                fs::write(path, &fixture.disk[&action.file].text).expect("disk edit");
            }
            "delete" => fs::remove_file(path).expect("disk deletion"),
            "open" | "change" | "close" => {}
            _ => panic!("unsupported signature action"),
        }
    }

    pub fn assert_disk(&self, fixture: &FixtureWorkspace, spec: &Spec) {
        let mut files = spec
            .files
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        for phase in spec.oracle["phases"].as_array().expect("phases") {
            files.extend(actions(phase).into_iter().map(|action| action.file));
        }
        for file in files.into_iter().filter(|file| file.ends_with(".vela")) {
            if let Some(document) = fixture.disk.get(&file) {
                assert_eq!(
                    fs::read_to_string(self.root.join(&file)).expect("disk source"),
                    document.text,
                    "disk bytes: {file}"
                );
            } else {
                assert!(!self.root.join(&file).exists(), "deleted source: {file}");
            }
        }
    }
}

impl Drop for Layout {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).expect("cleanup owned fixture root");
    }
}

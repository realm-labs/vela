//! Independent authored phase expectations. This module does not query providers
//! or derive expected metadata from analysis or source indexes.
use super::{Spec, load};
use serde_json::Value;

pub(crate) fn spec(name: &str, crlf: bool, missing_schema: bool) -> Spec {
    let mut spec = load(name);
    if crlf {
        for source in spec.files.values_mut() {
            *source = source.replace('\n', "\r\n");
        }
        for phase in spec.oracle["phases"].as_array_mut().expect("phases") {
            for action in phase["actions"].as_array_mut().expect("actions") {
                if let Some(source) = action["source"].as_str() {
                    action["source"] = Value::String(source.replace('\n', "\r\n"));
                }
            }
        }
    }
    if missing_schema {
        spec.files.remove("schema.json");
    }
    spec
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
            query
        })
        .collect()
}

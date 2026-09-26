//! Authored full signatures over reviewed source fixtures. Expected parameters,
//! labels, owners and active slots never come from language-service output.
use super::{Spec, load};
use serde_json::{Value, json};

pub(crate) fn specs(crlf: bool) -> Vec<Spec> {
    let authored = load("signature-s5");
    let mut specs = Vec::new();
    for group in authored.oracle["groups"].as_array().expect("groups") {
        let mut spec = load(group["fixture"].as_str().expect("source fixture"));
        let mode = group["mode"].as_str().expect("schema mode");
        spec.id = format!("{}-{mode}", spec.id);
        match mode {
            "full" => {}
            "ordinary" => {
                let mut schema: Value =
                    serde_json::from_str(&spec.files["schema.json"]).expect("schema JSON");
                schema
                    .as_object_mut()
                    .expect("artifact")
                    .remove("serviceSet");
                spec.files.insert("schema.json".into(), schema.to_string());
            }
            "missing" => {
                spec.files.insert(
                    "schema.json".into(),
                    json!({"formatVersion":1,"facts":{}}).to_string(),
                );
            }
            other => panic!("unreviewed schema mode {other}"),
        }
        spec.oracle = expand(&authored.oracle, &group["queries"]);
        specs.push(spec);
    }
    let mut own = authored.clone();
    own.oracle = expand(&authored.oracle, &authored.oracle["queries"]);
    specs.push(own);
    if crlf {
        for spec in &mut specs {
            for source in spec.files.values_mut() {
                *source = source.replace('\n', "\r\n");
            }
        }
    }
    specs
}

fn expand(oracle: &Value, queries: &Value) -> Value {
    let queries = queries
        .as_array()
        .expect("queries")
        .iter()
        .map(|query| {
            let mut query = query.clone();
            let signature = &oracle["signatures"][query["signature"].as_str().unwrap_or("")];
            query["result"] = if query["signature"].is_null() {
                Value::Null
            } else {
                assert!(signature.is_object(), "missing authored template {query}");
                json!({"activeSignature":0,"activeParameter":query["active"],"signatures":[
                    {"label":signature["label"],"parameters":signature["parameters"]}
                ]})
            };
            query["owner"] = signature["owner"].clone();
            query["named"] = signature["named"].clone();
            query
        })
        .collect::<Vec<_>>();
    json!({"queries":queries})
}

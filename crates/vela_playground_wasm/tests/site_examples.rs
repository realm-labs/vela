//! Explicit repository lane: npm --prefix site run test:playground.
//! Runtime IO keeps packaged/native/wasm32 builds independent of the site tree.

use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::PathBuf};
use vela_playground_wasm::{compile_script, run_script};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Example {
    title: String,
    entry: String,
    source_file: String,
}

#[test]
#[ignore = "repository site inputs required; run npm --prefix site run test:playground"]
fn advertised_site_examples_compile_and_run() {
    let root = PathBuf::from(
        std::env::var_os("VELA_SITE_DATA_DIR")
            .expect("explicit site/src/data path required; no fallback or substitute sources"),
    );
    let examples: Vec<Example> = serde_json::from_str(
        &fs::read_to_string(root.join("playground-examples.json")).expect("site example manifest"),
    )
    .expect("site example records");
    let expected = [
        ("Level reward", "level_reward.vela", Some(40)),
        ("Collections", "collections.vela", Some(37)),
        ("Methods", "methods.vela", Some(50)),
        ("Runtime error", "runtime_error.vela", None),
    ];
    assert_eq!(examples.len(), expected.len());
    let files: BTreeSet<_> = fs::read_dir(root.join("playground"))
        .expect("site source directory")
        .map(|entry| {
            entry
                .expect("source entry")
                .file_name()
                .into_string()
                .expect("UTF-8 site filename")
        })
        .filter(|name| name.ends_with(".vela"))
        .collect();
    assert_eq!(
        files,
        expected
            .iter()
            .map(|(_, file, _)| (*file).to_owned())
            .collect()
    );

    for (example, (title, file, value)) in examples.iter().zip(expected) {
        assert_eq!(example.title, title);
        assert_eq!(example.source_file, file);
        assert_eq!(example.entry, "main");
        let source = fs::read_to_string(root.join("playground").join(&example.source_file))
            .expect("exact UTF-8 site source");
        let compiled: Value =
            serde_json::from_str(&compile_script(&source)).expect("compile response JSON");
        assert_eq!(
            compiled,
            json!({"ok": true, "value": {"status": "compiled"}, "diagnostics": []}),
            "{title}: compile route"
        );
        let result: Value =
            serde_json::from_str(&run_script(&source, &example.entry)).expect("run response JSON");
        if let Some(value) = value {
            assert_eq!(
                result,
                json!({"ok": true, "value": value, "diagnostics": []}),
                "{title}: run route"
            );
        } else {
            assert_eq!(result["ok"], false, "{result}");
            assert!(result["value"].is_null(), "{result}");
            let diagnostics = result["diagnostics"].as_array().expect("diagnostics array");
            assert_eq!(diagnostics.len(), 1, "{result}");
            let diagnostic = &diagnostics[0];
            assert_eq!(diagnostic["severity"], "error");
            assert_eq!(diagnostic["code"], "vm::division_by_zero");
            let span = &diagnostic["span"];
            assert!(span["source"].is_u64(), "{result}");
            let start = usize::try_from(span["start"].as_u64().expect("span start"))
                .expect("start fits usize");
            let end =
                usize::try_from(span["end"].as_u64().expect("span end")).expect("end fits usize");
            assert_eq!(source.get(start..end), Some("before / 0"), "{result}");
        }
        println!(
            "{title}: {} bytes, entry {}, compile/run checked",
            source.len(),
            example.entry
        );
    }
}

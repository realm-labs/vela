use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};

use super::super::{Point, parse_markers};
use super::load_json;

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "vela-fixture-sources-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).expect("fresh isolated root");
        std::fs::create_dir_all(root.join("fixtures/sources")).expect("source directory");
        Self(root)
    }

    fn read(&self, spec: &Value) -> Result<Value, String> {
        let file = self.0.join("fixtures/case.json");
        std::fs::write(&file, serde_json::to_vec(spec).expect("serialize fixture"))
            .expect("write fixture JSON");
        load_json(&file)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).expect("remove owned fixture scratch");
    }
}
fn reference() -> Value {
    json!({"sourceFile": "sources/exact.vela"})
}
fn spec(source: Value) -> Value {
    json!({"version":1,"id":"reader","files":{"scripts/main.vela":source},"actions":[],"oracle":{}})
}

#[test]
fn source_files_preserve_exact_utf8_and_markers_before_transforms() {
    let scratch = Scratch::new();
    let text = "\u{feff}/* 中😀 */\r\n[[point]]fn main() {\0}\n\r// no final newline";
    std::fs::write(scratch.0.join("fixtures/sources/exact.vela"), text)
        .expect("fixture setup or authored data");
    let loaded = scratch.read(&spec(reference())).expect("resolve reference");
    let actual = loaded["files"]["scripts/main.vela"]
        .as_str()
        .expect("source string");
    assert_eq!(actual.as_bytes(), text.as_bytes());
    assert_eq!(parse_markers(actual), parse_markers(text));
    assert_eq!(
        parse_markers(actual).expect("authored markers").markers["point"].start,
        Point {
            byte: 18,
            line: 1,
            character: 0
        }
    );
    assert_eq!(
        loaded,
        scratch.read(&spec(json!(text))).expect("inline source")
    );
}

#[test]
fn source_files_reject_wrong_conflicting_unknown_missing_and_unsafe_references() {
    let scratch = Scratch::new();
    std::fs::write(
        scratch.0.join("fixtures/sources/exact.vela"),
        "fn main() {}",
    )
    .expect("fixture setup or authored data");
    let mut invalid = vec![
        Value::Null,
        json!(1),
        json!(false),
        json!([]),
        json!({}),
        json!({"sourceFile":1}),
        json!({"sourceFile":null}),
        json!({"sourceFile":"sources/exact.vela","source":"fallback"}),
        json!({"sourceFile":"sources/exact.vela","other":true}),
    ];
    for path in [
        "",
        "/tmp/exact.vela",
        "C:/exact.vela",
        "C:exact.vela",
        "\\\\server\\exact.vela",
        "sources\\exact.vela",
        "../exact.vela",
        "sources/../exact.vela",
        "sources/./exact.vela",
        "sources//exact.vela",
        "sources/exact.vela/",
        "sources/\0exact.vela",
        "exact.vela",
        "sources/missing.vela",
        "sources",
        "sources/directory",
    ] {
        invalid.push(json!({"sourceFile":path}));
    }
    std::fs::create_dir(scratch.0.join("fixtures/sources/directory"))
        .expect("fixture setup or authored data");
    for value in invalid {
        assert!(scratch.read(&spec(value.clone())).is_err(), "{value}");
    }
    let file = scratch.0.join("fixtures/case.json");
    std::fs::write(&file, r#"{"files":{"main":"\ud800"}}"#)
        .expect("fixture setup or authored data");
    assert!(load_json(&file).is_err());
}

#[test]
fn source_files_reject_unpaired_path_surrogates_with_existing_replacement_file() {
    let scratch = Scratch::new();
    let text = "fn replacement() {}";
    std::fs::write(scratch.0.join("fixtures/sources/\u{fffd}.vela"), text)
        .expect("write replacement-character filename");
    let loaded = scratch
        .read(&spec(json!({"sourceFile":"sources/\u{fffd}.vela"})))
        .expect("valid replacement-character path");
    assert_eq!(loaded["files"]["scripts/main.vela"], text);
    let file = scratch.0.join("fixtures/case.json");
    for source in [
        r#"{"files":{"scripts/main.vela":{"sourceFile":"sources/\ud800.vela"}}}"#,
        r#"{"files":{"scripts/main.vela":{"sourceFile":"sources/\udc00.vela"}}}"#,
    ] {
        std::fs::write(&file, source).expect("write escaped unpaired path surrogate");
        assert!(
            load_json(&file).is_err(),
            "reject path without substitution"
        );
    }
}

#[test]
fn source_files_reject_invalid_utf8_in_documents_and_json() {
    let scratch = Scratch::new();
    for bytes in [
        &[0xff][..],
        &[0xc0, 0xaf],
        &[0xed, 0xa0, 0x80],
        &[0xf0, 0x9f, 0x98],
    ] {
        std::fs::write(scratch.0.join("fixtures/sources/exact.vela"), bytes)
            .expect("fixture setup or authored data");
        assert!(scratch.read(&spec(reference())).is_err());
        let file = scratch.0.join("fixtures/case.json");
        std::fs::write(&file, bytes).expect("fixture setup or authored data");
        assert!(load_json(&file).is_err());
    }
}

#[test]
fn source_files_reject_escaping_and_internal_symlinks() {
    let scratch = Scratch::new();
    std::fs::write(scratch.0.join("outside.vela"), "outside")
        .expect("fixture setup or authored data");
    std::fs::write(scratch.0.join("fixtures/sources/exact.vela"), "inside")
        .expect("fixture setup or authored data");
    #[cfg(unix)]
    let link_file = std::os::unix::fs::symlink;
    #[cfg(windows)]
    let link_file = std::os::windows::fs::symlink_file;
    link_file(
        scratch.0.join("outside.vela"),
        scratch.0.join("fixtures/sources/escape.vela"),
    )
    .expect("fixture setup or authored data");
    link_file(
        scratch.0.join("fixtures/sources/exact.vela"),
        scratch.0.join("fixtures/sources/alias.vela"),
    )
    .expect("fixture setup or authored data");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&scratch.0, scratch.0.join("fixtures/sources/directory"))
        .expect("fixture setup or authored data");
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(&scratch.0, scratch.0.join("fixtures/sources/directory"))
        .expect("fixture setup or authored data");
    for name in ["escape.vela", "alias.vela", "directory/outside.vela"] {
        let error = scratch
            .read(&spec(json!({"sourceFile":format!("sources/{name}")})))
            .expect_err("reject symlink");
        assert!(error.contains("symlink"), "{error}");
    }
}

#[test]
fn source_files_resolve_lifecycle_slots_without_interpreting_metadata() {
    let scratch = Scratch::new();
    let text = "[[point]]fn value() {}";
    std::fs::write(scratch.0.join("fixtures/sources/exact.vela"), text)
        .expect("fixture setup or authored data");
    let mut spec = spec(reference());
    spec["id"] = json!("completion-source-overlay-lifecycle");
    spec["actions"] = json!([{"op":"write","file":"scripts/main.vela","source":reference()}]);
    spec["oracle"] = json!({"sourceLifecycle":[{
        "files":{"scripts/api.vela":reference(),"deleted.vela":null},
        "action":{"source":reference()},"diskSource":reference(),"openSource":null
    }],"metadata":{"sourceFile":"sources/missing.vela"}});
    let loaded = scratch.read(&spec).expect("resolve lifecycle");
    let phase = &loaded["oracle"]["sourceLifecycle"][0];
    assert_eq!(loaded["actions"][0]["source"], text);
    assert_eq!(phase["files"]["scripts/api.vela"], text);
    assert!(phase["files"]["deleted.vela"].is_null());
    assert_eq!(phase["action"]["source"], text);
    assert_eq!(phase["diskSource"], text);
    assert!(phase["openSource"].is_null());
    assert_eq!(loaded["oracle"]["metadata"], spec["oracle"]["metadata"]);
    spec["oracle"]["sourceLifecycle"][0]["openSource"] =
        json!({"sourceFile":"sources/exact.vela","text":"shadow"});
    assert!(scratch.read(&spec).is_err());
}

#[test]
fn source_files_resolve_every_authored_reference_without_changing_oracles() {
    fn compare(raw: &Value, resolved: &Value, root: &std::path::Path) -> usize {
        match raw {
            Value::Object(object) if object.contains_key("sourceFile") => {
                assert_eq!(object.len(), 1);
                let bytes =
                    std::fs::read(root.join(raw["sourceFile"].as_str().expect("source string")))
                        .expect("fixture setup or authored data");
                let actual = resolved.as_str().expect("declared source slot");
                assert_eq!(actual.as_bytes(), bytes);
                assert_eq!(
                    parse_markers(actual),
                    parse_markers(std::str::from_utf8(&bytes).expect("strict source UTF-8"))
                );
                1
            }
            Value::Object(object) => {
                assert_eq!(
                    object.len(),
                    resolved.as_object().expect("oracle object").len()
                );
                object
                    .iter()
                    .map(|(key, value)| compare(value, &resolved[key], root))
                    .sum()
            }
            Value::Array(array) => {
                assert_eq!(
                    array.len(),
                    resolved.as_array().expect("oracle array").len()
                );
                array
                    .iter()
                    .enumerate()
                    .map(|(index, value)| compare(value, &resolved[index], root))
                    .sum()
            }
            _ => {
                assert_eq!(raw, resolved);
                0
            }
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/lsp_matrix/fixtures");
    let mut references = 0;
    for entry in std::fs::read_dir(&root).expect("fixture corpus") {
        let file = entry.expect("fixture entry").path();
        if file
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            let raw =
                serde_json::from_str(&std::fs::read_to_string(&file).expect("read fixture JSON"))
                    .expect("fixture setup or authored data");
            references += compare(
                &raw,
                &load_json(&file).expect("resolve authored sources"),
                &root,
            );
        }
    }
    assert!(references > 0);
}

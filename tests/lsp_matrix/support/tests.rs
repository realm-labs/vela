use super::*;

#[test]
fn shared_markers_match_independent_unicode_lf_crlf_goldens() {
    let golden: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/marker-golden.json")).expect("golden JSON");
    for item in golden["valid"].as_array().expect("valid fixtures") {
        let actual =
            parse_markers(item["source"].as_str().expect("source")).expect("valid markers");
        assert_eq!(
            serde_json::to_value(actual).expect("serialize"),
            serde_json::json!({"text":item["text"],"markers":item["markers"]}),
            "{}",
            item["id"]
        );
    }
}

#[test]
fn shared_markers_reject_malformed_duplicates_crossing_and_split_crlf() {
    let golden: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/marker-golden.json")).expect("golden JSON");
    for item in golden["invalid"].as_array().expect("invalid fixtures") {
        assert!(
            parse_markers(item.as_str().expect("source")).is_err(),
            "{item}"
        );
    }
}

#[test]
fn shared_multifile_actions_preserve_overlays_and_restore_disk_oracles() {
    let spec = load("shared-unicode-lifecycle");
    let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
    for (index, action) in spec.actions.iter().enumerate() {
        fixture.apply(action).expect("valid action");
        let actual = fixture.document(spec.oracle["file"].as_str().expect("oracle file"));
        let expected = &spec.oracle["afterEachAction"][index];
        if expected.is_null() {
            assert!(actual.is_none());
            continue;
        }
        let actual = actual.expect("document");
        let marker = actual.markers["definition"];
        assert_eq!(
            serde_json::json!([
                marker.start.line,
                marker.start.character,
                marker.end.line,
                marker.end.character
            ]),
            serde_json::json!([
                expected["line"],
                expected["character"],
                expected["line"],
                expected["endCharacter"]
            ])
        );
        assert_eq!(
            &actual.text[marker.start.byte..marker.end.byte],
            spec.oracle["selectedText"].as_str().expect("text")
        );
        assert!(
            actual
                .text
                .ends_with(expected["tail"].as_str().expect("tail"))
        );
    }
}

#[test]
fn shared_fixture_paths_actions_and_materialization_are_isolated() {
    for file in [
        "../x", "/tmp/x", "a/../b", "a\\b", "C:x", "a//b", "a/./b", "a\0b",
    ] {
        assert!(safe_file(file).is_err());
    }
    let spec = load("shared-unicode-lifecycle");
    for op in ["change", "close", "save", "oops"] {
        let action = Action {
            op: op.to_owned(),
            file: "scripts/helper.vela".to_owned(),
            source: Some("x".to_owned()),
        };
        assert!(
            FixtureWorkspace::new(&spec)
                .expect("fixture")
                .apply(&action)
                .is_err()
        );
    }
    let fixture = FixtureWorkspace::new(&spec).expect("fixture");
    let root = std::env::temp_dir().join(format!("vela-shared-fixture-{}", std::process::id()));
    fixture.materialize(&root).expect("new isolated root");
    assert_eq!(
        std::fs::read_to_string(root.join("scripts/main.vela")).expect("materialized source"),
        fixture.disk["scripts/main.vela"].text
    );
    assert!(fixture.materialize(&root).is_err());
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn shared_edit_oracle_validates_unicode_containment_and_overlap() {
    let text = "中😀 first\r\nsecond";
    assert_eq!(
        apply_edits(
            text,
            &[
                Edit {
                    start: (1, 0),
                    end: (1, 6),
                    text: "尾"
                },
                Edit {
                    start: (0, 4),
                    end: (0, 9),
                    text: "next"
                }
            ]
        )
        .expect("edits"),
        "中😀 next\r\n尾"
    );
    for (line, character) in [(0, 2), (0, 10), (2, 0)] {
        assert!(offset_at(text, line, character).is_err());
    }
    assert!(
        apply_edits(
            text,
            &[Edit {
                start: (0, 5),
                end: (0, 4),
                text: "x"
            }]
        )
        .is_err()
    );
    assert!(
        apply_edits(
            text,
            &[
                Edit {
                    start: (0, 4),
                    end: (0, 7),
                    text: "a"
                },
                Edit {
                    start: (0, 6),
                    end: (0, 9),
                    text: "b"
                }
            ]
        )
        .is_err()
    );
    assert!(
        apply_edits(
            text,
            &[
                Edit {
                    start: (0, 4),
                    end: (0, 4),
                    text: "a"
                },
                Edit {
                    start: (0, 4),
                    end: (0, 4),
                    text: "b"
                }
            ]
        )
        .is_err()
    );
}

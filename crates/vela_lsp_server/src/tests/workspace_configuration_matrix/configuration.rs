use super::support::*;
use serde_json::{Value, json};
use vela_language_service::SourceVersion;

#[test]
fn configuration_invalid_settings_report_complete_error_and_preserve_current_facts() {
    for crlf in [false, true] {
        let s = spec(crlf);
        let layout = Layout::new(&s);
        let caller = doc(&s, "roots/left/caller.vela");
        let target = doc(&s, "roots/left/alpha.vela");
        for profile in profiles() {
            let mut server = layout.server(profile);
            open(
                &mut server,
                &layout.uri("roots/left/caller.vela"),
                &caller.text,
                73,
            );
            let frozen = server.snapshot();
            for invalid in s.oracle["invalidSettings"]
                .as_array()
                .expect("authored invalid settings")
            {
                let expected = format!(
                    "invalid didChangeConfiguration settings: {}",
                    invalid["error"].as_str().expect("error")
                );
                assert_eq!(
                    change(&mut server, invalid["settings"].clone()),
                    vec![
                        json!({"jsonrpc":"2.0","method":"window/logMessage","params":{"type":1,"message":expected}})
                    ]
                );
                let current = server.snapshot();
                assert_eq!(current.generation(), frozen.generation());
                assert_eq!(current.workspace_config(), frozen.workspace_config());
                assert_eq!(
                    current
                        .workspace()
                        .document(&layout.id("roots/left/caller.vela"))
                        .expect("overlay")
                        .text(),
                    caller.text
                );
                definition(
                    &mut server,
                    &layout,
                    "roots/left/caller.vela",
                    &caller,
                    "call",
                    Some(("roots/left/alpha.vela", &target)),
                );
            }
        }
    }
}

#[test]
fn configuration_root_forms_reindex_exact_disk_modules_keep_overlays_and_match_fresh() {
    for crlf in [false, true] {
        let s = fallback(crlf);
        let layout = Layout::new(&s);
        let caller = doc(&s, "roots/left/caller.vela");
        let target = doc(&s, "roots/left/alpha.vela");
        let overlay = parse_markers(
            &s.oracle["overlay"]
                .as_str()
                .expect("overlay")
                .replace('\n', if crlf { "\r\n" } else { "\n" }),
        )
        .expect("marked overlay");
        let left = layout.uri("roots/left");
        let right = layout.uri("roots/right");
        let steps = [
            (json!({"workspace":{"roots":[left.clone()]}}), true),
            (
                json!({"vela":{"workspace":{"roots":[right.clone()]}}}),
                false,
            ),
            (json!({"workspaceRoots":[left.clone(),right.clone()]}), true),
            (
                json!({"workspace":{"roots":[right]},"workspaceRoots":[left.clone()]}),
                false,
            ),
            (json!({"workspace":{"roots":[]}}), false),
            (json!({"workspaceRoots":[left]}), true),
        ];
        for profile in profiles() {
            let mut server = session(&layout, profile.clone(), &[""], steps[0].0.clone());
            open(
                &mut server,
                &layout.uri("roots/left/caller.vela"),
                &caller.text,
                73,
            );
            let frozen = server.snapshot();
            open(
                &mut server,
                &layout.uri("roots/left/alpha.vela"),
                &overlay.text,
                19,
            );
            for (settings, known) in &steps {
                let mut expected = if *known {
                    vec![]
                } else {
                    vec![missing(
                        &s,
                        &layout,
                        "roots/left/caller.vela",
                        &caller,
                        "import",
                        "alpha",
                    )]
                };
                if settings["workspace"]["roots"]
                    .as_array()
                    .is_some_and(Vec::is_empty)
                {
                    expected[0]["data"]["labels"][0]["message"] =
                        s.oracle["fallbackModuleLabel"].clone();
                }
                publications(
                    &change(&mut server, settings.clone()),
                    vec![
                        (layout.uri("roots/left/caller.vela"), expected.clone()),
                        (layout.uri("roots/left/alpha.vela"), vec![]),
                    ],
                    false,
                );
                let destination = known.then_some(("roots/left/alpha.vela", &overlay));
                definition(
                    &mut server,
                    &layout,
                    "roots/left/caller.vela",
                    &caller,
                    "call",
                    destination,
                );
                let snapshot = server.snapshot();
                let buffer = snapshot
                    .workspace()
                    .document(&layout.id("roots/left/caller.vela"))
                    .expect("owned overlay");
                assert_eq!(
                    (buffer.text(), buffer.version()),
                    (caller.text.as_str(), SourceVersion::new(73))
                );
                assert_eq!(
                    frozen.databases().source_db().records()[&layout.id("roots/left/alpha.vela")]
                        .text(),
                    target.text
                );
                let alpha = snapshot
                    .workspace()
                    .document(&layout.id("roots/left/alpha.vela"))
                    .expect("dirty alpha");
                assert_eq!(
                    (alpha.text(), alpha.version()),
                    (overlay.text.as_str(), SourceVersion::new(19))
                );
                let mut fresh = session(&layout, profile.clone(), &[""], Value::Null);
                assert!(change(&mut fresh, settings.clone()).is_empty());
                open(
                    &mut fresh,
                    &layout.uri("roots/left/alpha.vela"),
                    &overlay.text,
                    19,
                );
                publications(
                    &open(
                        &mut fresh,
                        &layout.uri("roots/left/caller.vela"),
                        &caller.text,
                        73,
                    ),
                    vec![(layout.uri("roots/left/caller.vela"), expected)],
                    false,
                );
                definition(
                    &mut fresh,
                    &layout,
                    "roots/left/caller.vela",
                    &caller,
                    "call",
                    destination,
                );
            }
        }
        layout.check_disk(&s);
    }
}

#[test]
fn configuration_schema_switch_invalid_missing_disable_and_restore_pin_complete_facts() {
    use std::collections::BTreeSet;
    for crlf in [false, true] {
        let s = fallback(crlf);
        let layout = Layout::new(&s);
        let source = doc(&s, "roots/left/schema_caller.vela");
        let mut server = session(&layout, json!({}), &[""], Value::Null);
        let mut seen = BTreeSet::new();
        let mut frozen = None;
        open(
            &mut server,
            &layout.uri("roots/left/schema_caller.vela"),
            &source.text,
            91,
        );
        for step in s.oracle["schemaStates"].as_array().expect("schema states") {
            let file = step["path"].as_str().expect("path");
            let path = if file.is_empty() {
                String::new()
            } else {
                layout.uri(file)
            };
            if !file.is_empty() {
                seen.insert(file.to_owned());
            }
            let settings =
                json!({"workspace":{"roots":[layout.uri("roots/left")]},"host":{"schema":path}});
            let message = step["error"].as_str().map(|text| {
                text.replace(
                    "{SCHEMA}",
                    &layout.path(file).display().to_string().replace('\\', "/"),
                )
            });
            let mut expected = vec![(
                layout.uri("roots/left/schema_caller.vela"),
                schema_source_diagnostics(&s, &layout, &source, file, message.as_deref()),
            )];
            expected.extend(seen.iter().map(|old| {
                (
                    layout.uri(old),
                    if old == file {
                        message.as_deref().map_or_else(Vec::new, |text| {
                            metadata_error("schema::diagnostic", text)
                        })
                    } else {
                        vec![]
                    },
                )
            }));
            publications(&change(&mut server, settings.clone()), expected, false);
            schema_queries(&mut server, &layout, &source, &step["fields"]);
            if frozen.is_none() {
                frozen = Some(server.snapshot());
            }
            assert_eq!(
                frozen
                    .as_ref()
                    .expect("original snapshot")
                    .databases()
                    .schema_db()
                    .facts()
                    .field_fact("HostCell", "value")
                    .expect("original field")
                    .display_name(),
                "i64"
            );
            assert_eq!(
                server
                    .snapshot()
                    .workspace()
                    .document(&layout.id("roots/left/schema_caller.vela"))
                    .expect("overlay")
                    .version(),
                SourceVersion::new(91)
            );
            let mut fresh = session(&layout, json!({}), &[""], Value::Null);
            let only = if file.is_empty() {
                vec![]
            } else {
                vec![(
                    layout.uri(file),
                    message
                        .as_deref()
                        .map_or_else(Vec::new, |text| metadata_error("schema::diagnostic", text)),
                )]
            };
            publications(&change(&mut fresh, settings), only, false);
            publications(
                &open(
                    &mut fresh,
                    &layout.uri("roots/left/schema_caller.vela"),
                    &source.text,
                    91,
                ),
                vec![(
                    layout.uri("roots/left/schema_caller.vela"),
                    schema_source_diagnostics(&s, &layout, &source, file, message.as_deref()),
                )],
                false,
            );
            schema_queries(&mut fresh, &layout, &source, &step["fields"]);
        }
        layout.check_disk(&s);
    }
}

#[test]
fn configuration_manifest_authority_persists_until_delete_then_editor_roots_and_schema_apply() {
    for crlf in [false, true] {
        let s = spec(crlf);
        let layout = Layout::new(&s);
        let caller = doc(&s, "roots/left/caller.vela");
        let target = doc(&s, "roots/left/alpha.vela");
        let schema = doc(&s, "roots/left/schema_caller.vela");
        let other = doc(&s, "roots/right/other_caller.vela");
        let beta = doc(&s, "roots/right/beta.vela");
        let mut server = layout.server(json!({}));
        open(
            &mut server,
            &layout.uri("roots/left/caller.vela"),
            &caller.text,
            73,
        );
        let frozen = server.snapshot();
        let settings = json!({"vela":{"workspace":{"roots":[layout.uri("roots/right")]},"host":{"schema":layout.uri("schema-two.json")}}});
        publications(
            &change(&mut server, settings),
            vec![
                (layout.uri("roots/left/caller.vela"), vec![]),
                (layout.uri("schema-one.json"), vec![]),
            ],
            false,
        );
        definition(
            &mut server,
            &layout,
            "roots/left/caller.vela",
            &caller,
            "call",
            Some(("roots/left/alpha.vela", &target)),
        );
        schema_queries(
            &mut server,
            &layout,
            &schema,
            &s.oracle["schemaStates"][0]["fields"],
        );
        layout.remove_disk("vela.toml");
        let expected = vec![missing(
            &s,
            &layout,
            "roots/left/caller.vela",
            &caller,
            "import",
            "alpha",
        )];
        publications(
            &watch(&mut server, &layout, &[("vela.toml", 3)]),
            vec![
                (layout.uri("roots/left/caller.vela"), expected),
                (layout.uri("vela.toml"), vec![]),
                (layout.uri("schema-one.json"), vec![]),
                (layout.uri("schema-two.json"), vec![]),
            ],
            false,
        );
        definition(
            &mut server,
            &layout,
            "roots/left/caller.vela",
            &caller,
            "call",
            None,
        );
        open(
            &mut server,
            &layout.uri("roots/right/other_caller.vela"),
            &other.text,
            13,
        );
        definition(
            &mut server,
            &layout,
            "roots/right/other_caller.vela",
            &other,
            "call",
            Some(("roots/right/beta.vela", &beta)),
        );
        // The retained left overlay can still read global static host metadata.
        open(
            &mut server,
            &layout.uri("roots/left/schema_caller.vela"),
            &schema.text,
            91,
        );
        schema_queries(
            &mut server,
            &layout,
            &schema,
            &s.oracle["schemaStates"][1]["fields"],
        );
        assert_eq!(
            frozen.databases().source_db().records()[&layout.id("roots/left/alpha.vela")].text(),
            target.text
        );
        assert_eq!(
            frozen
                .databases()
                .schema_db()
                .facts()
                .field_fact("HostCell", "value")
                .expect("old field")
                .display_name(),
            "i64"
        );
    }
}

#[test]
fn configuration_source_io_errors_have_real_owners_clear_on_repair_and_keep_overlays() {
    let s = fallback(false);
    let layout = Layout::new(&s);
    let caller = doc(&s, "roots/left/caller.vela");
    let alpha = doc(&s, "roots/left/alpha.vela");
    let mut server = session(&layout, json!({}), &["roots/left"], Value::Null);
    open(
        &mut server,
        &layout.uri("roots/left/caller.vela"),
        &caller.text,
        73,
    );
    open(
        &mut server,
        &layout.uri("roots/left/alpha.vela"),
        &alpha.text,
        19,
    );
    let frozen = server.snapshot();
    // A file used as a directory is a deterministic OS I/O failure on both profiles.
    let path = lsp_types::Url::parse(&layout.uri("roots/left/alpha.vela"))
        .expect("fixture URI")
        .to_file_path()
        .expect("file path");
    let os_error = std::fs::read_dir(&path)
        .expect_err("file is not a source directory")
        .to_string();
    let message = format!("{}: {os_error}", path.display());
    publications(
        &change(
            &mut server,
            json!({"workspace":{"roots":[layout.uri("roots/left/alpha.vela")]}}),
        ),
        vec![
            (
                layout.uri("roots/left/alpha.vela"),
                metadata_error("project::diagnostic", &message),
            ),
            (
                layout.uri("roots/left/caller.vela"),
                vec![missing(
                    &s,
                    &layout,
                    "roots/left/caller.vela",
                    &caller,
                    "import",
                    "alpha",
                )],
            ),
        ],
        false,
    );
    assert_eq!(
        server
            .snapshot()
            .workspace()
            .document(&layout.id("roots/left/alpha.vela"))
            .expect("overlay")
            .version(),
        SourceVersion::new(19)
    );
    assert_eq!(
        frozen.databases().source_db().records()[&layout.id("roots/left/alpha.vela")].text(),
        alpha.text
    );
    publications(
        &change(
            &mut server,
            json!({"workspace":{"roots":[layout.uri("roots/left")]}}),
        ),
        vec![
            (layout.uri("roots/left/alpha.vela"), vec![]),
            (layout.uri("roots/left/caller.vela"), vec![]),
        ],
        false,
    );
    definition(
        &mut server,
        &layout,
        "roots/left/caller.vela",
        &caller,
        "call",
        Some(("roots/left/alpha.vela", &alpha)),
    );
    layout.check_disk(&s);
}

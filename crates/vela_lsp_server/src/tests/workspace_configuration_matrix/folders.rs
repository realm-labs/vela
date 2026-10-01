use super::support::*;
use crate::LaunchConfiguration;
use serde_json::json;
use vela_language_service::SourceVersion;

#[test]
fn workspace_folder_addition_discovers_existing_disk_sources_without_synthetic_watch_events() {
    for crlf in [false, true] {
        let s = spec(crlf);
        let layout = Layout::new(&s);
        let caller = doc(&s, "roots/left/caller.vela");
        let target = doc(&s, "roots/left/alpha.vela");
        let mut config = LaunchConfiguration::new();
        config.set_watch_files_enabled(false);
        let mut server = TestServer::with_launch_configuration(config);
        let init = send(
            &mut server,
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":null,"capabilities":{},"workspaceFolders":[]}}),
        );
        assert_eq!(init.len(), 1);
        assert!(
            send(
                &mut server,
                json!({"jsonrpc":"2.0","method":"initialized","params":{}})
            )
            .is_empty()
        );
        open(
            &mut server,
            &layout.uri("roots/left/caller.vela"),
            &caller.text,
            73,
        );
        publications(
            &folders(&mut server, &layout, &["roots/left"], &[]),
            vec![(layout.uri("roots/left/caller.vela"), vec![])],
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
    }
}

#[test]
fn workspace_folder_remove_readd_preserves_dirty_sources_and_pins_cross_root_binding_loss() {
    for crlf in [false, true] {
        let s = fallback(crlf);
        let layout = Layout::new(&s);
        let cross = doc(&s, "roots/left/cross.vela");
        let original = doc(&s, "roots/left/alpha.vela");
        let beta = doc(&s, "roots/right/beta.vela");
        let other = doc(&s, "roots/right/other_caller.vela");
        let overlay = parse_markers(
            &s.oracle["overlay"]
                .as_str()
                .expect("overlay")
                .replace('\n', if crlf { "\r\n" } else { "\n" }),
        )
        .expect("marked overlay");
        for profile in profiles() {
            let mut server = session(
                &layout,
                profile.clone(),
                &["roots/left"],
                serde_json::Value::Null,
            );
            publications(
                &open(
                    &mut server,
                    &layout.uri("roots/left/cross.vela"),
                    &cross.text,
                    73,
                ),
                vec![(
                    layout.uri("roots/left/cross.vela"),
                    vec![missing(
                        &s,
                        &layout,
                        "roots/left/cross.vela",
                        &cross,
                        "import-beta",
                        "beta",
                    )],
                )],
                false,
            );
            publications(
                &folders(&mut server, &layout, &["roots/right"], &[]),
                vec![(layout.uri("roots/left/cross.vela"), vec![])],
                has_progress(&profile),
            );
            definition(
                &mut server,
                &layout,
                "roots/left/cross.vela",
                &cross,
                "alpha",
                Some(("roots/left/alpha.vela", &original)),
            );
            definition(
                &mut server,
                &layout,
                "roots/left/cross.vela",
                &cross,
                "beta",
                Some(("roots/right/beta.vela", &beta)),
            );
            let frozen = server.snapshot();
            open(
                &mut server,
                &layout.uri("roots/left/alpha.vela"),
                &overlay.text,
                19,
            );
            let errors = vec![
                missing(
                    &s,
                    &layout,
                    "roots/left/cross.vela",
                    &cross,
                    "import-alpha",
                    "alpha",
                ),
                missing(
                    &s,
                    &layout,
                    "roots/left/cross.vela",
                    &cross,
                    "import-beta",
                    "beta",
                ),
            ];
            publications(
                &folders(&mut server, &layout, &[], &["roots/left"]),
                vec![
                    (layout.uri("roots/left/cross.vela"), errors.clone()),
                    (layout.uri("roots/left/alpha.vela"), vec![]),
                ],
                has_progress(&profile),
            );
            definition(
                &mut server,
                &layout,
                "roots/left/cross.vela",
                &cross,
                "alpha",
                None,
            );
            definition(
                &mut server,
                &layout,
                "roots/left/cross.vela",
                &cross,
                "beta",
                None,
            );
            definition(
                &mut server,
                &layout,
                "roots/right/other_caller.vela",
                &other,
                "call",
                Some(("roots/right/beta.vela", &beta)),
            );
            let mut fresh = session(
                &layout,
                profile.clone(),
                &["roots/right"],
                serde_json::Value::Null,
            );
            open(
                &mut fresh,
                &layout.uri("roots/left/alpha.vela"),
                &overlay.text,
                19,
            );
            publications(
                &open(
                    &mut fresh,
                    &layout.uri("roots/left/cross.vela"),
                    &cross.text,
                    73,
                ),
                vec![(layout.uri("roots/left/cross.vela"), errors)],
                false,
            );
            definition(
                &mut fresh,
                &layout,
                "roots/left/cross.vela",
                &cross,
                "alpha",
                None,
            );
            definition(
                &mut fresh,
                &layout,
                "roots/left/cross.vela",
                &cross,
                "beta",
                None,
            );
            publications(
                &folders(&mut server, &layout, &["roots/left"], &[]),
                vec![
                    (layout.uri("roots/left/cross.vela"), vec![]),
                    (layout.uri("roots/left/alpha.vela"), vec![]),
                ],
                has_progress(&profile),
            );
            definition(
                &mut server,
                &layout,
                "roots/left/cross.vela",
                &cross,
                "alpha",
                Some(("roots/left/alpha.vela", &overlay)),
            );
            definition(
                &mut server,
                &layout,
                "roots/left/cross.vela",
                &cross,
                "beta",
                Some(("roots/right/beta.vela", &beta)),
            );
            let current = server.snapshot();
            assert_eq!(
                current
                    .workspace()
                    .document(&layout.id("roots/left/alpha.vela"))
                    .expect("dirty source")
                    .version(),
                SourceVersion::new(19)
            );
            assert_eq!(
                current.databases().source_db().records()[&layout.id("roots/left/alpha.vela")]
                    .text(),
                overlay.text
            );
            assert_eq!(
                frozen.databases().source_db().records()[&layout.id("roots/left/alpha.vela")]
                    .text(),
                original.text
            );
            let mut restored = session(
                &layout,
                profile.clone(),
                &["roots/left", "roots/right"],
                serde_json::Value::Null,
            );
            open(
                &mut restored,
                &layout.uri("roots/left/alpha.vela"),
                &overlay.text,
                19,
            );
            open(
                &mut restored,
                &layout.uri("roots/left/cross.vela"),
                &cross.text,
                73,
            );
            definition(
                &mut restored,
                &layout,
                "roots/left/cross.vela",
                &cross,
                "alpha",
                Some(("roots/left/alpha.vela", &overlay)),
            );
            definition(
                &mut restored,
                &layout,
                "roots/left/cross.vela",
                &cross,
                "beta",
                Some(("roots/right/beta.vela", &beta)),
            );
        }
        layout.check_disk(&s);
    }
}

#[test]
fn workspace_folder_package_removal_drops_old_graph_and_schema_then_readd_restores_them() {
    for crlf in [false, true] {
        let s = spec(crlf);
        let layout = Layout::new(&s);
        let caller = doc(&s, "roots/left/caller.vela");
        let target = doc(&s, "roots/left/alpha.vela");
        let schema = doc(&s, "roots/left/schema_caller.vela");
        let mut server = layout.server(json!({}));
        open(
            &mut server,
            &layout.uri("roots/left/caller.vela"),
            &caller.text,
            73,
        );
        let frozen = server.snapshot();
        publications(
            &folders(&mut server, &layout, &[], &[""]),
            vec![
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
            None,
        );
        assert!(
            !server
                .snapshot()
                .databases()
                .source_db()
                .records()
                .contains_key(&layout.id("roots/left/alpha.vela"))
        );
        assert!(
            server
                .snapshot()
                .databases()
                .schema_db()
                .facts()
                .field_fact("HostCell", "value")
                .is_none()
        );
        publications(
            &folders(&mut server, &layout, &[""], &[]),
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
        assert_eq!(
            frozen.databases().source_db().records()[&layout.id("roots/left/alpha.vela")].text(),
            target.text
        );
        assert_eq!(
            server
                .snapshot()
                .workspace()
                .document(&layout.id("roots/left/caller.vela"))
                .expect("overlay")
                .version(),
            SourceVersion::new(73)
        );
        layout.check_disk(&s);
    }
}

#[test]
fn workspace_folder_malformed_and_membership_noop_batches_preserve_exact_current_state() {
    let s = fallback(false);
    let layout = Layout::new(&s);
    let caller = doc(&s, "roots/left/caller.vela");
    let target = doc(&s, "roots/left/alpha.vela");
    let mut server = session(&layout, json!({}), &["roots/left"], serde_json::Value::Null);
    open(
        &mut server,
        &layout.uri("roots/left/caller.vela"),
        &caller.text,
        73,
    );
    let frozen = server.snapshot();
    for params in [
        serde_json::Value::Null,
        json!({}),
        json!({"event":null}),
        json!({"event":{"added":null,"removed":[]}}),
        json!({"event":{"added":[{"uri":"invalid","name":"bad"}],"removed":[]}}),
        json!({"event":{"added":[{"uri":layout.uri("roots/right"),"name":"valid"},{"uri":7,"name":"bad"}],"removed":[]}}),
    ] {
        assert!(send(&mut server,json!({"jsonrpc":"2.0","method":"workspace/didChangeWorkspaceFolders","params":params})).is_empty());
        assert_eq!(server.snapshot().generation(), frozen.generation());
        assert_eq!(
            server.snapshot().workspace_roots(),
            frozen.workspace_roots()
        );
    }
    for (added, removed) in [
        (vec![], vec![]),
        (vec!["roots/left", "roots/left"], vec![]),
        (vec![], vec!["missing-root"]),
        (vec!["roots/left"], vec!["roots/left"]),
    ] {
        publications(
            &folders(&mut server, &layout, &added, &removed),
            vec![(layout.uri("roots/left/caller.vela"), vec![])],
            false,
        );
        assert_eq!(server.snapshot().generation(), frozen.generation());
        assert_eq!(
            server.snapshot().workspace_roots(),
            frozen.workspace_roots()
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
    layout.check_disk(&s);
}

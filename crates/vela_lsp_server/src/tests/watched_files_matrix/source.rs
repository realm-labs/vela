use super::support::*;
use serde_json::{Value, json};
use vela_language_service::SourceVersion;

#[test]
fn watched_source_create_change_delete_rename_and_repair_keep_exact_current_locations() {
    for crlf in [false, true] {
        let s = spec(crlf);
        let layout = Layout::new(&s);
        let caller = doc(&s.files["scripts/main.vela"], false);
        let original = doc(&s.files["scripts/defs.vela"], false);
        let changed = variant(&s, "shifted");
        let renamed = variant(&s, "renamedCaller");
        for profile in profiles() {
            layout.write_disk("scripts/defs.vela", &original.text);
            let mut server = layout.server(profile.clone());
            publications(
                &open(
                    &mut server,
                    &layout.uri("scripts/main.vela"),
                    &caller.text,
                    7,
                ),
                vec![(layout.uri("scripts/main.vela"), vec![])],
                false,
            );
            let frozen = server.snapshot();
            for (operation, target) in [
                (3, None),
                (1, Some(&original)),
                (2, Some(&changed)),
                (3, None),
                (1, Some(&original)),
            ] {
                if let Some(document) = target {
                    layout.write_disk("scripts/defs.vela", &document.text);
                } else {
                    layout.remove_disk("scripts/defs.vela");
                }
                let expected = target.map_or_else(
                    || {
                        source_error(
                            &s,
                            &caller,
                            &layout.uri("scripts/main.vela"),
                            "missingModule",
                        )
                    },
                    |_| vec![],
                );
                let messages = watch(&mut server, &layout, &[("scripts/defs.vela", operation)]);
                let mut publications_expected =
                    vec![(layout.uri("scripts/main.vela"), expected.clone())];
                if target.is_none() {
                    publications_expected.push((layout.uri("scripts/defs.vela"), vec![]));
                }
                publications(&messages, publications_expected, has_progress(&profile));
                let destination = target.map(|d| ("scripts/defs.vela", d));
                equivalent(
                    &layout,
                    &profile,
                    &caller,
                    destination,
                    expected,
                    &mut server,
                );
                assert_eq!(
                    frozen.databases().source_db().records()[&layout.id("scripts/defs.vela")]
                        .text(),
                    original.text
                );
                assert_eq!(
                    server
                        .snapshot()
                        .workspace()
                        .document(&layout.id("scripts/main.vela"))
                        .expect("overlay")
                        .version(),
                    SourceVersion::new(7)
                );
            }
            layout.remove_disk("scripts/defs.vela");
            layout.write_disk("scripts/renamed.vela", &changed.text);
            let missing = source_error(
                &s,
                &caller,
                &layout.uri("scripts/main.vela"),
                "missingModule",
            );
            publications(
                &watch(
                    &mut server,
                    &layout,
                    &[("scripts/defs.vela", 3), ("scripts/renamed.vela", 1)],
                ),
                vec![
                    (layout.uri("scripts/main.vela"), missing.clone()),
                    (layout.uri("scripts/defs.vela"), vec![]),
                ],
                has_progress(&profile),
            );
            equivalent(&layout, &profile, &caller, None, missing, &mut server);
            publications(
                &open(
                    &mut server,
                    &layout.uri("scripts/main.vela"),
                    &renamed.text,
                    8,
                ),
                vec![(layout.uri("scripts/main.vela"), vec![])],
                false,
            );
            equivalent(
                &layout,
                &profile,
                &renamed,
                Some(("scripts/renamed.vela", &changed)),
                vec![],
                &mut server,
            );
            layout.remove_disk("scripts/renamed.vela");
            layout.write_disk("scripts/defs.vela", &original.text);
        }
    }
}

#[test]
fn watched_batches_coalesce_to_last_event_and_republish_only_the_final_complete_state() {
    for crlf in [false, true] {
        let s = spec(crlf);
        let layout = Layout::new(&s);
        let caller = doc(&s.files["scripts/main.vela"], false);
        let original = doc(&s.files["scripts/defs.vela"], false);
        let changed = variant(&s, "shifted");
        for events in [
            vec![1, 2, 3],
            vec![3, 1, 2],
            vec![1, 1, 2, 2],
            vec![3, 3],
            vec![2, 3, 1],
        ] {
            layout.write_disk("scripts/defs.vela", &original.text);
            let mut server = layout.server(json!({}));
            open(
                &mut server,
                &layout.uri("scripts/main.vela"),
                &caller.text,
                1,
            );
            // Disk is the final state; intermediate events never publish a partial state.
            let removed = events.last() == Some(&3);
            if removed {
                layout.remove_disk("scripts/defs.vela");
            } else {
                layout.write_disk("scripts/defs.vela", &changed.text);
            }
            let list = events
                .into_iter()
                .map(|typ| ("scripts/defs.vela", typ))
                .collect::<Vec<_>>();
            let expected = if removed {
                source_error(
                    &s,
                    &caller,
                    &layout.uri("scripts/main.vela"),
                    "missingModule",
                )
            } else {
                vec![]
            };
            let mut publications_expected =
                vec![(layout.uri("scripts/main.vela"), expected.clone())];
            if removed {
                publications_expected.push((layout.uri("scripts/defs.vela"), vec![]));
            }
            publications(
                &watch(&mut server, &layout, &list),
                publications_expected,
                false,
            );
            equivalent(
                &layout,
                &json!({}),
                &caller,
                (!removed).then_some(("scripts/defs.vela", &changed)),
                expected,
                &mut server,
            );
            layout.write_disk("scripts/defs.vela", &original.text);
        }
    }
}

#[test]
fn watched_disk_changes_and_deletion_preserve_dirty_overlays_until_close() {
    for crlf in [false, true] {
        let s = spec(crlf);
        let layout = Layout::new(&s);
        let caller = doc(&s.files["scripts/main.vela"], false);
        let original = doc(&s.files["scripts/defs.vela"], false);
        let overlay = variant(&s, "overlay");
        let changed = variant(&s, "shifted");
        for removed in [false, true] {
            layout.write_disk("scripts/defs.vela", &original.text);
            let mut server = layout.server(json!({}));
            open(
                &mut server,
                &layout.uri("scripts/main.vela"),
                &caller.text,
                1,
            );
            open(
                &mut server,
                &layout.uri("scripts/defs.vela"),
                &overlay.text,
                19,
            );
            let frozen = server.snapshot();
            if removed {
                layout.remove_disk("scripts/defs.vela");
            } else {
                layout.write_disk("scripts/defs.vela", &changed.text);
            }
            publications(
                &watch(
                    &mut server,
                    &layout,
                    &[("scripts/defs.vela", if removed { 3 } else { 2 })],
                ),
                vec![
                    (layout.uri("scripts/main.vela"), vec![]),
                    (layout.uri("scripts/defs.vela"), vec![]),
                ],
                false,
            );
            definition(
                &mut server,
                &layout,
                &caller,
                Some(("scripts/defs.vela", &overlay)),
            );
            for snapshot in [frozen.clone(), server.snapshot()] {
                let id = layout.id("scripts/defs.vela");
                let document = snapshot.workspace().document(&id).expect("owned overlay");
                assert_eq!(
                    (document.text(), document.version()),
                    (overlay.text.as_str(), SourceVersion::new(19))
                );
                assert_eq!(
                    snapshot.databases().source_db().records()[&id].text(),
                    overlay.text
                );
            }
            close(&mut server, &layout.uri("scripts/defs.vela"));
            let expected = if removed {
                source_error(
                    &s,
                    &caller,
                    &layout.uri("scripts/main.vela"),
                    "missingModule",
                )
            } else {
                vec![]
            };
            equivalent(
                &layout,
                &json!({}),
                &caller,
                (!removed).then_some(("scripts/defs.vela", &changed)),
                expected,
                &mut server,
            );
            layout.write_disk("scripts/defs.vela", &original.text);
        }
    }
}

#[test]
fn watched_missing_import_repair_drops_stale_declarations_and_pins_exact_diagnostics() {
    for crlf in [false, true] {
        let s = spec(crlf);
        let layout = Layout::new(&s);
        let caller = doc(&s.files["scripts/main.vela"], false);
        let original = doc(&s.files["scripts/defs.vela"], false);
        let mut server = layout.server(json!({}));
        open(
            &mut server,
            &layout.uri("scripts/main.vela"),
            &caller.text,
            1,
        );
        layout.write_disk("scripts/defs.vela", &variant(&s, "removedName").text);
        let expected = source_error(
            &s,
            &caller,
            &layout.uri("scripts/main.vela"),
            "missingImport",
        );
        publications(
            &watch(&mut server, &layout, &[("scripts/defs.vela", 2)]),
            vec![(layout.uri("scripts/main.vela"), expected.clone())],
            false,
        );
        equivalent(&layout, &json!({}), &caller, None, expected, &mut server);
        layout.write_disk("scripts/defs.vela", &original.text);
        publications(
            &watch(&mut server, &layout, &[("scripts/defs.vela", 2)]),
            vec![(layout.uri("scripts/main.vela"), vec![])],
            false,
        );
        definition(
            &mut server,
            &layout,
            &caller,
            Some(("scripts/defs.vela", &original)),
        );
    }
}

#[test]
fn watched_invalid_notifications_do_not_mutate_or_publish_and_ignored_paths_keep_facts() {
    let s = spec(false);
    let layout = Layout::new(&s);
    let caller = doc(&s.files["scripts/main.vela"], false);
    let original = doc(&s.files["scripts/defs.vela"], false);
    let mut server = layout.server(json!({}));
    open(
        &mut server,
        &layout.uri("scripts/main.vela"),
        &caller.text,
        1,
    );
    let before = server.snapshot();
    for params in [
        Value::Null,
        json!({}),
        json!({"changes":null}),
        json!({"changes":[{"uri":"invalid","type":1}]}),
        json!({"changes":[{"uri":layout.uri("scripts/defs.vela"),"type":"changed"}]}),
        json!({"changes":[{"uri":layout.uri("scripts/defs.vela"),"type":3},{"uri":layout.uri("scripts/defs.vela"),"type":4}]}),
    ] {
        assert!(
            send(
                &mut server,
                json!({"jsonrpc":"2.0","method":"workspace/didChangeWatchedFiles","params":params})
            )
            .is_empty()
        );
        assert_eq!(server.snapshot().generation(), before.generation());
    }
    for invalid in [-1, 0, 4] {
        for events in [vec![3, invalid], vec![invalid, 3]] {
            let changes = events
                .into_iter()
                .map(|typ| json!({"uri":layout.uri("scripts/defs.vela"),"type":typ}))
                .collect::<Vec<_>>();
            assert!(send(&mut server,json!({"jsonrpc":"2.0","method":"workspace/didChangeWatchedFiles","params":{"changes":changes}})).is_empty());
            let snapshot = server.snapshot();
            assert_eq!(snapshot.generation(), before.generation());
            assert_eq!(
                snapshot.databases().source_db().records()[&layout.id("scripts/defs.vela")].text(),
                original.text
            );
            assert_eq!(
                snapshot
                    .workspace()
                    .document(&layout.id("scripts/main.vela"))
                    .expect("overlay")
                    .version(),
                SourceVersion::new(1)
            );
            definition(
                &mut server,
                &layout,
                &caller,
                Some(("scripts/defs.vela", &original)),
            );
        }
    }
    for events in [
        vec![],
        vec![("notes.txt", 1)],
        vec![("scripts/missing.vela", 2)],
    ] {
        publications(
            &watch(&mut server, &layout, &events),
            vec![(layout.uri("scripts/main.vela"), vec![])],
            false,
        );
        assert_eq!(server.snapshot().generation(), before.generation());
        definition(
            &mut server,
            &layout,
            &caller,
            Some(("scripts/defs.vela", &original)),
        );
    }
}

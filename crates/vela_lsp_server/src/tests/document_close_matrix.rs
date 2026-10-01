use super::document_sync_matrix_support::{Layout, open, profiles, published, send};
use super::{TestServer, message_value};
use crate::matrix_fixture::{Document, Spec, document_open as oracle, load, parse_markers};
use serde_json::{Value, json};
use vela_language_service::SourceVersion;

fn spec(crlf: bool) -> Spec {
    let mut s = load("document-change");
    if crlf {
        for text in s.files.values_mut() {
            *text = text.replace('\n', "\r\n");
        }
    }
    s
}
fn doc(text: &str, crlf: bool) -> Document {
    parse_markers(&text.replace('\n', if crlf { "\r\n" } else { "\n" })).expect("authored input")
}
fn close(server: &mut TestServer, uri: &str) -> Vec<Value> {
    send(
        server,
        json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":uri}}}),
    )
}

#[test]
fn document_close_matrix_restores_exact_disk_diagnostics_clears_scratch_and_resets_reopen_versions()
{
    for crlf in [false, true] {
        let s = spec(crlf);
        let layout = Layout::new(&s);
        for profile in profiles() {
            for case in s.oracle["cases"].as_array().expect("cases") {
                let file = case["file"].as_str().expect("file");
                let uri = layout.uri(file);
                let id = layout.id(file);
                let disk = parse_markers(&s.files[file]).expect("disk");
                let overlay = doc(case["steps"][1]["text"].as_str().expect("repair"), crlf);
                let mut server = layout.server(profile.clone());
                assert_eq!(
                    published(&open(&mut server, &uri, &overlay.text, i32::MAX), &uri),
                    oracle::expected(&overlay, &case["steps"][1], &uri)
                );
                let frozen = server.snapshot();
                for params in [
                    Value::Null,
                    json!({}),
                    json!({"textDocument":{"uri":false}}),
                    json!({"textDocument":{"uri":"bad URI"}}),
                ] {
                    assert!(send(&mut server,json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":params})).is_empty());
                    assert_eq!(server.snapshot().generation(), frozen.generation());
                }
                let messages = close(&mut server, &uri);
                assert_eq!(messages.len(), 1);
                let present = file.ends_with(".vela") && !oracle::missing_disk(&s, file);
                let expected = if present {
                    oracle::expected(&disk, case, &uri)
                } else {
                    Vec::new()
                };
                assert_eq!(published(&messages, &uri), expected);
                let closed = server.snapshot();
                assert!(closed.workspace().document(&id).is_none());
                assert!(closed.workspace().open_document_ids().next().is_none());
                assert_eq!(
                    closed.databases().source_db().records().contains_key(&id),
                    present
                );
                if present {
                    let source = &closed.databases().source_db().records()[&id];
                    assert_eq!(
                        (source.text(), source.version()),
                        (disk.text.as_str(), SourceVersion::INITIAL)
                    );
                }
                assert_eq!(
                    frozen
                        .workspace()
                        .document(&id)
                        .expect("frozen overlay")
                        .text(),
                    overlay.text
                );
                assert_eq!(
                    frozen
                        .workspace()
                        .document(&id)
                        .expect("frozen overlay")
                        .version(),
                    SourceVersion::new(2_147_483_647)
                );
                assert_eq!(published(&close(&mut server, &uri), &uri), expected);
                let mut fresh = layout.server(profile.clone());
                assert_eq!(published(&close(&mut fresh, &uri), &uri), expected);
                assert_eq!(
                    published(&open(&mut server, &uri, &disk.text, i32::MIN), &uri),
                    oracle::expected(&disk, case, &uri)
                );
                assert_eq!(
                    server
                        .snapshot()
                        .workspace()
                        .document(&id)
                        .expect("reopened")
                        .version(),
                    SourceVersion::new(u64::from(u32::from_ne_bytes(i32::MIN.to_ne_bytes())))
                );
                assert_eq!(published(&close(&mut server, &uri), &uri), expected);
                layout.check_disk(&s);
            }
        }
    }
}

#[test]
fn document_close_rereads_changed_deleted_and_created_disk_files_without_touching_other_overlays() {
    for crlf in [false, true] {
        let s = spec(crlf);
        let layout = Layout::new(&s);
        let case = s.oracle["cases"]
            .as_array()
            .expect("cases")
            .iter()
            .find(|c| c["file"] == "scripts/invalid.vela")
            .expect("case");
        let file = "scripts/invalid.vela";
        let uri = layout.uri(file);
        let id = layout.id(file);
        let disk = parse_markers(&s.files[file]).expect("disk");
        let shifted = doc(case["steps"][0]["text"].as_str().expect("shifted"), crlf);
        let repaired = doc(case["steps"][1]["text"].as_str().expect("repair"), crlf);
        let mut server = layout.server(json!({}));
        let other = "scripts/valid.vela";
        let other_doc = parse_markers(&s.files[other]).expect("other");
        assert!(
            published(
                &open(&mut server, &layout.uri(other), &other_doc.text, 7),
                &layout.uri(other)
            )
            .is_empty()
        );
        assert!(published(&open(&mut server, &uri, &repaired.text, 9), &uri).is_empty());
        let frozen = server.snapshot();
        layout.write_disk(file, &shifted.text);
        let messages = close(&mut server, &uri);
        assert_eq!(
            published(&messages, &uri),
            oracle::expected(&shifted, &case["steps"][0], &uri)
        );
        assert_eq!(
            server.snapshot().databases().source_db().records()[&id].text(),
            shifted.text
        );
        assert_eq!(
            frozen.workspace().document(&id).expect("frozen").text(),
            repaired.text
        );
        assert_eq!(
            published(&open(&mut server, &uri, &shifted.text, 1), &uri),
            oracle::expected(&shifted, &case["steps"][0], &uri)
        );
        layout.remove_disk(file);
        assert!(published(&close(&mut server, &uri), &uri).is_empty());
        assert!(
            !server
                .snapshot()
                .databases()
                .source_db()
                .records()
                .contains_key(&id)
        );
        layout.write_disk(file, &repaired.text);
        assert!(published(&close(&mut server, &uri), &uri).is_empty());
        assert_eq!(
            server.snapshot().databases().source_db().records()[&id].text(),
            repaired.text
        );
        let scratch = "scripts/scratch.vela";
        let scratch_doc = parse_markers(&s.files[scratch]).expect("scratch");
        open(&mut server, &layout.uri(scratch), &scratch_doc.text, 1);
        layout.write_disk(scratch, &scratch_doc.text);
        assert!(
            published(
                &close(&mut server, &layout.uri(scratch)),
                &layout.uri(scratch)
            )
            .is_empty()
        );
        assert_eq!(
            server.snapshot().databases().source_db().records()[&layout.id(scratch)].text(),
            scratch_doc.text
        );
        layout.remove_disk(scratch);
        assert!(
            published(
                &close(&mut server, &layout.uri(scratch)),
                &layout.uri(scratch)
            )
            .is_empty()
        );
        assert!(
            !server
                .snapshot()
                .databases()
                .source_db()
                .records()
                .contains_key(&layout.id(scratch))
        );
        let before = server.snapshot();
        assert_eq!(
            close(&mut server, &layout.uri("scripts/unopened.vela")),
            vec![
                json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":layout.uri("scripts/unopened.vela"),"diagnostics":[]}})
            ]
        );
        assert_eq!(
            server
                .snapshot()
                .workspace()
                .document(&layout.id(other))
                .map(|d| (d.text().to_owned(), d.version())),
            before
                .workspace()
                .document(&layout.id(other))
                .map(|d| (d.text().to_owned(), d.version()))
        );
        assert_eq!(
            server
                .snapshot()
                .workspace()
                .open_document_ids()
                .collect::<Vec<_>>(),
            vec![layout.id(other)]
        );
        layout.write_disk(file, &disk.text);
        layout.check_disk(&s);
    }
}

#[test]
fn document_close_rebinds_importer_and_discards_real_held_overlay_task_results() {
    for crlf in [false, true] {
        let s = spec(crlf);
        let layout = Layout::new(&s);
        let caller_file = "scripts/open_caller.vela";
        let caller = parse_markers(&s.files[caller_file]).expect("caller");
        let api_file = "scripts/open_api.vela";
        let caller_uri = layout.uri(caller_file);
        let api_uri = layout.uri(api_file);
        let point = caller.markers["call"].start;
        let mut server = layout.server(json!({}));
        open(&mut server, &caller_uri, &caller.text, 7);
        let overlay = doc(
            s.oracle["dependency"][2]["text"].as_str().expect("overlay"),
            crlf,
        );
        open(&mut server, &api_uri, &overlay.text, i32::MAX);
        let before = server.snapshot();
        server.queue_request(20,"textDocument/references",json!({"textDocument":{"uri":caller_uri},"position":{"line":point.line,"character":point.character},"context":{"includeDeclaration":true}}));
        let task = server.receive_task();
        let messages = close(&mut server, &api_uri);
        assert_eq!(messages.len(), 2);
        assert!(published(&messages, &api_uri).is_empty());
        assert!(published(&messages, &caller_uri).is_empty());
        let (outcome, replies) = server.publish_task(task);
        assert_eq!(outcome, crate::task::TaskOutcome::StaleDiscarded);
        assert_eq!(
            replies.iter().map(message_value).collect::<Vec<_>>(),
            vec![
                json!({"jsonrpc":"2.0","id":20,"error":{"code":-32801,"message":"request result is stale because the document was modified"}})
            ]
        );
        for request_id in [30, 31] {
            assert_eq!(
                send(
                    &mut server,
                    json!({"jsonrpc":"2.0","id":request_id,"method":"textDocument/hover","params":{"textDocument":{"uri":caller_uri},"position":{"line":point.line,"character":point.character}}})
                ),
                vec![
                    json!({"jsonrpc":"2.0","id":request_id,"result":{"contents":{"kind":"markdown","value":s.oracle["dependency"][0]["hover"]},"range":oracle::span(&caller,"call")}})
                ]
            );
        }
        assert_eq!(
            server
                .snapshot()
                .workspace()
                .open_document_ids()
                .collect::<Vec<_>>(),
            vec![layout.id(caller_file)]
        );
        assert_eq!(
            server
                .snapshot()
                .workspace()
                .document(&layout.id(caller_file))
                .expect("caller")
                .version(),
            SourceVersion::new(7)
        );
        assert_eq!(
            before
                .workspace()
                .document(&layout.id(api_file))
                .expect("frozen overlay")
                .text(),
            overlay.text
        );
        layout.check_disk(&s);
    }
}

#[test]
fn document_save_ignores_optional_poison_text_and_keeps_open_closed_and_unknown_document_facts() {
    for crlf in [false, true] {
        let s = spec(crlf);
        let layout = Layout::new(&s);
        for profile in profiles() {
            for case in s.oracle["cases"].as_array().expect("cases") {
                let file = case["file"].as_str().expect("file");
                let uri = layout.uri(file);
                let id = layout.id(file);
                let initial = parse_markers(&s.files[file]).expect("initial");
                let mut server = layout.server(profile.clone());
                assert_eq!(
                    published(&open(&mut server, &uri, &initial.text, i32::MAX), &uri),
                    oracle::expected(&initial, case, &uri)
                );
                for params in [
                    json!({"textDocument":{"uri":uri}}),
                    json!({"textDocument":{"uri":uri},"text":"fn poisoned( 中😀"}),
                    json!({"textDocument":{"uri":uri},"text":null}),
                    Value::Null,
                    json!({}),
                    json!({"textDocument":{"uri":false}}),
                    json!({"textDocument":{"uri":uri},"text":false}),
                ] {
                    let before = server.snapshot();
                    let parses = before.databases().parse_db().parse_count();
                    assert!(
                        send(
                            &mut server,
                            json!({"jsonrpc":"2.0","method":"textDocument/didSave","params":params})
                        )
                        .is_empty()
                    );
                    let after = server.snapshot();
                    assert_eq!(after.generation(), before.generation());
                    assert_eq!(after.databases().parse_db().parse_count(), parses);
                    assert_eq!(
                        after
                            .workspace()
                            .document(&id)
                            .map(|d| (d.text().to_owned(), d.version())),
                        before
                            .workspace()
                            .document(&id)
                            .map(|d| (d.text().to_owned(), d.version()))
                    );
                    assert_eq!(
                        after
                            .databases()
                            .source_db()
                            .records()
                            .get(&id)
                            .map(|d| (d.text(), d.version())),
                        before
                            .databases()
                            .source_db()
                            .records()
                            .get(&id)
                            .map(|d| (d.text(), d.version()))
                    );
                }
                close(&mut server, &uri);
                for save_uri in [uri, layout.uri("scripts/unopened.vela")] {
                    let before = server.snapshot();
                    assert!(send(&mut server,json!({"jsonrpc":"2.0","method":"textDocument/didSave","params":{"textDocument":{"uri":save_uri},"text":"fn poisoned("}})).is_empty());
                    assert_eq!(server.snapshot().generation(), before.generation());
                    assert_eq!(server.snapshot().workspace().open_document_ids().count(), 0);
                    assert!(
                        !server
                            .snapshot()
                            .databases()
                            .source_db()
                            .records()
                            .contains_key(&layout.id("scripts/unopened.vela"))
                    );
                }
                layout.check_disk(&s);
            }
        }
        // Disk contents can advance while an overlay is open. Save is still a
        // no-op; close is the explicit point that reloads the current disk.
        let file = "scripts/invalid.vela";
        let case = s.oracle["cases"]
            .as_array()
            .expect("cases")
            .iter()
            .find(|c| c["file"] == file)
            .expect("case");
        let initial = parse_markers(&s.files[file]).expect("initial");
        let repaired = doc(case["steps"][1]["text"].as_str().expect("repair"), crlf);
        let uri = layout.uri(file);
        let mut server = layout.server(json!({}));
        assert_eq!(
            published(&open(&mut server, &uri, &initial.text, 9), &uri),
            oracle::expected(&initial, case, &uri)
        );
        let before = server.snapshot();
        layout.write_disk(file, &repaired.text);
        assert!(send(&mut server,json!({"jsonrpc":"2.0","method":"textDocument/didSave","params":{"textDocument":{"uri":uri},"text":repaired.text}})).is_empty());
        assert_eq!(server.snapshot().generation(), before.generation());
        assert_eq!(
            server
                .snapshot()
                .workspace()
                .document(&layout.id(file))
                .expect("overlay")
                .text(),
            initial.text
        );
        assert_eq!(
            server.snapshot().databases().source_db().records()[&layout.id(file)].text(),
            initial.text
        );
        assert!(published(&close(&mut server, &uri), &uri).is_empty());
        assert_eq!(
            server.snapshot().databases().source_db().records()[&layout.id(file)].text(),
            repaired.text
        );
        layout.write_disk(file, &initial.text);
        layout.check_disk(&s);
    }
}

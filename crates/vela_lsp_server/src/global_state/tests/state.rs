#[test]
fn snapshot_captures_read_only_global_state() {
    let (sender, _receiver) = unbounded();
    let mut launch_configuration = LaunchConfiguration::new();
    launch_configuration.add_workspace_root("/workspace/scripts");
    let mut state = GlobalState::new(sender, launch_configuration);
    let document = DocumentId::from("file:///workspace/scripts/main.vela");

    state
        .project
        .workspace_roots
        .insert("/workspace/scripts".to_owned());
    state.project.open_documents.insert(document.clone());
    state.project.workspace.open_document(
        document.clone(),
        "fn main() { 1 }",
        SourceVersion::new(3),
    );
    state.client_supports_work_done_progress = true;
    state.client_supports_watched_file_registration = true;
    state.project.editor_config = Some(
        EditorConfiguration::from_settings(serde_json::json!({
            "workspace": {
                "roots": ["/workspace/scripts"]
            }
        }))
        .expect("editor config should deserialize"),
    );
    state.project.config = Some(workspace_config_with_schema(
        "/workspace/scripts",
        "/workspace/target/vela/schema.json",
    ));
    state.semantic_token_projection = SemanticTokenProjection::for_client(
        Some(&["type".to_owned(), "function".to_owned()]),
        Some(&["declaration".to_owned()]),
    );
    state.watched_files_registered = true;
    state.watch_files_enabled = false;
    state.initialized = true;

    let snapshot = state.snapshot();
    state.project.workspace.change_document(
        document.clone(),
        "fn main() { 2 }",
        SourceVersion::new(4),
    );
    state.project.open_documents.clear();
    state.project.editor_config = None;
    state.project.config = None;
    state.client_supports_work_done_progress = false;
    state.client_supports_watched_file_registration = false;
    state.semantic_token_projection = SemanticTokenProjection::default();
    state.watched_files_registered = false;
    state.watch_files_enabled = true;
    state.shutdown_requested = true;

    assert_eq!(
        snapshot.launch_configuration().workspace_roots(),
        ["/workspace/scripts"]
    );
    assert_eq!(
        snapshot.workspace().document_text(&document),
        Some("fn main() { 1 }")
    );
    assert_eq!(snapshot.generation(), snapshot.databases().generation());
    assert!(snapshot.workspace_roots().contains("/workspace/scripts"));
    assert!(snapshot.open_documents().contains(&document));
    assert!(snapshot.editor_config().is_some());
    assert_eq!(
        snapshot
            .workspace_config()
            .and_then(|config| config.schema().path()),
        Some("/workspace/target/vela/schema.json")
    );
    assert!(snapshot.client_supports_work_done_progress());
    assert!(snapshot.client_supports_watched_file_registration());
    assert_ne!(
        snapshot.semantic_token_projection(),
        &SemanticTokenProjection::default()
    );
    assert!(snapshot.watched_files_registered());
    assert!(!snapshot.watch_files_enabled());
    assert!(snapshot.is_initialized());
    assert!(!snapshot.is_shutdown_requested());
}

#[test]
fn snapshots_are_immutable_across_authoritative_mutations() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    state.project.workspace.open_document(
        document.clone(),
        "fn main() { 1 }",
        SourceVersion::new(1),
    );
    state
        .project
        .databases
        .mark_schema_missing("/schema/one.json");

    let before = state.snapshot();

    state.project.workspace.change_document(
        document.clone(),
        "fn main() { 2 }",
        SourceVersion::new(2),
    );
    state.project.databases.clear_schema();
    let after = state.snapshot();

    assert_eq!(
        before.workspace().document_text(&document),
        Some("fn main() { 1 }")
    );
    assert!(!before.databases().schema_db().diagnostics().is_empty());
    assert_eq!(
        after.workspace().document_text(&document),
        Some("fn main() { 2 }")
    );
    assert!(after.databases().schema_db().diagnostics().is_empty());
}

#[test]
fn manifest_change_refreshes_one_project_generation() {
    let root = std::env::temp_dir().join(format!(
        "vela_lsp_manifest_generation_{}",
        std::process::id()
    ));
    std::fs::create_dir_all(root.join("src")).expect("fixture source directory");
    let manifest = root.join("vela.toml");
    std::fs::write(
        &manifest,
        "[package]\nid = \"dev.vela.editor\"\nname = \"editor\"\nversion = \"0.1.0\"\n[source]\nroots = [\"src\"]\n",
    )
    .expect("fixture manifest");
    std::fs::write(root.join("src/main.vela"), "pub fn main() {}\n").expect("fixture source");
    let uri = crate::paths::document_path_uri(&manifest.display().to_string());
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state
        .project
        .workspace_roots
        .insert(root.display().to_string());
    let change = state
        .project
        .upsert_watched_file(&uri)
        .expect("initial manifest change");
    state.project.apply_config_change(change);
    state.project.refresh_databases();
    let before = state.project.databases.generation();

    std::fs::write(
        &manifest,
        "[package]\nid = \"dev.vela.editor\"\nname = \"editor\"\nversion = \"0.1.1\"\n[source]\nroots = [\"src\"]\n",
    )
    .expect("updated manifest");
    let change = state
        .project
        .upsert_watched_file(&uri)
        .expect("updated manifest change");
    state.project.apply_config_change(change);
    state.project.refresh_databases();
    let after = state.project.databases.generation();

    assert_eq!(after.get(), before.get() + 1);
    std::fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn dependency_manifest_change_reloads_from_root_and_keeps_last_valid_graph() {
    let root = std::env::temp_dir().join(format!(
        "vela_lsp_dependency_manifest_{}",
        std::process::id()
    ));
    std::fs::create_dir_all(root.join("src")).expect("app source directory");
    std::fs::create_dir_all(root.join("plugin/src")).expect("plugin source directory");
    let root_manifest = root.join("vela.toml");
    let plugin_manifest = root.join("plugin/vela.toml");
    std::fs::write(
        &root_manifest,
        "[package]\nid=\"dev.vela.app\"\nname=\"app\"\nversion=\"0.1.0\"\n[dependencies]\nplugin={path=\"plugin\"}\n",
    )
    .expect("root manifest");
    std::fs::write(
        &plugin_manifest,
        "[package]\nid=\"dev.vela.plugin\"\nname=\"plugin\"\nversion=\"0.1.0\"\n",
    )
    .expect("plugin manifest");
    std::fs::write(root.join("src/main.vela"), "pub fn main() {}\n").expect("app source");
    std::fs::write(root.join("plugin/src/lib.vela"), "pub fn value() {}\n").expect("plugin source");

    let root_uri = crate::paths::document_path_uri(&root_manifest.display().to_string());
    let plugin_uri = crate::paths::document_path_uri(&plugin_manifest.display().to_string());
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state
        .project
        .workspace_roots
        .insert(root.display().to_string());
    let change = state
        .project
        .upsert_watched_file(&root_uri)
        .expect("initial root manifest load");
    state.project.apply_config_change(change);
    assert!(state.project.take_watched_project_changed());
    let app = vela_package::PackageId::new("dev.vela.app").expect("app package ID");
    let plugin = vela_package::PackageId::new("dev.vela.plugin").expect("plugin package ID");
    assert!(
        state
            .project
            .package_graph()
            .is_some_and(|graph| graph.packages().contains_key(&app))
    );

    std::fs::write(
        &plugin_manifest,
        "[package]\nid=\"dev.vela.plugin\"\nname=\"plugin\"\nversion=\"0.2.0\"\n",
    )
    .expect("updated plugin manifest");
    let change = state
        .project
        .upsert_watched_file(&plugin_uri)
        .expect("dependency change reloads root graph");
    state.project.apply_config_change(change);
    assert!(state.project.take_watched_project_changed());
    let graph = state.project.package_graph().expect("package graph");
    assert!(graph.packages().contains_key(&app));
    assert_eq!(
        graph
            .packages()
            .get(&plugin)
            .map(|package| package.version.as_str()),
        Some("0.2.0")
    );

    state.project.refresh_databases();
    let valid_generation = state.project.databases.generation();
    std::fs::write(&plugin_manifest, "[package\n").expect("invalid plugin manifest");
    state.did_change_watched_files(lsp_types::DidChangeWatchedFilesParams {
        changes: vec![lsp_types::FileEvent {
            uri: lsp_types::Url::parse(&plugin_uri).expect("plugin URI"),
            typ: lsp_types::FileChangeType::CHANGED,
        }],
    });
    assert_eq!(state.project.databases.generation(), valid_generation);
    assert!(!state.project.take_watched_project_changed());
    let graph = state
        .project
        .package_graph()
        .expect("last valid graph is retained");
    assert!(graph.packages().contains_key(&app));
    assert_eq!(
        graph
            .packages()
            .get(&plugin)
            .map(|package| package.version.as_str()),
        Some("0.2.0")
    );
    assert!(!state.project.config_diagnostics.is_empty());

    std::fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn client_capabilities_are_owned_by_global_state() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    let params = lsp_types::InitializeParams {
        process_id: None,
        capabilities: serde_json::from_value(serde_json::json!({
            "window": {
                "workDoneProgress": true
            },
            "workspace": {
                "didChangeWatchedFiles": {
                    "dynamicRegistration": true
                }
            },
            "textDocument": {
                "semanticTokens": {
                    "dynamicRegistration": false,
                    "requests": {
                        "range": true,
                        "full": {
                            "delta": true
                        }
                    },
                    "tokenTypes": ["type", "function"],
                    "tokenModifiers": ["declaration"],
                    "formats": ["relative"]
                }
            }
        }))
        .expect("client capabilities should deserialize"),
        ..lsp_types::InitializeParams::default()
    };
    let expected_projection = lsp_semantic_token_projection(&params);

    let initialize = state.initialize(lsp_server::RequestId::from(1), params);

    assert_has_messages(&initialize);
    assert!(state.client_supports_work_done_progress);
    assert!(state.client_supports_watched_file_registration);
    assert_eq!(state.semantic_token_projection, expected_projection);
}

#[test]
fn typed_initialized_uses_global_watcher_capability() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state
        .project
        .workspace_roots
        .insert("/workspace/scripts".to_owned());
    state.client_supports_watched_file_registration = true;

    let first = state.initialized(lsp_types::InitializedParams {});
    let second = state.initialized(lsp_types::InitializedParams {});

    let registration = single_message_value(first);
    assert_eq!(
        registration["method"],
        serde_json::json!("client/registerCapability")
    );
    assert!(state.watched_files_registered);
    assert_no_messages(second);
}

#[test]
fn typed_initialized_uses_global_watch_setting() {
    let (sender, _receiver) = unbounded();
    let mut launch_configuration = LaunchConfiguration::new();
    launch_configuration.set_watch_files_enabled(false);
    let mut state = GlobalState::new(sender, launch_configuration);
    state
        .project
        .workspace_roots
        .insert("/workspace/scripts".to_owned());
    state.client_supports_watched_file_registration = true;

    let result = state.initialized(lsp_types::InitializedParams {});

    assert_no_messages(result);
    assert!(!state.watch_files_enabled);
    assert!(!state.watched_files_registered);
}

#[test]
fn typed_initialized_uses_global_workspace_config() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state.project.config = Some(workspace_config_with_schema(
        "/workspace/scripts",
        "/workspace/target/vela/schema.json",
    ));
    state.client_supports_watched_file_registration = true;

    let result = state.initialized(lsp_types::InitializedParams {});

    let registration = single_message_value(result);
    let watchers = registration["params"]["registrations"][0]["registerOptions"]["watchers"]
        .as_array()
        .expect("watchers should be an array");
    assert!(watchers.iter().any(|watcher| {
        watcher["globPattern"] == serde_json::json!("/workspace/target/vela/schema.json")
    }));
    assert!(state.watched_files_registered);
}

#[test]
fn typed_workspace_folder_changes_use_global_roots() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state
        .project
        .workspace_roots
        .insert("/workspace/scripts".to_owned());

    let result = state.did_change_workspace_folders(lsp_types::DidChangeWorkspaceFoldersParams {
        event: lsp_types::WorkspaceFoldersChangeEvent {
            added: vec![lsp_types::WorkspaceFolder {
                uri: lsp_types::Url::parse("file:///workspace/tools")
                    .expect("workspace folder URI should parse"),
                name: "tools".to_owned(),
            }],
            removed: vec![lsp_types::WorkspaceFolder {
                uri: lsp_types::Url::parse("file:///workspace/scripts")
                    .expect("workspace folder URI should parse"),
                name: "scripts".to_owned(),
            }],
        },
    });

    assert_no_messages(result);
    assert!(!state.project.workspace_roots.contains("/workspace/scripts"));
    assert!(state.project.workspace_roots.contains("/workspace/tools"));
}

#[test]
fn typed_configuration_updates_global_editor_config() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    let before_generation = state.project.databases.generation();

    let result = state.did_change_configuration(lsp_types::DidChangeConfigurationParams {
        settings: serde_json::json!({
            "vela": {
                "workspace": {
                    "roots": ["/workspace/scripts"]
                }
            }
        }),
    });

    assert_no_messages(result);
    assert!(state.project.editor_config.is_some());
    assert!(state.project.config.is_some());
    assert_eq!(
        state.project.databases.generation().get(),
        before_generation.get() + 1
    );
}

#[test]
fn schema_path_is_owned_by_global_workspace_config() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state.project.config = Some(workspace_config_with_schema(
        "/workspace/scripts",
        "/workspace/target/vela/schema.json",
    ));

    assert_eq!(
        state.project.schema_path(),
        Some("/workspace/target/vela/schema.json")
    );
}

#[test]
fn typed_did_save_is_no_response_no_op() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    state.project.open_documents.insert(document.clone());

    let result = state.did_save(lsp_types::DidSaveTextDocumentParams {
        text_document: lsp_types::TextDocumentIdentifier {
            uri: lsp_types::Url::parse(document.as_str())
                .expect("document URI should parse as URL"),
        },
        text: Some("fn main() {}".to_owned()),
    });

    assert_no_messages(result);
    assert!(state.project.open_documents.contains(&document));
}

#[test]
fn typed_did_open_updates_global_workspace_and_diagnostics() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    let document = DocumentId::from("file:///workspace/scripts/main.vela");

    let result = state.did_open(lsp_types::DidOpenTextDocumentParams {
        text_document: lsp_types::TextDocumentItem {
            uri: lsp_types::Url::parse(document.as_str())
                .expect("document URI should parse as URL"),
            language_id: "vela".to_owned(),
            version: 3,
            text: "fn main() {}".to_owned(),
        },
    });

    assert_has_messages(&result);
    assert!(state.project.open_documents.contains(&document));
    assert_eq!(
        state.snapshot().workspace().document_text(&document),
        Some("fn main() {}")
    );
    assert_eq!(
        state.snapshot().generation(),
        state.snapshot().databases().generation()
    );
}

#[test]
fn typed_did_change_applies_incremental_edit_and_syncs_snapshot() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    state
        .project
        .workspace
        .open_document(document.clone(), "one\ntwo", SourceVersion::new(1));
    state.project.open_documents.insert(document.clone());

    let result = state.did_change(lsp_types::DidChangeTextDocumentParams {
        text_document: lsp_types::VersionedTextDocumentIdentifier {
            uri: lsp_types::Url::parse(document.as_str())
                .expect("document URI should parse as URL"),
            version: 2,
        },
        content_changes: vec![lsp_types::TextDocumentContentChangeEvent {
            range: Some(lsp_types::Range {
                start: lsp_types::Position {
                    line: 1,
                    character: 0,
                },
                end: lsp_types::Position {
                    line: 1,
                    character: 3,
                },
            }),
            range_length: None,
            text: "three".to_owned(),
        }],
    });

    assert_has_messages(&result);
    assert_eq!(
        state.snapshot().workspace().document_text(&document),
        Some("one\nthree")
    );
    assert!(state.project.open_documents.contains(&document));
}

#[test]
fn typed_did_close_removes_open_overlay_and_clears_scratch_diagnostics() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    state
        .project
        .workspace
        .open_document(document.clone(), "fn main() {}", SourceVersion::new(1));
    state.project.open_documents.insert(document.clone());

    let result = state.did_close(lsp_types::DidCloseTextDocumentParams {
        text_document: lsp_types::TextDocumentIdentifier {
            uri: lsp_types::Url::parse(document.as_str())
                .expect("document URI should parse as URL"),
        },
    });

    assert_has_messages(&result);
    assert!(!state.project.open_documents.contains(&document));
    assert_eq!(state.snapshot().workspace().document_text(&document), None);
}

#[test]
fn typed_message_sync_updates_global_open_documents() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state.initialized = true;
    let document = DocumentId::from("file:///workspace/scripts/main.vela");

    let result = state
        .handle_message(&lsp_server::Message::Notification(
            lsp_server::Notification {
                method: "textDocument/didOpen".to_owned(),
                params: serde_json::json!({
                    "textDocument": {
                        "uri": document.as_str(),
                        "languageId": "vela",
                        "version": 1,
                        "text": "fn main() {}"
                    }
                }),
            },
        ))
        .expect("message should dispatch");

    assert_has_messages(&result);
    assert!(
        result
            .iter()
            .all(|message| matches!(message, Message::Notification(_))),
        "{result:?}"
    );
    assert!(state.project.open_documents.contains(&document));
    assert_eq!(state.project.open_documents, state.project.open_documents);
}

#[test]
fn lifecycle_flags_are_owned_by_global_state() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());

    let initialize = state.initialize(
        lsp_server::RequestId::from(1),
        lsp_types::InitializeParams {
            process_id: None,
            capabilities: lsp_types::ClientCapabilities::default(),
            ..lsp_types::InitializeParams::default()
        },
    );
    assert_has_messages(&initialize);
    assert!(state.is_initialized());
    assert!(state.initialized);

    let shutdown = state.shutdown(lsp_server::RequestId::from(2), ());
    assert_has_messages(&shutdown);
    assert!(state.is_shutdown_requested());
    assert!(state.shutdown_requested);

    let exit = state.exit(());
    assert_no_messages(exit);
    assert!(state.is_exited());
    assert!(state.exited);

    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    let result = state
        .handle_message(&lsp_server::Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(3),
            method: "exit".to_owned(),
            params: serde_json::Value::Null,
        }))
        .expect("message should dispatch");
    let _response = response_message(result, "exit request should return a response");
    assert!(state.is_exited());
}
use super::*;

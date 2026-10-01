use super::support::*;
use serde_json::{Value, json};

fn registration(
    layout: &Layout,
    id: &str,
    sources: &[&str],
    configs: &[&str],
    schema: Option<&str>,
) -> Value {
    let mut watchers = Vec::new();
    for (roots, pattern) in [(sources, "**/*.vela"), (configs, "vela.toml")] {
        watchers.extend(roots.iter().map(
            |root| json!({"globPattern":{"baseUri":layout.uri(root),"pattern":pattern},"kind":7}),
        ));
    }
    if let Some(schema) = schema {
        watchers.push(json!({"globPattern":layout.path(schema).display().to_string().replace('\\', "/"),"kind":7}));
    }
    json!({"jsonrpc":"2.0","id":id,"method":"client/registerCapability","params":{"registrations":[{"id":id,"method":"workspace/didChangeWatchedFiles","registerOptions":{"watchers":watchers}}]}})
}

fn unregister(next: u32, previous: &str) -> Value {
    json!({"jsonrpc":"2.0","id":format!("vela/watched-files/unregister/{next}"),"method":"client/unregisterCapability","params":{"unregisterations":[{"id":previous,"method":"workspace/didChangeWatchedFiles"}]}})
}

fn package_registration(
    layout: &Layout,
    id: &str,
    sources: &[&str],
    configs: &[&str],
    schema: Option<&str>,
) -> Value {
    let expected = registration(layout, id, sources, configs, schema);
    // Package source roots are canonical physical paths; configuration watchers
    // retain the client's folder spelling. macOS temporary roots can be aliases.
    #[cfg(unix)]
    let expected = {
        let mut expected = expected;
        for (index, source) in sources.iter().enumerate() {
            let physical = layout
                .path(source)
                .canonicalize()
                .expect("owned source root");
            let uri = lsp_types::Url::from_file_path(physical).expect("physical source URI");
            expected["params"]["registrations"][0]["registerOptions"]["watchers"][index]["globPattern"]
                ["baseUri"] = json!(uri);
        }
        expected
    };
    expected
}

fn controls_and_clears(
    messages: Vec<Value>,
    expected: Vec<Value>,
    layout: &Layout,
    schemas: &[&str],
) {
    let (controls, diagnostics): (Vec<_>, Vec<_>) = messages
        .into_iter()
        .partition(|message| message["method"] != "textDocument/publishDiagnostics");
    assert_eq!(controls, expected);
    publications(
        &diagnostics,
        schemas
            .iter()
            .map(|schema| (layout.uri(schema), vec![]))
            .collect(),
        false,
    );
}

fn dynamic_session(layout: &Layout, roots: &[&str]) -> TestServer {
    let mut server = TestServer::new();
    let initialized = send(
        &mut server,
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "processId":null,"workspaceFolders":roots.iter().map(|root|json!({"uri":layout.uri(root),"name":root})).collect::<Vec<_>>(),
            "capabilities":{"workspace":{"didChangeWatchedFiles":{"dynamicRegistration":true,"relativePatternSupport":true}}}
        }}),
    );
    assert_eq!(initialized.len(), 1);
    assert!(initialized[0].get("error").is_none());
    server
}

#[test]
fn dynamic_watchers_follow_settings_and_folders_then_clear_and_reappear() {
    let s = fallback(false);
    let layout = Layout::new(&s);
    let mut server = dynamic_session(&layout, &["roots/left"]);
    controls_and_clears(
        send(
            &mut server,
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        ),
        vec![registration(
            &layout,
            "vela/watched-files",
            &["roots/left"],
            &["roots/left"],
            None,
        )],
        &layout,
        &[],
    );
    controls_and_clears(
        change(
            &mut server,
            json!({"workspaceRoots":[],"hostSchema":layout.path("schema-one.json")}),
        ),
        vec![
            unregister(1, "vela/watched-files"),
            registration(
                &layout,
                "vela/watched-files/1",
                &["roots/left"],
                &["roots/left"],
                Some("schema-one.json"),
            ),
        ],
        &layout,
        &["schema-one.json"],
    );
    controls_and_clears(
        folders(&mut server, &layout, &["roots/right"], &[]),
        vec![
            unregister(2, "vela/watched-files/1"),
            registration(
                &layout,
                "vela/watched-files/2",
                &["roots/left", "roots/right"],
                &["roots/left", "roots/right"],
                Some("schema-one.json"),
            ),
        ],
        &layout,
        &["schema-one.json"],
    );
    let selected = json!({"workspaceRoots":[layout.path("roots/right")],"hostSchema":layout.path("schema-two.json")});
    controls_and_clears(
        change(&mut server, selected.clone()),
        vec![
            unregister(3, "vela/watched-files/2"),
            registration(
                &layout,
                "vela/watched-files/3",
                &["roots/right"],
                &["roots/left", "roots/right"],
                Some("schema-two.json"),
            ),
        ],
        &layout,
        &["schema-one.json", "schema-two.json"],
    );
    assert_eq!(
        change(&mut server, json!({"workspaceRoots":7})),
        vec![
            json!({"jsonrpc":"2.0","method":"window/logMessage","params":{"type":1,"message":"invalid didChangeConfiguration settings: invalid type: integer `7`, expected a sequence"}})
        ]
    );
    controls_and_clears(
        change(&mut server, selected),
        vec![],
        &layout,
        &["schema-one.json", "schema-two.json"],
    );
    controls_and_clears(
        folders(&mut server, &layout, &[], &["roots/left"]),
        vec![
            unregister(4, "vela/watched-files/3"),
            registration(
                &layout,
                "vela/watched-files/4",
                &["roots/right"],
                &["roots/right"],
                Some("schema-two.json"),
            ),
        ],
        &layout,
        &["schema-one.json", "schema-two.json"],
    );
    controls_and_clears(
        folders(&mut server, &layout, &[], &["roots/right"]),
        vec![],
        &layout,
        &["schema-one.json", "schema-two.json"],
    );
    controls_and_clears(
        change(&mut server, json!({"workspaceRoots":[],"hostSchema":""})),
        vec![unregister(5, "vela/watched-files/4")],
        &layout,
        &["schema-one.json", "schema-two.json"],
    );
    assert!(!server.snapshot().watched_files_registered());
    controls_and_clears(
        folders(&mut server, &layout, &["roots/left"], &[]),
        vec![registration(
            &layout,
            "vela/watched-files/6",
            &["roots/left"],
            &["roots/left"],
            None,
        )],
        &layout,
        &["schema-one.json", "schema-two.json"],
    );
    assert!(server.snapshot().watched_files_registered());
    controls_and_clears(
        folders(&mut server, &layout, &["roots/left"], &[]),
        vec![],
        &layout,
        &["schema-one.json", "schema-two.json"],
    );
}

#[test]
fn dynamic_watchers_follow_manifest_roots_and_schema_replacement() {
    let s = spec(false);
    let layout = Layout::new(&s);
    let mut server = dynamic_session(&layout, &[""]);
    controls_and_clears(
        send(
            &mut server,
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        ),
        vec![package_registration(
            &layout,
            "vela/watched-files",
            &["roots/left"],
            &[""],
            Some("schema-one.json"),
        )],
        &layout,
        &[],
    );
    layout.write_disk("vela.toml", "[package]\nid=\"dev.vela.workspace_matrix\"\nname=\"workspace_matrix\"\nversion=\"0.1.0\"\n[source]\nroots=[\"roots/right\"]\n[host]\nschema=\"schema-two.json\"\n");
    controls_and_clears(
        watch(&mut server, &layout, &[("vela.toml", 2)]),
        vec![
            unregister(1, "vela/watched-files"),
            package_registration(
                &layout,
                "vela/watched-files/1",
                &["roots/right"],
                &[""],
                Some("schema-two.json"),
            ),
        ],
        &layout,
        &["schema-one.json", "schema-two.json"],
    );
}

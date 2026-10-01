use super::support::*;
use serde_json::{Value, json};

fn disk_definition(
    server: &mut TestServer,
    layout: &Layout,
    file: &str,
    caller: &Document,
    target: Option<(&str, &Document)>,
) {
    let expected = target.map_or(
        Value::Null,
        |(file, doc)| json!({"uri":layout.uri(file),"range":span(doc,"decl")}),
    );
    assert_eq!(
        send(
            server,
            json!({"jsonrpc":"2.0","id":31,"method":"textDocument/definition","params":params(&layout.uri(file),caller,"call")})
        ),
        vec![json!({"jsonrpc":"2.0","id":31,"result":expected})]
    );
}
fn unavailable_document(server: &mut TestServer, layout: &Layout, file: &str, caller: &Document) {
    assert_eq!(
        send(
            server,
            json!({"jsonrpc":"2.0","id":31,"method":"textDocument/definition","params":params(&layout.uri(file),caller,"call")})
        ),
        vec![
            json!({"jsonrpc":"2.0","id":31,"error":{"code":-32600,"message":"invalid definition position: LSP position line is outside the document"}})
        ]
    );
}

#[test]
fn watched_manifest_switches_roots_retains_last_valid_graph_and_clears_repaired_errors() {
    for crlf in [false, true] {
        let s = spec(crlf);
        let layout = Layout::new(&s);
        let main = doc(&s.files["scripts/main.vela"], false);
        let defs = doc(&s.files["scripts/defs.vela"], false);
        let alternate = doc(&s.files["other/main.vela"], false);
        let other = doc(&s.files["other/defs.vela"], false);
        let config = doc(&s.files["vela.toml"], false);
        let second = variant(&s, "configOther");
        let invalid = variant(&s, "configInvalid");
        let mut server = layout.server(json!({}));
        disk_definition(
            &mut server,
            &layout,
            "scripts/main.vela",
            &main,
            Some(("scripts/defs.vela", &defs)),
        );
        let frozen = server.snapshot();
        layout.write_disk("vela.toml", &second.text);
        publications(
            &watch(&mut server, &layout, &[("vela.toml", 2)]),
            vec![],
            false,
        );
        disk_definition(
            &mut server,
            &layout,
            "other/main.vela",
            &alternate,
            Some(("other/defs.vela", &other)),
        );
        unavailable_document(&mut server, &layout, "scripts/main.vela", &main);
        assert!(
            !server
                .snapshot()
                .databases()
                .source_db()
                .records()
                .contains_key(&layout.id("scripts/defs.vela"))
        );
        assert_eq!(
            frozen.databases().source_db().records()[&layout.id("scripts/defs.vela")].text(),
            defs.text
        );
        layout.write_disk("vela.toml", &invalid.text);
        let mark = invalid.markers["bad"];
        let error = s.oracle["configError"]
            .as_str()
            .expect("literal error")
            .replace("{START}", &mark.start.byte.to_string())
            .replace("{END}", &mark.end.byte.to_string());
        publications(
            &watch(&mut server, &layout, &[("vela.toml", 2)]),
            vec![(
                layout.uri("vela.toml"),
                metadata_error("project::diagnostic", &error),
            )],
            false,
        );
        disk_definition(
            &mut server,
            &layout,
            "other/main.vela",
            &alternate,
            Some(("other/defs.vela", &other)),
        );
        layout.write_disk("vela.toml", &config.text);
        publications(
            &watch(&mut server, &layout, &[("vela.toml", 2)]),
            vec![(layout.uri("vela.toml"), vec![])],
            false,
        );
        disk_definition(
            &mut server,
            &layout,
            "scripts/main.vela",
            &main,
            Some(("scripts/defs.vela", &defs)),
        );
        unavailable_document(&mut server, &layout, "other/main.vela", &alternate);
        let mut fresh = layout.server(json!({}));
        disk_definition(
            &mut fresh,
            &layout,
            "scripts/main.vela",
            &main,
            Some(("scripts/defs.vela", &defs)),
        );
        layout.remove_disk("vela.toml");
        publications(
            &watch(&mut server, &layout, &[("vela.toml", 3)]),
            vec![(layout.uri("vela.toml"), vec![])],
            false,
        );
        assert!(
            server
                .snapshot()
                .databases()
                .source_db()
                .records()
                .contains_key(&layout.id("scripts/defs.vela"))
        );
        disk_definition(&mut server, &layout, "scripts/main.vela", &main, None);
        layout.write_disk("vela.toml", &config.text);
        publications(
            &watch(&mut server, &layout, &[("vela.toml", 1)]),
            vec![(layout.uri("vela.toml"), vec![])],
            false,
        );
        disk_definition(
            &mut server,
            &layout,
            "scripts/main.vela",
            &main,
            Some(("scripts/defs.vela", &defs)),
        );
    }
}

fn schema_queries(server: &mut TestServer, layout: &Layout, source: &Document, fields: &Value) {
    for marker in ["value", "gone", "rank"] {
        let expected=fields[marker].as_str().map_or(Value::Null,|kind|json!({"contents":{"kind":"markdown",
            "value":format!("```vela\nHostCell.{marker}\n```\n\n_field_: {kind}")},"range":span(source,marker)}));
        for id in [51, 52] {
            assert_eq!(
                send(
                    server,
                    json!({"jsonrpc":"2.0","id":id,"method":"textDocument/hover",
                "params":params(&layout.uri("scripts/schema_caller.vela"),source,marker)})
                ),
                vec![json!({"jsonrpc":"2.0","id":id,"result":expected})]
            );
        }
        assert_eq!(
            send(
                server,
                json!({"jsonrpc":"2.0","id":53,"method":"textDocument/definition",
            "params":params(&layout.uri("scripts/schema_caller.vela"),source,marker)})
            ),
            vec![json!({"jsonrpc":"2.0","id":53,"result":null})]
        );
    }
}

#[test]
fn watched_schema_replacement_invalid_missing_and_empty_states_restore_exact_current_facts() {
    for crlf in [false, true] {
        let s = spec(crlf);
        let layout = Layout::new(&s);
        let config = format!(
            "{}{}",
            s.files["vela.toml"],
            if crlf {
                "[host]\r\nschema=\"schema.json\"\r\n"
            } else {
                "[host]\nschema=\"schema.json\"\n"
            }
        );
        layout.write_disk("vela.toml", &config);
        layout.write_disk("schema.json", &s.oracle["schemaOriginal"].to_string());
        let source = doc(&s.files["scripts/schema_caller.vela"], false);
        let mut server = layout.server(json!({}));
        let frozen = server.snapshot();
        for step in s.oracle["schemaStates"].as_array().expect("schema states") {
            let text = step["schema"]
                .as_str()
                .map(|key| s.oracle[key].to_string())
                .or_else(|| step["text"].as_str().map(str::to_owned));
            if let Some(text) = &text {
                layout.write_disk("schema.json", text);
            } else {
                layout.remove_disk("schema.json");
            }
            let error = step["error"].as_str().map(|text| {
                text.replace(
                    "{SCHEMA}",
                    &layout
                        .path("schema.json")
                        .display()
                        .to_string()
                        .replace('\\', "/"),
                )
            });
            publications(
                &watch(
                    &mut server,
                    &layout,
                    &[("schema.json", if text.is_some() { 2 } else { 3 })],
                ),
                vec![(
                    layout.uri("schema.json"),
                    error.as_deref().map_or_else(Vec::new, |message| {
                        metadata_error("schema::diagnostic", message)
                    }),
                )],
                false,
            );
            schema_queries(&mut server, &layout, &source, &step["fields"]);
            let mut fresh = layout.server(json!({}));
            schema_queries(&mut fresh, &layout, &source, &step["fields"]);
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
            for file in s.files.keys().filter(|file| file.ends_with(".vela")) {
                assert!(
                    server
                        .snapshot()
                        .workspace()
                        .document(&layout.id(file))
                        .is_none()
                );
            }
        }
    }
}

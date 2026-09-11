use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};

use crate::tests::{TestServer, notify, request, response_value};

#[test]
fn resolve_preserves_actual_candidate_and_tracks_schema_docs_across_client_profiles() {
    for capabilities in [
        json!({}),
        json!({"textDocument":{"completion":{"completionItem":{
            "snippetSupport":true,"documentationFormat":["markdown"],
            "resolveSupport":{"properties":["documentation"]},"labelDetailsSupport":true
        }}}}),
    ] {
        let temp = crate::tests::support::unique_temp_root("completion-resolve-lifecycle");
        let root = temp.join("中文 % workspace");
        std::fs::create_dir_all(root.join("scripts")).expect("workspace");
        std::fs::write(root.join("vela.toml"),
            "[package]\nid = \"dev.vela.resolve_fixture\"\nname = \"resolve_fixture\"\nversion = \"0.1.0\"\n[source]\nroots = [\"scripts\"]\n[host]\nschema = \"schema.json\"\n")
            .expect("config");
        let schema = root.join("schema.json");
        let schema_uri = lsp_types::Url::from_file_path(&schema).expect("schema URI");
        let schema_text = |docs: &str| {
            json!({"formatVersion":1,"facts":{"types":[{
                "name":"Player", "fact":{"kind":"host","name":"Player"},"docs":docs
            }]}})
            .to_string()
        };
        std::fs::write(&schema, schema_text("Player **first** 中😀 docs.")).expect("schema");
        let uri = lsp_types::Url::from_file_path(root.join("scripts/main.vela")).expect("URI");
        let source = crate::matrix_fixture::parse_markers(
            "fn main() { let text = \"中😀\"; Pla[[cursor]] }\n",
        )
        .expect("markers");
        let mut server = TestServer::new();
        let initialized = response_value(request::<r::Initialize>(
            &mut server,
            1,
            json!({
                "processId":null,"rootUri":lsp_types::Url::from_file_path(&root).expect("root URI"),
                "capabilities":capabilities
            }),
        ));
        assert_eq!(
            initialized["result"]["capabilities"]["completionProvider"]["resolveProvider"],
            true
        );
        let _ = notify::<n::DidChangeWatchedFiles>(
            &mut server,
            json!({"changes":[{
                "uri":lsp_types::Url::from_file_path(root.join("vela.toml")).expect("config URI"),"type":1
            }]}),
        );
        let _ = notify::<n::DidOpenTextDocument>(
            &mut server,
            json!({"textDocument":{
                "uri":uri,"languageId":"vela","version":1,"text":source.text
            }}),
        );
        let cursor = source.markers["cursor"].start;
        let completion = response_value(request::<r::Completion>(
            &mut server,
            2,
            json!({
                "textDocument":{"uri":uri},"position":{"line":cursor.line,"character":cursor.character}
            }),
        ));
        let candidates = completion["result"]["items"]
            .as_array()
            .expect("candidates");
        let matching: Vec<_> = candidates
            .iter()
            .filter(|item| item["label"] == "Player")
            .collect();
        assert_eq!(matching.len(), 1, "{completion}");
        let candidate = matching[0].clone();
        assert!(candidate.get("documentation").is_none());
        assert_eq!(candidate["kind"], 22);
        assert_eq!(candidate["detail"], "Player");
        assert_eq!(
            candidate["data"]["resolve"],
            json!({"kind":"documentation",
            "symbol":{"kind":"schema","name":"Player"}})
        );
        let before_parse = server.snapshot().databases().parse_db().parse_count();
        let before_hir = server.snapshot().databases().hir_db().rebuild_count();
        for id in [3, 4] {
            assert_resolved(
                &mut server,
                id,
                &candidate,
                Some("Player **first** 中😀 docs."),
            );
        }
        assert_eq!(
            server.snapshot().databases().parse_db().parse_count(),
            before_parse
        );
        assert_eq!(
            server.snapshot().databases().hir_db().rebuild_count(),
            before_hir
        );
        for (index, (bytes, docs)) in [
            (
                Some(schema_text("Player updated docs.")),
                Some("Player updated docs."),
            ),
            (Some("{ invalid schema".to_owned()), None),
            (None, None),
            (
                Some(schema_text("Player recovered docs.")),
                Some("Player recovered docs."),
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let deleted = bytes.is_none();
            if let Some(bytes) = bytes {
                std::fs::write(&schema, bytes).expect("change schema");
            } else {
                std::fs::remove_file(&schema).expect("delete schema");
            }
            let _ = notify::<n::DidChangeWatchedFiles>(
                &mut server,
                json!({"changes":[{
                    "uri":schema_uri,"type":if deleted {3} else if index == 3 {1} else {2}
                }]}),
            );
            assert_resolved(
                &mut server,
                10 + i32::try_from(index).expect("bounded"),
                &candidate,
                docs,
            );
        }
        std::fs::remove_dir_all(temp).expect("cleanup isolated workspace");
    }
}

fn assert_resolved(server: &mut TestServer, id: i32, candidate: &Value, docs: Option<&str>) {
    let response = response_value(request::<r::ResolveCompletionItem>(
        server,
        id,
        candidate.clone(),
    ));
    assert_eq!(response["id"], id);
    assert!(response.get("error").is_none(), "{response}");
    let mut expected = candidate.clone();
    if let Some(docs) = docs {
        expected["documentation"] = json!({"kind":"markdown","value":docs});
    }
    assert_eq!(
        response["result"], expected,
        "resolve may only enrich documentation"
    );
}

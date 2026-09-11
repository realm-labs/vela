use std::{collections::BTreeMap, fs};

use lsp_types::{notification as n, request as r};
use serde_json::json;

use super::{TestServer, notify, request, response_value};
use crate::matrix_fixture::{FixtureWorkspace, load};

#[test]
fn shared_fixture_protocol_queries_follow_unicode_disk_overlay_lifecycle() {
    for crlf in [false, true] {
        let mut spec = load("shared-unicode-lifecycle");
        if crlf {
            for source in spec.files.values_mut() {
                *source = source.replace('\n', "\r\n");
            }
            for action in &mut spec.actions {
                if let Some(source) = &mut action.source {
                    *source = source.replace('\n', "\r\n");
                }
            }
        }
        let mut fixture = FixtureWorkspace::new(&spec).expect("shared fixture");
        let root = std::env::temp_dir().join(format!(
            "vela-protocol-shared-{}-{crlf}",
            std::process::id()
        ));
        fixture.materialize(&root).expect("isolated root");
        let uri = |file: &str| {
            lsp_types::Url::from_file_path(root.join(file))
                .expect("file URI")
                .to_string()
        };
        let mut server = TestServer::new();
        let _ = request::<r::Initialize>(
            &mut server,
            1,
            json!({"processId":null,"rootUri":uri(""),"capabilities":{}}),
        );
        let mut versions = BTreeMap::new();
        for (index, action) in spec.actions.iter().enumerate() {
            fixture.apply(action).expect("valid action");
            let file_uri = uri(&action.file);
            match action.op.as_str() {
                "open" => {
                    versions.insert(action.file.clone(), 1);
                    let _ = notify::<n::DidOpenTextDocument>(
                        &mut server,
                        json!({"textDocument":{
                            "uri":file_uri,"languageId":"vela","version":1,"text":fixture.open[&action.file].text
                        }}),
                    );
                }
                "change" => {
                    let version = versions.get_mut(&action.file).expect("open version");
                    *version += 1;
                    let _ = notify::<n::DidChangeTextDocument>(
                        &mut server,
                        json!({"textDocument":{
                        "uri":file_uri,"version":version},"contentChanges":[{"text":fixture.open[&action.file].text}]}),
                    );
                }
                "close" => {
                    let _ = notify::<n::DidCloseTextDocument>(
                        &mut server,
                        json!({"textDocument":{"uri":file_uri}}),
                    );
                }
                "save" => {
                    fs::write(root.join(&action.file), &fixture.disk[&action.file].text)
                        .expect("save fixture");
                    let _ = notify::<n::DidSaveTextDocument>(
                        &mut server,
                        json!({"textDocument":{"uri":file_uri}}),
                    );
                }
                "write" | "delete" => {
                    let exists = root.join(&action.file).exists();
                    let kind = if action.op == "delete" {
                        fs::remove_file(root.join(&action.file)).expect("delete fixture");
                        3
                    } else {
                        fs::write(root.join(&action.file), &fixture.disk[&action.file].text)
                            .expect("write fixture");
                        if exists { 2 } else { 1 }
                    };
                    let _ = notify::<n::DidChangeWatchedFiles>(
                        &mut server,
                        json!({"changes":[{"uri":file_uri,"type":kind}]}),
                    );
                }
                _ => panic!("unsupported action"),
            }
            let caller = fixture
                .document("scripts/main.vela")
                .expect("caller")
                .markers["call"]
                .start;
            let target = response_value(request::<r::GotoDefinition>(
                &mut server,
                100 + i32::try_from(index).expect("small fixture"),
                json!({"textDocument":{"uri":uri("scripts/main.vela")},"position":{"line":caller.line,"character":caller.character}}),
            ));
            let expected = &spec.oracle["afterEachAction"][index];
            if expected.is_null() {
                assert!(target["result"].is_null(), "{target}");
            } else {
                assert_eq!(
                    target["result"],
                    json!({"uri":uri("scripts/helper.vela"),"range":{
                    "start":{"line":expected["line"],"character":expected["character"]},
                    "end":{"line":expected["line"],"character":expected["endCharacter"]}}}),
                    "action {index}, CRLF={crlf}"
                );
            }
        }
        fs::remove_dir_all(root).expect("remove fixture");
    }
}

use std::{fs, path::Path};

use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};

use crate::matrix_fixture::{Document, FixtureWorkspace, load, parse_markers};
use crate::tests::{TestServer, request, response_value, sync_diagnostics};

fn uri(root: &Path, file: &str) -> String {
    lsp_types::Url::from_file_path(root.join(file))
        .expect("file URI")
        .to_string()
}

fn initialized(root: &Path) -> TestServer {
    let mut server = TestServer::new();
    let _ = response_value(request::<r::Initialize>(
        &mut server,
        1,
        json!({"processId":null,"rootUri":uri(root, ""),"capabilities":{}}),
    ));
    server
}

fn expected(document: &Document, uri: &str) -> Value {
    let marker = document.markers["typo"];
    let span = json!({"start":{"line":marker.start.line,"character":marker.start.character},
        "end":{"line":marker.end.line,"character":marker.end.character}});
    let label = |message: &str| json!({"uri":uri,"range":span,"message":message});
    json!([{
        "code":"analysis::unknown_method",
        "message":"unknown method `frist` for `Array(i64)`",
        "severity":1,"source":"vela",
        "range":span,
        "data":{"candidates":[
            {"replacement":"first"},{"replacement":"find"},{"replacement":"last"}
        ],"repairHints":[],"labels":[label("unknown member access"),
            label("did you mean `first`?"),
            label("similar candidates: first, find, last")]}
    }])
}

fn publication(server: &mut TestServer, uri: &str, text: &str, version: i32) -> Value {
    sync_diagnostics::<n::DidOpenTextDocument>(
        server,
        json!({"textDocument":{"uri":uri,"languageId":"vela","version":version,"text":text}}),
    )
}

#[test]
fn configured_disk_overlay_and_outside_scratch_publish_exact_diagnostics() {
    let spec = load("diagnostic-source-state");
    let fixture = FixtureWorkspace::new(&spec).expect("fixture");
    for crlf in [false, true] {
        let parent = crate::tests::support::unique_temp_root("diagnostic-source-state");
        let root = parent.join("中文 % source state");
        fixture.materialize(&root).expect("workspace");
        let main = uri(&root, "scripts/game/main.vela");
        let scratch = uri(&root, "outside/独立 scratch.vela");
        assert!(main.contains('%') && scratch.contains('%'));
        let line_ending = if crlf { "\r\n" } else { "\n" };
        let disk = spec.files["scripts/game/main.vela"].replace('\n', line_ending);
        let overlay = parse_markers(
            &spec.oracle["overlay"]
                .as_str()
                .expect("overlay")
                .replace('\n', line_ending),
        )
        .expect("marked overlay");
        let outside = parse_markers(
            &spec.oracle["scratch"]
                .as_str()
                .expect("scratch")
                .replace('\n', line_ending),
        )
        .expect("marked scratch");
        if crlf {
            fs::write(root.join("scripts/game/main.vela"), &disk).expect("CRLF disk");
        }

        let mut server = initialized(&root);
        let opened = publication(&mut server, &main, &disk, 1);
        assert_eq!(
            opened["params"]["diagnostics"],
            json!([]),
            "valid disk source"
        );
        let changed = sync_diagnostics::<n::DidChangeTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":main,"version":2},
                "contentChanges":[{"text":overlay.text}]}),
        );
        assert_eq!(changed["params"]["diagnostics"], expected(&overlay, &main));
        let mut fresh = initialized(&root);
        let fresh_overlay = publication(&mut fresh, &main, &overlay.text, 2);
        assert_eq!(
            changed["params"]["diagnostics"],
            fresh_overlay["params"]["diagnostics"]
        );

        let closed = sync_diagnostics::<n::DidCloseTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":main}}),
        );
        assert_eq!(closed["params"]["diagnostics"], json!([]), "disk restored");
        let outside_open = publication(&mut server, &scratch, &outside.text, 3);
        assert_eq!(
            outside_open["params"]["diagnostics"],
            expected(&outside, &scratch)
        );
        let mut fresh = initialized(&root);
        let fresh_outside = publication(&mut fresh, &scratch, &outside.text, 3);
        assert_eq!(
            outside_open["params"]["diagnostics"],
            fresh_outside["params"]["diagnostics"]
        );
        let outside_close = sync_diagnostics::<n::DidCloseTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":scratch}}),
        );
        assert_eq!(
            outside_close["params"]["diagnostics"],
            json!([]),
            "scratch cleared"
        );
        fs::remove_dir_all(parent).expect("cleanup");
    }
}

use std::{fs, path::Path};

use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};
use vela_language_service::DocumentId;

use crate::matrix_fixture::{FixtureWorkspace, Spec, load, schema_artifact};
use crate::tests::{
    TestServer, notification_values, notify, request, response_value, sync_diagnostics,
};

#[derive(Clone, Copy)]
enum Phase {
    Original,
    Changed,
    Deleted,
}

fn uri(root: &Path, file: &str) -> String {
    lsp_types::Url::from_file_path(root.join(file))
        .expect("URI")
        .to_string()
}

fn range(line: u32, start: u32, end: u32) -> Value {
    json!({"start":{"line":line,"character":start},"end":{"line":line,"character":end}})
}

fn diagnostic(code: &str, message: &str, severity: u8, span: Value, labels: Vec<Value>) -> Value {
    json!({"code":code,"message":message,"severity":severity,"source":"vela","range":span,
        "data":{"candidates":[],"repairHints":[],"labels":labels}})
}

fn label(uri: &str, span: Value, message: &str) -> Value {
    json!({"uri":uri,"range":span,"message":message})
}

fn expected(root: &Path, phase: Phase) -> Value {
    let main = uri(root, "scripts/game/main.vela");
    let reward = uri(root, "scripts/game/reward.vela");
    let spans = [
        range(0, 10, 41),
        range(1, 0, 32),
        range(2, 0, 31),
        range(3, 0, 34),
        range(4, 0, 34),
    ];
    let module_error = |line: usize, module: &str| {
        diagnostic(
            "hir::unresolved_module",
            &format!("unresolved module `{module}`"),
            1,
            spans[line].clone(),
            vec![label(
                &main,
                spans[line].clone(),
                "no similar modules found",
            )],
        )
    };
    if matches!(phase, Phase::Deleted) {
        return Value::Array(
            (0..4)
                .map(|line| module_error(line, "game::reward"))
                .chain(std::iter::once(module_error(4, "game::absent")))
                .collect(),
        );
    }
    let (typo_line, typo_name, suggestion) = if matches!(phase, Phase::Changed) {
        (0, "grant", "grnat")
    } else {
        (2, "grnat", "grant")
    };
    let mut result = vec![
        diagnostic(
            "hir::unresolved_import",
            &format!("unresolved import `{typo_name}` in module `game::reward`"),
            1,
            spans[typo_line].clone(),
            vec![label(
                &main,
                spans[typo_line].clone(),
                &format!("did you mean `{suggestion}`?"),
            )],
        ),
        diagnostic(
            "hir::private_import",
            "declaration `secret` in module `game::reward` is private",
            1,
            spans[3].clone(),
            vec![
                label(
                    &main,
                    spans[3].clone(),
                    "private declaration cannot be imported from another module",
                ),
                label(&reward, range(2, 0, 25), "declaration is private"),
            ],
        ),
        module_error(4, "game::absent"),
        diagnostic(
            "lsp::unused_import",
            "unused import `spare`",
            2,
            spans[1].clone(),
            vec![label(&main, spans[1].clone(), "import is never used")],
        ),
    ];
    if matches!(phase, Phase::Changed) {
        result.push(diagnostic(
            "lsp::unused_import",
            "unused import `typo`",
            2,
            spans[2].clone(),
            vec![label(&main, spans[2].clone(), "import is never used")],
        ));
    }
    Value::Array(result)
}

fn main_diagnostics(messages: Vec<Value>, root: &Path) -> Value {
    let main = uri(root, "scripts/game/main.vela");
    let publications: Vec<_> = messages
        .into_iter()
        .filter(|message| message["params"]["uri"] == main)
        .collect();
    assert_eq!(
        publications.len(),
        1,
        "one publication for the changed importer"
    );
    publications[0]["params"]["diagnostics"].clone()
}

fn initialized_server(root: &Path, spec: &Spec, fixture: &FixtureWorkspace) -> TestServer {
    let mut server = TestServer::new();
    let _ = response_value(request::<r::Initialize>(
        &mut server,
        1,
        json!({"processId":null,"rootUri":uri(root, ""),"capabilities":{}}),
    ));
    let snapshot = server.snapshot();
    let artifact = schema_artifact(&spec.oracle["schema"], fixture, |file| {
        snapshot.databases().source_db().records()[&DocumentId::from(uri(root, file))]
            .source_id()
            .get()
    });
    fs::create_dir_all(root.join("target")).expect("target");
    fs::write(root.join("target/schema.json"), artifact.to_string()).expect("schema");
    let _ = notify::<n::DidChangeWatchedFiles>(
        &mut server,
        json!({"changes":[{"uri":uri(root, "target/schema.json"),"type":1}]}),
    );
    assert!(
        server
            .snapshot()
            .databases()
            .schema_db()
            .facts()
            .function_fact("host::stamp")
            .is_some(),
        "source-backed schema must load"
    );
    server
}

fn fresh_diagnostics(root: &Path, spec: &Spec, fixture: &FixtureWorkspace) -> Value {
    let mut server = initialized_server(root, spec, fixture);
    let text = fs::read_to_string(root.join("scripts/game/main.vela")).expect("main source");
    let opened = sync_diagnostics::<n::DidOpenTextDocument>(
        &mut server,
        json!({"textDocument":{"uri":uri(root, "scripts/game/main.vela"),
            "languageId":"vela","version":1,"text":text}}),
    );
    opened["params"]["diagnostics"].clone()
}

#[test]
fn cross_module_import_diagnostics_project_exact_owners() {
    let spec = load("diagnostic-import-partitions");
    let fixture = FixtureWorkspace::new(&spec).expect("fixture");
    for crlf in [false, true] {
        let parent = crate::tests::support::unique_temp_root("diagnostic-imports");
        let root = parent.join("中文 % imports");
        fixture.materialize(&root).expect("workspace");
        let original_reward = fixture.disk["scripts/game/reward.vela"].text.clone();
        let main = fixture.disk["scripts/game/main.vela"].text.clone();
        if crlf {
            for file in [
                "scripts/game/reward.vela",
                "scripts/game/main.vela",
                "scripts/schema_defs.vela",
            ] {
                let path = root.join(file);
                let text = fs::read_to_string(&path).expect("source");
                fs::write(path, text.replace('\n', "\r\n")).expect("CRLF source");
            }
        }
        let mut server = initialized_server(&root, &spec, &fixture);
        let opened = sync_diagnostics::<n::DidOpenTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":uri(&root, "scripts/game/main.vela"),"languageId":"vela",
                "version":1,"text":if crlf { main.replace('\n', "\r\n") } else { main }}}),
        );
        assert_eq!(
            opened["params"]["diagnostics"],
            expected(&root, Phase::Original)
        );
        assert_eq!(
            opened["params"]["diagnostics"],
            fresh_diagnostics(&root, &spec, &fixture)
        );
        let reward = root.join("scripts/game/reward.vela");
        let changed_reward = spec.oracle["changedDependency"]
            .as_str()
            .expect("changed dependency");
        fs::write(
            &reward,
            if crlf {
                changed_reward.replace('\n', "\r\n")
            } else {
                changed_reward.to_owned()
            },
        )
        .expect("change reward");
        let changed = notification_values(notify::<n::DidChangeWatchedFiles>(
            &mut server,
            json!({"changes":[{"uri":uri(&root, "scripts/game/reward.vela"),"type":2}]}),
        ));
        assert_eq!(
            main_diagnostics(changed, &root),
            expected(&root, Phase::Changed)
        );
        assert_eq!(
            fresh_diagnostics(&root, &spec, &fixture),
            expected(&root, Phase::Changed)
        );
        fs::remove_file(&reward).expect("delete reward");
        let deleted = notification_values(notify::<n::DidChangeWatchedFiles>(
            &mut server,
            json!({"changes":[{"uri":uri(&root, "scripts/game/reward.vela"),"type":3}]}),
        ));
        assert_eq!(
            main_diagnostics(deleted, &root),
            expected(&root, Phase::Deleted)
        );
        assert_eq!(
            fresh_diagnostics(&root, &spec, &fixture),
            expected(&root, Phase::Deleted)
        );
        fs::write(
            &reward,
            if crlf {
                original_reward.replace('\n', "\r\n")
            } else {
                original_reward
            },
        )
        .expect("restore reward");
        let restored = notification_values(notify::<n::DidChangeWatchedFiles>(
            &mut server,
            json!({"changes":[{"uri":uri(&root, "scripts/game/reward.vela"),"type":1}]}),
        ));
        assert_eq!(
            main_diagnostics(restored, &root),
            expected(&root, Phase::Original)
        );
        assert_eq!(
            fresh_diagnostics(&root, &spec, &fixture),
            expected(&root, Phase::Original)
        );
        fs::remove_dir_all(parent).expect("cleanup");
    }
}

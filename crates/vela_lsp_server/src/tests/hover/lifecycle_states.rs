use crate::matrix_fixture::{Action, FixtureWorkspace, hover_recovery as oracle};
use crate::tests::{TestServer, notification_values, notify, support::unique_temp_root};
use lsp_types::notification as n;
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::PathBuf};

struct Layout {
    parent: PathBuf,
    root: PathBuf,
}
impl Layout {
    fn new(fixture: &FixtureWorkspace) -> Self {
        let parent = unique_temp_root("hover-lifecycle");
        let root = parent.join("中文 % hover lifecycle");
        fixture
            .materialize(&root)
            .expect("new owned physical inputs");
        Self { parent, root }
    }
    fn assert_disk(&self, fixture: &FixtureWorkspace, files: &BTreeSet<String>) {
        for file in files {
            let target = self.root.join(file);
            if let Some(source) = fixture.disk.get(file) {
                assert_eq!(
                    fs::read_to_string(target).expect("physical input"),
                    source.text,
                    "current physical {file}"
                );
            } else {
                assert!(!target.exists(), "physically absent {file}");
            }
        }
    }
    fn write_action(&self, fixture: &FixtureWorkspace, action: &Action, existed: bool) {
        let owned = self.root.canonicalize().expect("owned root");
        let target = self.root.join(&action.file);
        if existed {
            assert!(
                target
                    .canonicalize()
                    .expect("owned file")
                    .starts_with(&owned)
            );
        } else {
            assert!(
                target
                    .parent()
                    .expect("parent")
                    .canonicalize()
                    .expect("owned parent")
                    .starts_with(&owned)
            );
        }
        if action.op == "delete" {
            fs::remove_file(target).expect("delete owned input");
        } else {
            fs::write(target, &fixture.disk[&action.file].text).expect("replace owned input");
        }
    }
}
impl Drop for Layout {
    fn drop(&mut self) {
        let owned = self.parent.canonicalize().expect("owned root");
        assert_eq!(
            owned.parent(),
            Some(
                std::env::temp_dir()
                    .canonicalize()
                    .expect("temp root")
                    .as_path()
            )
        );
        assert!(
            owned
                .file_name()
                .expect("name")
                .to_string_lossy()
                .starts_with("vela-lsp-hover-lifecycle-")
        );
        fs::remove_dir_all(owned).expect("remove owned lifecycle inputs");
    }
}

fn apply(
    server: &mut TestServer,
    fixture: &mut FixtureWorkspace,
    layout: &Layout,
    action: &Action,
    version: i32,
) -> Vec<Value> {
    let existed = fixture.disk.contains_key(&action.file);
    fixture.apply(action).expect("authored lifecycle action");
    let target = super::matrix::uri(&layout.root, &action.file);
    let messages = match action.op.as_str() {
        "open" => notify::<n::DidOpenTextDocument>(
            server,
            json!({"textDocument":{"uri":target,"languageId":"vela","version":version,"text":fixture.open[&action.file].text}}),
        ),
        "change" => notify::<n::DidChangeTextDocument>(
            server,
            json!({"textDocument":{"uri":target,"version":version},"contentChanges":[{"text":fixture.open[&action.file].text}]}),
        ),
        "close" => {
            notify::<n::DidCloseTextDocument>(server, json!({"textDocument":{"uri":target}}))
        }
        "write" | "delete" => {
            layout.write_action(fixture, action, existed);
            let change = if action.op == "delete" {
                3
            } else if existed {
                2
            } else {
                1
            };
            notify::<n::DidChangeWatchedFiles>(
                server,
                json!({"changes":[{"uri":target,"type":change}]}),
            )
        }
        other => panic!("unexpected lifecycle action {other}"),
    };
    notification_values(messages)
}

fn assert_state(server: &TestServer, fixture: &FixtureWorkspace, layout: &Layout, phase: &Value) {
    let snapshot = server.snapshot();
    let db = snapshot.databases();
    for (file, expected) in phase["parseFiles"].as_object().expect("parse files") {
        let id = vela_language_service::DocumentId::from(super::matrix::uri(&layout.root, file));
        assert_eq!(
            !db.parse_db()
                .parse_diagnostics(&id)
                .expect("current parse")
                .is_empty(),
            expected.as_bool().expect("errors"),
            "{} {file}",
            phase["id"]
        );
    }
    for file in phase["absentFiles"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|file| file.ends_with(".vela"))
    {
        assert!(!fixture.disk.contains_key(file));
        let id = vela_language_service::DocumentId::from(super::matrix::uri(&layout.root, file));
        assert!(
            !db.source_db().records().contains_key(&id),
            "{} removed source {file}",
            phase["id"]
        );
    }
    assert_eq!(
        !db.schema_db().diagnostics().is_empty(),
        phase["schemaUnavailable"].as_bool().expect("schema state"),
        "{} schema diagnostics",
        phase["id"]
    );
}

fn assert_publication(layout: &Layout, phase: &Value, messages: &[Value]) {
    let target = super::matrix::uri(&layout.root, "scripts/main.vela");
    let publications = messages
        .iter()
        .filter(|message| {
            message["method"] == "textDocument/publishDiagnostics"
                && message["params"]["uri"] == target
        })
        .collect::<Vec<_>>();
    let expected = phase["callerPublication"]
        .as_bool()
        .expect("publication policy");
    assert_eq!(
        publications.len(),
        usize::from(expected),
        "{} affected caller publications: {messages:?}",
        phase["id"]
    );
    if !expected {
        return;
    }
    let last = publications.last().unwrap_or_else(|| {
        panic!(
            "{} actual caller diagnostic publication: {messages:?}",
            phase["id"]
        )
    });
    let diagnostics = last["params"]["diagnostics"]
        .as_array()
        .expect("diagnostics");
    // Disk-backed close publishes current disk diagnostics. Only scratch close
    // requires an empty publication; these inputs remain in the workspace.
    assert_eq!(
        diagnostics
            .iter()
            .filter(|d| d["code"] == "schema::unavailable")
            .count(),
        usize::from(phase["schemaUnavailable"].as_bool().expect("schema state")),
        "{} actual schema publication: {diagnostics:?}",
        phase["id"]
    );
}

#[test]
fn hover_state_matrix_preserves_current_facts_through_source_and_schema_lifecycle() {
    let mut positions = 0;
    for crlf in [false, true] {
        let spec = oracle::spec("hover-lifecycle", crlf, false);
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let original = fixture.disk.clone();
        let files = fixture.disk.keys().cloned().collect::<BTreeSet<_>>();
        let layout = Layout::new(&fixture);
        let mut server = super::matrix::initialize(&layout.root, &fixture);
        let mut baseline = None;
        let mut last = Vec::new();
        let mut version = 1;
        assert_eq!(
            spec.oracle["queries"].as_array().expect("queries").len(),
            34
        );
        assert_eq!(spec.oracle["phases"].as_array().expect("phases").len(), 30);
        for phase in spec.oracle["phases"].as_array().expect("phases") {
            let mut publications = Vec::new();
            let actions: Vec<Action> =
                serde_json::from_value(phase["actions"].clone()).expect("actions");
            for action in actions {
                version += 1;
                publications.extend(apply(&mut server, &mut fixture, &layout, &action, version));
            }
            assert_publication(&layout, phase, &publications);
            assert_state(&server, &fixture, &layout, phase);
            layout.assert_disk(&fixture, &files);
            let queries = oracle::cases(&spec, phase);
            let (count, actual) = super::matrix::verify_queries(
                &mut server,
                &fixture,
                &layout.root,
                &queries,
                false,
                crlf,
                false,
            );
            positions += count;
            last = actual;
            let fresh_layout = Layout::new(&fixture);
            let mut fresh = super::matrix::initialize(&fresh_layout.root, &fixture);
            assert_state(&fresh, &fixture, &fresh_layout, phase);
            assert_eq!(
                last,
                super::matrix::verify_queries(
                    &mut fresh,
                    &fixture,
                    &fresh_layout.root,
                    &queries,
                    false,
                    crlf,
                    false
                )
                .1,
                "{} incremental/fresh CRLF={crlf}",
                phase["id"]
            );
            layout.assert_disk(&fixture, &files);
            fresh_layout.assert_disk(&fixture, &files);
            baseline.get_or_insert_with(|| last.clone());
        }
        assert_eq!(last, baseline.expect("baseline"), "complete restoration");
        assert_eq!(
            fixture.disk, original,
            "authored disk replacements restore original inputs"
        );
        assert!(fixture.open.is_empty(), "all overlays closed");
    }
    assert_eq!(positions, 3748, "all authored phases and newline variants");
}

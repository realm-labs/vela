use crate::matrix_fixture::{Action, FixtureWorkspace, hover_recovery as oracle};
use crate::tests::{support::unique_temp_root, sync_diagnostics};
use lsp_types::notification as n;
use serde_json::json;
use std::{fs, path::PathBuf};

struct Layout {
    parent: PathBuf,
    root: PathBuf,
}

impl Layout {
    fn new(fixture: &FixtureWorkspace) -> Self {
        let parent = unique_temp_root("hover-s9-recovery");
        let root = parent.join("中文 % hover recovery");
        fixture.materialize(&root).expect("owned fresh inputs");
        Self { parent, root }
    }

    fn assert_disk(&self, fixture: &FixtureWorkspace) {
        for (file, source) in &fixture.disk {
            assert_eq!(
                fs::read_to_string(self.root.join(file)).expect("disk bytes"),
                source.text,
                "unchanged physical {file}"
            );
        }
        assert_eq!(
            self.root.join("schema.json").exists(),
            fixture.disk.contains_key("schema.json"),
            "physical schema mode"
        );
    }
}

impl Drop for Layout {
    fn drop(&mut self) {
        let owned = self.parent.canonicalize().expect("owned root");
        let temp = std::env::temp_dir().canonicalize().expect("temp root");
        assert_eq!(owned.parent(), Some(temp.as_path()));
        assert!(
            owned
                .file_name()
                .expect("name")
                .to_string_lossy()
                .starts_with("vela-lsp-hover-s9-recovery-")
        );
        fs::remove_dir_all(owned).expect("remove owned recovery inputs");
    }
}

pub(super) fn verify(expected_positions: usize) {
    let mut positions = 0;
    for crlf in [false, true] {
        for missing_schema in [false, true] {
            let spec = oracle::spec("hover-s9-recovery", crlf, missing_schema);
            let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
            let disk = fixture.disk.clone();
            let layout = Layout::new(&fixture);
            let mut server = super::matrix::initialize(&layout.root, &fixture);
            let mut version = 1;
            let mut initial = None;
            let mut last = Vec::new();
            for phase in spec.oracle["phases"].as_array().expect("phases") {
                let actions: Vec<Action> =
                    serde_json::from_value(phase["actions"].clone()).expect("actions");
                for action in actions {
                    fixture.apply(&action).expect("overlay action");
                    version += 1;
                    let target = super::matrix::uri(&layout.root, &action.file);
                    match action.op.as_str() {
                        "open" => {
                            let _ = sync_diagnostics::<n::DidOpenTextDocument>(
                                &mut server,
                                json!({"textDocument":{"uri":target,"languageId":"vela","version":version,"text":fixture.open[&action.file].text}}),
                            );
                        }
                        "change" => {
                            let _ = sync_diagnostics::<n::DidChangeTextDocument>(
                                &mut server,
                                json!({"textDocument":{"uri":target,"version":version},"contentChanges":[{"text":fixture.open[&action.file].text}]}),
                            );
                        }
                        "close" => {
                            let _ = sync_diagnostics::<n::DidCloseTextDocument>(
                                &mut server,
                                json!({"textDocument":{"uri":target}}),
                            );
                        }
                        other => panic!("unexpected recovery action {other}"),
                    }
                }
                let queries = oracle::cases(&spec, phase);
                let (count, actual) = super::matrix::verify_queries(
                    &mut server,
                    &fixture,
                    &layout.root,
                    &queries,
                    missing_schema,
                    crlf,
                    false,
                );
                positions += count;
                last = actual;
                for (file, expected) in phase["parseFiles"].as_object().expect("parse files") {
                    let snapshot = server.snapshot();
                    let id = vela_language_service::DocumentId::from(super::matrix::uri(
                        &layout.root,
                        file,
                    ));
                    assert_eq!(
                        !snapshot
                            .databases()
                            .parse_db()
                            .parse_diagnostics(&id)
                            .expect("current parse")
                            .is_empty(),
                        expected.as_bool().expect("error presence"),
                        "{} {file} parser recovery",
                        phase["id"]
                    );
                }
                let fresh_layout = Layout::new(&fixture);
                let mut fresh = super::matrix::initialize(&fresh_layout.root, &fixture);
                assert_eq!(
                    last,
                    super::matrix::verify_queries(
                        &mut fresh,
                        &fixture,
                        &fresh_layout.root,
                        &queries,
                        missing_schema,
                        crlf,
                        false
                    )
                    .1,
                    "{} incremental/fresh CRLF={crlf} missing={missing_schema}",
                    phase["id"]
                );
                initial.get_or_insert_with(|| last.clone());
                assert_eq!(fixture.disk, disk, "unsaved changes preserve disk");
                layout.assert_disk(&fixture);
                fresh_layout.assert_disk(&fixture);
            }
            assert_eq!(
                last,
                initial.expect("baseline"),
                "close restores disk hover"
            );
            assert!(fixture.open.is_empty(), "all overlays closed");
        }
    }
    assert_eq!(
        positions, expected_positions,
        "all phases/newlines/schema modes"
    );
}

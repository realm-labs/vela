use super::rows;
use crate::matrix_fixture::{Document, load, parse_markers, semantic_tokens as oracle};
use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, Workspace,
    WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

fn update(db: &mut LanguageServiceDatabases, document: &Document, helper: &str) {
    db.update(&assemble_project_sources(
        &WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]),
        &[
            SourceFileSnapshot::new("/workspace/scripts/main.vela", document.text.as_str()),
            SourceFileSnapshot::new("/workspace/scripts/defs.vela", helper),
        ],
        &Workspace::new().snapshot(),
    ));
}

pub(in super::super) fn assert_fixture(fixture: &str) {
    let spec = load(fixture);
    let id = DocumentId::from("/workspace/scripts/main.vela");
    for crlf in [false, true] {
        let source = |text: &str| {
            parse_markers(&if crlf {
                text.replace('\n', "\r\n")
            } else {
                text.to_owned()
            })
            .expect("document")
        };
        let positive = source(&spec.files["scripts/main.vela"]);
        let negative = source(
            spec.oracle["negative"]["source"]
                .as_str()
                .expect("negative source"),
        );
        let mut db = LanguageServiceDatabases::new();
        update(&mut db, &positive, &spec.files["scripts/defs.vela"]);
        let original = db.semantic_tokens(&id);
        let mut previous = original.clone();
        for (document, expected) in [
            (&positive, &spec.oracle["positive"]),
            (&negative, &spec.oracle["negative"]["tokens"]),
            (&positive, &spec.oracle["positive"]),
        ] {
            update(&mut db, document, &spec.files["scripts/defs.vela"]);
            let expected = oracle::expected(document, expected, false);
            let full = db.semantic_tokens(&id);
            oracle::assert_stream(&rows(document, full.tokens()), &expected);
            let delta = db.semantic_token_delta(&id, previous.result_id());
            let mut applied = previous.tokens().to_vec();
            for edit in delta.edits().iter().rev() {
                applied.splice(
                    edit.start()..edit.start() + edit.delete_count(),
                    edit.tokens().iter().copied(),
                );
            }
            assert_eq!(applied, full.tokens(), "actual applied delta");
            assert_eq!(delta.result_id(), full.result_id());
            assert!(
                db.semantic_token_delta(&id, full.result_id())
                    .edits()
                    .is_empty()
            );
            let mut fresh = LanguageServiceDatabases::new();
            update(&mut fresh, document, &spec.files["scripts/defs.vela"]);
            assert_eq!(fresh.semantic_tokens(&id), full, "incremental equals fresh");
            for line in 0..document.text.lines().count() {
                let selected: Vec<_> = expected
                    .iter()
                    .filter(|row| row.line == line)
                    .cloned()
                    .collect();
                let range = db.semantic_tokens_in_range(
                    &id,
                    DiagnosticRange::new(Position::new(line, 0), Position::new(line + 1, 0)),
                );
                oracle::assert_stream(&rows(document, range.tokens()), &selected);
            }
            for token in &expected {
                let start = Position::new(token.line, token.column);
                let end = Position::new(token.line, token.column + token.length);
                let range = db.semantic_tokens_in_range(&id, DiagnosticRange::new(start, end));
                oracle::assert_stream(&rows(document, range.tokens()), std::slice::from_ref(token));
                let empty = db.semantic_tokens_in_range(&id, DiagnosticRange::new(end, end));
                assert!(empty.tokens().is_empty(), "empty boundary at {end:?}");
            }
            previous = full;
        }
        assert_eq!(previous, original, "repair restores complete stream and ID");
    }
}

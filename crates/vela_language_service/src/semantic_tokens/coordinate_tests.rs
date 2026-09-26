use super::test_support::rows;
use crate::matrix_fixture::{Document, load, parse_markers, semantic_tokens as oracle};
use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, Workspace,
    WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

fn update(db: &mut LanguageServiceDatabases, id: &DocumentId, document: &Document) {
    db.update(&assemble_project_sources(
        &WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]),
        &[SourceFileSnapshot::new(id.clone(), document.text.as_str())],
        &Workspace::new().snapshot(),
    ));
}

#[test]
fn coordinate_streams_and_applied_deltas_keep_exact_unicode_byte_tokens() {
    let spec = load("semantic-token-coordinates");
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
        let disk = source(&spec.files["scripts/main.vela"]);
        let dirty = source(spec.oracle["dirty"]["source"].as_str().expect("dirty"));
        let same_bytes = source(
            spec.oracle["sameByteLength"]["source"]
                .as_str()
                .expect("same bytes"),
        );
        assert_eq!(same_bytes.text.len(), disk.text.len());
        let mut db = LanguageServiceDatabases::new();
        update(&mut db, &id, &disk);
        let original = db.semantic_tokens(&id);
        let mut previous = original.clone();
        for (document, expected) in [
            (&disk, &spec.oracle["disk"]),
            (&same_bytes, &spec.oracle["sameByteLength"]["tokens"]),
            (&dirty, &spec.oracle["dirty"]["tokens"]),
            (&disk, &spec.oracle["disk"]),
        ] {
            update(&mut db, &id, document);
            let full = db.semantic_tokens(&id);
            if document.text == same_bytes.text {
                assert_eq!(full.tokens(), original.tokens(), "equal byte stream");
                assert_ne!(
                    full.result_id(),
                    original.result_id(),
                    "source-sensitive ID"
                );
            }
            oracle::assert_stream(
                &rows(document, full.tokens()),
                &oracle::expected(document, expected, false),
            );
            let delta = db.semantic_token_delta(&id, previous.result_id());
            let mut applied = previous.tokens().to_vec();
            for edit in delta.edits().iter().rev() {
                applied.splice(
                    edit.start()..edit.start() + edit.delete_count(),
                    edit.tokens().iter().copied(),
                );
            }
            assert_eq!(delta.result_id(), full.result_id());
            assert_eq!(applied, full.tokens(), "applied service delta CRLF={crlf}");
            assert!(
                db.semantic_token_delta(&id, full.result_id())
                    .edits()
                    .is_empty()
            );
            let mut fresh = LanguageServiceDatabases::new();
            update(&mut fresh, &id, document);
            assert_eq!(fresh.semantic_tokens(&id), full);
            previous = full;
        }
        assert_eq!(previous, original, "restored stream and ID CRLF={crlf}");
    }
}

#[test]
fn coordinate_ranges_keep_whole_overlap_tokens_and_reject_empty_interiors() {
    let spec = load("semantic-token-coordinates");
    let id = DocumentId::from("/workspace/scripts/main.vela");
    for crlf in [false, true] {
        let document = parse_markers(&if crlf {
            spec.files["scripts/main.vela"].replace('\n', "\r\n")
        } else {
            spec.files["scripts/main.vela"].clone()
        })
        .expect("document");
        let mut db = LanguageServiceDatabases::new();
        update(&mut db, &id, &document);
        let expected = oracle::expected(&document, &spec.oracle["disk"], false);
        let overlaps = db.semantic_tokens_in_range(
            &id,
            DiagnosticRange::new(Position::new(0, 1), Position::new(0, 2)),
        );
        oracle::assert_stream(&rows(&document, overlaps.tokens()), &expected[..1]);
        for (start, end) in [
            (Position::new(0, 0), Position::new(0, 0)),
            (Position::new(0, 1), Position::new(0, 1)),
            (Position::new(0, 2), Position::new(0, 1)),
            (
                Position::new(1, expected[1].length),
                Position::new(1, expected[2].column),
            ),
        ] {
            assert_eq!(
                db.semantic_tokens_in_range(&id, DiagnosticRange::new(start, end))
                    .tokens(),
                [],
                "empty, reversed and whitespace ranges CRLF={crlf}: {start:?}..{end:?}"
            );
        }
    }
}

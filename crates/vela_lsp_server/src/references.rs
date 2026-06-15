use serde_json::{Value as JsonValue, json};
use vela_language_service::{DiagnosticRange, DocumentHighlight, DocumentHighlightKind, Reference};

pub(crate) fn lsp_references(references: &[Reference]) -> JsonValue {
    JsonValue::Array(references.iter().map(lsp_reference).collect())
}

pub(crate) fn lsp_document_highlights(highlights: &[DocumentHighlight]) -> JsonValue {
    JsonValue::Array(highlights.iter().map(lsp_document_highlight).collect())
}

fn lsp_reference(reference: &Reference) -> JsonValue {
    json!({
        "uri": reference.document_id().as_str(),
        "range": lsp_range(reference.range())
    })
}

fn lsp_document_highlight(highlight: &DocumentHighlight) -> JsonValue {
    json!({
        "range": lsp_range(highlight.range()),
        "kind": lsp_document_highlight_kind(highlight.kind())
    })
}

const fn lsp_document_highlight_kind(kind: DocumentHighlightKind) -> u8 {
    match kind {
        DocumentHighlightKind::Text | DocumentHighlightKind::Call => 1,
        DocumentHighlightKind::Read => 2,
        DocumentHighlightKind::Write => 3,
    }
}

fn lsp_range(range: DiagnosticRange) -> JsonValue {
    json!({
        "start": {
            "line": range.start().line,
            "character": range.start().character
        },
        "end": {
            "line": range.end().line,
            "character": range.end().character
        }
    })
}

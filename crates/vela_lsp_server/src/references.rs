use serde_json::{Value as JsonValue, json};
use vela_language_service::{DiagnosticRange, Reference};

pub(crate) fn lsp_references(references: &[Reference]) -> JsonValue {
    JsonValue::Array(references.iter().map(lsp_reference).collect())
}

fn lsp_reference(reference: &Reference) -> JsonValue {
    json!({
        "uri": reference.document_id().as_str(),
        "range": lsp_range(reference.range())
    })
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

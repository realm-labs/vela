use serde_json::{Value as JsonValue, json};
use vela_language_service::{DiagnosticRange, TextEdit};

pub(crate) fn lsp_text_edits(edits: &[TextEdit]) -> JsonValue {
    JsonValue::Array(edits.iter().map(lsp_text_edit).collect())
}

fn lsp_text_edit(edit: &TextEdit) -> JsonValue {
    json!({
        "range": lsp_range(edit.range()),
        "newText": edit.new_text()
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

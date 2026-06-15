use serde_json::{Value as JsonValue, json};
use vela_language_service::{Definition, DiagnosticRange};

pub(crate) fn lsp_definition(definition: &Definition) -> JsonValue {
    json!({
        "uri": definition.document_id().as_str(),
        "range": lsp_range(definition.range())
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

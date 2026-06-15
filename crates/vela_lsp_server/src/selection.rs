use serde_json::{Value as JsonValue, json};
use vela_language_service::SelectionRange;

pub(crate) fn lsp_selection_ranges(ranges: &[SelectionRange]) -> JsonValue {
    JsonValue::Array(ranges.iter().map(lsp_selection_range).collect())
}

fn lsp_selection_range(range: &SelectionRange) -> JsonValue {
    let mut value = json!({
        "range": lsp_range(range)
    });
    if let Some(parent) = range.parent()
        && let Some(object) = value.as_object_mut()
    {
        object.insert("parent".to_owned(), lsp_selection_range(parent));
    }
    value
}

fn lsp_range(range: &SelectionRange) -> JsonValue {
    let range = range.range();
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

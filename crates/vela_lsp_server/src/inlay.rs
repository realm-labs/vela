use serde_json::{Value as JsonValue, json};
use vela_language_service::{InlayHint, InlayHintKind, Position};

pub(crate) fn lsp_inlay_hints(hints: &[InlayHint]) -> JsonValue {
    JsonValue::Array(hints.iter().map(lsp_inlay_hint).collect())
}

fn lsp_inlay_hint(hint: &InlayHint) -> JsonValue {
    json!({
        "position": lsp_position(hint.position()),
        "label": hint.label(),
        "kind": lsp_inlay_hint_kind(hint.kind()),
        "paddingRight": true
    })
}

fn lsp_inlay_hint_kind(kind: InlayHintKind) -> u8 {
    match kind {
        InlayHintKind::Type => 1,
        InlayHintKind::Parameter => 2,
    }
}

fn lsp_position(position: Position) -> JsonValue {
    json!({
        "line": position.line,
        "character": position.character
    })
}

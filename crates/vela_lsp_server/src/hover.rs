use serde_json::{Value as JsonValue, json};
use vela_language_service::{DiagnosticRange, Hover, HoverKind};

pub(crate) fn lsp_hover(hover: &Hover) -> JsonValue {
    json!({
        "contents": {
            "kind": "markdown",
            "value": hover_markdown(hover)
        },
        "range": lsp_range(hover.range())
    })
}

fn hover_markdown(hover: &Hover) -> String {
    let mut sections = vec![format!(
        "```vela\n{}\n```\n\n_{}_: {}",
        hover.label(),
        hover_kind(hover.kind()),
        hover.detail()
    )];
    if let Some(docs) = hover.docs() {
        sections.push(docs.to_owned());
    }
    sections.join("\n\n")
}

fn hover_kind(kind: HoverKind) -> &'static str {
    match kind {
        HoverKind::Local => "local",
        HoverKind::Parameter => "parameter",
        HoverKind::Global => "global",
        HoverKind::Const => "const",
        HoverKind::Function => "function",
        HoverKind::Type => "type",
        HoverKind::Field => "field",
        HoverKind::Method => "method",
        HoverKind::Variant => "variant",
        HoverKind::Module => "module",
        HoverKind::Unknown => "unknown",
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

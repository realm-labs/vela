use serde_json::{Value as JsonValue, json};
use vela_language_service::{DiagnosticRange, DocumentSymbol, DocumentSymbolKind};

pub(crate) fn lsp_document_symbols(symbols: &[DocumentSymbol]) -> JsonValue {
    JsonValue::Array(symbols.iter().map(lsp_document_symbol).collect())
}

fn lsp_document_symbol(symbol: &DocumentSymbol) -> JsonValue {
    let mut value = json!({
        "name": symbol.name(),
        "kind": lsp_symbol_kind(symbol.kind()),
        "range": lsp_range(symbol.range()),
        "selectionRange": lsp_range(symbol.selection_range()),
        "children": symbol.children().iter().map(lsp_document_symbol).collect::<Vec<_>>()
    });
    if let Some(detail) = symbol.detail()
        && let Some(object) = value.as_object_mut()
    {
        object.insert("detail".to_owned(), JsonValue::String(detail.to_owned()));
    }
    value
}

fn lsp_symbol_kind(kind: DocumentSymbolKind) -> u8 {
    match kind {
        DocumentSymbolKind::Method => 6,
        DocumentSymbolKind::Field => 8,
        DocumentSymbolKind::Enum => 10,
        DocumentSymbolKind::Interface => 11,
        DocumentSymbolKind::Function => 12,
        DocumentSymbolKind::Variable => 13,
        DocumentSymbolKind::Constant => 14,
        DocumentSymbolKind::Object => 19,
        DocumentSymbolKind::EnumMember => 22,
        DocumentSymbolKind::Struct => 23,
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

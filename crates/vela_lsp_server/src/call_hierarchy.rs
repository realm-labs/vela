use serde_json::{Value as JsonValue, json};
use vela_language_service::{CallHierarchyItem, DiagnosticRange, IncomingCall, OutgoingCall};

const SYMBOL_KIND_FUNCTION: u8 = 12;

pub(crate) fn lsp_call_hierarchy_items(items: &[CallHierarchyItem]) -> JsonValue {
    JsonValue::Array(items.iter().map(lsp_call_hierarchy_item).collect())
}

pub(crate) fn lsp_incoming_calls(calls: &[IncomingCall]) -> JsonValue {
    JsonValue::Array(calls.iter().map(lsp_incoming_call).collect())
}

pub(crate) fn lsp_outgoing_calls(calls: &[OutgoingCall]) -> JsonValue {
    JsonValue::Array(calls.iter().map(lsp_outgoing_call).collect())
}

pub(crate) fn service_call_hierarchy_item(
    item: &crate::protocol::CallHierarchyItem,
    document_text: &str,
) -> Result<CallHierarchyItem, String> {
    let line_index = crate::line_index::LineIndex::new(document_text);
    Ok(CallHierarchyItem::new(
        item.name.clone(),
        vela_language_service::DocumentId::from(item.uri.clone()),
        line_index.service_range(item.range)?,
        line_index.service_range(item.selection_range)?,
    ))
}

fn lsp_incoming_call(call: &IncomingCall) -> JsonValue {
    json!({
        "from": lsp_call_hierarchy_item(call.from()),
        "fromRanges": call.from_ranges().iter().copied().map(lsp_range).collect::<Vec<_>>()
    })
}

fn lsp_outgoing_call(call: &OutgoingCall) -> JsonValue {
    json!({
        "to": lsp_call_hierarchy_item(call.to()),
        "fromRanges": call.from_ranges().iter().copied().map(lsp_range).collect::<Vec<_>>()
    })
}

fn lsp_call_hierarchy_item(item: &CallHierarchyItem) -> JsonValue {
    json!({
        "name": item.name(),
        "kind": SYMBOL_KIND_FUNCTION,
        "uri": item.document_id().as_str(),
        "range": lsp_range(item.range()),
        "selectionRange": lsp_range(item.selection_range()),
        "data": {
            "name": item.name(),
            "uri": item.document_id().as_str(),
            "selectionRange": lsp_range(item.selection_range())
        }
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

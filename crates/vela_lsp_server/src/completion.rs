use serde_json::{Value as JsonValue, json};
use vela_language_service::{
    CompletionInsertFormat, CompletionKind, CompletionLabelDetails, CompletionList,
    CompletionResolvePayload, CompletionSymbol, LineIndex, TextRange,
};

pub(crate) fn lsp_completion_list(
    completions: &CompletionList,
    line_index: &LineIndex,
) -> JsonValue {
    json!({
        "isIncomplete": false,
        "items": completions.items().iter().enumerate().map(|(index, item)| {
            lsp_completion_item(
                item,
                line_index,
                index == 0,
            )
        }).collect::<Vec<_>>()
    })
}

fn lsp_completion_item(
    item: &vela_language_service::CompletionItem,
    line_index: &LineIndex,
    preselect: bool,
) -> JsonValue {
    let mut data = json!({
        "source": "vela"
    });
    if let Some(payload) = item.resolve_payload() {
        data["resolve"] = lsp_resolve_payload(payload);
    }
    let mut value = json!({
        "label": item.label(),
        "kind": lsp_completion_kind(item.kind()),
        "detail": item.detail(),
        "filterText": item.filter_text(),
        "labelDetails": lsp_label_details(item.label_details()),
        "preselect": preselect,
        "data": data
    });
    if let Some(insert_text) = item.insert_text() {
        value["insertText"] = json!(insert_text);
    }
    if let Some(text_edit) = item.text_edit() {
        value["textEdit"] = json!({
            "range": lsp_range(text_edit.range(), line_index),
            "newText": text_edit.new_text()
        });
    } else if let (Some(edit_range), Some(insert_text)) = (item.edit_range(), item.insert_text()) {
        value["textEdit"] = json!({
            "range": lsp_range(edit_range, line_index),
            "newText": insert_text
        });
    }
    if item.insert_text().is_some()
        && matches!(item.insert_format(), CompletionInsertFormat::Snippet)
    {
        value["insertTextFormat"] = json!(2);
    }
    value["sortText"] = json!(lsp_sort_text(item, preselect));
    if let Some(documentation) = item.documentation() {
        value["documentation"] = json!({
            "kind": "markdown",
            "value": documentation
        });
    }
    if item.deprecated() {
        value["tags"] = json!([1]);
    }
    value
}

fn lsp_resolve_payload(payload: &CompletionResolvePayload) -> JsonValue {
    match payload {
        CompletionResolvePayload::Documentation { symbol } => json!({
            "kind": "documentation",
            "symbol": lsp_completion_symbol(symbol)
        }),
    }
}

fn lsp_completion_symbol(symbol: &CompletionSymbol) -> JsonValue {
    match symbol {
        CompletionSymbol::Source(name) => json!({ "kind": "source", "name": name }),
        CompletionSymbol::Schema(name) => json!({ "kind": "schema", "name": name }),
        CompletionSymbol::Builtin(name) => json!({ "kind": "builtin", "name": name }),
        CompletionSymbol::Local(name) => json!({ "kind": "local", "name": name }),
    }
}

fn lsp_sort_text(item: &vela_language_service::CompletionItem, preselect: bool) -> String {
    if let Some(sort_text) = item.sort_text() {
        return sort_text.to_owned();
    }
    let relevance = item.relevance();
    let preselect_rank = u8::from(!preselect);
    format!(
        "{:04}_{:02}_{:01}_{}",
        relevance.kind_rank(),
        relevance.match_rank(),
        preselect_rank,
        item.filter_text()
    )
}

fn lsp_label_details(details: &CompletionLabelDetails) -> JsonValue {
    let mut value = json!({});
    if let Some(detail) = details.detail() {
        value["detail"] = json!(detail);
    }
    if let Some(description) = details.description() {
        value["description"] = json!(description);
    }
    value
}

fn lsp_range(range: TextRange, line_index: &LineIndex) -> JsonValue {
    let start = line_index.position(range.start);
    let end = line_index.position(range.end);
    json!({
        "start": {
            "line": start.line,
            "character": start.character
        },
        "end": {
            "line": end.line,
            "character": end.character
        }
    })
}

fn lsp_completion_kind(kind: CompletionKind) -> u8 {
    match kind {
        CompletionKind::Keyword => 14,
        CompletionKind::Snippet => 15,
        CompletionKind::Binding => 6,
        CompletionKind::Value => 12,
        CompletionKind::Const => 21,
        CompletionKind::Field => 5,
        CompletionKind::Method => 2,
        CompletionKind::Module => 9,
        CompletionKind::Variant => 20,
        CompletionKind::Function => 3,
        CompletionKind::Type => 22,
        CompletionKind::Trait => 8,
        CompletionKind::Parameter => 6,
    }
}

use serde_json::{Value as JsonValue, json};
use vela_language_service::{
    CompletionInsertFormat, CompletionKind, CompletionLabelDetails, CompletionList, LineIndex,
    TextRange,
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
                completions.context().replace_range(),
                line_index,
                index == 0,
            )
        }).collect::<Vec<_>>()
    })
}

fn lsp_completion_item(
    item: &vela_language_service::CompletionItem,
    replace_range: TextRange,
    line_index: &LineIndex,
    preselect: bool,
) -> JsonValue {
    let mut value = json!({
        "label": item.label(),
        "kind": lsp_completion_kind(item.kind()),
        "detail": item.detail(),
        "filterText": item.filter_text(),
        "labelDetails": lsp_label_details(item.label_details()),
        "preselect": preselect,
        "data": {
            "source": "vela"
        }
    });
    if let Some(insert_text) = item.insert_text() {
        value["insertText"] = json!(insert_text);
    }
    if let Some(text_edit) = item.text_edit() {
        value["textEdit"] = json!({
            "range": lsp_range(text_edit.range(), line_index),
            "newText": text_edit.new_text()
        });
    } else if item.source_range().is_some() && item.insert_text().is_some() {
        value["textEdit"] = json!({
            "range": lsp_range(replace_range, line_index),
            "newText": item.insert_text().expect("checked insert text")
        });
    }
    if item.insert_text().is_some()
        && matches!(item.insert_format(), CompletionInsertFormat::Snippet)
    {
        value["insertTextFormat"] = json!(2);
    }
    if let Some(sort_text) = item.sort_text() {
        value["sortText"] = json!(sort_text);
    }
    value
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
        CompletionKind::Binding => 6,
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

use serde_json::{Value as JsonValue, json};
use vela_language_service::{CompletionKind, CompletionList};

pub(crate) fn lsp_completion_list(completions: &CompletionList) -> JsonValue {
    json!({
        "isIncomplete": false,
        "items": completions.items().iter().map(lsp_completion_item).collect::<Vec<_>>()
    })
}

fn lsp_completion_item(item: &vela_language_service::CompletionItem) -> JsonValue {
    let mut value = json!({
        "label": item.label(),
        "kind": lsp_completion_kind(item.kind()),
        "detail": item.detail(),
        "data": {
            "source": "vela"
        }
    });
    if let Some(insert_text) = item.insert_text() {
        value["insertText"] = json!(insert_text);
    } else if matches!(
        item.kind(),
        CompletionKind::Function | CompletionKind::Method
    ) {
        value["insertText"] = json!(format!("{}($0)", item.label()));
        value["insertTextFormat"] = json!(2);
    }
    if let Some(sort_text) = item.sort_text() {
        value["sortText"] = json!(sort_text);
    }
    value
}

fn lsp_completion_kind(kind: CompletionKind) -> u8 {
    match kind {
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

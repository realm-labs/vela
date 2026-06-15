use serde_json::{Value as JsonValue, json};
use vela_language_service::{CodeAction, CodeActionKind};

use crate::rename::lsp_workspace_edit;

pub(crate) fn lsp_code_actions(actions: &[CodeAction]) -> JsonValue {
    JsonValue::Array(actions.iter().map(lsp_code_action).collect())
}

fn lsp_code_action(action: &CodeAction) -> JsonValue {
    json!({
        "title": action.title(),
        "kind": lsp_code_action_kind(action.kind()),
        "edit": lsp_workspace_edit(action.edit())
    })
}

fn lsp_code_action_kind(kind: CodeActionKind) -> &'static str {
    kind.as_lsp_kind()
}

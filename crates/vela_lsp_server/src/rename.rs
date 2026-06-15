use serde_json::{Value as JsonValue, json};
use vela_language_service::{
    DiagnosticRange, PrepareRename, RenameRiskKind, TextEdit, WorkspaceEdit,
};

pub(crate) fn lsp_prepare_rename(rename: &PrepareRename) -> JsonValue {
    json!({
        "range": lsp_range(rename.range()),
        "placeholder": rename.placeholder()
    })
}

pub(crate) fn lsp_workspace_edit(edit: &WorkspaceEdit) -> JsonValue {
    let changes = edit
        .document_edits()
        .iter()
        .map(|document_edit| {
            (
                document_edit.document_id().as_str().to_owned(),
                JsonValue::Array(
                    document_edit
                        .edits()
                        .iter()
                        .map(lsp_text_edit)
                        .collect::<Vec<_>>(),
                ),
            )
        })
        .collect::<serde_json::Map<_, _>>();

    if edit.risks().is_empty() {
        json!({ "changes": changes })
    } else {
        json!({
            "changes": changes,
            "changeAnnotations": lsp_change_annotations(edit)
        })
    }
}

fn lsp_text_edit(edit: &TextEdit) -> JsonValue {
    json!({
        "range": lsp_range(edit.range()),
        "newText": edit.new_text()
    })
}

fn lsp_change_annotations(edit: &WorkspaceEdit) -> JsonValue {
    JsonValue::Object(
        edit.risks()
            .iter()
            .enumerate()
            .map(|(index, risk)| {
                let kind = match risk.kind() {
                    RenameRiskKind::HotReloadAbi => "hotReloadAbi",
                };
                (
                    format!("renameRisk{index}"),
                    json!({
                        "label": risk.message(),
                        "description": kind,
                        "needsConfirmation": true
                    }),
                )
            })
            .collect(),
    )
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

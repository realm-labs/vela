use lsp_server::{Message, Notification};
use serde_json::{Value as JsonValue, json};
use vela_language_service::DocumentId;

use super::project_state::ProjectState;
use crate::{lsp::to_proto, paths::document_path_uri};

const WORKSPACE_DIAGNOSTICS_PROGRESS_TOKEN: &str = "vela/workspace-diagnostics";

impl ProjectState {
    pub(super) fn publish_open_diagnostics(&mut self) -> Vec<Message> {
        let mut notifications = Vec::new();
        if !self.open_documents.is_empty() {
            notifications.extend(self.open_documents.iter().map(|document_id| {
                let diagnostics = self.databases.diagnostics_for_document(document_id);
                let mut diagnostics = to_proto::diagnostics(&diagnostics);
                diagnostics.extend(to_proto::project_diagnostics(
                    &self.analysis_diagnostics,
                    document_id,
                ));
                publish_diagnostics_notification(document_id.as_str(), diagnostics, None)
            }));
        }

        notifications.extend(self.config_diagnostic_notifications());
        notifications.extend(self.schema_diagnostic_notifications());
        notifications
    }

    pub(super) fn publish_document_diagnostics(
        &mut self,
        uri: &str,
        document_id: &DocumentId,
    ) -> Message {
        let diagnostics = self.databases.diagnostics_for_document(document_id);
        let mut diagnostics = to_proto::diagnostics(&diagnostics);
        diagnostics.extend(to_proto::project_diagnostics(
            &self.analysis_diagnostics,
            document_id,
        ));
        publish_diagnostics_notification(uri, diagnostics, None)
    }

    fn config_diagnostic_notifications(&self) -> Vec<Message> {
        self.config_documents
            .iter()
            .map(|document_id| {
                publish_diagnostics_notification(
                    document_id.as_str(),
                    to_proto::project_diagnostics(&self.config_diagnostics, document_id),
                    None,
                )
            })
            .collect()
    }

    fn schema_diagnostic_notifications(&self) -> Vec<Message> {
        let diagnostics = to_proto::schema_diagnostics(self.databases.schema_db().diagnostics());
        let active_document = self
            .schema_path()
            .map(document_path_uri)
            .map(DocumentId::from);
        self.schema_documents
            .iter()
            .map(|document_id| {
                let diagnostics = if active_document.as_ref() == Some(document_id) {
                    diagnostics.clone()
                } else {
                    Vec::new()
                };
                publish_diagnostics_notification(document_id.as_str(), diagnostics, None)
            })
            .collect()
    }
}

pub(super) fn publish_diagnostics_notification(
    uri: &str,
    diagnostics: Vec<lsp_types::Diagnostic>,
    error: Option<String>,
) -> Message {
    let uri = lsp_types::Url::parse(uri).expect("diagnostic document URI should parse");
    let params = lsp_types::PublishDiagnosticsParams {
        uri,
        diagnostics,
        version: None,
    };
    let mut params =
        serde_json::to_value(params).expect("typed publishDiagnostics params should serialize");
    if let Some(error) = error
        && let Some(object) = params.as_object_mut()
    {
        object.insert("error".to_owned(), JsonValue::String(error));
    }
    Message::Notification(Notification {
        method: "textDocument/publishDiagnostics".to_owned(),
        params,
    })
}

pub(super) fn with_work_done_progress(mut messages: Vec<Message>, title: &str) -> Vec<Message> {
    if messages.is_empty()
        || messages
            .iter()
            .any(|message| !matches!(message, Message::Notification(_)))
    {
        return messages;
    }

    let mut wrapped = Vec::with_capacity(messages.len() + 2);
    wrapped.push(work_done_progress_notification(json!({
        "kind": "begin",
        "title": title,
        "message": "updating open-file diagnostics"
    })));
    wrapped.append(&mut messages);
    wrapped.push(work_done_progress_notification(json!({
        "kind": "end",
        "message": "workspace diagnostics updated"
    })));
    wrapped
}

fn work_done_progress_notification(value: JsonValue) -> Message {
    Message::Notification(Notification {
        method: "$/progress".to_owned(),
        params: json!({
            "token": WORKSPACE_DIAGNOSTICS_PROGRESS_TOKEN,
            "value": value
        }),
    })
}

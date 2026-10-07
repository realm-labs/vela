use lsp_server::{Message, Notification};
use serde_json::{Value as JsonValue, json};
use vela_language_service::DocumentId;

use super::project_state::ProjectState;
use crate::{lsp::to_proto, paths::document_path_uri};

const WORKSPACE_DIAGNOSTICS_PROGRESS_TOKEN: &str = "vela/workspace-diagnostics";

impl ProjectState {
    pub(super) fn publish_sync_diagnostics(&self, changed: &DocumentId) -> Vec<Message> {
        let current =
            if self.open_documents.contains(changed) || self.disk_sources.contains_key(changed) {
                self.publish_document_diagnostics(changed.as_str(), changed)
            } else {
                publish_diagnostics_notification(changed.as_str(), Vec::new(), None)
            };
        let invalidated = self.databases.analysis_db().invalidated_modules();
        let modules = self.databases.project_db().module_by_document();
        let mut messages = vec![current];
        messages.extend(
            self.open_documents
                .iter()
                .filter(|document| {
                    *document != changed
                        && modules
                            .get(*document)
                            .is_some_and(|module| invalidated.contains(module))
                })
                .map(|document| self.publish_document_diagnostics(document.as_str(), document)),
        );
        messages
    }

    pub(super) fn publish_open_diagnostics(&self) -> Vec<Message> {
        self.publish_open_and_changed_diagnostics(&std::collections::BTreeSet::new())
    }

    pub(super) fn publish_open_and_changed_diagnostics(
        &self,
        changed: &std::collections::BTreeSet<DocumentId>,
    ) -> Vec<Message> {
        let mut notifications = Vec::new();
        let documents = self
            .open_documents
            .iter()
            .chain(changed)
            .collect::<std::collections::BTreeSet<_>>();
        notifications.extend(self.open_documents.iter().map(|document_id| {
            self.publish_document_diagnostics(document_id.as_str(), document_id)
        }));

        // Metadata errors may belong to an open source or share an owner.
        // Publish each URI once with all its current facts, including clears.
        let metadata_documents = self
            .config_documents
            .iter()
            .chain(&self.schema_documents)
            .chain(&self.source_documents)
            .filter(|document| !documents.contains(document))
            .collect::<std::collections::BTreeSet<_>>();
        notifications.extend(metadata_documents.into_iter().map(|document| {
            publish_diagnostics_notification(
                document.as_str(),
                self.metadata_diagnostics(document),
                None,
            )
        }));
        // Keep the existing open/metadata publication order, then update prior
        // closed owners (including removed-source clears) without duplicates.
        notifications.extend(
            changed
                .difference(&self.open_documents)
                .map(|document| self.publish_document_diagnostics(document.as_str(), document)),
        );
        notifications
    }

    pub(super) fn publish_document_diagnostics(
        &self,
        uri: &str,
        document_id: &DocumentId,
    ) -> Message {
        let diagnostics = self.databases.diagnostics_for_document(document_id);
        let mut diagnostics = match to_proto::diagnostics(&diagnostics, &self.databases) {
            Ok(diagnostics) => diagnostics,
            Err(error) => return publish_diagnostics_notification(uri, Vec::new(), Some(error)),
        };
        diagnostics.extend(to_proto::project_diagnostics(
            &self.analysis_diagnostics,
            document_id,
        ));
        diagnostics.extend(self.metadata_diagnostics(document_id));
        publish_diagnostics_notification(uri, diagnostics, None)
    }

    fn metadata_diagnostics(&self, document_id: &DocumentId) -> Vec<lsp_types::Diagnostic> {
        let mut diagnostics = to_proto::project_diagnostics(&self.config_diagnostics, document_id);
        diagnostics.extend(to_proto::project_diagnostics(
            &self.source_diagnostics,
            document_id,
        ));
        if self.schema_path().map(document_path_uri).as_deref() == Some(document_id.as_str()) {
            diagnostics.extend(to_proto::schema_diagnostics(
                self.databases.schema_db().diagnostics(),
            ));
        }
        diagnostics
    }

    pub(super) fn publish_document_sync_error(
        &self,
        document: &DocumentId,
        error: String,
    ) -> Message {
        // A rejected edit leaves the source unchanged. Keep its diagnostics in
        // the notification so clients do not clear valid problems on bad input.
        let mut message = self.publish_document_diagnostics(document.as_str(), document);
        let Message::Notification(notification) = &mut message else {
            unreachable!("diagnostics are notifications");
        };
        notification
            .params
            .as_object_mut()
            .expect("typed diagnostic params")
            .insert("error".to_owned(), JsonValue::String(error));
        message
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

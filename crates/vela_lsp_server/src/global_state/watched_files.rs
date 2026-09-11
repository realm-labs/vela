use std::collections::BTreeSet;

use lsp_server::Message;
use lsp_types::DidChangeWatchedFilesParams;
use vela_language_service::DocumentId;

use super::{GlobalState, diagnostics::publish_diagnostics_notification};
use crate::{
    paths::{document_uri_path, workspace_document_uri},
    reload::{ReloadOperation, ReloadTarget, ReloadWork},
};

impl GlobalState {
    pub(crate) fn did_change_watched_files(
        &mut self,
        params: DidChangeWatchedFilesParams,
    ) -> Vec<Message> {
        let schema_path = self.project.schema_path().map(str::to_owned);
        self.reload_scheduler.schedule_watched_files(
            params.changes,
            schema_path.as_deref(),
            &self.project.open_documents,
        );
        let mut deleted_documents = BTreeSet::new();
        for work in self.reload_scheduler.drain() {
            if let ReloadWork::WatchedFile {
                uri,
                operation: ReloadOperation::Remove,
                target: ReloadTarget::Source,
                ..
            } = &work
            {
                let document = DocumentId::from(workspace_document_uri(
                    &document_uri_path(uri),
                    &self.project.workspace_roots,
                ));
                if self
                    .project
                    .databases
                    .source_db()
                    .records()
                    .contains_key(&document)
                {
                    deleted_documents.insert(document);
                }
            }
            self.apply_reload_work(work);
        }
        self.project.refresh_databases_after_watched_changes();
        let mut messages = self.project.publish_open_diagnostics();
        // Closing a file can leave published disk diagnostics. Clear them only
        // after the whole batch, when neither disk nor an open overlay survives.
        for document in deleted_documents {
            if !self
                .project
                .databases
                .source_db()
                .records()
                .contains_key(&document)
            {
                messages.push(publish_diagnostics_notification(
                    document.as_str(),
                    Vec::new(),
                    None,
                ));
            }
        }
        self.wrap_workspace_diagnostics(messages)
    }
}

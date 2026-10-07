use std::collections::BTreeSet;

use lsp_server::Message;
use lsp_types::{DidChangeWatchedFilesParams, FileChangeType};
use vela_language_service::DocumentId;

use super::GlobalState;
use crate::{
    paths::{document_uri_path, workspace_document_uri},
    reload::{ReloadOperation, ReloadTarget, ReloadWork},
};

impl GlobalState {
    pub(crate) fn did_change_watched_files(
        &mut self,
        params: DidChangeWatchedFilesParams,
    ) -> Vec<Message> {
        // The wire type accepts unknown integer values. Validate the entire
        // notification before coalescing or applying any otherwise valid event.
        if params.changes.iter().any(|change| {
            ![
                FileChangeType::CREATED,
                FileChangeType::CHANGED,
                FileChangeType::DELETED,
            ]
            .contains(&change.typ)
        }) {
            return Vec::new();
        }
        let schema_path = self.project.schema_path().map(str::to_owned);
        self.reload_scheduler.schedule_watched_files(
            params.changes,
            schema_path.as_deref(),
            &self.project.open_documents,
        );
        let mut changed_documents = BTreeSet::new();
        for work in self.reload_scheduler.drain() {
            if let ReloadWork::WatchedFile {
                uri,
                operation,
                target: ReloadTarget::Source,
                ..
            } = &work
            {
                let document = DocumentId::from(workspace_document_uri(
                    &document_uri_path(uri),
                    &self.project.workspace_roots,
                ));
                if self.project.closed_diagnostic_documents.contains(&document)
                    || (*operation == ReloadOperation::Remove
                        && self
                            .project
                            .databases
                            .source_db()
                            .records()
                            .contains_key(&document))
                {
                    changed_documents.insert(document);
                }
            }
            self.apply_reload_work(work);
        }
        self.project.refresh_databases_after_watched_changes();
        // Read current facts only after the whole coalesced batch. A surviving
        // overlay wins; a removed source clears its previous publication.
        let messages = self
            .project
            .publish_open_and_changed_diagnostics(&changed_documents);
        let mut messages = self.wrap_workspace_diagnostics(messages);
        messages.extend(self.refresh_watched_files());
        messages
    }
}

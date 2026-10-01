use lsp_server::{Message, Notification};
use lsp_types::notification::{LogMessage, Notification as LspNotification};
use lsp_types::{
    DidChangeConfigurationParams, DidChangeWorkspaceFoldersParams, LogMessageParams, MessageType,
};
use vela_language_service::WorkspaceRoot;

use super::GlobalState;
use crate::{config::EditorConfiguration, config_change::ConfigChange};

impl GlobalState {
    pub(crate) fn did_change_configuration(
        &mut self,
        params: DidChangeConfigurationParams,
    ) -> Vec<Message> {
        let editor_config = match EditorConfiguration::from_settings(params.settings) {
            Ok(config) => config,
            Err(error) => {
                // Settings have no document URI. Report the error through the
                // standard client log rather than inventing a diagnostic owner.
                return vec![Message::Notification(Notification::new(
                    LogMessage::METHOD.to_owned(),
                    LogMessageParams {
                        typ: MessageType::ERROR,
                        message: format!("invalid didChangeConfiguration settings: {error}"),
                    },
                ))];
            }
        };
        self.apply_config_change(ConfigChange::from_editor_settings(editor_config));
        self.project.refresh_databases();
        self.project.publish_open_diagnostics()
    }

    pub(crate) fn did_change_workspace_folders(
        &mut self,
        params: DidChangeWorkspaceFoldersParams,
    ) -> Vec<Message> {
        let mut roots = self.project.workspace_roots.clone();
        for folder in params.event.removed {
            roots.remove(WorkspaceRoot::from(folder.uri.to_string()).path());
        }
        for folder in params.event.added {
            roots.insert(
                WorkspaceRoot::from(folder.uri.to_string())
                    .path()
                    .to_owned(),
            );
        }
        if roots != self.project.workspace_roots {
            self.reload_scheduler.schedule_workspace_roots(roots);
            for work in self.reload_scheduler.drain() {
                self.apply_reload_work(work);
            }
            self.project.refresh_databases();
        }
        self.publish_workspace_diagnostics()
    }
}

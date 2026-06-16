use serde::Deserialize;

use crate::config::EditorConfiguration;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InitializeParams {
    pub(crate) root_uri: Option<String>,
    pub(crate) workspace_folders: Option<Vec<WorkspaceFolder>>,
    pub(crate) initialization_options: Option<EditorConfiguration>,
    #[serde(default)]
    pub(crate) capabilities: ClientCapabilities,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClientCapabilities {
    window: Option<WindowClientCapabilities>,
    workspace: Option<WorkspaceClientCapabilities>,
}

impl ClientCapabilities {
    pub(crate) fn supports_work_done_progress(&self) -> bool {
        self.window
            .as_ref()
            .is_some_and(|window| window.work_done_progress)
    }

    pub(crate) fn supports_watched_file_registration(&self) -> bool {
        self.workspace
            .as_ref()
            .and_then(|workspace| workspace.did_change_watched_files.as_ref())
            .is_some_and(|watched_files| watched_files.dynamic_registration)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WindowClientCapabilities {
    work_done_progress: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceClientCapabilities {
    did_change_watched_files: Option<DidChangeWatchedFilesClientCapabilities>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DidChangeWatchedFilesClientCapabilities {
    dynamic_registration: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceFolder {
    pub(crate) uri: String,
    #[allow(dead_code)]
    pub(crate) name: Option<String>,
}

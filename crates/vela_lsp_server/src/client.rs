use serde::Deserialize;

use crate::config::EditorConfiguration;
use crate::semantic_tokens::SemanticTokenProjection;

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
    text_document: Option<TextDocumentClientCapabilities>,
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

    pub(crate) fn semantic_token_projection(&self) -> SemanticTokenProjection {
        let semantic_tokens = self
            .text_document
            .as_ref()
            .and_then(|text_document| text_document.semantic_tokens.as_ref());
        SemanticTokenProjection::for_client(
            semantic_tokens.map(|semantic_tokens| semantic_tokens.token_types.as_slice()),
            semantic_tokens.map(|semantic_tokens| semantic_tokens.token_modifiers.as_slice()),
        )
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

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TextDocumentClientCapabilities {
    semantic_tokens: Option<SemanticTokensClientCapabilities>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SemanticTokensClientCapabilities {
    #[serde(default)]
    token_types: Vec<String>,
    #[serde(default)]
    token_modifiers: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceFolder {
    pub(crate) uri: String,
    #[allow(dead_code)]
    pub(crate) name: Option<String>,
}

use std::collections::BTreeSet;

use lsp_types::InitializeParams as LspInitializeParams;
use vela_language_service::WorkspaceRoot;

pub(crate) fn workspace_roots_from_lsp_initialize(
    params: &LspInitializeParams,
) -> BTreeSet<String> {
    params
        .workspace_folders
        .iter()
        .flatten()
        .map(|folder| WorkspaceRoot::from(folder.uri.to_string()))
        .chain(
            #[allow(deprecated)]
            params
                .root_uri
                .iter()
                .map(ToString::to_string)
                .map(WorkspaceRoot::from),
        )
        .map(|root| root.path().to_owned())
        .collect()
}

pub(crate) fn lsp_supports_work_done_progress(params: &LspInitializeParams) -> bool {
    params
        .capabilities
        .window
        .as_ref()
        .and_then(|window| window.work_done_progress)
        .unwrap_or(false)
}

pub(crate) fn lsp_supports_watched_file_registration(params: &LspInitializeParams) -> bool {
    params
        .capabilities
        .workspace
        .as_ref()
        .and_then(|workspace| workspace.did_change_watched_files.as_ref())
        .and_then(|watched_files| watched_files.dynamic_registration)
        .unwrap_or(false)
}

pub(crate) fn lsp_semantic_token_projection(
    params: &LspInitializeParams,
) -> crate::semantic_tokens::SemanticTokenProjection {
    let semantic_tokens = params
        .capabilities
        .text_document
        .as_ref()
        .and_then(|text_document| text_document.semantic_tokens.as_ref());
    let token_types = semantic_tokens.map(|semantic_tokens| {
        semantic_tokens
            .token_types
            .iter()
            .map(lsp_types::SemanticTokenType::as_str)
            .map(str::to_owned)
            .collect::<Vec<_>>()
    });
    let token_modifiers = semantic_tokens.map(|semantic_tokens| {
        semantic_tokens
            .token_modifiers
            .iter()
            .map(lsp_types::SemanticTokenModifier::as_str)
            .map(str::to_owned)
            .collect::<Vec<_>>()
    });
    crate::semantic_tokens::SemanticTokenProjection::for_client(
        token_types.as_deref(),
        token_modifiers.as_deref(),
    )
}

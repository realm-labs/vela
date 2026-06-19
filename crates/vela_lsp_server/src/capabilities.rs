use lsp_types::{
    CallHierarchyServerCapability, CodeActionKind, CodeActionOptions, CodeActionProviderCapability,
    CompletionOptions, DeclarationCapability, DocumentOnTypeFormattingOptions,
    FoldingRangeProviderCapability, HoverProviderCapability, InitializeResult, InlayHintOptions,
    InlayHintServerCapabilities, OneOf, RenameOptions, SemanticTokensFullOptions,
    SemanticTokensOptions, SemanticTokensServerCapabilities, ServerCapabilities, ServerInfo,
    SignatureHelpOptions, TextDocumentSyncCapability, TextDocumentSyncKind,
    TextDocumentSyncOptions, TextDocumentSyncSaveOptions, WorkDoneProgressOptions,
    WorkspaceFoldersServerCapabilities, WorkspaceServerCapabilities,
};
use serde_json::Value as JsonValue;

use crate::semantic_tokens::{self, SemanticTokenProjection};

pub(crate) fn initialize_result(semantic_token_projection: &SemanticTokenProjection) -> JsonValue {
    let result = InitializeResult {
        capabilities: ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Options(
                TextDocumentSyncOptions {
                    open_close: Some(true),
                    change: Some(TextDocumentSyncKind::INCREMENTAL),
                    will_save: None,
                    will_save_wait_until: None,
                    save: Some(TextDocumentSyncSaveOptions::Supported(false)),
                },
            )),
            selection_range_provider: Some(true.into()),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            completion_provider: Some(CompletionOptions {
                resolve_provider: Some(true),
                trigger_characters: Some(trigger_characters(&[".", ":", "{", "(", ",", "|"])),
                all_commit_characters: None,
                work_done_progress_options: WorkDoneProgressOptions {
                    work_done_progress: None,
                },
                completion_item: None,
            }),
            signature_help_provider: Some(SignatureHelpOptions {
                trigger_characters: Some(trigger_characters(&["(", ","])),
                retrigger_characters: Some(trigger_characters(&[","])),
                work_done_progress_options: WorkDoneProgressOptions {
                    work_done_progress: None,
                },
            }),
            definition_provider: Some(OneOf::Left(true)),
            type_definition_provider: Some(true.into()),
            references_provider: Some(OneOf::Left(true)),
            document_highlight_provider: Some(OneOf::Left(true)),
            document_symbol_provider: Some(OneOf::Left(true)),
            workspace_symbol_provider: Some(OneOf::Left(true)),
            code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions {
                code_action_kinds: Some(vec![CodeActionKind::QUICKFIX]),
                work_done_progress_options: WorkDoneProgressOptions {
                    work_done_progress: None,
                },
                resolve_provider: None,
            })),
            document_formatting_provider: Some(OneOf::Left(true)),
            document_range_formatting_provider: Some(OneOf::Left(true)),
            document_on_type_formatting_provider: Some(DocumentOnTypeFormattingOptions {
                first_trigger_character: "}".to_owned(),
                more_trigger_character: Some(trigger_characters(&["\n"])),
            }),
            rename_provider: Some(OneOf::Right(RenameOptions {
                prepare_provider: Some(true),
                work_done_progress_options: WorkDoneProgressOptions {
                    work_done_progress: None,
                },
            })),
            folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
            declaration_provider: Some(DeclarationCapability::Simple(true)),
            workspace: Some(WorkspaceServerCapabilities {
                workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                    supported: Some(true),
                    change_notifications: Some(OneOf::Left(true)),
                }),
                file_operations: None,
            }),
            call_hierarchy_provider: Some(CallHierarchyServerCapability::Simple(true)),
            semantic_tokens_provider: Some(
                SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                    legend: semantic_tokens::semantic_tokens_legend(semantic_token_projection),
                    range: Some(true),
                    full: Some(SemanticTokensFullOptions::Delta { delta: Some(true) }),
                }),
            ),
            inlay_hint_provider: Some(OneOf::Right(InlayHintServerCapabilities::Options(
                InlayHintOptions {
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                    resolve_provider: Some(false),
                },
            ))),
            ..Default::default()
        },
        server_info: Some(ServerInfo {
            name: "vela_lsp_server".to_owned(),
            version: Some(env!("CARGO_PKG_VERSION").to_owned()),
        }),
    };
    let mut value = serde_json::to_value(result).expect("initialize result should serialize");
    value["capabilities"]["workDoneProgress"] = JsonValue::Bool(true);
    value
}

fn trigger_characters(characters: &[&str]) -> Vec<String> {
    characters
        .iter()
        .map(|character| (*character).to_owned())
        .collect()
}

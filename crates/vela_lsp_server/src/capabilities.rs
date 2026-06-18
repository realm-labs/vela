use serde_json::{Value as JsonValue, json};

use crate::semantic_tokens::{self, SemanticTokenProjection};

pub(crate) fn initialize_result(semantic_token_projection: &SemanticTokenProjection) -> JsonValue {
    json!({
        "capabilities": {
            "workDoneProgress": true,
            "textDocumentSync": {
                "openClose": true,
                "change": 2,
                "save": false
            },
            "completionProvider": {
                "resolveProvider": true,
                "triggerCharacters": [".", ":", "{", "(", ",", "|"]
            },
            "signatureHelpProvider": {
                "triggerCharacters": ["(", ","],
                "retriggerCharacters": [","]
            },
            "hoverProvider": true,
            "definitionProvider": true,
            "declarationProvider": true,
            "typeDefinitionProvider": true,
            "referencesProvider": true,
            "renameProvider": {
                "prepareProvider": true
            },
            "codeActionProvider": {
                "codeActionKinds": ["quickfix"]
            },
            "callHierarchyProvider": true,
            "documentHighlightProvider": true,
            "documentSymbolProvider": true,
            "foldingRangeProvider": true,
            "documentFormattingProvider": true,
            "documentRangeFormattingProvider": true,
            "documentOnTypeFormattingProvider": {
                "firstTriggerCharacter": "}",
                "moreTriggerCharacter": ["\n"]
            },
            "selectionRangeProvider": true,
            "semanticTokensProvider": {
                "legend": semantic_tokens::semantic_tokens_legend(semantic_token_projection),
                "range": true,
                "full": {
                    "delta": true
                }
            },
            "inlayHintProvider": {
                "resolveProvider": false
            },
            "workspaceSymbolProvider": true,
            "workspace": {
                "workspaceFolders": {
                    "supported": true,
                    "changeNotifications": true
                }
            }
        },
        "serverInfo": {
            "name": "vela_lsp_server",
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TextDocumentPositionParams {
    pub(crate) text_document: TextDocumentIdentifier,
    pub(crate) position: LspPosition,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TextDocumentIdentifier {
    pub(crate) uri: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodeActionParams {
    pub(crate) text_document: TextDocumentIdentifier,
    pub(crate) range: LspRange,
    #[allow(dead_code)]
    pub(crate) context: CodeActionContext,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodeActionContext {
    #[allow(dead_code)]
    pub(crate) diagnostics: Vec<serde_json::Value>,
    #[allow(dead_code)]
    pub(crate) only: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DocumentFormattingParams {
    pub(crate) text_document: TextDocumentIdentifier,
    #[allow(dead_code)]
    pub(crate) options: FormattingOptions,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DocumentRangeFormattingParams {
    pub(crate) text_document: TextDocumentIdentifier,
    pub(crate) range: LspRange,
    #[allow(dead_code)]
    pub(crate) options: FormattingOptions,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DocumentOnTypeFormattingParams {
    pub(crate) text_document: TextDocumentIdentifier,
    pub(crate) position: LspPosition,
    pub(crate) ch: String,
    #[allow(dead_code)]
    pub(crate) options: FormattingOptions,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FormattingOptions {
    #[allow(dead_code)]
    pub(crate) tab_size: u32,
    #[allow(dead_code)]
    pub(crate) insert_spaces: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReferencesParams {
    pub(crate) text_document: TextDocumentIdentifier,
    pub(crate) position: LspPosition,
    pub(crate) context: ReferenceContext,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReferenceContext {
    pub(crate) include_declaration: bool,
}

pub(crate) type PrepareRenameParams = TextDocumentPositionParams;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RenameParams {
    pub(crate) text_document: TextDocumentIdentifier,
    pub(crate) position: LspPosition,
    pub(crate) new_name: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) struct LspRange {
    pub(crate) start: LspPosition,
    pub(crate) end: LspPosition,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) struct LspPosition {
    pub(crate) line: u32,
    pub(crate) character: u32,
}

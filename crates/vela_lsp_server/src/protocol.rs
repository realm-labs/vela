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

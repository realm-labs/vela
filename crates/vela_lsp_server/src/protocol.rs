use serde::Deserialize;
use vela_language_service::Position;

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
pub(crate) struct DocumentSymbolParams {
    pub(crate) text_document: TextDocumentIdentifier,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FoldingRangeParams {
    pub(crate) text_document: TextDocumentIdentifier,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SelectionRangeParams {
    pub(crate) text_document: TextDocumentIdentifier,
    pub(crate) positions: Vec<LspPosition>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSymbolParams {
    pub(crate) query: String,
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

pub(crate) fn service_position(position: LspPosition) -> Position {
    Position::new(position.line as usize, position.character as usize)
}

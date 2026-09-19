use super::{GlobalStateSnapshot, documents::snapshot_document_text};
use crate::{ErrorCode, line_index::LineIndex, lsp::to_proto};
use vela_language_service::{
    DiagnosticRange, DocumentHighlight, DocumentId, PrepareRename, Reference, WorkspaceEdit,
};

pub(super) fn respond<T: serde::Serialize>(
    id: lsp_server::RequestId,
    result: Result<T, String>,
    context: &'static str,
) -> Vec<lsp_server::Message> {
    match result {
        Ok(value) => super::responses::ok_typed(id, value, context),
        Err(error) => {
            super::responses::error(id, ErrorCode::InternalError, format!("{context}: {error}"))
        }
    }
}

fn range(index: &LineIndex<'_>, range: DiagnosticRange) -> Result<lsp_types::Range, String> {
    Ok(lsp_types::Range::new(
        index.lsp_position(range.start())?,
        index.lsp_position(range.end())?,
    ))
}

pub(super) fn locations(
    snapshot: &GlobalStateSnapshot,
    references: &[Reference],
) -> Result<Vec<lsp_types::Location>, String> {
    let mut result = to_proto::reference_locations(references);
    for (location, reference) in result.iter_mut().zip(references) {
        let text = snapshot_document_text(snapshot, reference.document_id());
        location.range = range(&LineIndex::new(&text), reference.range())?;
    }
    Ok(result)
}

pub(super) fn highlights(
    snapshot: &GlobalStateSnapshot,
    document: &DocumentId,
    highlights: &[DocumentHighlight],
) -> Result<Vec<lsp_types::DocumentHighlight>, String> {
    let text = snapshot_document_text(snapshot, document);
    let index = LineIndex::new(&text);
    let mut result = to_proto::document_highlights(highlights);
    for (item, highlight) in result.iter_mut().zip(highlights) {
        item.range = range(&index, highlight.range())?;
    }
    Ok(result)
}

pub(super) fn prepare(
    snapshot: &GlobalStateSnapshot,
    rename: &PrepareRename,
) -> Result<lsp_types::PrepareRenameResponse, String> {
    let text = snapshot_document_text(snapshot, rename.document_id());
    to_proto::prepare_rename(rename, &LineIndex::new(&text))
}

pub(super) fn edit(
    snapshot: &GlobalStateSnapshot,
    edit: &WorkspaceEdit,
) -> Result<lsp_types::WorkspaceEdit, String> {
    let mut result = to_proto::workspace_edit_with_versions(edit, |document| {
        if !snapshot.open_documents.contains(document.document_id()) {
            return None;
        }
        snapshot
            .workspace
            .document(document.document_id())
            .map(|document| document.version())
    });
    for document in edit.document_edits() {
        let text = snapshot_document_text(snapshot, document.document_id());
        let index = LineIndex::new(&text);
        let uri = lsp_types::Url::parse(document.document_id().as_str())
            .map_err(|error| error.to_string())?;
        let ranges = document
            .edits()
            .iter()
            .map(|edit| range(&index, edit.range()))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(changes) = result
            .changes
            .as_mut()
            .and_then(|changes| changes.get_mut(&uri))
        {
            for (edit, range) in changes.iter_mut().zip(&ranges) {
                edit.range = *range;
            }
        }
        if let Some(lsp_types::DocumentChanges::Edits(changes)) = &mut result.document_changes {
            for change in changes
                .iter_mut()
                .filter(|change| change.text_document.uri == uri)
            {
                for (edit, range) in change.edits.iter_mut().zip(&ranges) {
                    match edit {
                        lsp_types::OneOf::Left(edit) => edit.range = *range,
                        lsp_types::OneOf::Right(edit) => edit.text_edit.range = *range,
                    }
                }
            }
        }
    }
    Ok(result)
}

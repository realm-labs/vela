use super::{GlobalStateSnapshot, documents::snapshot_document_text};
use crate::{line_index::LineIndex, lsp::to_proto};
use vela_language_service::{
    CodeAction, DiagnosticRange, DocumentHighlight, DocumentId, PrepareRename, Reference,
    WorkspaceEdit,
};

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
        let original_uri = lsp_types::Url::parse(document.document_id().as_str())
            .map_err(|error| error.to_string())?;
        let uri = client_spelled_edit_uri(&original_uri, &snapshot.open_documents)?;
        if uri != original_uri {
            if let Some(changes) = result.changes.as_mut()
                && let Some(edits) = changes.remove(&original_uri)
            {
                changes.insert(uri.clone(), edits);
            }
            if let Some(lsp_types::DocumentChanges::Edits(changes)) = &mut result.document_changes {
                for change in changes
                    .iter_mut()
                    .filter(|change| change.text_document.uri == original_uri)
                {
                    change.text_document.uri = uri.clone();
                }
            }
        }
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

pub(super) fn code_actions(
    snapshot: &GlobalStateSnapshot,
    actions: &[CodeAction],
) -> Result<lsp_types::CodeActionResponse, String> {
    let mut result = to_proto::code_actions(actions);
    for (projected, source) in result.iter_mut().zip(actions) {
        let lsp_types::CodeActionOrCommand::CodeAction(projected) = projected else {
            return Err("code action projection produced a command".to_owned());
        };
        projected.edit = Some(edit(snapshot, source.edit())?);
    }
    Ok(result)
}

fn client_spelled_edit_uri(
    uri: &lsp_types::Url,
    open_documents: &std::collections::BTreeSet<DocumentId>,
) -> Result<lsp_types::Url, String> {
    if !cfg!(windows) {
        return Ok(uri.clone());
    }
    let Some(suffix) = uri.as_str().strip_prefix("file:///") else {
        return Ok(uri.clone());
    };
    let Some((drive, tail)) = suffix.split_at_checked(1) else {
        return Ok(uri.clone());
    };
    let Some(tail) = tail.strip_prefix(':') else {
        return Ok(uri.clone());
    };
    for open in open_documents {
        let Some(spelling) = open.as_str().strip_prefix("file:///") else {
            continue;
        };
        let Some((open_drive, rest)) = spelling.split_at_checked(1) else {
            continue;
        };
        if !open_drive.eq_ignore_ascii_case(drive) {
            continue;
        }
        let colon = if rest.starts_with("%3A") {
            "%3A"
        } else if rest.starts_with("%3a") {
            "%3a"
        } else if rest.starts_with(':') {
            ":"
        } else {
            continue;
        };
        return lsp_types::Url::parse(&format!("file:///{open_drive}{colon}{tail}"))
            .map_err(|error| error.to_string());
    }
    Ok(uri.clone())
}

#[cfg(all(test, windows))]
mod tests {
    use super::client_spelled_edit_uri;
    use std::collections::BTreeSet;
    use vela_language_service::DocumentId;

    #[test]
    fn closed_windows_edit_uses_the_clients_open_drive_uri_spelling() {
        let open = BTreeSet::from([DocumentId::from(
            "file:///f%3A/workspace/scripts/open.vela".to_owned(),
        )]);
        let closed = lsp_types::Url::parse("file:///F:/workspace/scripts/closed.vela")
            .expect("closed file URI");
        assert_eq!(
            client_spelled_edit_uri(&closed, &open)
                .expect("client URI")
                .as_str(),
            "file:///f%3A/workspace/scripts/closed.vela"
        );
    }
}

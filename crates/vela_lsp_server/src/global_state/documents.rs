use vela_language_service::SourceVersion;

use crate::{line_index::LineIndex, protocol::LspPosition};

impl super::GlobalState {
    pub(crate) fn did_save(
        &mut self,
        _params: lsp_types::DidSaveTextDocumentParams,
    ) -> Vec<lsp_server::Message> {
        Vec::new()
    }
}

pub(super) fn source_version(version: i32) -> SourceVersion {
    u64::try_from(version)
        .ok()
        .map_or(SourceVersion::INITIAL, SourceVersion::new)
}

pub(super) fn apply_document_changes(
    current_text: Option<&str>,
    changes: Vec<lsp_types::TextDocumentContentChangeEvent>,
) -> Result<String, String> {
    let mut text = current_text.map(str::to_owned);
    for change in changes {
        match change.range {
            Some(range) => {
                let Some(current) = text.as_mut() else {
                    return Err("ranged didChange requires an open document".to_owned());
                };
                apply_range_edit(current, range, &change.text)?;
            }
            None => text = Some(change.text),
        }
    }
    text.ok_or_else(|| "didChange requires at least one content change".to_owned())
}

fn apply_range_edit(
    text: &mut String,
    range: lsp_types::Range,
    replacement: &str,
) -> Result<(), String> {
    let line_index = LineIndex::new(text);
    let start = line_index.offset(position(range.start))?;
    let end = line_index.offset(position(range.end))?;
    if start > end {
        return Err("didChange range start must not be after the end".to_owned());
    }
    text.replace_range(start..end, replacement);
    Ok(())
}

const fn position(position: lsp_types::Position) -> LspPosition {
    LspPosition {
        line: position.line,
        character: position.character,
    }
}

use crate::line_index::LineIndex;
use std::collections::HashMap;
use vela_language_service::{
    DiagnosticRange, DocumentSymbol, LanguageServiceDatabases, WorkspaceSymbol,
    WorkspaceSymbolLocation,
};

pub(crate) fn workspace_symbols(
    symbols: &[WorkspaceSymbol],
    databases: &LanguageServiceDatabases,
) -> Result<lsp_types::WorkspaceSymbolResponse, String> {
    // Construct at most one index per requested source, from the same immutable
    // database snapshot that supplied the symbols. Service columns stay bytes.
    let mut indexes = HashMap::new();
    let mut rows = Vec::with_capacity(symbols.len());
    for symbol in symbols {
        let location = match symbol.location() {
            WorkspaceSymbolLocation::Source { document_id, range } => {
                let index = match indexes.entry(document_id) {
                    std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        let source = databases
                            .source_db()
                            .records()
                            .get(document_id)
                            .ok_or_else(|| {
                                format!(
                                    "workspace symbol source is unavailable: {}",
                                    document_id.as_str()
                                )
                            })?;
                        entry.insert(LineIndex::new(source.text()))
                    }
                };
                let start = index.lsp_position(range.start())?;
                let end = index.lsp_position(range.end())?;
                if start > end {
                    return Err("workspace symbol range start is after its end".to_owned());
                }
                let uri = lsp_types::Url::parse(document_id.as_str())
                    .map_err(|error| format!("invalid workspace symbol URI: {error}"))?;
                lsp_types::OneOf::Left(lsp_types::Location::new(
                    uri,
                    lsp_types::Range::new(start, end),
                ))
            }
            WorkspaceSymbolLocation::Schema => {
                lsp_types::OneOf::Right(lsp_types::WorkspaceLocation {
                    uri: lsp_types::Url::parse("vela-schema:").expect("fixed schema URI"),
                })
            }
        };
        rows.push(lsp_types::WorkspaceSymbol {
            name: symbol.name().to_owned(),
            kind: super::symbol_kind(symbol.kind()),
            tags: None,
            container_name: symbol.container_name().map(str::to_owned),
            location,
            data: symbol
                .detail()
                .map(|detail| serde_json::json!({"detail":detail})),
        });
    }
    Ok(lsp_types::WorkspaceSymbolResponse::Nested(rows))
}

pub(crate) fn document_symbols(
    symbols: &[DocumentSymbol],
    text: &str,
) -> Result<lsp_types::DocumentSymbolResponse, String> {
    let index = LineIndex::new(text);
    Ok(lsp_types::DocumentSymbolResponse::Nested(
        symbols
            .iter()
            .map(|s| project(s, &index))
            .collect::<Result<_, _>>()?,
    ))
}

fn project(
    symbol: &DocumentSymbol,
    index: &LineIndex<'_>,
) -> Result<lsp_types::DocumentSymbol, String> {
    let range = |r: DiagnosticRange| -> Result<lsp_types::Range, String> {
        Ok(lsp_types::Range::new(
            index.lsp_position(r.start())?,
            index.lsp_position(r.end())?,
        ))
    };
    #[allow(deprecated)]
    Ok(lsp_types::DocumentSymbol {
        name: symbol.name().to_owned(),
        detail: symbol.detail().map(str::to_owned),
        kind: super::symbol_kind(symbol.kind()),
        tags: None,
        deprecated: None,
        range: range(symbol.range())?,
        selection_range: range(symbol.selection_range())?,
        children: if symbol.children().is_empty() {
            None
        } else {
            Some(
                symbol
                    .children()
                    .iter()
                    .map(|s| project(s, index))
                    .collect::<Result<_, _>>()?,
            )
        },
    })
}

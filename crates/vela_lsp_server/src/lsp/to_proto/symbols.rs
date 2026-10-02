use crate::line_index::LineIndex;
use vela_language_service::{DiagnosticRange, DocumentSymbol};

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

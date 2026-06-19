#![allow(dead_code)]

use vela_language_service::{
    CallHierarchyItem as ServiceCallHierarchyItem, DiagnosticRange, DocumentId, Position,
};

use crate::{
    line_index::LineIndex,
    protocol::{LspPosition, LspRange},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FormattingOptions {
    pub(crate) tab_size: u32,
    pub(crate) insert_spaces: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextDocumentPositionInput {
    pub(crate) document_id: DocumentId,
    pub(crate) position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextDocumentRangeInput {
    pub(crate) document_id: DocumentId,
    pub(crate) range: DiagnosticRange,
}

pub(crate) fn document_id(uri: &lsp_types::Url) -> DocumentId {
    DocumentId::from(uri.to_string())
}

pub(crate) fn position(text: &str, position: lsp_types::Position) -> Result<Position, String> {
    LineIndex::new(text).service_position(local_position(position))
}

pub(crate) fn range(text: &str, range: lsp_types::Range) -> Result<DiagnosticRange, String> {
    LineIndex::new(text).service_range(local_range(range))
}

pub(crate) fn formatting_options(options: &lsp_types::FormattingOptions) -> FormattingOptions {
    FormattingOptions {
        tab_size: options.tab_size,
        insert_spaces: options.insert_spaces,
    }
}

pub(crate) fn text_document_position(
    text: &str,
    params: &lsp_types::TextDocumentPositionParams,
) -> Result<TextDocumentPositionInput, String> {
    Ok(TextDocumentPositionInput {
        document_id: document_id(&params.text_document.uri),
        position: position(text, params.position)?,
    })
}

pub(crate) fn completion_params(
    text: &str,
    params: &lsp_types::CompletionParams,
) -> Result<TextDocumentPositionInput, String> {
    text_document_position(text, &params.text_document_position)
}

pub(crate) fn hover_params(
    text: &str,
    params: &lsp_types::HoverParams,
) -> Result<TextDocumentPositionInput, String> {
    text_document_position(text, &params.text_document_position_params)
}

pub(crate) fn signature_help_params(
    text: &str,
    params: &lsp_types::SignatureHelpParams,
) -> Result<TextDocumentPositionInput, String> {
    text_document_position(text, &params.text_document_position_params)
}

pub(crate) fn goto_definition_params(
    text: &str,
    params: &lsp_types::GotoDefinitionParams,
) -> Result<TextDocumentPositionInput, String> {
    text_document_position(text, &params.text_document_position_params)
}

pub(crate) fn reference_params(
    text: &str,
    params: &lsp_types::ReferenceParams,
) -> Result<TextDocumentPositionInput, String> {
    text_document_position(text, &params.text_document_position)
}

pub(crate) fn document_highlight_params(
    text: &str,
    params: &lsp_types::DocumentHighlightParams,
) -> Result<TextDocumentPositionInput, String> {
    text_document_position(text, &params.text_document_position_params)
}

pub(crate) fn document_symbol_params(params: &lsp_types::DocumentSymbolParams) -> DocumentId {
    document_id(&params.text_document.uri)
}

pub(crate) fn prepare_rename_params(
    text: &str,
    params: &lsp_types::TextDocumentPositionParams,
) -> Result<TextDocumentPositionInput, String> {
    text_document_position(text, params)
}

pub(crate) fn rename_params(
    text: &str,
    params: &lsp_types::RenameParams,
) -> Result<TextDocumentPositionInput, String> {
    text_document_position(text, &params.text_document_position)
}

pub(crate) fn prepare_call_hierarchy_params(
    text: &str,
    params: &lsp_types::CallHierarchyPrepareParams,
) -> Result<TextDocumentPositionInput, String> {
    text_document_position(text, &params.text_document_position_params)
}

pub(crate) fn call_hierarchy_item(
    text: &str,
    item: &lsp_types::CallHierarchyItem,
) -> Result<ServiceCallHierarchyItem, String> {
    Ok(ServiceCallHierarchyItem::new(
        item.name.clone(),
        document_id(&item.uri),
        range(text, item.range)?,
        range(text, item.selection_range)?,
    ))
}

pub(crate) fn text_document_range(
    text: &str,
    text_document: &lsp_types::TextDocumentIdentifier,
    range: lsp_types::Range,
) -> Result<TextDocumentRangeInput, String> {
    Ok(TextDocumentRangeInput {
        document_id: document_id(&text_document.uri),
        range: self::range(text, range)?,
    })
}

fn local_range(range: lsp_types::Range) -> LspRange {
    LspRange {
        start: local_position(range.start),
        end: local_position(range.end),
    }
}

fn local_position(position: lsp_types::Position) -> LspPosition {
    LspPosition {
        line: position.line,
        character: position.character,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_id_uses_lsp_uri_text() {
        let uri = lsp_types::Url::parse("file:///workspace/scripts/main.vela").expect("valid URI");

        assert_eq!(
            document_id(&uri),
            DocumentId::from("file:///workspace/scripts/main.vela")
        );
    }

    #[test]
    fn position_uses_utf16_line_index_conversion() {
        let text = "let icon = \"💎\"\nnext";

        assert_eq!(
            position(text, lsp_types::Position::new(0, 14))
                .expect("position after wide character should convert"),
            Position::new(0, 16)
        );
        assert!(
            position(text, lsp_types::Position::new(0, 13)).is_err(),
            "halfway through a UTF-16 surrogate pair should be rejected"
        );
    }

    #[test]
    fn range_uses_utf16_line_index_conversion() {
        let text = "let icon = \"💎\"\nnext";

        let range = range(
            text,
            lsp_types::Range::new(
                lsp_types::Position::new(0, 12),
                lsp_types::Position::new(0, 14),
            ),
        )
        .expect("range around wide character should convert");

        assert_eq!(range.start(), Position::new(0, 12));
        assert_eq!(range.end(), Position::new(0, 16));
    }

    #[test]
    fn text_document_position_converts_uri_and_position() {
        let params = lsp_types::TextDocumentPositionParams {
            text_document: lsp_types::TextDocumentIdentifier {
                uri: lsp_types::Url::parse("file:///workspace/scripts/main.vela")
                    .expect("valid URI"),
            },
            position: lsp_types::Position::new(1, 0),
        };

        let input =
            text_document_position("first\nsecond", &params).expect("position should convert");

        assert_eq!(
            input.document_id,
            DocumentId::from("file:///workspace/scripts/main.vela")
        );
        assert_eq!(input.position, Position::new(1, 0));
    }

    #[test]
    fn completion_params_convert_nested_position_input() {
        let params = lsp_types::CompletionParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse("file:///workspace/scripts/main.vela")
                        .expect("valid URI"),
                },
                position: lsp_types::Position::new(0, 4),
            },
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
            context: None,
        };

        let input = completion_params("main", &params).expect("position should convert");

        assert_eq!(
            input.document_id,
            DocumentId::from("file:///workspace/scripts/main.vela")
        );
        assert_eq!(input.position, Position::new(0, 4));
    }

    #[test]
    fn hover_params_convert_nested_position_input() {
        let params = lsp_types::HoverParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse("file:///workspace/scripts/main.vela")
                        .expect("valid URI"),
                },
                position: lsp_types::Position::new(0, 4),
            },
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
        };

        let input = hover_params("main", &params).expect("position should convert");

        assert_eq!(
            input.document_id,
            DocumentId::from("file:///workspace/scripts/main.vela")
        );
        assert_eq!(input.position, Position::new(0, 4));
    }

    #[test]
    fn signature_help_params_convert_nested_position_input() {
        let params = lsp_types::SignatureHelpParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse("file:///workspace/scripts/main.vela")
                        .expect("valid URI"),
                },
                position: lsp_types::Position::new(0, 4),
            },
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            context: None,
        };

        let input = signature_help_params("main", &params).expect("position should convert");

        assert_eq!(
            input.document_id,
            DocumentId::from("file:///workspace/scripts/main.vela")
        );
        assert_eq!(input.position, Position::new(0, 4));
    }

    #[test]
    fn goto_definition_params_convert_nested_position_input() {
        let params = lsp_types::GotoDefinitionParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse("file:///workspace/scripts/main.vela")
                        .expect("valid URI"),
                },
                position: lsp_types::Position::new(0, 4),
            },
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        };

        let input = goto_definition_params("main", &params).expect("position should convert");

        assert_eq!(
            input.document_id,
            DocumentId::from("file:///workspace/scripts/main.vela")
        );
        assert_eq!(input.position, Position::new(0, 4));
    }

    #[test]
    fn reference_params_convert_nested_position_input() {
        let params = lsp_types::ReferenceParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse("file:///workspace/scripts/main.vela")
                        .expect("valid URI"),
                },
                position: lsp_types::Position::new(0, 4),
            },
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
            context: lsp_types::ReferenceContext {
                include_declaration: true,
            },
        };

        let input = reference_params("main", &params).expect("position should convert");

        assert_eq!(
            input.document_id,
            DocumentId::from("file:///workspace/scripts/main.vela")
        );
        assert_eq!(input.position, Position::new(0, 4));
    }

    #[test]
    fn document_highlight_params_convert_nested_position_input() {
        let params = lsp_types::DocumentHighlightParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse("file:///workspace/scripts/main.vela")
                        .expect("valid URI"),
                },
                position: lsp_types::Position::new(0, 4),
            },
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        };

        let input = document_highlight_params("main", &params).expect("position should convert");

        assert_eq!(
            input.document_id,
            DocumentId::from("file:///workspace/scripts/main.vela")
        );
        assert_eq!(input.position, Position::new(0, 4));
    }

    #[test]
    fn document_symbol_params_convert_document_id() {
        let params = lsp_types::DocumentSymbolParams {
            text_document: lsp_types::TextDocumentIdentifier {
                uri: lsp_types::Url::parse("file:///workspace/scripts/main.vela")
                    .expect("valid URI"),
            },
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        };

        assert_eq!(
            document_symbol_params(&params),
            DocumentId::from("file:///workspace/scripts/main.vela")
        );
    }

    #[test]
    fn prepare_rename_params_convert_position_input() {
        let params = lsp_types::TextDocumentPositionParams {
            text_document: lsp_types::TextDocumentIdentifier {
                uri: lsp_types::Url::parse("file:///workspace/scripts/main.vela")
                    .expect("valid URI"),
            },
            position: lsp_types::Position::new(0, 4),
        };

        let input = prepare_rename_params("main", &params).expect("position should convert");

        assert_eq!(
            input.document_id,
            DocumentId::from("file:///workspace/scripts/main.vela")
        );
        assert_eq!(input.position, Position::new(0, 4));
    }

    #[test]
    fn rename_params_convert_nested_position_input() {
        let params = lsp_types::RenameParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse("file:///workspace/scripts/main.vela")
                        .expect("valid URI"),
                },
                position: lsp_types::Position::new(0, 4),
            },
            new_name: "renamed".to_owned(),
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
        };

        let input = rename_params("main", &params).expect("position should convert");

        assert_eq!(
            input.document_id,
            DocumentId::from("file:///workspace/scripts/main.vela")
        );
        assert_eq!(input.position, Position::new(0, 4));
    }

    #[test]
    fn prepare_call_hierarchy_params_convert_nested_position_input() {
        let params = lsp_types::CallHierarchyPrepareParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse("file:///workspace/scripts/main.vela")
                        .expect("valid URI"),
                },
                position: lsp_types::Position::new(0, 4),
            },
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
        };

        let input =
            prepare_call_hierarchy_params("main", &params).expect("position should convert");

        assert_eq!(
            input.document_id,
            DocumentId::from("file:///workspace/scripts/main.vela")
        );
        assert_eq!(input.position, Position::new(0, 4));
    }

    #[test]
    fn call_hierarchy_item_converts_ranges_and_document_id() {
        let item = lsp_types::CallHierarchyItem {
            name: "grant".to_owned(),
            kind: lsp_types::SymbolKind::FUNCTION,
            tags: None,
            detail: None,
            uri: lsp_types::Url::parse("file:///workspace/scripts/main.vela").expect("valid URI"),
            range: lsp_types::Range::new(
                lsp_types::Position::new(0, 0),
                lsp_types::Position::new(0, 12),
            ),
            selection_range: lsp_types::Range::new(
                lsp_types::Position::new(0, 7),
                lsp_types::Position::new(0, 12),
            ),
            data: None,
        };

        let item = call_hierarchy_item("pub fn grant()", &item).expect("item should convert");

        assert_eq!(item.name(), "grant");
        assert_eq!(
            item.document_id(),
            &DocumentId::from("file:///workspace/scripts/main.vela")
        );
        assert_eq!(item.selection_range().start(), Position::new(0, 7));
        assert_eq!(item.selection_range().end(), Position::new(0, 12));
    }

    #[test]
    fn formatting_options_copy_lsp_settings() {
        let options = lsp_types::FormattingOptions {
            tab_size: 2,
            insert_spaces: true,
            properties: Default::default(),
            trim_trailing_whitespace: None,
            insert_final_newline: None,
            trim_final_newlines: None,
        };

        assert_eq!(
            formatting_options(&options),
            FormattingOptions {
                tab_size: 2,
                insert_spaces: true,
            }
        );
    }
}

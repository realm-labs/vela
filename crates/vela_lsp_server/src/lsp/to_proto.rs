use serde_json::{Value as JsonValue, json};
use vela_language_service::{
    CompletionInsertFormat, CompletionKind, CompletionLabelDetails, CompletionList,
    CompletionResolvePayload, CompletionSymbol, LineIndex, TextRange,
};

pub(crate) fn completion_response(
    completions: &CompletionList,
    line_index: &LineIndex,
) -> lsp_types::CompletionResponse {
    lsp_types::CompletionResponse::List(lsp_types::CompletionList {
        is_incomplete: false,
        items: completions
            .items()
            .iter()
            .enumerate()
            .map(|(index, item)| completion_item(item, line_index, index == 0))
            .collect(),
    })
}

fn completion_item(
    item: &vela_language_service::CompletionItem,
    line_index: &LineIndex,
    preselect: bool,
) -> lsp_types::CompletionItem {
    let mut data = json!({
        "source": "vela"
    });
    if let Some(payload) = item.resolve_payload() {
        data["resolve"] = resolve_payload(payload);
    }

    let text_edit = if let Some(text_edit) = item.text_edit() {
        Some(lsp_types::CompletionTextEdit::Edit(lsp_types::TextEdit {
            range: range(text_edit.range(), line_index),
            new_text: text_edit.new_text().to_owned(),
        }))
    } else if let (Some(edit_range), Some(insert_text)) = (item.edit_range(), item.insert_text()) {
        Some(lsp_types::CompletionTextEdit::Edit(lsp_types::TextEdit {
            range: range(edit_range, line_index),
            new_text: insert_text.to_owned(),
        }))
    } else {
        None
    };

    lsp_types::CompletionItem {
        label: item.label().to_owned(),
        label_details: label_details(item.label_details()),
        kind: Some(completion_kind(item.kind())),
        detail: Some(item.detail().to_owned()),
        documentation: item.documentation().map(|documentation| {
            lsp_types::Documentation::MarkupContent(lsp_types::MarkupContent {
                kind: lsp_types::MarkupKind::Markdown,
                value: documentation.to_owned(),
            })
        }),
        deprecated: None,
        preselect: Some(preselect),
        sort_text: Some(sort_text(item, preselect)),
        filter_text: Some(item.filter_text().to_owned()),
        insert_text: item.insert_text().map(str::to_owned),
        insert_text_format: if item.insert_text().is_some()
            && matches!(item.insert_format(), CompletionInsertFormat::Snippet)
        {
            Some(lsp_types::InsertTextFormat::SNIPPET)
        } else {
            None
        },
        text_edit,
        data: Some(data),
        tags: item
            .deprecated()
            .then_some(vec![lsp_types::CompletionItemTag::DEPRECATED]),
        ..lsp_types::CompletionItem::default()
    }
}

fn resolve_payload(payload: &CompletionResolvePayload) -> JsonValue {
    match payload {
        CompletionResolvePayload::Documentation { symbol } => json!({
            "kind": "documentation",
            "symbol": completion_symbol(symbol)
        }),
    }
}

fn completion_symbol(symbol: &CompletionSymbol) -> JsonValue {
    match symbol {
        CompletionSymbol::Source(name) => json!({ "kind": "source", "name": name }),
        CompletionSymbol::Schema(name) => json!({ "kind": "schema", "name": name }),
        CompletionSymbol::Builtin(name) => json!({ "kind": "builtin", "name": name }),
        CompletionSymbol::Local(local) => {
            let mut value = json!({ "kind": "local", "name": local.name() });
            if let Some(document_id) = local.document_id() {
                value["documentId"] = json!(document_id.as_str());
            }
            if let Some(range) = local.range() {
                value["range"] = json!({ "start": range.start, "end": range.end });
            }
            value
        }
    }
}

fn sort_text(item: &vela_language_service::CompletionItem, preselect: bool) -> String {
    if let Some(sort_text) = item.sort_text() {
        return sort_text.to_owned();
    }
    let relevance = item.relevance();
    let preselect_rank = u8::from(!preselect);
    format!(
        "{:04}_{:02}_{:01}_{}",
        relevance.kind_rank(),
        relevance.match_rank(),
        preselect_rank,
        item.filter_text()
    )
}

fn label_details(
    details: &CompletionLabelDetails,
) -> Option<lsp_types::CompletionItemLabelDetails> {
    let detail = details.detail().map(str::to_owned);
    let description = details.description().map(str::to_owned);
    (detail.is_some() || description.is_some()).then_some(lsp_types::CompletionItemLabelDetails {
        detail,
        description,
    })
}

fn range(range: TextRange, line_index: &LineIndex) -> lsp_types::Range {
    let start = line_index.position(range.start);
    let end = line_index.position(range.end);
    lsp_types::Range {
        start: lsp_types::Position {
            line: u32::try_from(start.line).expect("line should fit in LSP u32"),
            character: u32::try_from(start.character).expect("character should fit in LSP u32"),
        },
        end: lsp_types::Position {
            line: u32::try_from(end.line).expect("line should fit in LSP u32"),
            character: u32::try_from(end.character).expect("character should fit in LSP u32"),
        },
    }
}

fn completion_kind(kind: CompletionKind) -> lsp_types::CompletionItemKind {
    match kind {
        CompletionKind::Keyword => lsp_types::CompletionItemKind::KEYWORD,
        CompletionKind::Snippet => lsp_types::CompletionItemKind::SNIPPET,
        CompletionKind::Binding => lsp_types::CompletionItemKind::VARIABLE,
        CompletionKind::Value => lsp_types::CompletionItemKind::VALUE,
        CompletionKind::Const => lsp_types::CompletionItemKind::CONSTANT,
        CompletionKind::Field => lsp_types::CompletionItemKind::FIELD,
        CompletionKind::Method => lsp_types::CompletionItemKind::METHOD,
        CompletionKind::Module => lsp_types::CompletionItemKind::MODULE,
        CompletionKind::Variant => lsp_types::CompletionItemKind::ENUM_MEMBER,
        CompletionKind::Function => lsp_types::CompletionItemKind::FUNCTION,
        CompletionKind::Type => lsp_types::CompletionItemKind::STRUCT,
        CompletionKind::Trait => lsp_types::CompletionItemKind::INTERFACE,
        CompletionKind::Parameter => lsp_types::CompletionItemKind::VARIABLE,
    }
}

#[cfg(test)]
mod tests {
    use vela_language_service::{
        DocumentId, LanguageServiceDatabases, LineIndex, Position, SourceFileSnapshot, Workspace,
        WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
    };

    use super::*;

    #[test]
    fn completion_response_projects_typed_lsp_items() {
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        let source = "pub fn overlay_only() { return 2 }";
        let files = vec![SourceFileSnapshot::new(document.clone(), source)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);

        let completions = databases.completion_items(&document, Position::new(0, 7));
        let response = completion_response(&completions, &LineIndex::new(source));

        let lsp_types::CompletionResponse::List(list) = response else {
            panic!("completion response should be a list");
        };
        assert!(!list.is_incomplete);
        assert_eq!(
            list.items
                .iter()
                .filter(|item| item.preselect == Some(true))
                .count(),
            1
        );
        let item = list
            .items
            .iter()
            .find(|item| item.label == "overlay_only")
            .expect("function completion should be projected");
        assert_eq!(item.kind, Some(lsp_types::CompletionItemKind::FUNCTION));
        assert!(item.data.is_some());
    }
}

use std::collections::HashMap;

use serde_json::{Value as JsonValue, json};
use vela_language_service::{
    CallHierarchyItem as ServiceCallHierarchyItem, CompletionInsertFormat, CompletionKind,
    CompletionLabelDetails, CompletionList, CompletionResolvePayload, CompletionSymbol, Definition,
    DiagnosticRange, DocumentHighlight, DocumentHighlightKind, DocumentTextEdit, Hover, HoverKind,
    IncomingCall, LineIndex, OutgoingCall, PrepareRename, Reference, RenameRiskKind, SignatureHelp,
    TextEdit as ServiceTextEdit, TextRange, WorkspaceEdit,
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

pub(crate) fn completion_item_resolved(
    mut item: lsp_types::CompletionItem,
    documentation: Option<String>,
) -> lsp_types::CompletionItem {
    if let Some(documentation) = documentation {
        item.documentation = Some(lsp_types::Documentation::MarkupContent(
            lsp_types::MarkupContent {
                kind: lsp_types::MarkupKind::Markdown,
                value: documentation,
            },
        ));
    }
    item
}

pub(crate) fn hover(hover: &Hover) -> lsp_types::Hover {
    lsp_types::Hover {
        contents: lsp_types::HoverContents::Markup(lsp_types::MarkupContent {
            kind: lsp_types::MarkupKind::Markdown,
            value: hover_markdown(hover),
        }),
        range: Some(diagnostic_range(hover.range())),
    }
}

pub(crate) fn signature_help(help: &SignatureHelp) -> lsp_types::SignatureHelp {
    lsp_types::SignatureHelp {
        signatures: help
            .signatures()
            .iter()
            .map(signature_information)
            .collect(),
        active_signature: Some(
            u32::try_from(help.active_signature()).expect("active signature should fit in u32"),
        ),
        active_parameter: Some(
            u32::try_from(help.active_parameter()).expect("active parameter should fit in u32"),
        ),
    }
}

pub(crate) fn definition_location(definition: &Definition) -> lsp_types::Location {
    location(definition.document_id(), definition.range())
}

pub(crate) fn reference_locations(references: &[Reference]) -> Vec<lsp_types::Location> {
    references
        .iter()
        .map(|reference| location(reference.document_id(), reference.range()))
        .collect()
}

pub(crate) fn document_highlights(
    highlights: &[DocumentHighlight],
) -> Vec<lsp_types::DocumentHighlight> {
    highlights
        .iter()
        .map(|highlight| lsp_types::DocumentHighlight {
            range: diagnostic_range(highlight.range()),
            kind: document_highlight_kind(highlight.kind()),
        })
        .collect()
}

pub(crate) fn prepare_rename(rename: &PrepareRename) -> lsp_types::PrepareRenameResponse {
    lsp_types::PrepareRenameResponse::RangeWithPlaceholder {
        range: diagnostic_range(rename.range()),
        placeholder: rename.placeholder().to_owned(),
    }
}

pub(crate) fn workspace_edit(edit: &WorkspaceEdit) -> lsp_types::WorkspaceEdit {
    lsp_types::WorkspaceEdit {
        changes: Some(workspace_edit_changes(edit)),
        document_changes: Some(lsp_types::DocumentChanges::Edits(
            edit.document_edits()
                .iter()
                .map(text_document_edit)
                .collect(),
        )),
        change_annotations: (!edit.risks().is_empty()).then(|| change_annotations(edit)),
    }
}

pub(crate) fn call_hierarchy_items(
    items: &[ServiceCallHierarchyItem],
) -> Vec<lsp_types::CallHierarchyItem> {
    items.iter().map(call_hierarchy_item).collect()
}

pub(crate) fn incoming_calls(calls: &[IncomingCall]) -> Vec<lsp_types::CallHierarchyIncomingCall> {
    calls
        .iter()
        .map(|call| lsp_types::CallHierarchyIncomingCall {
            from: call_hierarchy_item(call.from()),
            from_ranges: call
                .from_ranges()
                .iter()
                .copied()
                .map(diagnostic_range)
                .collect(),
        })
        .collect()
}

pub(crate) fn outgoing_calls(calls: &[OutgoingCall]) -> Vec<lsp_types::CallHierarchyOutgoingCall> {
    calls
        .iter()
        .map(|call| lsp_types::CallHierarchyOutgoingCall {
            to: call_hierarchy_item(call.to()),
            from_ranges: call
                .from_ranges()
                .iter()
                .copied()
                .map(diagnostic_range)
                .collect(),
        })
        .collect()
}

fn location(
    document_id: &vela_language_service::DocumentId,
    range: DiagnosticRange,
) -> lsp_types::Location {
    lsp_types::Location {
        uri: lsp_types::Url::parse(document_id.as_str())
            .expect("location document id should be a valid LSP URI"),
        range: diagnostic_range(range),
    }
}

fn workspace_edit_changes(
    edit: &WorkspaceEdit,
) -> HashMap<lsp_types::Url, Vec<lsp_types::TextEdit>> {
    edit.document_edits()
        .iter()
        .map(|document_edit| {
            (
                lsp_types::Url::parse(document_edit.document_id().as_str())
                    .expect("workspace edit document id should be a valid LSP URI"),
                document_edit.edits().iter().map(text_edit).collect(),
            )
        })
        .collect()
}

fn call_hierarchy_item(item: &ServiceCallHierarchyItem) -> lsp_types::CallHierarchyItem {
    let uri = lsp_types::Url::parse(item.document_id().as_str())
        .expect("call hierarchy document id should be a valid LSP URI");
    let selection_range = diagnostic_range(item.selection_range());
    lsp_types::CallHierarchyItem {
        name: item.name().to_owned(),
        kind: lsp_types::SymbolKind::FUNCTION,
        tags: None,
        detail: None,
        uri: uri.clone(),
        range: diagnostic_range(item.range()),
        selection_range,
        data: Some(json!({
            "name": item.name(),
            "uri": uri.as_str(),
            "selectionRange": selection_range,
        })),
    }
}

const fn document_highlight_kind(
    kind: DocumentHighlightKind,
) -> Option<lsp_types::DocumentHighlightKind> {
    match kind {
        DocumentHighlightKind::Text | DocumentHighlightKind::Call => {
            Some(lsp_types::DocumentHighlightKind::TEXT)
        }
        DocumentHighlightKind::Read => Some(lsp_types::DocumentHighlightKind::READ),
        DocumentHighlightKind::Write => Some(lsp_types::DocumentHighlightKind::WRITE),
    }
}

fn text_document_edit(document_edit: &DocumentTextEdit) -> lsp_types::TextDocumentEdit {
    lsp_types::TextDocumentEdit {
        text_document: lsp_types::OptionalVersionedTextDocumentIdentifier {
            uri: lsp_types::Url::parse(document_edit.document_id().as_str())
                .expect("workspace edit document id should be a valid LSP URI"),
            version: document_edit.document_version().map(|version| {
                i32::try_from(version.get()).expect("document version should fit in i32")
            }),
        },
        edits: document_edit
            .edits()
            .iter()
            .map(|edit| lsp_types::OneOf::Left(text_edit(edit)))
            .collect(),
    }
}

fn text_edit(edit: &ServiceTextEdit) -> lsp_types::TextEdit {
    lsp_types::TextEdit {
        range: diagnostic_range(edit.range()),
        new_text: edit.new_text().to_owned(),
    }
}

fn change_annotations(
    edit: &WorkspaceEdit,
) -> HashMap<lsp_types::ChangeAnnotationIdentifier, lsp_types::ChangeAnnotation> {
    edit.risks()
        .iter()
        .enumerate()
        .map(|(index, risk)| {
            let description = match risk.kind() {
                RenameRiskKind::HotReloadAbi => "hotReloadAbi",
                RenameRiskKind::SchemaAbi => "schemaAbi",
            };
            (
                format!("renameRisk{index}"),
                lsp_types::ChangeAnnotation {
                    label: risk.message().to_owned(),
                    needs_confirmation: Some(true),
                    description: Some(description.to_owned()),
                },
            )
        })
        .collect()
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

fn signature_information(
    signature: &vela_language_service::SignatureInformation,
) -> lsp_types::SignatureInformation {
    lsp_types::SignatureInformation {
        label: signature.label().to_owned(),
        documentation: None,
        parameters: Some(
            signature
                .parameters()
                .iter()
                .map(signature_parameter)
                .collect(),
        ),
        active_parameter: None,
    }
}

fn signature_parameter(
    parameter: &vela_language_service::SignatureParameter,
) -> lsp_types::ParameterInformation {
    lsp_types::ParameterInformation {
        label: lsp_types::ParameterLabel::Simple(parameter.label().to_owned()),
        documentation: None,
    }
}

fn hover_markdown(hover: &Hover) -> String {
    let mut sections = vec![format!(
        "```vela\n{}\n```\n\n_{}_: {}",
        hover.label(),
        hover_kind(hover.kind()),
        hover.detail()
    )];
    if let Some(docs) = hover.docs() {
        sections.push(docs.to_owned());
    }
    sections.join("\n\n")
}

fn hover_kind(kind: HoverKind) -> &'static str {
    match kind {
        HoverKind::Local => "local",
        HoverKind::Parameter => "parameter",
        HoverKind::Global => "global",
        HoverKind::Const => "const",
        HoverKind::Function => "function",
        HoverKind::Type => "type",
        HoverKind::Trait => "trait",
        HoverKind::Field => "field",
        HoverKind::Method => "method",
        HoverKind::Variant => "variant",
        HoverKind::Module => "module",
        HoverKind::Unknown => "unknown",
    }
}

fn diagnostic_range(range: DiagnosticRange) -> lsp_types::Range {
    let start = range.start();
    let end = range.end();
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

    #[test]
    fn completion_item_resolved_projects_markdown_documentation() {
        let item = lsp_types::CompletionItem {
            label: "Player".to_owned(),
            ..lsp_types::CompletionItem::default()
        };

        let item = completion_item_resolved(item, Some("Player docs.".to_owned()));

        let Some(lsp_types::Documentation::MarkupContent(documentation)) = item.documentation
        else {
            panic!("resolved completion should contain markdown documentation");
        };
        assert_eq!(documentation.kind, lsp_types::MarkupKind::Markdown);
        assert_eq!(documentation.value, "Player docs.");
    }

    #[test]
    fn hover_projects_markdown_and_range() {
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        let source = "pub fn main(amount: i64) -> i64 { return amount }";
        let files = vec![SourceFileSnapshot::new(document.clone(), source)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);
        let position = Position::new(
            0,
            source
                .rfind("amount")
                .expect("hover fixture should contain amount use"),
        );
        let hover = databases
            .hover(&document, position)
            .expect("parameter use should have hover");

        let hover = super::hover(&hover);

        let lsp_types::HoverContents::Markup(contents) = hover.contents else {
            panic!("hover should project markdown contents");
        };
        assert_eq!(contents.kind, lsp_types::MarkupKind::Markdown);
        assert!(contents.value.contains("amount"));
        assert!(contents.value.contains("_parameter_: i64"));
        assert_eq!(
            hover.range,
            Some(lsp_types::Range::new(
                lsp_types::Position::new(0, 41),
                lsp_types::Position::new(0, 47)
            ))
        );
    }

    #[test]
    fn signature_help_projects_typed_lsp_shape() {
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        let source = "pub fn grant(amount: i64, bonus: i64) -> bool { return true } pub fn main() { grant(1, 2) }";
        let files = vec![SourceFileSnapshot::new(document.clone(), source)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);
        let position = Position::new(
            0,
            source
                .find("2)")
                .expect("signature fixture should contain second argument"),
        );
        let help = databases
            .signature_help(&document, position)
            .expect("call should have signature help");

        let help = signature_help(&help);

        assert_eq!(help.active_signature, Some(0));
        assert_eq!(help.active_parameter, Some(1));
        assert_eq!(
            help.signatures[0].label,
            "grant(amount: i64, bonus: i64) -> bool"
        );
        let parameters = help.signatures[0]
            .parameters
            .as_ref()
            .expect("parameters should be projected");
        assert_eq!(
            parameters[1].label,
            lsp_types::ParameterLabel::Simple("bonus: i64".to_owned())
        );
    }

    #[test]
    fn definition_location_projects_typed_location() {
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        let source = "pub fn grant() -> i64 { return 1 }\npub fn main() { return grant() }";
        let files = vec![SourceFileSnapshot::new(document.clone(), source)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);
        let position = Position::new(
            1,
            source
                .lines()
                .nth(1)
                .expect("main line should exist")
                .find("grant")
                .expect("call should contain grant"),
        );
        let definition = databases
            .definition(&document, position)
            .expect("call should have definition");

        let location = definition_location(&definition);

        assert_eq!(location.uri.as_str(), document.as_str());
        assert_eq!(
            location.range,
            lsp_types::Range::new(
                lsp_types::Position::new(0, 7),
                lsp_types::Position::new(0, 12)
            )
        );
    }

    #[test]
    fn reference_locations_project_typed_locations() {
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        let source = "\
pub fn main(amount: i64) -> i64 {
    let next = amount + 1
    return next + amount
}";
        let files = vec![SourceFileSnapshot::new(document.clone(), source)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);
        let position = Position::new(
            2,
            source
                .lines()
                .nth(2)
                .expect("return line should exist")
                .find("amount")
                .expect("line should contain amount"),
        );
        let references = databases.references(&document, position, true);

        let locations = reference_locations(&references);

        assert_eq!(locations.len(), 3);
        assert!(
            locations
                .iter()
                .all(|location| location.uri.as_str() == document.as_str())
        );
        assert_eq!(
            locations[0].range,
            lsp_types::Range::new(
                lsp_types::Position::new(0, 12),
                lsp_types::Position::new(0, 18)
            )
        );
        assert_eq!(
            locations[2].range,
            lsp_types::Range::new(
                lsp_types::Position::new(2, 18),
                lsp_types::Position::new(2, 24)
            )
        );
    }

    #[test]
    fn document_highlights_project_typed_highlights() {
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        let source = "\
pub fn main(amount: i64) -> i64 {
    let next = amount + 1
    return next + amount
}";
        let files = vec![SourceFileSnapshot::new(document.clone(), source)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);
        let position = Position::new(
            2,
            source
                .lines()
                .nth(2)
                .expect("return line should exist")
                .find("amount")
                .expect("line should contain amount"),
        );
        let highlights = databases.document_highlights(&document, position);

        let highlights = document_highlights(&highlights);

        assert_eq!(highlights.len(), 3);
        assert_eq!(
            highlights[0].kind,
            Some(lsp_types::DocumentHighlightKind::TEXT)
        );
        assert_eq!(
            highlights[1].kind,
            Some(lsp_types::DocumentHighlightKind::READ)
        );
        assert_eq!(
            highlights[2].range,
            lsp_types::Range::new(
                lsp_types::Position::new(2, 18),
                lsp_types::Position::new(2, 24)
            )
        );
    }

    #[test]
    fn prepare_rename_projects_typed_response() {
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        let source = "\
pub fn main(amount: i64) -> i64 {
    return amount
}";
        let files = vec![SourceFileSnapshot::new(document.clone(), source)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);
        let position = Position::new(
            1,
            source
                .lines()
                .nth(1)
                .expect("return line should exist")
                .find("amount")
                .expect("line should contain amount"),
        );
        let prepare = databases
            .prepare_rename(&document, position)
            .expect("local binding should prepare rename");

        let response = prepare_rename(&prepare);

        assert_eq!(
            response,
            lsp_types::PrepareRenameResponse::RangeWithPlaceholder {
                range: lsp_types::Range::new(
                    lsp_types::Position::new(1, 11),
                    lsp_types::Position::new(1, 17)
                ),
                placeholder: "amount".to_owned(),
            }
        );
    }

    #[test]
    fn workspace_edit_projects_typed_rename_edits() {
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        let source = "\
pub fn main(amount: i64) -> i64 {
    return amount
}";
        let files = vec![SourceFileSnapshot::new(document.clone(), source)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);
        let position = Position::new(
            1,
            source
                .lines()
                .nth(1)
                .expect("return line should exist")
                .find("amount")
                .expect("line should contain amount"),
        );
        let edit = databases
            .rename(&document, position, "total")
            .expect("local binding should rename");

        let edit = workspace_edit(&edit);
        let value = serde_json::to_value(&edit).expect("workspace edit should serialize");

        assert!(value["changes"][document.as_str()].is_array());
        assert_eq!(
            value["changes"][document.as_str()]
                .as_array()
                .expect("changes should contain document edit array")
                .len(),
            2
        );
        assert_eq!(
            value["documentChanges"][0]["textDocument"]["uri"],
            document.as_str()
        );
        assert_eq!(
            value["documentChanges"][0]["edits"]
                .as_array()
                .expect("documentChanges should contain edit array")
                .len(),
            2
        );
        assert!(value.get("changeAnnotations").is_none());
    }

    #[test]
    fn call_hierarchy_items_project_typed_items() {
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        let source = "pub fn grant() -> i64 { return 1 }\npub fn main() { return grant() }";
        let files = vec![SourceFileSnapshot::new(document.clone(), source)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);
        let position = Position::new(
            1,
            source
                .lines()
                .nth(1)
                .expect("main line should exist")
                .find("grant")
                .expect("call should contain grant"),
        );
        let items = databases.prepare_call_hierarchy(&document, position);

        let items = call_hierarchy_items(&items);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "grant");
        assert_eq!(items[0].kind, lsp_types::SymbolKind::FUNCTION);
        assert_eq!(items[0].uri.as_str(), document.as_str());
        assert_eq!(
            items[0].selection_range,
            lsp_types::Range::new(
                lsp_types::Position::new(0, 7),
                lsp_types::Position::new(0, 12)
            )
        );
        assert!(items[0].data.is_some());
    }

    #[test]
    fn incoming_and_outgoing_calls_project_typed_calls() {
        let document = DocumentId::from("file:///workspace/scripts/main.vela");
        let source = "pub fn grant() -> i64 { return 1 }\npub fn main() { return grant() }";
        let files = vec![SourceFileSnapshot::new(document.clone(), source)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);
        let grant_position = Position::new(
            1,
            source
                .lines()
                .nth(1)
                .expect("main line should exist")
                .find("grant")
                .expect("call should contain grant"),
        );
        let main_position = Position::new(
            1,
            source
                .lines()
                .nth(1)
                .expect("main line should exist")
                .find("main")
                .expect("line should contain main"),
        );
        let grant = databases
            .prepare_call_hierarchy(&document, grant_position)
            .into_iter()
            .next()
            .expect("grant should prepare call hierarchy");
        let main = databases
            .prepare_call_hierarchy(&document, main_position)
            .into_iter()
            .next()
            .expect("main should prepare call hierarchy");

        let incoming = incoming_calls(&databases.incoming_calls(&grant));
        let outgoing = outgoing_calls(&databases.outgoing_calls(&main));

        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].from.name, "main");
        assert_eq!(incoming[0].from_ranges.len(), 1);
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].to.name, "grant");
        assert_eq!(outgoing[0].from_ranges.len(), 1);
    }
}

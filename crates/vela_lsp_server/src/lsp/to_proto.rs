use std::collections::HashMap;

use serde_json::{Value as JsonValue, json};
use vela_language_service::{
    CallHierarchyItem as ServiceCallHierarchyItem, CodeAction as ServiceCodeAction,
    CodeActionKind as ServiceCodeActionKind, CompletionInsertFormat, CompletionKind,
    CompletionLabelDetails, CompletionList, CompletionResolvePayload, CompletionSymbol, Definition,
    DiagnosticRange, DocumentDiagnostics, DocumentHighlight, DocumentHighlightKind, DocumentSymbol,
    DocumentSymbolKind, DocumentTextEdit, FoldingRange as ServiceFoldingRange,
    FoldingRangeKind as ServiceFoldingRangeKind, Hover, HoverKind, IncomingCall,
    InlayHint as ServiceInlayHint, InlayHintKind as ServiceInlayHintKind, LineIndex, OutgoingCall,
    PrepareRename, ProjectDiagnostic, Reference, RenameRiskKind, SchemaDiagnostic,
    SelectionRange as ServiceSelectionRange, SemanticToken as ServiceSemanticToken,
    SemanticTokenDelta as ServiceSemanticTokenDelta, SemanticTokens as ServiceSemanticTokens,
    ServiceDiagnostic, ServiceDiagnosticSeverity, SignatureHelp, TextEdit as ServiceTextEdit,
    TextRange, WorkspaceEdit, WorkspaceSymbol, WorkspaceSymbolLocation,
};

use crate::semantic_tokens::SemanticTokenProjection;

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

pub(crate) fn diagnostics(diagnostics: &DocumentDiagnostics) -> Vec<lsp_types::Diagnostic> {
    diagnostics
        .diagnostics()
        .iter()
        .map(service_diagnostic)
        .collect()
}

pub(crate) fn project_diagnostics(
    diagnostics: &[ProjectDiagnostic],
    document_id: &vela_language_service::DocumentId,
) -> Vec<lsp_types::Diagnostic> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.document_id() == Some(document_id))
        .map(|diagnostic| lsp_types::Diagnostic {
            range: zero_range(),
            severity: Some(lsp_types::DiagnosticSeverity::ERROR),
            code: Some(lsp_types::NumberOrString::String(
                "project::diagnostic".to_owned(),
            )),
            code_description: None,
            source: Some("vela".to_owned()),
            message: diagnostic.message().to_owned(),
            related_information: None,
            tags: None,
            data: Some(empty_diagnostic_data()),
        })
        .collect()
}

pub(crate) fn schema_diagnostics(diagnostics: &[SchemaDiagnostic]) -> Vec<lsp_types::Diagnostic> {
    diagnostics
        .iter()
        .map(|diagnostic| lsp_types::Diagnostic {
            range: zero_range(),
            severity: Some(lsp_types::DiagnosticSeverity::ERROR),
            code: Some(lsp_types::NumberOrString::String(
                "schema::diagnostic".to_owned(),
            )),
            code_description: None,
            source: Some("vela".to_owned()),
            message: diagnostic.message().to_owned(),
            related_information: None,
            tags: None,
            data: Some(empty_diagnostic_data()),
        })
        .collect()
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

pub(crate) fn document_symbols(symbols: &[DocumentSymbol]) -> lsp_types::DocumentSymbolResponse {
    lsp_types::DocumentSymbolResponse::Nested(symbols.iter().map(document_symbol).collect())
}

pub(crate) fn workspace_symbols(symbols: &[WorkspaceSymbol]) -> lsp_types::WorkspaceSymbolResponse {
    lsp_types::WorkspaceSymbolResponse::Nested(symbols.iter().map(workspace_symbol).collect())
}

pub(crate) fn folding_ranges(ranges: &[ServiceFoldingRange]) -> Vec<lsp_types::FoldingRange> {
    ranges.iter().map(folding_range).collect()
}

pub(crate) fn selection_ranges(ranges: &[ServiceSelectionRange]) -> Vec<lsp_types::SelectionRange> {
    ranges.iter().map(selection_range).collect()
}

pub(crate) fn text_edits(edits: &[ServiceTextEdit]) -> Vec<lsp_types::TextEdit> {
    edits.iter().map(text_edit).collect()
}

pub(crate) fn code_actions(actions: &[ServiceCodeAction]) -> lsp_types::CodeActionResponse {
    actions
        .iter()
        .map(code_action)
        .map(lsp_types::CodeActionOrCommand::CodeAction)
        .collect()
}

pub(crate) fn inlay_hints(hints: &[ServiceInlayHint]) -> Vec<lsp_types::InlayHint> {
    hints.iter().map(inlay_hint).collect()
}

pub(crate) fn semantic_tokens(
    tokens: &ServiceSemanticTokens,
    projection: &SemanticTokenProjection,
) -> lsp_types::SemanticTokensResult {
    lsp_types::SemanticTokensResult::Tokens(lsp_types::SemanticTokens {
        result_id: Some(tokens.result_id().to_owned()),
        data: semantic_token_data(tokens.tokens(), projection),
    })
}

pub(crate) fn semantic_tokens_range(
    tokens: &ServiceSemanticTokens,
    projection: &SemanticTokenProjection,
) -> lsp_types::SemanticTokensRangeResult {
    lsp_types::SemanticTokensRangeResult::Tokens(lsp_types::SemanticTokens {
        result_id: Some(tokens.result_id().to_owned()),
        data: semantic_token_data(tokens.tokens(), projection),
    })
}

pub(crate) fn semantic_tokens_delta(
    delta: &ServiceSemanticTokenDelta,
    projection: &SemanticTokenProjection,
) -> lsp_types::SemanticTokensFullDeltaResult {
    lsp_types::SemanticTokensFullDeltaResult::TokensDelta(lsp_types::SemanticTokensDelta {
        result_id: Some(delta.result_id().to_owned()),
        edits: delta
            .edits()
            .iter()
            .map(|edit| lsp_types::SemanticTokensEdit {
                start: u32::try_from(edit.start() * 5)
                    .expect("semantic token edit start should fit u32"),
                delete_count: u32::try_from(edit.delete_count() * 5)
                    .expect("semantic token edit delete count should fit u32"),
                data: Some(semantic_token_data(edit.tokens(), projection)),
            })
            .collect(),
    })
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

fn service_diagnostic(diagnostic: &ServiceDiagnostic) -> lsp_types::Diagnostic {
    lsp_types::Diagnostic {
        range: diagnostic.range().map_or_else(zero_range, diagnostic_range),
        severity: Some(diagnostic_severity(diagnostic.severity())),
        code: diagnostic
            .code()
            .map(str::to_owned)
            .map(lsp_types::NumberOrString::String),
        code_description: None,
        source: Some("vela".to_owned()),
        message: diagnostic.message().to_owned(),
        related_information: None,
        tags: None,
        data: Some(diagnostic_data(diagnostic)),
    }
}

fn diagnostic_data(diagnostic: &ServiceDiagnostic) -> JsonValue {
    json!({
        "labels": diagnostic.labels().iter().map(|label| {
            json!({
                "uri": label.document_id().as_str(),
                "range": diagnostic_range(label.range()),
                "message": label.message()
            })
        }).collect::<Vec<_>>(),
        "candidates": diagnostic.candidates().iter().map(|candidate| {
            json!({ "replacement": candidate.replacement() })
        }).collect::<Vec<_>>(),
        "repairHints": diagnostic.repair_hints().iter().map(|hint| {
            json!({
                "uri": hint.document_id().as_str(),
                "range": diagnostic_range(hint.range()),
                "title": hint.title(),
                "replacement": hint.replacement()
            })
        }).collect::<Vec<_>>()
    })
}

fn empty_diagnostic_data() -> JsonValue {
    json!({
        "labels": [],
        "candidates": [],
        "repairHints": []
    })
}

const fn diagnostic_severity(severity: ServiceDiagnosticSeverity) -> lsp_types::DiagnosticSeverity {
    match severity {
        ServiceDiagnosticSeverity::Error => lsp_types::DiagnosticSeverity::ERROR,
        ServiceDiagnosticSeverity::Warning => lsp_types::DiagnosticSeverity::WARNING,
        ServiceDiagnosticSeverity::Note => lsp_types::DiagnosticSeverity::INFORMATION,
        ServiceDiagnosticSeverity::Help => lsp_types::DiagnosticSeverity::HINT,
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

fn document_symbol(symbol: &DocumentSymbol) -> lsp_types::DocumentSymbol {
    #[allow(deprecated)]
    lsp_types::DocumentSymbol {
        name: symbol.name().to_owned(),
        detail: symbol.detail().map(str::to_owned),
        kind: symbol_kind(symbol.kind()),
        tags: None,
        deprecated: None,
        range: diagnostic_range(symbol.range()),
        selection_range: diagnostic_range(symbol.selection_range()),
        children: (!symbol.children().is_empty())
            .then(|| symbol.children().iter().map(document_symbol).collect()),
    }
}

fn workspace_symbol(symbol: &WorkspaceSymbol) -> lsp_types::WorkspaceSymbol {
    lsp_types::WorkspaceSymbol {
        name: symbol.name().to_owned(),
        kind: symbol_kind(symbol.kind()),
        tags: None,
        container_name: symbol.container_name().map(str::to_owned),
        location: workspace_symbol_location(symbol.location()),
        data: workspace_symbol_data(symbol),
    }
}

fn workspace_symbol_location(
    location: &WorkspaceSymbolLocation,
) -> lsp_types::OneOf<lsp_types::Location, lsp_types::WorkspaceLocation> {
    match location {
        WorkspaceSymbolLocation::Source { document_id, range } => {
            lsp_types::OneOf::Left(self::location(document_id, *range))
        }
        WorkspaceSymbolLocation::Schema => lsp_types::OneOf::Right(lsp_types::WorkspaceLocation {
            uri: lsp_types::Url::parse("vela-schema:")
                .expect("schema workspace symbol URI should parse"),
        }),
    }
}

fn workspace_symbol_data(symbol: &WorkspaceSymbol) -> Option<JsonValue> {
    symbol.detail().map(|detail| json!({ "detail": detail }))
}

fn folding_range(range: &ServiceFoldingRange) -> lsp_types::FoldingRange {
    lsp_types::FoldingRange {
        start_line: u32::try_from(range.start().line).expect("line should fit in LSP u32"),
        start_character: Some(
            u32::try_from(range.start().character).expect("character should fit in LSP u32"),
        ),
        end_line: u32::try_from(range.end().line).expect("line should fit in LSP u32"),
        end_character: Some(
            u32::try_from(range.end().character).expect("character should fit in LSP u32"),
        ),
        kind: Some(folding_range_kind(range.kind())),
        collapsed_text: None,
    }
}

const fn folding_range_kind(kind: ServiceFoldingRangeKind) -> lsp_types::FoldingRangeKind {
    match kind {
        ServiceFoldingRangeKind::Imports => lsp_types::FoldingRangeKind::Imports,
        ServiceFoldingRangeKind::Region => lsp_types::FoldingRangeKind::Region,
    }
}

fn selection_range(range: &ServiceSelectionRange) -> lsp_types::SelectionRange {
    lsp_types::SelectionRange {
        range: diagnostic_range(range.range()),
        parent: range.parent().map(selection_range).map(Box::new),
    }
}

fn code_action(action: &ServiceCodeAction) -> lsp_types::CodeAction {
    lsp_types::CodeAction {
        title: action.title().to_owned(),
        kind: Some(code_action_kind(action.kind())),
        diagnostics: None,
        edit: Some(workspace_edit(action.edit())),
        command: None,
        is_preferred: None,
        disabled: None,
        data: None,
    }
}

const fn code_action_kind(kind: ServiceCodeActionKind) -> lsp_types::CodeActionKind {
    match kind {
        ServiceCodeActionKind::QuickFix => lsp_types::CodeActionKind::QUICKFIX,
    }
}

fn inlay_hint(hint: &ServiceInlayHint) -> lsp_types::InlayHint {
    lsp_types::InlayHint {
        position: service_position(hint.position()),
        label: lsp_types::InlayHintLabel::String(hint.label()),
        kind: Some(inlay_hint_kind(hint.kind())),
        text_edits: None,
        tooltip: None,
        padding_left: None,
        padding_right: Some(true),
        data: None,
    }
}

const fn inlay_hint_kind(kind: ServiceInlayHintKind) -> lsp_types::InlayHintKind {
    match kind {
        ServiceInlayHintKind::Type => lsp_types::InlayHintKind::TYPE,
        ServiceInlayHintKind::Parameter => lsp_types::InlayHintKind::PARAMETER,
    }
}

fn semantic_token_data(
    tokens: &[ServiceSemanticToken],
    projection: &SemanticTokenProjection,
) -> Vec<lsp_types::SemanticToken> {
    let mut data = Vec::with_capacity(tokens.len());
    let mut previous_line = 0usize;
    let mut previous_start = 0usize;

    for token in tokens {
        let start = token.start();
        let delta_line = start.line.saturating_sub(previous_line);
        let delta_start = if delta_line == 0 {
            start.character.saturating_sub(previous_start)
        } else {
            start.character
        };
        data.push(lsp_types::SemanticToken {
            delta_line: u32::try_from(delta_line)
                .expect("semantic token line delta should fit u32"),
            delta_start: u32::try_from(delta_start)
                .expect("semantic token start delta should fit u32"),
            length: u32::try_from(token.length()).expect("semantic token length should fit u32"),
            token_type: projection.token_type_index(token.token_type()),
            token_modifiers_bitset: projection.modifier_bits(token.modifiers()),
        });
        previous_line = start.line;
        previous_start = start.character;
    }

    data
}

const fn symbol_kind(kind: DocumentSymbolKind) -> lsp_types::SymbolKind {
    match kind {
        DocumentSymbolKind::File => lsp_types::SymbolKind::FILE,
        DocumentSymbolKind::Class => lsp_types::SymbolKind::CLASS,
        DocumentSymbolKind::Module => lsp_types::SymbolKind::MODULE,
        DocumentSymbolKind::Method => lsp_types::SymbolKind::METHOD,
        DocumentSymbolKind::Field => lsp_types::SymbolKind::FIELD,
        DocumentSymbolKind::Enum => lsp_types::SymbolKind::ENUM,
        DocumentSymbolKind::Interface => lsp_types::SymbolKind::INTERFACE,
        DocumentSymbolKind::Function => lsp_types::SymbolKind::FUNCTION,
        DocumentSymbolKind::Variable => lsp_types::SymbolKind::VARIABLE,
        DocumentSymbolKind::Constant => lsp_types::SymbolKind::CONSTANT,
        DocumentSymbolKind::Object => lsp_types::SymbolKind::OBJECT,
        DocumentSymbolKind::EnumMember => lsp_types::SymbolKind::ENUM_MEMBER,
        DocumentSymbolKind::Struct => lsp_types::SymbolKind::STRUCT,
        DocumentSymbolKind::TypeParameter => lsp_types::SymbolKind::TYPE_PARAMETER,
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
        HoverKind::VmState => "state",
        HoverKind::ExternState => "extern state",
        HoverKind::Const => "const",
        HoverKind::Function => "function",
        HoverKind::Type => "type",
        HoverKind::Trait => "trait",
        HoverKind::Field => "field",
        HoverKind::Method => "method",
        HoverKind::Variant => "variant",
        HoverKind::Module => "module",
        HoverKind::Provider => "provider",
        HoverKind::Unknown => "unknown",
    }
}

fn diagnostic_range(range: DiagnosticRange) -> lsp_types::Range {
    let start = range.start();
    let end = range.end();
    lsp_types::Range {
        start: service_position(start),
        end: service_position(end),
    }
}

fn zero_range() -> lsp_types::Range {
    lsp_types::Range {
        start: lsp_types::Position {
            line: 0,
            character: 0,
        },
        end: lsp_types::Position {
            line: 0,
            character: 0,
        },
    }
}

fn service_position(position: vela_language_service::Position) -> lsp_types::Position {
    lsp_types::Position {
        line: u32::try_from(position.line).expect("line should fit in LSP u32"),
        character: u32::try_from(position.character).expect("character should fit in LSP u32"),
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
mod tests;

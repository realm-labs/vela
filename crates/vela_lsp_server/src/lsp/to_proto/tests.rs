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

    let Some(lsp_types::Documentation::MarkupContent(documentation)) = item.documentation else {
        panic!("resolved completion should contain markdown documentation");
    };
    assert_eq!(documentation.kind, lsp_types::MarkupKind::Markdown);
    assert_eq!(documentation.value, "Player docs.");
}

#[test]
fn diagnostics_project_typed_lsp_shape_and_extension_data() {
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    let source = "pub fn main( {";
    let files = vec![SourceFileSnapshot::new(document.clone(), source)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);

    let diagnostics = diagnostics(&databases.diagnostics_for_document(&document));

    let diagnostic = diagnostics
        .first()
        .expect("invalid source should produce a diagnostic");
    assert_eq!(diagnostic.source.as_deref(), Some("vela"));
    assert_eq!(
        diagnostic.severity,
        Some(lsp_types::DiagnosticSeverity::ERROR)
    );
    let data = diagnostic
        .data
        .as_ref()
        .expect("diagnostic should preserve structured extension data");
    assert!(data.get("labels").is_some());
    assert!(data.get("candidates").is_some());
    assert!(data.get("repairHints").is_some());

    let project = project_diagnostics(
        &[ProjectDiagnostic::new(
            Some(document.clone()),
            "project issue",
        )],
        &document,
    );
    assert_eq!(project[0].message, "project issue");
    assert_eq!(
        project[0].code,
        Some(lsp_types::NumberOrString::String(
            "project::diagnostic".to_owned()
        ))
    );

    let schema = schema_diagnostics(&[SchemaDiagnostic::new("schema issue")]);
    assert_eq!(schema[0].message, "schema issue");
    assert_eq!(
        schema[0].code,
        Some(lsp_types::NumberOrString::String(
            "schema::diagnostic".to_owned()
        ))
    );
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
fn document_symbols_project_typed_nested_symbols() {
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    let source = "\
struct Player {
    level: i64,
}

pub fn main(player: Player) -> i64 {
    return player.level
}";
    let files = vec![SourceFileSnapshot::new(document.clone(), source)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    let symbols = databases.document_symbols(&document);

    let symbols = document_symbols(&symbols);

    let lsp_types::DocumentSymbolResponse::Nested(symbols) = symbols else {
        panic!("document symbols should project nested response");
    };
    let player = symbols
        .iter()
        .find(|symbol| symbol.name == "Player")
        .expect("Player symbol should project");
    assert_eq!(player.kind, lsp_types::SymbolKind::STRUCT);
    let children = player
        .children
        .as_ref()
        .expect("Player should include field children");
    assert!(
        children
            .iter()
            .any(|child| { child.name == "level" && child.kind == lsp_types::SymbolKind::FIELD })
    );
    assert!(
        symbols.iter().any(|symbol| {
            symbol.name == "main" && symbol.kind == lsp_types::SymbolKind::FUNCTION
        })
    );
}

#[test]
fn workspace_symbols_project_typed_nested_symbols() {
    let document = DocumentId::from("file:///workspace/scripts/game/reward.vela");
    let source = "pub fn grant() -> i64 { return 1 }";
    let files = vec![SourceFileSnapshot::new(document.clone(), source)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    let symbols = databases.workspace_symbols("reward.vela");

    let symbols = workspace_symbols(&symbols);

    let lsp_types::WorkspaceSymbolResponse::Nested(symbols) = symbols else {
        panic!("workspace symbols should project nested response");
    };
    let reward = symbols
        .iter()
        .find(|symbol| symbol.name == "reward.vela")
        .expect("file symbol should project");
    assert_eq!(reward.kind, lsp_types::SymbolKind::FILE);
    assert_eq!(
        reward.data.as_ref().and_then(|data| data.get("detail")),
        Some(&json!("game::reward"))
    );
    let lsp_types::OneOf::Left(location) = &reward.location else {
        panic!("source workspace symbol should use source location");
    };
    assert_eq!(location.uri.as_str(), document.as_str());
}

#[test]
fn folding_ranges_project_typed_ranges() {
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    let source = "\
use game::reward
use game::player

pub fn main() {
    if true {
        return
    }
}";
    let files = vec![SourceFileSnapshot::new(document.clone(), source)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    let ranges = databases.folding_ranges(&document);

    let ranges = folding_ranges(&ranges);

    assert!(ranges.iter().any(|range| {
        range.kind == Some(lsp_types::FoldingRangeKind::Imports)
            && range.start_line == 0
            && range.end_line == 1
    }));
    assert!(ranges.iter().any(|range| {
        range.kind == Some(lsp_types::FoldingRangeKind::Region)
            && range.start_line == 3
            && range.end_line == 7
            && range.start_character == Some(14)
    }));
}

#[test]
fn selection_ranges_project_typed_parent_chains() {
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    let source = "\
pub fn main(player: Player) -> i64 {
    let next = player.level + 1
    return next
}";
    let files = vec![SourceFileSnapshot::new(document.clone(), source)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    let ranges = databases.selection_ranges(&document, &[Position::new(1, 22)]);

    let ranges = selection_ranges(&ranges);

    assert_eq!(ranges.len(), 1);
    let mut chain = Vec::new();
    let mut current = Some(&ranges[0]);
    while let Some(range) = current {
        chain.push(range.range);
        current = range.parent.as_deref();
    }
    assert!(chain.contains(&lsp_types::Range::new(
        lsp_types::Position::new(1, 22),
        lsp_types::Position::new(1, 27)
    )));
    assert!(chain.contains(&lsp_types::Range::new(
        lsp_types::Position::new(1, 15),
        lsp_types::Position::new(1, 27)
    )));
}

#[test]
fn text_edits_project_typed_formatting_edits() {
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    let source = "pub fn main(){return 1}";
    let files = vec![SourceFileSnapshot::new(document.clone(), source)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    let edits = databases.document_formatting(&document);

    let edits = text_edits(&edits);

    assert_eq!(edits.len(), 1);
    assert_eq!(
        edits[0].range,
        lsp_types::Range::new(
            lsp_types::Position::new(0, 0),
            lsp_types::Position::new(0, 23)
        )
    );
    assert!(edits[0].new_text.contains("pub fn main() {"));
}

#[test]
fn code_actions_project_typed_quickfix_edits() {
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    let source = "pub fn main(scores: Array<i64>) { return scores.frist() }";
    let files = vec![SourceFileSnapshot::new(document.clone(), source)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    let typo_start = source.find("frist").expect("fixture should contain typo");
    let actions = databases.code_actions(
        &document,
        DiagnosticRange::new(
            Position::new(0, typo_start),
            Position::new(0, typo_start + "frist".len()),
        ),
    );

    let actions = code_actions(&actions);

    let action = actions
        .iter()
        .find_map(|action| match action {
            lsp_types::CodeActionOrCommand::CodeAction(action)
                if action.title == "Replace with `first`" =>
            {
                Some(action)
            }
            _ => None,
        })
        .expect("quickfix should project");
    assert_eq!(action.kind, Some(lsp_types::CodeActionKind::QUICKFIX));
    let edit = action.edit.as_ref().expect("quickfix should include edit");
    let changes = edit.changes.as_ref().expect("edit should include changes");
    let uri = lsp_types::Url::parse(document.as_str()).expect("document URI should parse");
    let edits = changes
        .get(&uri)
        .expect("document edit should be keyed by URI");
    assert_eq!(edits[0].new_text, "first");
}

#[test]
fn inlay_hints_project_typed_labels_and_kinds() {
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    let source = "pub fn grant(amount: i64, reason: String) -> i64 { return amount }\npub fn main() { return grant(10, \"quest\") }";
    let files = vec![SourceFileSnapshot::new(document.clone(), source)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    let hints = databases.inlay_hints(
        &document,
        DiagnosticRange::new(Position::new(1, 0), Position::new(1, 80)),
    );

    let hints = inlay_hints(&hints);

    assert_eq!(hints.len(), 2);
    assert_eq!(hints[0].position, lsp_types::Position::new(1, 29));
    let lsp_types::InlayHintLabel::String(label) = &hints[0].label else {
        panic!("first hint should use a string label");
    };
    assert_eq!(label, "amount:");
    assert_eq!(hints[0].kind, Some(lsp_types::InlayHintKind::PARAMETER));
    assert_eq!(hints[0].padding_right, Some(true));
    let lsp_types::InlayHintLabel::String(label) = &hints[1].label else {
        panic!("second hint should use a string label");
    };
    assert_eq!(label, "reason:");
}

#[test]
fn semantic_tokens_project_relative_data_and_result_id() {
    let projection = SemanticTokenProjection::default();
    let tokens = ServiceSemanticTokens::new(vec![
        ServiceSemanticToken::new(
            Position::new(0, 4),
            3,
            vela_language_service::SemanticTokenType::Function,
            vela_language_service::SemanticTokenModifiers::DECLARATION,
        ),
        ServiceSemanticToken::new(
            Position::new(1, 2),
            5,
            vela_language_service::SemanticTokenType::Variable,
            vela_language_service::SemanticTokenModifiers::NONE,
        ),
    ]);

    let lsp_types::SemanticTokensResult::Tokens(result) = semantic_tokens(&tokens, &projection)
    else {
        panic!("semantic tokens should project a full token result");
    };

    assert_eq!(result.result_id.as_deref(), Some(tokens.result_id()));
    assert_eq!(result.data.len(), 2);
    assert_eq!(result.data[0].delta_line, 0);
    assert_eq!(result.data[0].delta_start, 4);
    assert_eq!(result.data[0].length, 3);
    assert_eq!(result.data[1].delta_line, 1);
    assert_eq!(result.data[1].delta_start, 2);
    assert_eq!(result.data[1].length, 5);
}

#[test]
fn semantic_tokens_delta_projects_edit_units_as_encoded_u32s() {
    let projection = SemanticTokenProjection::default();
    let tokens = vec![ServiceSemanticToken::new(
        Position::new(0, 4),
        3,
        vela_language_service::SemanticTokenType::Function,
        vela_language_service::SemanticTokenModifiers::NONE,
    )];
    let delta = ServiceSemanticTokenDelta::new(
        "next".to_owned(),
        vec![vela_language_service::SemanticTokenEdit::new(1, 2, tokens)],
    );

    let lsp_types::SemanticTokensFullDeltaResult::TokensDelta(result) =
        semantic_tokens_delta(&delta, &projection)
    else {
        panic!("semantic token delta should project a delta result");
    };

    assert_eq!(result.result_id.as_deref(), Some("next"));
    assert_eq!(result.edits.len(), 1);
    assert_eq!(result.edits[0].start, 5);
    assert_eq!(result.edits[0].delete_count, 10);
    assert_eq!(
        result.edits[0]
            .data
            .as_ref()
            .expect("delta edit should include replacement tokens")
            .len(),
        1
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

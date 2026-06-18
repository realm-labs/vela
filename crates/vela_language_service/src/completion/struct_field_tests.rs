use super::{
    CompletionContextKind, CompletionInsertFormat, CompletionItem, CompletionKind, CompletionList,
};
use crate::{
    DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, Workspace, WorkspaceConfig,
    WorkspaceRoot, assemble_project_sources,
};

#[test]
fn struct_body_completion_enters_field_declaration_context() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub fn spawn_player() { return 1 }\npub struct Player {  }";
    let databases = databases_for_text(document.clone(), text);
    let line = text.lines().nth(1).expect("struct line should exist");

    let completions = databases.completion_items(
        &document,
        Position::new(1, line.find("{  }").expect("struct body") + "{ ".len()),
    );

    assert_eq!(
        completions.context().kind(),
        CompletionContextKind::StructFieldDeclaration
    );
    assert_snippet(
        &completions,
        "field",
        "struct field",
        "${1:name}: ${2:Type}",
    );
    assert_snippet(
        &completions,
        "field default",
        "struct field with default",
        "${1:name}: ${2:Type} = ${3:value}",
    );
    assert_no_completion(&completions, "spawn_player");
    assert_no_completion(&completions, "fn");

    let type_text = "pub struct Player { level: i }";
    let type_databases = databases_for_text(document.clone(), type_text);
    let type_completions = type_databases.completion_items(
        &document,
        Position::new(0, type_text.find("i }").expect("type prefix") + "i".len()),
    );

    assert_eq!(
        type_completions.context().kind(),
        CompletionContextKind::TypeHint
    );
    assert_completion(&type_completions, "i64", CompletionKind::Type);
    assert_no_completion(&type_completions, "field");
}

fn databases_for_text(document: DocumentId, text: &str) -> LanguageServiceDatabases {
    let files = vec![SourceFileSnapshot::new(document, text)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    databases
}

fn assert_snippet(completions: &CompletionList, label: &str, detail: &str, insert_text: &str) {
    let item = completion(completions, label);
    assert_eq!(item.kind(), CompletionKind::Snippet);
    assert_eq!(item.detail(), detail);
    assert_eq!(item.insert_text(), Some(insert_text));
    assert_eq!(item.insert_format(), CompletionInsertFormat::Snippet);
}

fn assert_completion(completions: &CompletionList, label: &str, kind: CompletionKind) {
    let item = completion(completions, label);
    assert_eq!(item.kind(), kind);
}

fn assert_no_completion(completions: &CompletionList, label: &str) {
    assert!(
        completions.items().iter().all(|item| item.label() != label),
        "{:?}",
        completions.items()
    );
}

fn completion<'a>(completions: &'a CompletionList, label: &str) -> &'a CompletionItem {
    completions
        .items()
        .iter()
        .find(|item| item.label() == label)
        .unwrap_or_else(|| panic!("missing completion {label}: {:?}", completions.items()))
}

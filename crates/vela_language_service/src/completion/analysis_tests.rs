use vela_analysis::type_fact::TypeFact;

use super::{
    CompletionAnalysisKind, CompletionContextKind, CompletionDeclarationKind, PathCompletionKind,
    TypeLocation,
};
use crate::{
    DocumentId, LanguageServiceDatabases, LineIndex, SourceFileSnapshot, Workspace,
    WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn completion_analysis_classifies_empty_dot_access() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"
struct Player { level: i64 }
pub fn main(player: Player) {
    player.
}"#;
    let completions = completions_for(document, text, "player.");
    let CompletionAnalysisKind::DotAccess(dot) = completions.analysis().kind() else {
        panic!("expected dot access analysis: {:?}", completions.analysis());
    };
    let receiver = dot.receiver_range().expect("dot access receiver range");

    assert_eq!(completions.context().kind(), CompletionContextKind::Member);
    assert_eq!(&text[receiver.start..receiver.end], "player");
    assert_eq!(
        dot.receiver_fact().map(TypeFact::display_name).as_deref(),
        Some("game::main::Player")
    );
}

#[test]
fn completion_analysis_classifies_type_argument_location() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub fn main() { let scores: Array<i }";
    let completions = completions_for(document, text, "Array<i");
    let CompletionAnalysisKind::Path(path) = completions.analysis().kind() else {
        panic!("expected path analysis: {:?}", completions.analysis());
    };

    assert_eq!(
        completions.context().kind(),
        CompletionContextKind::TypeHint
    );
    assert_eq!(path.kind(), PathCompletionKind::Type);
    assert_eq!(
        path.type_location(),
        Some(&TypeLocation::BuiltinTypeArgument {
            container: "Array".to_owned(),
            argument_index: 0,
        })
    );
}

#[test]
fn completion_analysis_classifies_struct_field_declaration_body() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub struct Player {  }";
    let completions = completions_for(document, text, "{ ");
    let CompletionAnalysisKind::Declaration(declaration) = completions.analysis().kind() else {
        panic!(
            "expected declaration analysis: {:?}",
            completions.analysis()
        );
    };

    assert_eq!(
        completions.context().kind(),
        CompletionContextKind::StructFieldDeclaration
    );
    assert_eq!(declaration.kind(), CompletionDeclarationKind::StructField);
}

#[test]
fn completion_analysis_tracks_expected_type_and_name() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"
struct Player { level: i64 }
fn grant(player: Player, amount: i64) { return amount }
pub fn main(player: Player) {
    grant(player, a)
}"#;
    let completions = completions_for(document, text, "grant(player, a");
    let CompletionAnalysisKind::CallArgument(call) = completions.analysis().kind() else {
        panic!(
            "expected call argument analysis: {:?}",
            completions.analysis()
        );
    };

    assert_eq!(call.active_parameter(), 1);
    assert_eq!(
        completions.analysis().expected_name(),
        Some("amount"),
        "{:?}",
        completions.analysis()
    );
    assert_eq!(
        completions
            .analysis()
            .expected_type()
            .map(TypeFact::display_name)
            .as_deref(),
        Some("i64")
    );
    assert!(
        completions
            .analysis()
            .visible_scope()
            .iter()
            .any(|name| name == "player"),
        "{:?}",
        completions.analysis().visible_scope()
    );
}

fn completions_for(document: DocumentId, text: &str, needle: &str) -> super::CompletionList {
    let files = vec![SourceFileSnapshot::new(document.clone(), text)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    let offset = text.find(needle).expect("completion needle") + needle.len();
    databases.completion_items(&document, LineIndex::new(text).position(offset))
}

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
fn completion_analysis_keeps_empty_type_argument_slots_with_their_nearest_container() {
    for (source, container, argument_index) in [
        ("fn f(value: Array<@>) {}", "Array", 0),
        ("fn f(value: Map<i64, @>) {}", "Map", 1),
        ("fn f(value: Result<Map<String, i64>, @>) {}", "Result", 1),
        ("fn f(value: Map<(i64, String), @>) {}", "Map", 1),
    ] {
        let offset = source.find('@').expect("cursor");
        let text = source.replacen('@', "", 1);
        let result = completions_for(
            DocumentId::from("/workspace/scripts/main.vela"),
            &text,
            &text[..offset],
        );
        let CompletionAnalysisKind::Path(path) = result.analysis().kind() else {
            panic!("{source}: {:?}", result.analysis());
        };
        assert_eq!(path.kind(), PathCompletionKind::Type, "{source}");
        assert_eq!(
            path.type_location(),
            Some(&TypeLocation::BuiltinTypeArgument {
                container: container.to_owned(),
                argument_index
            }),
            "{source}"
        );
    }
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

#[test]
fn completion_analysis_matrix_preserves_structured_contexts_and_current_expectations() {
    use crate::matrix_fixture::{load, parse_markers};
    use serde_json::json;
    let spec = load("completion-analysis-contexts");
    for crlf in [false, true] {
        for case in spec.oracle["queries"].as_array().expect("queries") {
            let source = parse_markers(
                &spec.files[case["file"].as_str().expect("file")]
                    .replace('\n', if crlf { "\r\n" } else { "\n" }),
            )
            .expect("source");
            let document = DocumentId::from("/workspace/scripts/main.vela");
            let project = assemble_project_sources(
                &WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]),
                &[SourceFileSnapshot::new(
                    document.clone(),
                    source.text.as_str(),
                )],
                &Workspace::new().snapshot(),
            );
            let mut db = LanguageServiceDatabases::new();
            db.update(&project);
            let byte = source.markers["cursor"].start.byte;
            let position = crate::Position::new(
                source.text[..byte].bytes().filter(|c| *c == b'\n').count(),
                byte - source.text[..byte].rfind('\n').map_or(0, |i| i + 1),
            );
            let result = db.completion_items(&document, position);
            assert_eq!(result, db.completion_items(&document, position));
            assert_eq!(
                serde_json::json!(
                    result
                        .items()
                        .iter()
                        .map(|item| item.label())
                        .collect::<Vec<_>>()
                ),
                serde_json::json!(
                    case["items"]
                        .as_array()
                        .expect("items")
                        .iter()
                        .map(|item| &item["label"])
                        .collect::<Vec<_>>()
                ),
                "{case}"
            );
            for (item, expected) in result
                .items()
                .iter()
                .zip(case["items"].as_array().expect("items"))
            {
                assert_eq!(item.detail(), expected["detail"], "{case}");
                assert_eq!(item.detail_parts().render(), expected["detail"]);
                assert_eq!(json!(item.label_details().detail()), expected["detail"]);
                assert_eq!(json!(item.insert_text()), expected["insert"]);
                let marker = source.markers["replace"];
                let edit = item.text_edit().expect("explicit edit");
                assert_eq!(
                    edit.range(),
                    crate::TextRange::new(marker.start.byte, marker.end.byte)
                );
                assert_eq!(edit.new_text(), expected["insert"]);
                assert!(item.documentation().is_none());
            }
            let analysis = result.analysis();
            let expected = &case["analysis"];
            assert_eq!(
                json!(analysis.expected_name()),
                expected["expectedName"],
                "{case}"
            );
            assert_eq!(
                json!(analysis.expected_type().map(TypeFact::display_name)),
                expected["expectedType"],
                "{case}"
            );
            assert_eq!(json!(analysis.visible_scope()), expected["scope"], "{case}");
            assert_eq!(
                format!("{:?}", analysis.context_kind()),
                expected["context"],
                "{case}"
            );
            let kind = match analysis.kind() {
                CompletionAnalysisKind::Path(path) => {
                    assert_eq!(format!("{:?}", path.kind()), expected["pathKind"]);
                    assert_eq!(json!(path.qualifier()), expected["qualifier"]);
                    assert!(path.type_location().is_none());
                    "Path"
                }
                CompletionAnalysisKind::DotAccess(dot) => {
                    let marker = source.markers["receiver"];
                    assert_eq!(
                        dot.receiver_range(),
                        Some(crate::TextRange::new(marker.start.byte, marker.end.byte))
                    );
                    assert_eq!(
                        json!(dot.receiver_fact().map(TypeFact::display_name)),
                        expected["receiverFact"]
                    );
                    "DotAccess"
                }
                CompletionAnalysisKind::Declaration(declaration) => {
                    assert_eq!(format!("{:?}", declaration.kind()), expected["declaration"]);
                    "Declaration"
                }
                CompletionAnalysisKind::CallArgument(call) => {
                    assert_eq!(json!(call.active_parameter()), expected["active"], "{case}");
                    "CallArgument"
                }
                CompletionAnalysisKind::RecordField(record) => {
                    assert_eq!(json!(record.owner_type()), expected["owner"]);
                    "RecordField"
                }
                CompletionAnalysisKind::Pattern(_) => "Pattern",
                CompletionAnalysisKind::Statement(_) => "Statement",
                other => panic!("unexpected analysis: {case}: {other:?}"),
            };
            assert_eq!(kind, expected["kind"], "{case}");
            if expected["empty"] == true {
                assert!(result.items().is_empty(), "{case}");
            }
        }
    }
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

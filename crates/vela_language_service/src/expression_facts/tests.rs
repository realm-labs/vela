use vela_analysis::type_fact::TypeFact;

use super::fact_for_range;
use crate::{
    DocumentId, LanguageServiceDatabases, SourceFileSnapshot, TextRange, Workspace,
    WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn expression_facts_include_unit_and_tuple_literals() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let source = r#"pub fn main() {
            let unit = ()
            let pair = ("Ada", 1)
        }"#;
    let files = vec![SourceFileSnapshot::new(document.clone(), source)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    let source_id = databases
        .source_db()
        .records()
        .get(&document)
        .expect("document source record should exist")
        .source_id();

    let unit_start = source
        .find("unit = ()")
        .map(|offset| offset + "unit = ".len())
        .expect("unit expression should exist");
    let unit_range = TextRange::new(unit_start, unit_start + "()".len());
    assert_eq!(
        fact_for_range(&databases, source_id, unit_range),
        Some(TypeFact::UNIT)
    );

    let tuple_text = r#"("Ada", 1)"#;
    let tuple_start = source
        .find(tuple_text)
        .expect("tuple expression should exist");
    let tuple_range = TextRange::new(tuple_start, tuple_start + tuple_text.len());
    assert_eq!(
        fact_for_range(&databases, source_id, tuple_range),
        Some(TypeFact::tuple([TypeFact::STRING, TypeFact::I64]))
    );
}

#[test]
fn expression_facts_include_tuple_destructuring_bindings() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let source = r#"pub fn main(pairs: Array<(String, i64)>) {
            let (let_name, let_score) = ("Ada", 1)
            let_name
            let_score
            for (for_name, for_score) in pairs {
                for_name
                for_score
            }
            match ("Grace", 2) {
                (match_name, match_score) => {
                    match_name
                    match_score
                },
            }
        }"#;
    let files = vec![SourceFileSnapshot::new(document.clone(), source)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    let source_id = databases
        .source_db()
        .records()
        .get(&document)
        .expect("document source record should exist")
        .source_id();

    assert_eq!(
        fact_for_range(&databases, source_id, range_for_nth(source, "let_name", 2)),
        Some(TypeFact::STRING)
    );
    assert_eq!(
        fact_for_range(&databases, source_id, range_for_nth(source, "let_score", 2)),
        Some(TypeFact::I64)
    );
    assert_eq!(
        fact_for_range(&databases, source_id, range_for_nth(source, "for_name", 2)),
        Some(TypeFact::STRING)
    );
    assert_eq!(
        fact_for_range(&databases, source_id, range_for_nth(source, "for_score", 2)),
        Some(TypeFact::I64)
    );
    assert_eq!(
        fact_for_range(
            &databases,
            source_id,
            range_for_nth(source, "match_name", 2)
        ),
        Some(TypeFact::STRING)
    );
    assert_eq!(
        fact_for_range(
            &databases,
            source_id,
            range_for_nth(source, "match_score", 2)
        ),
        Some(TypeFact::I64)
    );
}

#[test]
fn expression_facts_include_tuple_projection_fields() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let source = r#"pub fn main() {
            let pair = ("Ada", 1)
            pair.0
            pair.1
        }"#;
    let files = vec![SourceFileSnapshot::new(document.clone(), source)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    let source_id = databases
        .source_db()
        .records()
        .get(&document)
        .expect("document source record should exist")
        .source_id();

    assert_eq!(
        fact_for_range(&databases, source_id, range_for_nth(source, "pair.0", 1)),
        Some(TypeFact::STRING)
    );
    assert_eq!(
        fact_for_range(&databases, source_id, range_for_nth(source, "pair.1", 1)),
        Some(TypeFact::I64)
    );
}

#[test]
fn expression_facts_use_hir_index_operands_for_field_receivers() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let source = r#"pub fn main() {
            let pairs = [("Ada", 1)]
            pairs[0].1
        }"#;
    let files = vec![SourceFileSnapshot::new(document.clone(), source)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    let source_id = databases
        .source_db()
        .records()
        .get(&document)
        .expect("document source record should exist")
        .source_id();

    assert_eq!(
        fact_for_range(
            &databases,
            source_id,
            range_for_nth(source, "pairs[0].1", 1)
        ),
        Some(TypeFact::I64)
    );
}

#[test]
fn expression_facts_use_hir_paths_for_record_constructors_and_calls() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let source = r#"struct Reward {
            count: i64
        }
        pub fn main() {
            let reward = Reward { count: 1 }
            option::some(reward)
        }"#;
    let files = vec![SourceFileSnapshot::new(document.clone(), source)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    let source_id = databases
        .source_db()
        .records()
        .get(&document)
        .expect("document source record should exist")
        .source_id();

    assert_eq!(
        fact_for_range(
            &databases,
            source_id,
            range_for_nth(source, "Reward { count: 1 }", 1)
        ),
        Some(TypeFact::record("game::main::Reward"))
    );
    assert_eq!(
        fact_for_range(
            &databases,
            source_id,
            range_for_nth(source, "option::some(reward)", 1)
        ),
        Some(TypeFact::option(TypeFact::record("game::main::Reward")))
    );
}

#[test]
fn expression_facts_use_hir_locals_for_lambda_parameter_scope() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let source = r#"pub fn main() {
            let callback = |amount: i64| amount
            callback(1)
        }"#;
    let files = vec![SourceFileSnapshot::new(document.clone(), source)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    let source_id = databases
        .source_db()
        .records()
        .get(&document)
        .expect("document source record should exist")
        .source_id();

    assert_eq!(
        fact_for_range(&databases, source_id, range_for_nth(source, "amount", 2)),
        Some(TypeFact::I64)
    );
}

fn range_for_nth(source: &str, needle: &str, occurrence: usize) -> TextRange {
    let start = source
        .match_indices(needle)
        .nth(occurrence - 1)
        .map(|(start, _)| start)
        .unwrap_or_else(|| panic!("{needle} occurrence {occurrence} should exist"));
    TextRange::new(start, start + needle.len())
}

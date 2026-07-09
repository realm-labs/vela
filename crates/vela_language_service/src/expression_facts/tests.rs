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

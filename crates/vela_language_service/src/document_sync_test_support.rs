use crate::matrix_fixture::FixtureWorkspace;
use crate::matrix_fixture::document_open as oracle;
use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, Workspace,
    WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;
pub(super) fn id(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/中文 % open/{file}"))
}
pub(super) fn range(text: &str, r: DiagnosticRange) -> Value {
    let point = |p: Position| {
        let line = text
            .split('\n')
            .nth(p.line)
            .expect("existing diagnostic line");
        json!({"line":p.line,"character":line[..p.character].encode_utf16().count()})
    };
    json!({"start":point(r.start()),"end":point(r.end())})
}
pub(super) fn diagnostics(db: &LanguageServiceDatabases, document: &DocumentId) -> Vec<Value> {
    let text = db
        .source_db()
        .records()
        .get(document)
        .map_or("", |r| r.text());
    db.diagnostics_for_document(document).diagnostics().iter().map(|d| json!({
        "code":d.code(),"message":d.message(),"severity":match d.severity() {
            crate::ServiceDiagnosticSeverity::Error=>1, crate::ServiceDiagnosticSeverity::Warning=>2,
            crate::ServiceDiagnosticSeverity::Note=>3,crate::ServiceDiagnosticSeverity::Help=>4 },
        "range":d.range().map(|r|range(text,r)),
        "labels":d.labels().iter().map(|l|json!({"uri":l.document_id().as_str(),
            "range":range(db.source_db().records()[l.document_id()].text(),l.range()),"message":l.message()})).collect::<Vec<_>>(),
        "candidates":d.candidates().iter().map(|c|c.replacement()).collect::<Vec<_>>(),"repairHints":d.repair_hints().len()
    })).collect()
}
pub(super) fn inputs(spec: &crate::matrix_fixture::Spec) -> Vec<SourceFileSnapshot> {
    let fixture = FixtureWorkspace::new(spec).expect("fixture");
    fixture
        .disk
        .iter()
        .filter(|(f, _)| f.ends_with(".vela") && !oracle::missing_disk(spec, f))
        .map(|(f, d)| SourceFileSnapshot::new(id(f), d.text.as_str()))
        .collect()
}
pub(super) fn update(
    db: &mut LanguageServiceDatabases,
    files: &[SourceFileSnapshot],
    workspace: &Workspace,
) {
    let config =
        WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/中文 % open/scripts")]);
    db.update_with_open_documents(
        &assemble_project_sources(&config, files, &workspace.snapshot()),
        &workspace
            .snapshot()
            .open_document_ids()
            .collect::<BTreeSet<_>>(),
    );
}

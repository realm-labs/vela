use crate::matrix_fixture::{FixtureWorkspace, load};
use crate::{
    DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, Workspace, WorkspaceConfig,
    WorkspaceRoot, assemble_project_sources,
};

#[test]
fn shared_fixture_service_queries_use_independent_unicode_lifecycle_ranges() {
    for crlf in [false, true] {
        let mut spec = load("shared-unicode-lifecycle");
        if crlf {
            for source in spec.files.values_mut() {
                *source = source.replace('\n', "\r\n");
            }
            for action in &mut spec.actions {
                if let Some(source) = &mut action.source {
                    *source = source.replace('\n', "\r\n");
                }
            }
        }
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let uri = |file: &str| DocumentId::from(format!("/workspace/{file}"));
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let mut databases = LanguageServiceDatabases::new();
        for (index, action) in spec.actions.iter().enumerate() {
            fixture.apply(action).expect("action");
            let mut effective = fixture.disk.clone();
            effective.extend(fixture.open.clone());
            let sources = effective
                .iter()
                .filter(|(file, _)| file.ends_with(".vela"))
                .map(|(file, document)| SourceFileSnapshot::new(uri(file), document.text.as_str()))
                .collect::<Vec<_>>();
            let project = assemble_project_sources(&config, &sources, &Workspace::new().snapshot());
            databases.update(&project);
            let document = fixture.document("scripts/main.vela").expect("caller");
            let caller = document.markers["call"].start;
            // Service coordinates are byte columns; only the protocol/editor
            // boundary takes UTF-16. Derive both from the same stripped marker.
            let byte_column = caller.byte
                - document.text[..caller.byte]
                    .rfind('\n')
                    .map_or(0, |index| index + 1);
            let target = databases.definition(
                &uri("scripts/main.vela"),
                Position::new(caller.line, byte_column),
            );
            let expected = &spec.oracle["afterEachAction"][index];
            if expected.is_null() {
                assert!(target.is_none());
            } else {
                let target = target.expect("definition");
                assert_eq!(target.document_id(), &uri("scripts/helper.vela"));
                assert_eq!(
                    serde_json::json!([
                        target.range().start().line,
                        target.range().start().character,
                        target.range().end().line,
                        target.range().end().character
                    ]),
                    serde_json::json!([
                        expected["line"],
                        expected["character"],
                        expected["line"],
                        expected["endCharacter"]
                    ]),
                    "action {index}, CRLF={crlf}"
                );
            }
        }
    }
}

use vela_analysis::registry::RegistryFacts;
use vela_analysis::type_fact::TypeFact;

use crate::{
    DocumentId, LanguageServiceDatabases, SourceFileSnapshot, TextRange, Workspace,
    WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

use super::member_index::{MemberCompletionIndex, MemberCompletionSurface};

#[test]
fn member_completion_index_unifies_source_schema_trait_and_builtin_members() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = include_str!("../../tests/fixtures/completion/member-index.vela");
    let files = vec![SourceFileSnapshot::new(document, text)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    let mut schema = RegistryFacts::default();
    schema.insert_field("Player", "rank", TypeFact::I64);
    schema.insert_method(
        "Player",
        "persist",
        TypeFact::function(Vec::new(), TypeFact::BOOL),
    );
    schema.insert_trait("SchemaRewardable", TypeFact::trait_type("SchemaRewardable"));
    schema.insert_trait_method(
        "SchemaRewardable",
        "schema_preview",
        TypeFact::function(vec![TypeFact::I64], TypeFact::BOOL),
    );
    databases.set_schema_facts(schema);
    databases.update(&project);

    let player_index = MemberCompletionIndex::for_receiver(
        databases.hir_db().graph(),
        databases.schema_db().facts(),
        &TypeFact::record("Player"),
        None,
        TextRange::new(0, 0),
        "",
    );
    assert_eq!(
        player_index.surfaces_for_label("level"),
        vec![MemberCompletionSurface::Source]
    );
    assert_eq!(
        player_index.surfaces_for_label("level_up"),
        vec![MemberCompletionSurface::Source]
    );
    assert_eq!(
        player_index.surfaces_for_label("grant"),
        vec![MemberCompletionSurface::Source]
    );
    assert_eq!(
        player_index.surfaces_for_label("preview"),
        vec![MemberCompletionSurface::Source]
    );
    assert_eq!(player_index.surfaces_for_label("rank"), vec![]);
    assert_eq!(player_index.surfaces_for_label("persist"), vec![]);

    // A registered Value record with a distinct owner uses schema fields.
    let mut value_schema = RegistryFacts::default();
    value_schema.insert_type("RegisteredPlayer", TypeFact::record("RegisteredPlayer"));
    value_schema.insert_field("RegisteredPlayer", "rank", TypeFact::I64);
    let value_index = MemberCompletionIndex::for_receiver(
        databases.hir_db().graph(),
        &value_schema,
        &TypeFact::record("RegisteredPlayer"),
        None,
        TextRange::new(0, 0),
        "",
    );
    assert_eq!(
        value_index.surfaces_for_label("rank"),
        vec![MemberCompletionSurface::Schema]
    );
    assert!(value_index.surfaces_for_label("level").is_empty());

    let schema_trait_index = MemberCompletionIndex::for_receiver(
        databases.hir_db().graph(),
        databases.schema_db().facts(),
        &TypeFact::trait_type("SchemaRewardable"),
        None,
        TextRange::new(0, 0),
        "",
    );
    assert_eq!(
        schema_trait_index.surfaces_for_label("schema_preview"),
        vec![MemberCompletionSurface::Schema]
    );

    let array_index = MemberCompletionIndex::for_receiver(
        databases.hir_db().graph(),
        databases.schema_db().facts(),
        &TypeFact::array(TypeFact::STRING),
        None,
        TextRange::new(0, 0),
        "",
    );
    assert_eq!(
        array_index.surfaces_for_label("join"),
        vec![MemberCompletionSurface::Builtin]
    );
    assert_eq!(
        array_index.surfaces_for_label("map"),
        vec![MemberCompletionSurface::Builtin]
    );
}

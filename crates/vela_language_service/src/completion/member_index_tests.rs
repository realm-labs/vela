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
    let text = r#"
struct Player { level: i64 }
trait Rewardable {
    fn preview(self, amount: i64) -> bool { return amount > 0 }
    fn grant(self, amount: i64) -> bool { return amount > 0 }
}
impl Player {
    fn level_up(self, amount: i64) -> bool { return amount > 0 }
}
impl Rewardable for Player {
    fn grant(self, amount: i64) -> bool { return amount > 0 }
}
"#;
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
    assert_eq!(
        player_index.surfaces_for_label("rank"),
        vec![MemberCompletionSurface::Schema]
    );
    assert_eq!(
        player_index.surfaces_for_label("persist"),
        vec![MemberCompletionSurface::Schema]
    );

    let schema_trait_index = MemberCompletionIndex::for_receiver(
        databases.hir_db().graph(),
        databases.schema_db().facts(),
        &TypeFact::trait_type("SchemaRewardable"),
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

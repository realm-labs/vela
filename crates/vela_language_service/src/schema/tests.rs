use crate::LanguageServiceDatabases;

use super::*;

fn sample_facts() -> RegistryFacts {
    let mut facts = RegistryFacts::default();
    facts.insert_type("Player", TypeFact::host("Player"));
    facts.insert_type_docs("Player", "Player host object.");
    facts.insert_trait("Rewardable", TypeFact::trait_type("Rewardable"));
    facts.insert_trait_docs("Rewardable", "Rewardable host trait.");
    facts.insert_module(RegistryModuleFact::from_parts(
        "game::reward",
        TypeFact::module("game::reward"),
        Some("Reward module.".to_owned()),
        Some(Span::new(SourceId::new(7), 10, 20)),
    ));
    facts.insert_field("Player", "level", TypeFact::I64);
    facts.insert_field_docs("Player", "level", "Current player level.");
    facts.insert_field_access(RegistryFieldAccessFact {
        owner: "Player".to_owned(),
        name: "level".to_owned(),
        readable: true,
        writable: true,
        reflect_readable: true,
        reflect_writable: false,
        required_permissions: vec!["player.read".to_owned()],
    });
    facts.insert_method(
        "Player",
        "grant_exp",
        TypeFact::function(vec![TypeFact::I64], TypeFact::BOOL),
    );
    facts.insert_method_docs("Player", "grant_exp", "Grant player experience.");
    facts.insert_method_effect("Player", "grant_exp", RegistryEffectFact::host_write());
    facts.insert_method_access(RegistryMethodAccessFact {
        owner: "Player".to_owned(),
        name: "grant_exp".to_owned(),
        public: true,
        reflect_callable: true,
        required_permissions: vec!["player.reward".to_owned()],
    });
    facts.insert_function(
        "game::reward::grant",
        TypeFact::function(
            vec![TypeFact::host("Player"), TypeFact::I64],
            TypeFact::BOOL,
        ),
    );
    facts.insert_function_docs("game::reward::grant", "Grant reward.");
    facts.insert_function_effect("game::reward::grant", RegistryEffectFact::host_write());
    facts.insert_variant(
        "QuestState",
        "Active",
        TypeFact::enum_type("QuestState", Some("Active")),
    );
    facts.insert_variant_docs("QuestState", "Active", "Active quest state.");
    facts.insert_trait_method(
        "Rewardable",
        "preview",
        TypeFact::function(vec![TypeFact::I64], TypeFact::BOOL),
    );
    facts.insert_trait_method_docs("Rewardable", "preview", "Preview reward.");
    facts.insert_index_capability(RegistryIndexCapabilityFact {
        owner: "Inventory".to_owned(),
        readable: true,
        writable: true,
        addable: false,
        removable: false,
        key: TypeFact::STRING,
        value: TypeFact::I64,
    });
    facts
}

#[test]
fn schema_export_round_trips_registry_facts() {
    let facts = sample_facts();
    let artifact = SchemaArtifact::from_registry_facts(&facts);
    let json = artifact
        .to_json()
        .expect("schema artifact should encode as JSON");
    let parsed = SchemaArtifact::from_json(&json).expect("schema artifact should decode from JSON");
    let round_tripped = parsed.to_registry_facts();

    assert_eq!(round_tripped, facts);
    assert_eq!(
        round_tripped.module_fact("game::reward"),
        Some(&TypeFact::module("game::reward"))
    );
    assert_eq!(
        round_tripped.module_docs("game::reward"),
        Some("Reward module.")
    );
    assert_eq!(
        round_tripped.module_source_span("game::reward"),
        Some(Span::new(SourceId::new(7), 10, 20))
    );
}

#[test]
fn schema_artifact_accepts_docs_metadata() {
    let artifact = SchemaArtifact::from_json(
        r#"{
            "formatVersion": 1,
            "facts": {
                "types": [
                    {
                        "name": "Player",
                        "fact": { "kind": "host", "name": "Player" },
                        "docs": "Player host object."
                    }
                ],
                "modules": [
                    {
                        "name": "game::reward",
                        "fact": { "kind": "module", "name": "game::reward" },
                        "docs": "Reward module.",
                        "sourceSpan": { "source": 7, "start": 10, "end": 20 }
                    }
                ],
                "fields": [
                    {
                        "owner": "Player",
                        "name": "level",
                        "fact": { "kind": "primitive", "name": "i64" },
                        "docs": "Current player level."
                    }
                ],
                "functions": [
                    {
                        "name": "game::reward::grant",
                        "fact": {
                            "kind": "function",
                            "params": [{ "kind": "host", "name": "Player" }],
                            "returns": { "kind": "primitive", "name": "bool" }
                        },
                        "docs": "Grant reward."
                    }
                ]
            }
        }"#,
    )
    .expect("schema docs metadata should decode");

    let facts = artifact.to_registry_facts();

    assert_eq!(facts.type_docs("Player"), Some("Player host object."));
    assert_eq!(
        facts.field_docs("Player", "level"),
        Some("Current player level.")
    );
    assert_eq!(
        facts.function_docs("game::reward::grant"),
        Some("Grant reward.")
    );
    assert_eq!(
        facts.module_fact("game::reward"),
        Some(&TypeFact::module("game::reward"))
    );
    assert_eq!(facts.module_docs("game::reward"), Some("Reward module."));
    assert_eq!(
        facts.module_source_span("game::reward"),
        Some(Span::new(SourceId::new(7), 10, 20))
    );
    assert_eq!(
        artifact.source_locations().module_span("game::reward"),
        Some(Span::new(SourceId::new(7), 10, 20))
    );
}

#[test]
fn schema_hash_compatibility_accepts_matching_facts() {
    let facts = sample_facts();
    let mut artifact = SchemaArtifact::from_registry_facts(&facts);
    let computed = artifact
        .computed_schema_hash()
        .expect("schema hash should be computable");
    let expected_hash = format!("0x{computed:016x}");
    artifact.schema_version = Some("2026-06-16T00:00:00Z".to_owned());
    artifact.schema_hash = Some(expected_hash.clone());
    let json = artifact
        .to_json()
        .expect("schema artifact should encode as JSON");

    let parsed = SchemaArtifact::from_json(&json).expect("matching schema hash should validate");

    assert_eq!(parsed.schema_version(), Some("2026-06-16T00:00:00Z"));
    assert_eq!(parsed.schema_hash(), Some(expected_hash.as_str()));
    assert_eq!(parsed.to_registry_facts(), facts);
}

#[test]
fn schema_hash_compatibility_rejects_stale_facts() {
    let facts = sample_facts();
    let mut artifact = SchemaArtifact::from_registry_facts(&facts);
    artifact.schema_hash = Some("0x0000000000000001".to_owned());
    let json = artifact
        .to_json()
        .expect("schema artifact should encode as JSON");

    let error = SchemaArtifact::from_json(&json)
        .expect_err("stale schema hash should fail compatibility validation");

    assert!(
        error.message().contains("schema hash mismatch"),
        "{}",
        error.message()
    );
}

#[test]
fn invalid_schema_reports_diagnostic() {
    let error = SchemaArtifact::from_json(r#"{ "formatVersion": 999 }"#)
        .expect_err("unsupported format version should fail");

    assert!(
        error
            .message()
            .contains("unsupported schema artifact format version"),
        "{}",
        error.message()
    );
}

#[test]
fn invalid_schema_metadata_reports_diagnostic() {
    let mut databases = LanguageServiceDatabases::new();
    databases.load_schema_artifact_json(
        "/workspace/target/vela/schema.json",
        r#"{ "formatVersion": 1, "schemaVersion": " ", "facts": {} }"#,
    );

    let diagnostics = databases.schema_db().diagnostics();
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message()
            .contains("schemaVersion must be non-empty"),
        "{}",
        diagnostics[0].message()
    );
    assert!(
        databases.schema_db().facts().types().next().is_none(),
        "invalid schema metadata should not install facts"
    );
}

#[test]
fn invalid_schema_artifact_records_schema_diagnostic() {
    let mut databases = LanguageServiceDatabases::new();
    databases.load_schema_artifact_json("/workspace/target/vela/schema.json", "{");

    let diagnostics = databases.schema_db().diagnostics();
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0].message().contains("schema.json` is invalid"),
        "{}",
        diagnostics[0].message()
    );
    assert!(
        databases.schema_db().facts().types().next().is_none(),
        "invalid schema should not leave stale facts installed"
    );
}

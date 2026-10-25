use crate::SchemaArtifact;
use serde_json::{Value, json};
use vela_analysis::registry::{
    CallableParameterFact, CallableParameterRequirementFact as Required, CallableSignatureFact,
    RegistryFacts,
};
use vela_analysis::type_fact::TypeFact;
use vela_common::{SourceId, Span};

fn registry() -> RegistryFacts {
    let signature = CallableSignatureFact::new(
        [
            CallableParameterFact::new("arg0", TypeFact::I64, Required::Required)
                .declared_at(Span::new(SourceId::new(7), 12, 16)),
            CallableParameterFact::new("reason", TypeFact::STRING, Required::Defaulted),
        ],
        TypeFact::BOOL,
    );
    let fact = TypeFact::function(vec![TypeFact::I64, TypeFact::STRING], TypeFact::BOOL);
    let mut facts = RegistryFacts::default();
    facts.insert_function("host::grant", fact.clone());
    facts.insert_function_signature("host::grant", signature.clone());
    facts.insert_method("host::Player", "grant", fact.clone());
    facts.insert_method_signature("host::Player", "grant", signature.clone());
    facts.insert_trait_method("host::Rewardable", "grant", fact);
    facts.insert_trait_method_signature("host::Rewardable", "grant", signature);
    facts
}

fn wire() -> Value {
    serde_json::from_str(
        &SchemaArtifact::from_registry_facts(&registry())
            .to_json()
            .expect("serialize"),
    )
    .expect("JSON")
}

#[test]
fn schema_callable_signatures_round_trip_names_defaults_types_and_spans() {
    let expected = json!({"parameters":[
        {"name":"arg0","typeFact":{"kind":"primitive","name":"i64"},"requirement":"required","sourceSpan":{"source":7,"start":12,"end":16}},
        {"name":"reason","typeFact":{"kind":"primitive","name":"string"},"requirement":"defaulted"}
    ],"returns":{"kind":"primitive","name":"bool"}});
    let value = wire();
    for group in ["functions", "methods", "traitMethods"] {
        assert_eq!(value["facts"][group][0]["signature"], expected, "{group}");
    }
    let restored = SchemaArtifact::from_json(&value.to_string())
        .expect("load")
        .to_registry_facts();
    let original = registry();
    assert_eq!(
        restored.function_signature_fact("host::grant"),
        original.function_signature_fact("host::grant")
    );
    assert_eq!(
        restored.method_signature_fact("host::Player", "grant"),
        original.method_signature_fact("host::Player", "grant")
    );
    assert_eq!(
        restored.trait_method_signature_fact("host::Rewardable", "grant"),
        original.trait_method_signature_fact("host::Rewardable", "grant")
    );
    assert_eq!(
        SchemaArtifact::from_registry_facts(&restored)
            .to_json()
            .expect("re-export"),
        serde_json::to_string_pretty(&SchemaArtifact::from_json(&value.to_string()).expect("load"))
            .expect("wire")
    );
}

#[test]
fn schema_callable_signature_metadata_participates_in_hash_and_rejects_stale_hash() {
    let value = wire();
    let hash = SchemaArtifact::from_json(&value.to_string())
        .expect("load")
        .computed_schema_hash()
        .expect("hash");
    for (path, replacement) in [
        (
            "/facts/functions/0/signature/parameters/0/name",
            json!("amount"),
        ),
        (
            "/facts/methods/0/signature/parameters/1/requirement",
            json!("required"),
        ),
        (
            "/facts/traitMethods/0/signature/parameters/0/name",
            json!("other"),
        ),
    ] {
        let mut changed = value.clone();
        *changed.pointer_mut(path).expect("path") = replacement;
        assert_ne!(
            SchemaArtifact::from_json(&changed.to_string())
                .expect("changed")
                .computed_schema_hash()
                .expect("hash"),
            hash,
            "{path}"
        );
        changed["schemaHash"] = json!(format!("0x{hash:016x}"));
        assert!(
            SchemaArtifact::from_json(&changed.to_string())
                .expect_err("stale hash")
                .message()
                .contains("hash mismatch")
        );
    }
}

#[test]
fn schema_callable_signatures_reject_malformed_and_disagreeing_metadata() {
    for group in ["functions", "methods", "traitMethods"] {
        for (suffix, replacement) in [
            ("/parameters/0/name", json!("")),
            ("/parameters/1/name", json!("arg0")),
            ("/parameters/0/sourceSpan/end", json!(1)),
            (
                "/parameters/0/typeFact",
                json!({"kind":"primitive","name":"bool"}),
            ),
            ("/returns", json!({"kind":"primitive","name":"string"})),
            ("/parameters/0/requirement", json!("optional")),
        ] {
            let mut value = wire();
            *value
                .pointer_mut(&format!("/facts/{group}/0/signature{suffix}"))
                .expect("path") = replacement;
            assert!(
                SchemaArtifact::from_json(&value.to_string()).is_err(),
                "{group}{suffix}"
            );
        }
        let mut value = wire();
        value["facts"][group][0]["signature"]["unexpected"] = json!(true);
        assert!(
            SchemaArtifact::from_json(&value.to_string()).is_err(),
            "unknown metadata: {group}"
        );
    }
    for group in ["fields", "variants"] {
        let mut value = wire();
        value["facts"][group] = json!([value["facts"]["methods"][0].clone()]);
        assert!(
            SchemaArtifact::from_json(&value.to_string()).is_err(),
            "{group}"
        );
    }
}

#[test]
fn schema_v3_rejects_legacy_async_and_task_metadata_without_defaulting() {
    for group in ["functions", "methods", "traitMethods"] {
        for asyncness in [json!("sync"), json!("async"), Value::Null] {
            let mut value = wire();
            value["facts"][group][0]["signature"]["asyncness"] = asyncness;
            let error = SchemaArtifact::from_json(&value.to_string())
                .expect_err("legacy callable metadata");
            assert!(
                error.message().contains("unknown field `asyncness`"),
                "{error:?}"
            );
        }
    }
    for field in ["taskTargets", "taskMetadata", "asyncness"] {
        let mut value = wire();
        value["facts"][field] = json!([]);
        assert!(
            SchemaArtifact::from_json(&value.to_string()).is_err(),
            "{field}"
        );
    }
    for effects in ["functionEffects", "methodEffects", "traitMethodEffects"] {
        for spawns in [false, true] {
            let mut value = wire();
            let mut effect = json!({"readsHost":false,"writesHost":false,"emitsEvents":false,"readsTime":false,"usesRandom":false,"readsIo":false,"writesIo":false,"readsReflection":false,"writesReflection":false,"callsReflection":false});
            effect["spawnsTasks"] = json!(spawns);
            let row = if effects == "functionEffects" {
                json!({"name":"host::grant","effect":effect})
            } else {
                json!({"owner":"host::Player","name":"grant","effect":effect})
            };
            value["facts"][effects] = json!([row]);
            let error =
                SchemaArtifact::from_json(&value.to_string()).expect_err("legacy task effect");
            assert!(
                error.message().contains("unknown field `spawnsTasks`"),
                "{error:?}"
            );
        }
    }
    for ceiling in [Value::Null, json!({}), json!({"spawnsTasks":false})] {
        let mut value = wire();
        value["facts"]["executionEffectCeiling"] = ceiling;
        let error =
            SchemaArtifact::from_json(&value.to_string()).expect_err("removed task ceiling");
        assert!(
            error
                .message()
                .contains("unknown field `executionEffectCeiling`"),
            "{error:?}"
        );
    }
}

#[test]
fn schema_without_callable_metadata_keeps_absent_names_and_canonical_encoding() {
    let mut value = wire();
    for group in ["functions", "methods", "traitMethods"] {
        value["facts"][group][0]
            .as_object_mut()
            .expect("object")
            .remove("signature");
    }
    let artifact = SchemaArtifact::from_json(&value.to_string()).expect("current type-only schema");
    let facts = artifact.to_registry_facts();
    assert!(facts.function_signature_fact("host::grant").is_none());
    assert!(
        facts
            .method_signature_fact("host::Player", "grant")
            .is_none()
    );
    assert!(
        facts
            .trait_method_signature_fact("host::Rewardable", "grant")
            .is_none()
    );
    let again = serde_json::from_str::<Value>(
        &SchemaArtifact::from_registry_facts(&facts)
            .to_json()
            .expect("export"),
    )
    .expect("JSON");
    assert_eq!(again, value);
}

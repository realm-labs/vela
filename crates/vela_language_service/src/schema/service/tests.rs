use super::*;
use crate::SchemaArtifact;
use vela_analysis::registry::{CallableParameterRequirementFact, CallableSignatureFact};
use vela_common::{CallableAsyncness, CollectionViewMutation};

fn artifact() -> SchemaArtifact {
    let mut facts = RegistryFacts::default();
    facts.insert_type("host::Context", TypeFact::host("host::Context"));
    facts.insert_type("host::Reply", TypeFact::record("host::Reply"));
    facts.insert_type(
        "host::Failure",
        TypeFact::enum_type("host::Failure", None::<String>),
    );
    // A same-owner ordinary trait signature must not survive service projection.
    facts.insert_trait_method_signature(
        "api::Handler",
        "handle",
        CallableSignatureFact::new([], TypeFact::BOOL),
    );
    SchemaArtifact::from_registry_facts(&facts).with_service_set(SchemaServiceSetFact::new(
        "set",
        "api::Services",
        "abi",
        "bindings",
        [
            SchemaServiceFact::new(
                "handler",
                "handler",
                "api::Handler",
                "handler-abi",
                [
                    SchemaServiceMethodFact::new(
                        "handle",
                        "handle",
                        "api::Handler::handle",
                        true,
                        ["host_write".to_owned()],
                        [
                            SchemaServiceParameterFact::new(
                                "context",
                                "host::Context",
                                "exclusive_host",
                            ),
                            SchemaServiceParameterFact::new(
                                "values",
                                "ArrayMut<Option<i64>, growable>",
                                "owned",
                            ),
                            SchemaServiceParameterFact::new("peer", "api::Peer", "owned"),
                            SchemaServiceParameterFact::new(
                                "pair",
                                "Tuple<String, Array<i64>>",
                                "owned",
                            ),
                        ],
                        "Result<host::Reply, host::Failure>",
                    ),
                    SchemaServiceMethodFact::new(
                        "empty",
                        "empty",
                        "api::Handler::empty",
                        false,
                        [],
                        [],
                        "()",
                    ),
                ],
            ),
            SchemaServiceFact::new("peer", "peer", "api::Peer", "peer-abi", []),
        ],
    ))
}

#[test]
fn service_projection_preserves_names_types_asyncness_and_forward_service_references() {
    let json = artifact().to_json().expect("encode");
    let facts = SchemaArtifact::from_json(&json)
        .expect("decode")
        .to_registry_facts();
    let params = vec![
        TypeFact::host("host::Context"),
        TypeFact::array_mut(
            TypeFact::option(TypeFact::I64),
            CollectionViewMutation::Growable,
        ),
        TypeFact::trait_type("api::Peer"),
        TypeFact::tuple([TypeFact::STRING, TypeFact::array(TypeFact::I64)]),
    ];
    let returns = TypeFact::result(
        TypeFact::record("host::Reply"),
        TypeFact::enum_type("host::Failure", None::<String>),
    );
    assert_eq!(
        facts.trait_method_fact("api::Handler", "handle"),
        Some(&TypeFact::function(params.clone(), returns.clone()))
    );
    let signature = facts
        .trait_method_signature_fact("api::Handler", "handle")
        .expect("signature");
    assert_eq!(signature.asyncness, CallableAsyncness::Async);
    assert_eq!(signature.returns, returns);
    assert_eq!(
        signature
            .parameters
            .iter()
            .map(|p| (p.name.as_str(), p.type_fact.clone(), p.requirement))
            .collect::<Vec<_>>(),
        ["context", "values", "peer", "pair"]
            .into_iter()
            .zip(params)
            .map(|(name, fact)| (name, fact, CallableParameterRequirementFact::Required))
            .collect::<Vec<_>>()
    );
    let empty = facts
        .trait_method_signature_fact("api::Handler", "empty")
        .expect("empty signature");
    assert_eq!(empty, &CallableSignatureFact::new([], TypeFact::UNIT));
    let effects = facts
        .trait_method_effect_fact("api::Handler", "handle")
        .expect("effects");
    assert!(effects.reads_host && effects.writes_host);
}

#[test]
fn service_projection_rejects_duplicate_parameter_names_and_malformed_type_metadata() {
    let value: serde_json::Value =
        serde_json::from_str(&artifact().to_json().expect("json")).expect("schema value");
    for (field, replacement) in [
        ("name", "context"),
        ("typeHint", "Array<i64"),
        ("typeHint", "ArrayMut<i64, mystery>"),
    ] {
        let mut bad = value.clone();
        bad.as_object_mut()
            .expect("schema object")
            .remove("schemaHash");
        bad["serviceSet"]["services"][0]["methods"][0]["parameters"][1][field] = replacement.into();
        assert!(
            SchemaArtifact::from_json(&bad.to_string()).is_err(),
            "{field}: {replacement}"
        );
    }
}

#[test]
fn service_type_hints_cover_wire_shapes_and_keep_missing_qualified_types_unknown() {
    use super::type_hint::service_type_hint;
    let mut facts = RegistryFacts::default();
    facts.insert_type("Context", TypeFact::host("Context"));
    facts.insert_type("host::Context", TypeFact::host("host::Context"));
    for (text, expected) in [
        ("Any", TypeFact::Any),
        ("bool", TypeFact::BOOL),
        ("u64", TypeFact::U64),
        ("f32", TypeFact::F32),
        ("char", TypeFact::CHAR),
        ("Bytes", TypeFact::BYTES),
        ("Array", TypeFact::array(TypeFact::Unknown)),
        (
            "ArrayView<host::Context>",
            TypeFact::array_view(TypeFact::host("host::Context")),
        ),
        (
            "ArrayMut<i64, fixed>",
            TypeFact::array_mut(TypeFact::I64, CollectionViewMutation::Fixed),
        ),
        ("Map", TypeFact::map(TypeFact::Unknown, TypeFact::Unknown)),
        (
            "MapView<String, i64>",
            TypeFact::map_view(TypeFact::STRING, TypeFact::I64),
        ),
        (
            "MapMut<String, i64, growable>",
            TypeFact::map_mut(
                TypeFact::STRING,
                TypeFact::I64,
                CollectionViewMutation::Growable,
            ),
        ),
        (
            "MapMut<String, i64, fixed>",
            TypeFact::map_mut(
                TypeFact::STRING,
                TypeFact::I64,
                CollectionViewMutation::Fixed,
            ),
        ),
        ("Set", TypeFact::set(TypeFact::Unknown)),
        (
            "SetMut<String, fixed>",
            TypeFact::set_mut(TypeFact::STRING, CollectionViewMutation::Fixed),
        ),
        (
            "SetMut<String, growable>",
            TypeFact::set_mut(TypeFact::STRING, CollectionViewMutation::Growable),
        ),
        ("Iterator", TypeFact::iterator(TypeFact::Unknown)),
        (
            "Iterator<Option<i64>>",
            TypeFact::iterator(TypeFact::option(TypeFact::I64)),
        ),
        (
            "Function",
            TypeFact::function(Vec::new(), TypeFact::Unknown),
        ),
        (
            "(String, i64)",
            TypeFact::tuple([TypeFact::STRING, TypeFact::I64]),
        ),
        (
            "Result<Tuple<String, ArrayMut<i64, growable>>, ()>",
            TypeFact::result(
                TypeFact::tuple([
                    TypeFact::STRING,
                    TypeFact::array_mut(TypeFact::I64, CollectionViewMutation::Growable),
                ]),
                TypeFact::UNIT,
            ),
        ),
        ("missing::Context", TypeFact::Unknown),
        (
            "Option<missing::Context>",
            TypeFact::option(TypeFact::Unknown),
        ),
        ("Array<i64, String>", TypeFact::Unknown),
    ] {
        assert_eq!(
            facts.type_hint_fact(&service_type_hint(text).expect("parse")),
            expected,
            "{text}"
        );
    }
    for text in [
        "Tuple<i64>",
        "ArrayMut<i64, fixed, growable>",
        "MapMut<String, i64, fixed<i64>>",
        "Result<i64,",
        "SetMut<i64, mystery>",
    ] {
        assert!(service_type_hint(text).is_none(), "{text}");
    }
}

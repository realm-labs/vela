use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use vela_common::StoragePolicy;
use vela_engine::args::{FromScriptArg, IntoScriptArg};
use vela_engine::engine::Engine;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_macros::{Value, script_function};
use vela_reflect::registry::TypeKind;
use vela_vm::owned_value::OwnedValue;

#[derive(Debug, Eq, PartialEq, Value)]
#[script(path = "host::ItemGrant", docs = "One structural grant value.")]
struct ItemGrant {
    count: i64,
    #[script(name = "item_name")]
    name: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Value)]
#[script(path = "host::GrantDecision")]
enum GrantDecision {
    Pending,
    #[script(name = "Accepted")]
    Granted {
        #[script(name = "total")]
        count: i64,
        item: String,
    },
}

#[derive(Debug, Eq, PartialEq, Value)]
#[script(path = "host::GrantBundle")]
struct GrantBundle {
    grants: Vec<ItemGrant>,
    decisions: BTreeMap<String, Option<GrantDecision>>,
    summary: (i64, bool),
}

#[derive(Debug, Eq, PartialEq, Value)]
#[script(path = "host::EmptyValue")]
struct EmptyValue {}

#[derive(Debug, Eq, PartialEq, Value)]
#[script(path = "host::ByteRecord")]
struct ByteRecord {
    bytes: Vec<u8>,
    chunks: Vec<Vec<u8>>,
}

#[script_function(name = "host::current_decision", effect = "pure")]
fn current_decision() -> GrantDecision {
    GrantDecision::Granted {
        count: 6,
        item: "token".to_owned(),
    }
}

#[script_function(name = "host::async_decision", effect = "pure")]
async fn async_decision() -> GrantDecision {
    GrantDecision::Pending
}

#[test]
fn value_derive_generates_schema_codec_and_unified_binding() {
    let desc = ItemGrant::vela_value_type_desc();
    assert_eq!(desc.key.name, "host::ItemGrant");
    assert_eq!(desc.kind, TypeKind::ScriptStruct);
    assert_eq!(desc.attrs.get("module"), Some("host"));
    assert_eq!(desc.fields.len(), 2);
    assert_eq!(desc.fields[0].name, "count");
    assert_eq!(desc.fields[0].type_hint.as_deref(), Some("i64"));
    assert_eq!(desc.fields[1].name, "item_name");
    assert_eq!(desc.fields[1].type_hint.as_deref(), Some("String"));

    let original = ItemGrant {
        count: 3,
        name: "potion".to_owned(),
    };
    let encoded = original.into_script_arg();
    assert_eq!(
        ItemGrant::from_script_arg(&encoded).expect("derived structural decode"),
        ItemGrant {
            count: 3,
            name: "potion".to_owned(),
        }
    );

    let engine = Engine::builder()
        .register_rust_type::<ItemGrant>(ItemGrant::vela_type_binding())
        .build()
        .expect("derived Value binding should seal");
    let type_bindings = engine.type_bindings();
    let binding = type_bindings
        .get_for::<ItemGrant>()
        .expect("derived binding should use typed lookup");
    assert_eq!(binding.storage, StoragePolicy::Value);
    assert_eq!(binding.key, desc.key);
}

#[test]
fn empty_value_struct_has_an_unambiguous_record_codec() {
    let encoded = EmptyValue {}.into_script_arg();

    assert_eq!(
        EmptyValue::from_script_arg(&encoded).expect("empty structural decode"),
        EmptyValue {}
    );
}

#[test]
fn byte_vector_fields_use_the_runtime_bytes_contract() {
    let desc = ByteRecord::vela_value_type_desc();
    assert_eq!(desc.fields[0].type_hint.as_deref(), Some("Bytes"));
    assert_eq!(desc.fields[1].type_hint.as_deref(), Some("Array<Bytes>"));

    let engine = Engine::builder()
        .register_rust_value_closure::<ByteRecord>()
        .build()
        .expect("byte record binding should seal");
    let program = engine
        .compile_source(
            r#"
fn make_bytes() {
    return host::ByteRecord {
        bytes: b"",
        chunks: [b""],
    };
}
"#,
        )
        .expect("derived byte fields should accept Vela Bytes literals");
    let codec = engine
        .type_bindings()
        .value_codec::<ByteRecord>()
        .expect("derived byte record codec");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let result = runtime
        .call("make_bytes", CallArgs::new(), CallOptions::unbounded())
        .expect("script should construct the byte record");
    let result = runtime
        .value_to_owned(&result)
        .expect("byte record should materialize");

    assert_eq!(
        codec.decode(&result).expect("byte fields should decode"),
        ByteRecord {
            bytes: Vec::new(),
            chunks: vec![Vec::new()],
        }
    );
}

#[test]
fn value_derive_registers_its_complete_nested_type_closure() {
    let engine = Engine::builder()
        .register_rust_value_closure::<GrantBundle>()
        .build()
        .expect("derived Value should recursively register all concrete dependencies");
    let bindings = engine.type_bindings();

    assert!(bindings.get_for::<GrantBundle>().is_some());
    assert!(bindings.get_for::<ItemGrant>().is_some());
    assert!(bindings.get_for::<GrantDecision>().is_some());
    assert!(bindings.get_for::<Vec<ItemGrant>>().is_some());
    assert!(bindings.get_for::<Option<GrantDecision>>().is_some());
    assert!(
        bindings
            .get_for::<BTreeMap<String, Option<GrantDecision>>>()
            .is_some()
    );
    assert!(bindings.get_for::<(i64, bool)>().is_some());
    assert!(bindings.get_for::<String>().is_some());
    assert!(bindings.get_for::<i64>().is_some());
    assert!(bindings.get_for::<bool>().is_some());

    let codec = bindings
        .value_codec::<GrantBundle>()
        .expect("root derived Value codec");
    let value = GrantBundle {
        grants: vec![ItemGrant {
            count: 4,
            name: "token".to_owned(),
        }],
        decisions: BTreeMap::from([(
            "token".to_owned(),
            Some(GrantDecision::Granted {
                count: 4,
                item: "token".to_owned(),
            }),
        )]),
        summary: (4, true),
    };
    let owned = codec.encode(value);
    assert_eq!(
        codec.decode(&owned),
        Ok(GrantBundle {
            grants: vec![ItemGrant {
                count: 4,
                name: "token".to_owned(),
            }],
            decisions: BTreeMap::from([(
                "token".to_owned(),
                Some(GrantDecision::Granted {
                    count: 4,
                    item: "token".to_owned(),
                }),
            )]),
            summary: (4, true),
        })
    );
}

#[test]
fn derived_value_round_trips_through_real_vela_execution() {
    let engine = Engine::builder()
        .register_rust_type::<ItemGrant>(ItemGrant::vela_type_binding())
        .build()
        .expect("derived Value binding should seal");
    let program = engine
        .compile_source(
            r#"
fn increase(value: host::ItemGrant) {
    return host::ItemGrant {
        count: value.count + 2,
        item_name: value.item_name,
    };
}
"#,
        )
        .expect("derived Value schema should compile");
    let codec = engine
        .type_bindings()
        .value_codec::<ItemGrant>()
        .expect("derived binding should install its structural codec");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let result = runtime
        .call(
            "increase",
            CallArgs::from_positional([codec.encode(ItemGrant {
                count: 5,
                name: "elixir".to_owned(),
            })]),
            CallOptions::unbounded(),
        )
        .expect("script should transform the derived Value");
    let result = runtime
        .value_to_owned(&result)
        .expect("script value should materialize");

    assert_eq!(
        codec.decode(&result).expect("derived codec should decode"),
        ItemGrant {
            count: 7,
            name: "elixir".to_owned(),
        }
    );
    assert!(matches!(result, OwnedValue::Record { .. }));
}

#[test]
fn enum_value_derive_generates_schema_and_structural_codec() {
    let desc = GrantDecision::vela_value_type_desc();
    assert_eq!(desc.key.name, "host::GrantDecision");
    assert_eq!(desc.kind, TypeKind::ScriptEnum);
    assert_eq!(desc.variants.len(), 2);
    assert_eq!(desc.variants[0].name, "Pending");
    assert!(desc.variants[0].fields.is_empty());
    assert_eq!(desc.variants[1].name, "Accepted");
    assert_eq!(desc.variants[1].fields[0].name, "total");
    assert_eq!(desc.variants[1].fields[1].name, "item");

    for original in [
        GrantDecision::Pending,
        GrantDecision::Granted {
            count: 4,
            item: "key".to_owned(),
        },
    ] {
        let encoded = original.clone().into_script_arg();
        let decoded = GrantDecision::from_script_arg(&encoded).expect("derived enum decode");
        assert_eq!(decoded, original);
    }
}

#[test]
fn derived_enum_round_trips_through_real_vela_match() {
    let engine = Engine::builder()
        .register_rust_type::<GrantDecision>(GrantDecision::vela_type_binding())
        .build()
        .expect("derived enum binding should seal");
    let program = engine
        .compile_source(
            r#"
fn increase(value: host::GrantDecision) {
    return match value {
        host::GrantDecision::Pending {} => host::GrantDecision::Pending {},
        host::GrantDecision::Accepted { total, item } => host::GrantDecision::Accepted {
            total: total + 3,
            item: item,
        },
    };
}
"#,
        )
        .expect("derived enum schema should compile");
    let codec = engine
        .type_bindings()
        .value_codec::<GrantDecision>()
        .expect("derived enum binding should install its structural codec");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let result = runtime
        .call(
            "increase",
            CallArgs::from_positional([codec.encode(GrantDecision::Granted {
                count: 5,
                item: "gem".to_owned(),
            })]),
            CallOptions::unbounded(),
        )
        .expect("script should match and transform the derived enum");
    let result = runtime
        .value_to_owned(&result)
        .expect("script enum should materialize");

    assert_eq!(
        codec.decode(&result).expect("derived enum should decode"),
        GrantDecision::Granted {
            count: 8,
            item: "gem".to_owned(),
        }
    );
    assert!(matches!(result, OwnedValue::Enum { .. }));
}

#[test]
fn derived_enum_returned_by_rust_native_keeps_match_identity() {
    let builder =
        Engine::builder().register_rust_type::<GrantDecision>(GrantDecision::vela_type_binding());
    let engine = vela_register_native_function_current_decision(builder)
        .build()
        .expect("derived enum and native should seal together");
    let program = engine
        .compile_source(
            r#"
fn count_decision() {
    let value = host::current_decision();
    return match value {
        host::GrantDecision::Accepted { total, item } => total + item.len(),
        _ => 0,
    };
}
"#,
        )
        .expect("native enum result should compile against its binding");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let result = runtime
        .call("count_decision", CallArgs::new(), CallOptions::unbounded())
        .expect("native enum result should retain nominal match identity");

    assert_eq!(
        runtime.value_to_owned(&result),
        Ok(OwnedValue::from(11_i64))
    );
}

#[test]
fn derived_enum_returned_by_async_rust_native_keeps_match_identity() {
    let builder =
        Engine::builder().register_rust_type::<GrantDecision>(GrantDecision::vela_type_binding());
    let engine = vela_register_native_function_async_decision(builder)
        .build()
        .expect("derived enum and async native should seal together");
    let program = engine
        .compile_source(
            r#"
async fn is_pending() {
    let value = host::async_decision().await;
    return match value {
        host::GrantDecision::Pending {} => true,
        _ => false,
    };
}
"#,
        )
        .expect("async native enum result should compile against its binding");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let mut future = runtime.call_async("is_pending", CallArgs::new(), CallOptions::unbounded());
    let mut context = Context::from_waker(Waker::noop());
    let Poll::Ready(result) = Pin::new(&mut future).poll(&mut context) else {
        panic!("ready async enum native should complete in one poll");
    };
    let result = result.expect("async native enum result should retain nominal match identity");
    drop(future);

    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
}

use vela_common::StoragePolicy;
use vela_engine::args::{FromScriptArg, IntoScriptArg};
use vela_engine::engine::Engine;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_macros::Value;
use vela_reflect::registry::TypeKind;
use vela_vm::owned_value::OwnedValue;

#[derive(Debug, Eq, PartialEq, Value)]
#[script(path = "host::ItemGrant", docs = "One structural grant value.")]
struct ItemGrant {
    count: i64,
    #[script(name = "item_name")]
    name: String,
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

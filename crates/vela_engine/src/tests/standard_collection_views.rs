use std::collections::BTreeMap;

use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;
use crate::permission::Capability;
use crate::runtime::{CallArgs, CallOptions, Runtime};

#[test]
fn complex_map_group_by_keeps_live_child_host_refs() {
    let engine = Engine::builder()
        .capability(Capability::HostRead)
        .register_rust_value_closure::<BTreeMap<String, Vec<i64>>>()
        .build()
        .expect("nested standard Map binding should seal");
    let program = engine
        .compile_source(
            "fn group(values) { \
                 let grouped = values.group_by(|key, value| \
                     if value.len() >= 2 { \"many\" } else { key }); \
                 return grouped[\"many\"][\"alpha\"][1] + grouped[\"beta\"][\"beta\"][0]; \
             }",
        )
        .expect("complex Map group_by fixture should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let values = BTreeMap::from([
        ("alpha".to_owned(), vec![1_i64, 2]),
        ("beta".to_owned(), vec![3]),
    ]);
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);

    let result = runtime
        .call("group", args, CallOptions::unbounded())
        .expect("grouping should preserve complex child HostRefs");

    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(5)));
    assert_eq!(values["alpha"], vec![1, 2]);
    assert_eq!(values["beta"], vec![3]);
}

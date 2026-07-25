use vela_common::Capability;
use vela_engine::engine::Engine;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_macros::{ScriptHost, methods};
use vela_vm::owned_value::OwnedValue;

#[derive(Debug, ScriptHost)]
#[script(path = "config::server::Equipment")]
struct Equipment {
    id: i64,
}

#[methods(path = "config::server::Equipment")]
impl Equipment {
    pub fn id(&self) -> i64 {
        self.id
    }
}

#[derive(Debug, ScriptHost)]
#[script(path = "config::server::EquipmentTable")]
struct EquipmentTable {
    rows: Box<[Equipment]>,
}

#[methods(path = "config::server::EquipmentTable")]
impl EquipmentTable {
    #[script_method(host_collection)]
    pub fn values(&self) -> &[Equipment] {
        &self.rows
    }
}

#[test]
fn borrowed_host_object_slice_seals_and_compiles_as_a_collection_view() {
    let engine = Engine::builder()
        .capability(Capability::HostRead)
        .register_rust_type::<Equipment>(Equipment::vela_type_binding())
        .register_exports(Equipment::vela_inherent_exports())
        .register_rust_type::<EquipmentTable>(EquipmentTable::vela_type_binding())
        .register_exports(EquipmentTable::vela_inherent_exports())
        .build()
        .expect("host slice bindings seal");

    let program = engine
        .compile_source(
            r#"
pub fn count(table: EquipmentTable) -> i64 {
    let rows = table.values();
    if rows.len() == 0 {
        return 0;
    }
    let row = rows[0];
    return row.id() + rows.len();
}
"#,
        )
        .expect("host slice is visible as a typed read-only collection");

    let mut runtime = Runtime::new(engine, program).expect("host slice runtime");
    let table = EquipmentTable {
        rows: vec![Equipment { id: 41 }].into_boxed_slice(),
    };
    let result = runtime
        .call(
            "count",
            CallArgs::new().with_host_ref("table", &table),
            CallOptions::unbounded(),
        )
        .expect("host slice call");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(42)));
}

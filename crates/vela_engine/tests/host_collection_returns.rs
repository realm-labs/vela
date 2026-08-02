use vela_common::Capability;
use vela_engine::engine::Engine;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_macros::{ScriptHost, methods};
use vela_vm::owned_value::OwnedValue;

#[derive(Debug, ScriptHost)]
#[vela(path = "config::server::Equipment")]
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
#[vela(path = "config::server::EquipmentEntry")]
enum EquipmentEntry {
    Equipment(Box<Equipment>),
}

#[methods(path = "config::server::EquipmentEntry")]
impl EquipmentEntry {
    pub fn as_equipment(&self) -> Option<&Equipment> {
        match self {
            Self::Equipment(value) => Some(value),
        }
    }
}

#[derive(Debug, ScriptHost)]
#[vela(path = "config::server::EquipmentTable")]
struct EquipmentTable {
    rows: Box<[EquipmentEntry]>,
}

#[methods(path = "config::server::EquipmentTable")]
impl EquipmentTable {
    #[vela(host_collection)]
    pub fn values(&self) -> &[EquipmentEntry] {
        &self.rows
    }
}

#[test]
fn borrowed_host_object_slice_seals_and_compiles_as_a_collection_view() {
    let engine = Engine::builder()
        .capability(Capability::HostRead)
        .install_generated_type::<Equipment>()
        .install_registration(Equipment::vela_methods())
        .install_generated_type::<EquipmentEntry>()
        .install_registration(EquipmentEntry::vela_methods())
        .install_generated_type::<EquipmentTable>()
        .install_registration(EquipmentTable::vela_methods())
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
    let entry = rows[0];
    let row = entry.as_equipment()?;
    return row.id() + rows.len();
}
"#,
        )
        .expect("host slice is visible as a typed read-only collection");

    let mut runtime = Runtime::new(engine, program).expect("host slice runtime");
    let table = EquipmentTable {
        rows: vec![EquipmentEntry::Equipment(Box::new(Equipment { id: 41 }))].into_boxed_slice(),
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

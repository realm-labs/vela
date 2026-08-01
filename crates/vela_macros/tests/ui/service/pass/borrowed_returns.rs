use vela_macros::{ScriptHost, Value, service};

#[derive(ScriptHost)]
#[vela(path = "coverage::Row")]
pub struct Row {
    value: i64,
}

#[derive(ScriptHost)]
#[vela(path = "coverage::Table")]
pub struct Table {
    row: Row,
    values: Vec<i64>,
}

#[derive(Clone, Value)]
#[vela(path = "coverage::ServiceError")]
pub struct ServiceError {
    message: String,
}

#[service(path = "coverage::lookup")]
pub trait LookupService: Send + Sync {
    fn shared<'a>(&self, row: &'a Row) -> &'a Row;
    fn exclusive<'a>(&self, row: &'a mut Row) -> &'a mut Row;
    fn values<'a>(&self, values: &'a [i64]) -> &'a [i64];
    fn optional<'a>(&self, row: &'a Row) -> Option<&'a Row>;
    fn checked<'a>(&self, row: &'a Row) -> Result<&'a Row, ServiceError>;
}

fn main() {
    let engine = __vela_register_service_LookupService(
        vela_engine::engine::Engine::builder()
            .register_type::<Row>()
            .register_type::<Table>(),
    )
    .build()
    .unwrap();
    __vela_service_schema_LookupService(
        &engine.type_bindings(),
        vela_engine::native::EffectSet::task_spawn(),
    )
    .unwrap();
}

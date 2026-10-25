use vela_macros::export;

struct Table;
struct Row;

#[export(path = "host::value")]
pub async fn value() -> i64 {
    1
}

#[export(path = "host::lookup")]
pub async fn lookup(_table: &Table) -> Option<&Row> {
    todo!()
}

#[export(path = "host::context")]
pub async fn context(_context: &mut vela_engine::context::NativeCallContext<'_, '_>) {}

#[export(path = "host::collection")]
pub async fn collection(_values: &[i64]) -> i64 {
    1
}

fn main() {}

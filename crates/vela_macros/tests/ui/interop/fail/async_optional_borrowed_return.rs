use vela_macros::export;

struct Table;
struct Row;

#[export(path = "host::lookup")]
pub async fn lookup(_table: &Table) -> Option<&Row> {
    todo!()
}

fn main() {}

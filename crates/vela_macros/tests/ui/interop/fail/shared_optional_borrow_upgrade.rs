use vela_macros::export;

struct Table;
struct Row;

#[export(path = "host::lookup")]
pub fn lookup(_table: &Table) -> Option<&mut Row> {
    todo!()
}

fn main() {}

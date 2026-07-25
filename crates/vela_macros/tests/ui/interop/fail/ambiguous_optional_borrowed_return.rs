use vela_macros::export;

struct Table;
struct Row;

#[export(path = "host::lookup")]
pub fn lookup(_left: &Table, _right: &Table) -> Option<&Row> {
    todo!()
}

fn main() {}

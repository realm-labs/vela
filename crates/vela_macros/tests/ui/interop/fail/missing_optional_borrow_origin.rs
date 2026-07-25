use vela_macros::export;

struct Row;

#[export(path = "host::current")]
pub fn current() -> Option<&'static Row> {
    todo!()
}

fn main() {}

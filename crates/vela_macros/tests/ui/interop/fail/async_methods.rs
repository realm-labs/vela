use vela_macros::methods;

struct Table;
struct Row;

#[methods(path = "host::Table")]
impl Table {
    pub async fn value(&self) -> i64 {
        1
    }
}

#[methods(path = "host::Table")]
impl Table {
    #[vela]
    pub async fn lookup(&self) -> Option<&Row> {
        todo!()
    }
}

fn main() {}

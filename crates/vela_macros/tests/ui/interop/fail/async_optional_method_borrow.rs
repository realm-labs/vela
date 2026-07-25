use vela_macros::methods;

struct Table;
struct Row;

#[methods(path = "host::Table")]
impl Table {
    pub async fn get(&self) -> Option<&Row> {
        todo!()
    }
}

fn main() {}

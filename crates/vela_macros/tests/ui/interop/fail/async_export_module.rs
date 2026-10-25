use vela_macros::export_module;

#[export_module(path = "host")]
mod exports {
    pub async fn value() -> i64 {
        1
    }
}

fn main() {}

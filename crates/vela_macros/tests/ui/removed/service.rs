#[vela_macros::service(path = "host::rules")]
pub trait Rules: Send + Sync {
    fn apply(&self, amount: i64) -> i64;
}

fn main() {}

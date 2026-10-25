use vela_macros::trait_export;

#[trait_export(path = "host::Lookup")]
pub trait Lookup {
    async fn value(&self) -> i64;
}

fn main() {}

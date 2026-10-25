use vela_macros::export_external_trait_impl;

struct Table;

trait Lookup {
    async fn value(&self) -> i64;
}

export_external_trait_impl! {
    type Table;
    trait Lookup as "host::Lookup";
    async fn value(&self) -> i64;
}

fn main() {}

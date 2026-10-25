use vela_macros::Value;

#[derive(Value)]
#[vela(path = "removed::Projected", fields)]
struct Projected { count: i64 }

#[derive(Value)]
#[vela(path = "removed::HostField")]
struct HostField {
    #[vela(host = "removed::Host")]
    count: i64,
}

#[derive(Value)]
#[vela(path = "removed::Deref")]
struct Deref {
    #[vela(deref)]
    count: i64,
}

fn main() {}

#[derive(vela_macros::ScriptHost)]
struct Host;

#[derive(vela_macros::ScriptReflect)]
struct Reflect;

#[vela_macros::external_host(path = "data::External", register = "register_external")]
impl Host {}

#[vela_macros::methods(path = "data::Host")]
impl Host {}

#[vela_macros::trait_export(path = "data::Protocol")]
trait Protocol {
    fn read(&self) -> i64;
}

vela_macros::export_external_trait_impl! {
    type Host;
    trait Protocol as "data::Protocol";
    fn read(&self) -> i64;
}

fn main() {}

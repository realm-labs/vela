use vela_macros::{Value, export, export_module, external_value_enum};
use vela_vm::error::VmResult;

#[derive(Value)]
#[vela(path = "data::Record")]
pub struct Record {
    count: i64,
}

#[export(path = "data::increase")]
pub fn increase(mut value: Record, amount: i64) -> VmResult<Record> {
    value.count += amount;
    Ok(value)
}

#[export(path = "data::inspect")]
pub fn inspect(value: &Record, name: &str) -> String {
    format!("{}:{name}", value.count)
}

#[export_module(path = "rules")]
mod rules {
    pub fn normalize(amount: i64) -> i64 {
        amount.max(0)
    }

    pub fn string_length(value: &str) -> i64 {
        value.len() as i64
    }

    fn helper() {}
}

enum Quality { Normal, Rare }

external_value_enum! {
    path = "data::Quality",
    ty = Quality,
    variants = [Normal, Rare],
}

fn main() {
    let _ = vela_callable_contract_increase();
    let _ = vela_function_increase();
    let _ = vela_callable_contract_inspect();
    let _ = vela_function_inspect();
    let _ = rules::vela_module();
}

use vela_macros::{export, export_module};
use vela_vm::error::VmResult;

pub struct Record;

#[export(path = "removed::slice")]
pub fn slice(_record: &Record) -> &[i64] { todo!() }

#[export(path = "removed::nested")]
pub fn nested(_record: &Record) -> Vec<&Record> { todo!() }

#[export(path = "removed::result")]
pub fn result(_record: &Record) -> VmResult<Option<(&Record, &Record)>> { todo!() }

#[export_module(path = "removed")]
mod exports {
    pub fn borrowed() -> Option<&'static str> { None }
}

fn main() {}

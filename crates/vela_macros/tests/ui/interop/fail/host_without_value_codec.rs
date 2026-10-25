use vela_macros::export;

pub struct HostData;

#[export(path = "removed::read")]
pub fn read(_host: &HostData) -> i64 { 0 }

#[export(path = "removed::owned")]
pub fn owned(_host: HostData) -> i64 { 0 }

fn main() {}

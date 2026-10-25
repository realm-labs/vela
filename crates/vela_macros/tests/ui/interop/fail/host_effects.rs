use vela_macros::export;

#[export(path = "removed::read", effects(host_read))]
pub fn read() {}

#[export(path = "removed::write", effects(host_write))]
pub fn write() {}

fn main() {}

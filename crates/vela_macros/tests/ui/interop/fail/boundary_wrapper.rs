use vela_macros::export;

#[export(path = "game::bad")]
pub fn bad(_player: vela_host::path::HostRef) {}

fn main() {}

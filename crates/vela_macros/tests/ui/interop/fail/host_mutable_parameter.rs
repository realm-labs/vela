use vela_macros::export;

struct Host;

#[export(path = "removed::mutate")]
pub fn mutate(_host: &mut Host) {}

fn main() {}

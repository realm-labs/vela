use vela_macros::export;

#[export(path = "game::name")]
pub fn name(value: &str) -> &str {
    value
}

fn main() {}

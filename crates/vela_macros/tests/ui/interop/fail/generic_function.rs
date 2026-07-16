use vela_macros::export;

#[export(path = "game::identity")]
pub fn identity<T>(value: T) -> T {
    value
}

fn main() {}

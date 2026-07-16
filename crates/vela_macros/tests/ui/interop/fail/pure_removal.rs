use vela_macros::export;

struct Player;

#[export(path = "game::grant", effects(pure))]
pub fn grant(_player: &mut Player) {}

fn main() {}

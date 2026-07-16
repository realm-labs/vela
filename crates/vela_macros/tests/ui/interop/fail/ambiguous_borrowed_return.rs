use vela_macros::export;

struct Service;
struct Player;

#[export(path = "game::player")]
pub fn player(_left: &Service, _right: &Service) -> &Player {
    todo!()
}

fn main() {}

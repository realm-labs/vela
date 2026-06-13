use std::error::Error;

use vela_examples::game_server;

fn main() -> Result<(), Box<dyn Error>> {
    game_server::run_script("level_up.vela", include_str!("level_up.vela"))
}

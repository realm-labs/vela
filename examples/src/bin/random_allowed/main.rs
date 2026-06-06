use std::error::Error;

use vela_examples::game_server;

fn main() -> Result<(), Box<dyn Error>> {
    game_server::run_script_with_random(game_server::script("random_allowed.vela"))
}

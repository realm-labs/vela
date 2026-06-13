use std::error::Error;

use vela_examples::game_server;

fn main() -> Result<(), Box<dyn Error>> {
    game_server::run_script(
        "gameplay_helpers.vela",
        include_str!("gameplay_helpers.vela"),
    )
}

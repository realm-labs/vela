use std::error::Error;

use vela_examples::game_server;

fn main() -> Result<(), Box<dyn Error>> {
    game_server::run_script("time_clock.vela", include_str!("time_clock.vela"))
}

use std::error::Error;

use vela_examples::game_server;

fn main() -> Result<(), Box<dyn Error>> {
    game_server::run_script("context_event.vela", include_str!("context_event.vela"))
}

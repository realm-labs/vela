use std::error::Error;

use vela_examples::game_server;

fn main() -> Result<(), Box<dyn Error>> {
    game_server::run_script("quest_progress.vela", include_str!("quest_progress.vela"))
}

use std::error::Error;

use vela_examples::game_server;

fn main() -> Result<(), Box<dyn Error>> {
    game_server::run_script(
        "monster_kill_reward.vela",
        include_str!("monster_kill_reward.vela"),
    )
}

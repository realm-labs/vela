use std::error::Error;

use vela_examples::{expect_error, game_server};

fn main() -> Result<(), Box<dyn Error>> {
    expect_error(
        || {
            game_server::run_script_with_stale_player(
                "stale_host_ref.vela",
                include_str!("stale_host_ref.vela"),
            )
        },
        "StaleGeneration",
    )
}

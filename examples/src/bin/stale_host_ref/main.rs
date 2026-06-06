use std::error::Error;

use vela_examples::{expect_error, game_server};

fn main() -> Result<(), Box<dyn Error>> {
    let path = game_server::script("stale_host_ref.vela");
    expect_error(
        || game_server::run_script_with_stale_player(&path),
        "StaleGeneration",
    )
}

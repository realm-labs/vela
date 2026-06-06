use std::error::Error;

use vela_examples::{expect_error, game_server};

fn main() -> Result<(), Box<dyn Error>> {
    let path = game_server::script("host_compound_write_denied.vela");
    expect_error(
        || game_server::run_script_with_denied_player_level_write(&path),
        "action: \"write\"",
    )
}

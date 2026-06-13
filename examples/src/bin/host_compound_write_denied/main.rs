use std::error::Error;

use vela_examples::{expect_error, game_server};

fn main() -> Result<(), Box<dyn Error>> {
    expect_error(
        || {
            game_server::run_script_with_denied_player_level_write(
                "host_compound_write_denied.vela",
                include_str!("host_compound_write_denied.vela"),
            )
        },
        "action: \"write\"",
    )
}

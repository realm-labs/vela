use std::error::Error;

use vela_examples::{expect_error, game_server};

fn main() -> Result<(), Box<dyn Error>> {
    expect_error(
        || {
            game_server::run_script(
                "host_read_only_denied.vela",
                include_str!("host_read_only_denied.vela"),
            )
        },
        "field `Player.id` is read-only for script writes",
    )
}

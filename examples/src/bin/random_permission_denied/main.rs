use std::error::Error;

use vela_examples::{expect_error, game_server};

fn main() -> Result<(), Box<dyn Error>> {
    expect_error(
        || {
            game_server::run_script(
                "random_permission_denied.vela",
                include_str!("random_permission_denied.vela"),
            )
        },
        "native `math::random` requires capability `random`",
    )
}

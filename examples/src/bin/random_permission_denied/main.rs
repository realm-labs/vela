use std::error::Error;

use vela_examples::{expect_error, game_server};

fn main() -> Result<(), Box<dyn Error>> {
    let path = game_server::script("random_permission_denied.vela");
    expect_error(
        || game_server::run_script(&path),
        "native `math::random` requires capability `random`",
    )
}

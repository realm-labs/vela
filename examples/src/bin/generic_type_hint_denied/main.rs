use std::error::Error;

use vela_examples::{expect_error, game_server};

fn main() -> Result<(), Box<dyn Error>> {
    let path = game_server::script("generic_type_hint_denied.vela");
    expect_error(
        || game_server::run_script(&path),
        "script type hints do not support generics",
    )
}

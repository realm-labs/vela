use std::error::Error;

use vela_examples::{expect_error, game_server};

fn main() -> Result<(), Box<dyn Error>> {
    expect_error(
        || {
            game_server::run_script(
                "generic_type_hint_denied.vela",
                include_str!("generic_type_hint_denied.vela"),
            )
        },
        "script type hints do not support generics",
    )
}

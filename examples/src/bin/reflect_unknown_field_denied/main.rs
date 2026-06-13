use std::error::Error;

use vela_examples::{expect_error, game_server};

fn main() -> Result<(), Box<dyn Error>> {
    expect_error(
        || {
            game_server::run_script(
                "reflect_unknown_field_denied.vela",
                include_str!("reflect_unknown_field_denied.vela"),
            )
        },
        "unknown reflected field `leve`",
    )
}

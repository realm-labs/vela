use std::error::Error;

use vela_examples::{expect_error, game_server};

fn main() -> Result<(), Box<dyn Error>> {
    expect_error(
        || {
            game_server::run_script(
                "bad_schema_duplicate_field.vela",
                include_str!("bad_schema_duplicate_field.vela"),
            )
        },
        "duplicate field `item_id`",
    )
}

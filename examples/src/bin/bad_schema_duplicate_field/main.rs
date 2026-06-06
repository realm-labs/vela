use std::error::Error;

use vela_examples::{expect_error, game_server};

fn main() -> Result<(), Box<dyn Error>> {
    let path = game_server::script("bad_schema_duplicate_field.vela");
    expect_error(
        || game_server::run_script(&path),
        "duplicate field `item_id`",
    )
}

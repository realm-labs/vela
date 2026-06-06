use std::error::Error;

use vela_examples::{expect_error, game_server};

fn main() -> Result<(), Box<dyn Error>> {
    let path = game_server::script("reflect_schema_mutation_denied.vela");
    expect_error(
        || game_server::run_script(&path),
        "invalid reflection target",
    )
}

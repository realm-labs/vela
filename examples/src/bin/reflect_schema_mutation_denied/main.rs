use std::error::Error;

use vela_examples::{expect_error, game_server};

fn main() -> Result<(), Box<dyn Error>> {
    expect_error(
        || {
            game_server::run_script(
                "reflect_schema_mutation_denied.vela",
                include_str!("reflect_schema_mutation_denied.vela"),
            )
        },
        "invalid reflection target",
    )
}

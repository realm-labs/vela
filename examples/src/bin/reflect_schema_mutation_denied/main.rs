use std::error::Error;

use vela_examples::{expect_error, gameplay::GameScript};

const SOURCE_LABEL: &str = "reflect_schema_mutation_denied.vela";
const SOURCE: &str = include_str!("reflect_schema_mutation_denied.vela");

fn main() -> Result<(), Box<dyn Error>> {
    expect_error(
        || {
            GameScript::new(SOURCE_LABEL, SOURCE)
                .player()
                .reflection()
                .run()
        },
        "invalid reflection target",
    )
}

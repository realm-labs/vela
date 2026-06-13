use std::error::Error;

use vela_examples::{expect_error, gameplay::GameScript};

const SOURCE_LABEL: &str = "bad_schema_duplicate_field.vela";
const SOURCE: &str = include_str!("bad_schema_duplicate_field.vela");

fn main() -> Result<(), Box<dyn Error>> {
    expect_error(
        || GameScript::new(SOURCE_LABEL, SOURCE).run(),
        "duplicate field `item_id`",
    )
}

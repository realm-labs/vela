use std::error::Error;

use vela_examples::{expect_error, gameplay::GameScript};

const SOURCE_LABEL: &str = "generic_type_hint_denied.vela";
const SOURCE: &str = include_str!("generic_type_hint_denied.vela");

fn main() -> Result<(), Box<dyn Error>> {
    expect_error(
        || GameScript::new(SOURCE_LABEL, SOURCE).run(),
        "script type hints do not support generics",
    )
}

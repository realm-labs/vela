use std::error::Error;

use vela_examples::{expect_error, gameplay::GameScript};

const SOURCE_LABEL: &str = "reflect_unknown_field_denied.vela";
const SOURCE: &str = include_str!("reflect_unknown_field_denied.vela");

fn main() -> Result<(), Box<dyn Error>> {
    expect_error(
        || {
            GameScript::new(SOURCE_LABEL, SOURCE)
                .player()
                .host_read()
                .reflection()
                .run()
        },
        "unknown reflected field `leve`",
    )
}

use std::error::Error;

use vela_examples::{expect_error, gameplay::GameScript};

const SOURCE_LABEL: &str = "host_read_only_denied.vela";
const SOURCE: &str = include_str!("host_read_only_denied.vela");

fn main() -> Result<(), Box<dyn Error>> {
    expect_error(
        || {
            GameScript::new(SOURCE_LABEL, SOURCE)
                .player()
                .host_read()
                .host_write()
                .run()
        },
        "field `Player.id` is read-only for script writes",
    )
}

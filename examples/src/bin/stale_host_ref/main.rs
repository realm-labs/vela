use std::error::Error;

use vela_examples::{expect_error, gameplay::GameScript};

const SOURCE_LABEL: &str = "stale_host_ref.vela";
const SOURCE: &str = include_str!("stale_host_ref.vela");

fn main() -> Result<(), Box<dyn Error>> {
    expect_error(
        || {
            GameScript::new(SOURCE_LABEL, SOURCE)
                .player()
                .host_read()
                .stale_player_arg()
                .run()
        },
        "StaleGeneration",
    )
}

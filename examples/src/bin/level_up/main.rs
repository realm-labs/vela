use std::error::Error;

use vela_examples::gameplay::GameScript;

const SOURCE_LABEL: &str = "level_up.vela";
const SOURCE: &str = include_str!("level_up.vela");

fn main() -> Result<(), Box<dyn Error>> {
    GameScript::new(SOURCE_LABEL, SOURCE)
        .player()
        .host_read()
        .host_write()
        .run()
}

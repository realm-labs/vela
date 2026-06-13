use std::error::Error;

use vela_examples::gameplay::GameScript;

const SOURCE_LABEL: &str = "reflect_debug.vela";
const SOURCE: &str = include_str!("reflect_debug.vela");

fn main() -> Result<(), Box<dyn Error>> {
    GameScript::new(SOURCE_LABEL, SOURCE)
        .context()
        .player()
        .inventory()
        .quest()
        .config()
        .reward()
        .host_read()
        .host_write()
        .event_emit()
        .time()
        .random_function()
        .reflection()
        .run()
}

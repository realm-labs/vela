use std::error::Error;

use vela_examples::gameplay::GameScript;

const SOURCE_LABEL: &str = "context_event.vela";
const SOURCE: &str = include_str!("context_event.vela");

fn main() -> Result<(), Box<dyn Error>> {
    GameScript::new(SOURCE_LABEL, SOURCE)
        .context()
        .player()
        .host_read()
        .event_emit()
        .run()
}

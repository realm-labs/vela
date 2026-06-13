use std::error::Error;

use vela_examples::gameplay::GameScript;

const SOURCE_LABEL: &str = "quest_progress.vela";
const SOURCE: &str = include_str!("quest_progress.vela");

fn main() -> Result<(), Box<dyn Error>> {
    GameScript::new(SOURCE_LABEL, SOURCE)
        .context()
        .player()
        .monster()
        .quest()
        .host_read()
        .host_write()
        .event_emit()
        .run()
}

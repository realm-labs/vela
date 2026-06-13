use std::error::Error;

use vela_examples::gameplay::GameScript;

const SOURCE_LABEL: &str = "monster_kill_reward.vela";
const SOURCE: &str = include_str!("monster_kill_reward.vela");

fn main() -> Result<(), Box<dyn Error>> {
    GameScript::new(SOURCE_LABEL, SOURCE)
        .context()
        .player()
        .monster()
        .inventory()
        .quest()
        .config()
        .host_read()
        .host_write()
        .event_emit()
        .run()
}

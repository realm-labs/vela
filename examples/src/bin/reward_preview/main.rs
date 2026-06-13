use std::error::Error;

use vela_examples::gameplay::GameScript;

const SOURCE_LABEL: &str = "reward_preview.vela";
const SOURCE: &str = include_str!("reward_preview.vela");

fn main() -> Result<(), Box<dyn Error>> {
    GameScript::new(SOURCE_LABEL, SOURCE)
        .context()
        .player()
        .monster()
        .reward()
        .host_read()
        .run()
}

use std::error::Error;

use vela_examples::gameplay::GameScript;

const SOURCE_LABEL: &str = "gameplay_helpers.vela";
const SOURCE: &str = include_str!("gameplay_helpers.vela");

fn main() -> Result<(), Box<dyn Error>> {
    GameScript::new(SOURCE_LABEL, SOURCE).run()
}

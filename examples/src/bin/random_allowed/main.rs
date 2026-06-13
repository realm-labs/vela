use std::error::Error;

use vela_examples::gameplay::GameScript;

const SOURCE_LABEL: &str = "random_allowed.vela";
const SOURCE: &str = include_str!("random_allowed.vela");

fn main() -> Result<(), Box<dyn Error>> {
    GameScript::new(SOURCE_LABEL, SOURCE).random().run()
}

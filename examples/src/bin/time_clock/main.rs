use std::error::Error;

use vela_examples::gameplay::GameScript;

const SOURCE_LABEL: &str = "time_clock.vela";
const SOURCE: &str = include_str!("time_clock.vela");

fn main() -> Result<(), Box<dyn Error>> {
    GameScript::new(SOURCE_LABEL, SOURCE).time().run()
}

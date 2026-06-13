use std::error::Error;

use vela_examples::{expect_error, gameplay::GameScript};

const SOURCE_LABEL: &str = "random_permission_denied.vela";
const SOURCE: &str = include_str!("random_permission_denied.vela");

fn main() -> Result<(), Box<dyn Error>> {
    expect_error(
        || {
            GameScript::new(SOURCE_LABEL, SOURCE)
                .random_function()
                .run()
        },
        "native `math::random` requires capability `random`",
    )
}

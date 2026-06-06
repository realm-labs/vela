use std::error::Error;

use vela_examples::{example_file, expect_error, hot_reload_demo};

fn main() -> Result<(), Box<dyn Error>> {
    let initial = example_file(
        "hot_reload_function_swap_invalid",
        "hot_reload_function_swap_v1.vela",
    );
    let invalid = example_file(
        "hot_reload_function_swap_invalid",
        "hot_reload_function_swap_invalid.vela",
    );
    expect_error(
        || hot_reload_demo::run(&initial, &invalid),
        "hot reload rejected: v0 unchanged",
    )
}

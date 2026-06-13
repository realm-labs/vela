use std::error::Error;

use vela_examples::{expect_error, hot_reload_demo};

fn main() -> Result<(), Box<dyn Error>> {
    expect_error(
        || {
            hot_reload_demo::run(
                "hot_reload_function_swap_v1.vela",
                include_str!("hot_reload_function_swap_v1.vela"),
                "hot_reload_function_swap_invalid.vela",
                include_str!("hot_reload_function_swap_invalid.vela"),
            )
        },
        "hot reload rejected: v0 unchanged",
    )
}

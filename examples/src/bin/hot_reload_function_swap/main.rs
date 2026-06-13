use std::error::Error;

use vela_examples::hot_reload_demo;

fn main() -> Result<(), Box<dyn Error>> {
    hot_reload_demo::run(
        "hot_reload_function_swap_v1.vela",
        include_str!("hot_reload_function_swap_v1.vela"),
        "hot_reload_function_swap_v2.vela",
        include_str!("hot_reload_function_swap_v2.vela"),
    )
}

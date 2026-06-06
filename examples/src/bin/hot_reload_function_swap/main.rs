use std::error::Error;

use vela_examples::{game_server, hot_reload_demo};

fn main() -> Result<(), Box<dyn Error>> {
    hot_reload_demo::run(
        game_server::script("hot_reload_function_swap_v1.vela"),
        game_server::script("hot_reload_function_swap_v2.vela"),
    )
}

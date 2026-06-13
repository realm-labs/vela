use std::error::Error;

use vela_examples::{expect_error, game_server};

fn main() -> Result<(), Box<dyn Error>> {
    expect_error(
        || {
            game_server::run_script_with_denied_context_emit_call(
                "host_call_permission_denied.vela",
                include_str!("host_call_permission_denied.vela"),
            )
        },
        "action: \"call\"",
    )
}

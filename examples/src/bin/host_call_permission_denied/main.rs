use std::error::Error;

use vela_examples::{expect_error, gameplay::GameScript};

const SOURCE_LABEL: &str = "host_call_permission_denied.vela";
const SOURCE: &str = include_str!("host_call_permission_denied.vela");

fn main() -> Result<(), Box<dyn Error>> {
    expect_error(
        || {
            GameScript::new(SOURCE_LABEL, SOURCE)
                .context()
                .event_emit()
                .deny_context_emit_call()
                .run()
        },
        "action: \"call\"",
    )
}

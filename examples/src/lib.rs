use std::error::Error;
use std::path::{Path, PathBuf};

pub mod diagnostics;
pub mod gameplay;
pub mod hot_reload_demo;

pub fn example_dir(example: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/bin")
        .join(example)
}

pub fn expect_error<F>(run: F, expected: &str) -> Result<(), Box<dyn Error>>
where
    F: FnOnce() -> Result<(), Box<dyn Error>>,
{
    match run() {
        Ok(()) => Err(format!("expected example error containing `{expected}`").into()),
        Err(error) => {
            let message = error.to_string();
            if !message.contains(expected) {
                return Err(format!(
                    "expected example error containing `{expected}`\nactual:\n{message}"
                )
                .into());
            }
            println!("{message}");
            Ok(())
        }
    }
}

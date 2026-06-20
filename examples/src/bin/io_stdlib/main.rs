use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use vela_engine::prelude::*;

fn main() -> Result<(), Box<dyn Error>> {
    let root = TempDir::new("vela_io_stdlib")?;
    std::fs::write(root.path().join("input.txt"), "hello from fs")?;

    let engine = Engine::builder()
        .with_standard_natives()
        .capability(Capability::IoRead)
        .capability(Capability::IoWrite)
        .with_stdio()
        .with_fs_io(root.path())
        .build()?;
    let program = engine.compile_source(include_str!("main.vela"))?;
    let mut runtime = Runtime::new(engine, program);

    let output = runtime.call("main", CallArgs::new(), CallOptions::unbounded())?;
    let written = std::fs::read_to_string(root.path().join("output.txt"))?;
    println!(
        "io_stdlib len={:?} output={written}",
        runtime.value_to_owned(&output)?
    );
    Ok(())
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> std::io::Result<Self> {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);

        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let sequence = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "{prefix}_{}_{}_{}",
            std::process::id(),
            suffix,
            sequence
        ));
        std::fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

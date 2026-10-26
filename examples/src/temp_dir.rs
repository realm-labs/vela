use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new() -> io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self::acquire((0..128).map(|_| {
            std::env::temp_dir().join(format!(
                "vela_examples_{}_{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ))
        }))
    }

    fn acquire(paths: impl IntoIterator<Item = PathBuf>) -> io::Result<Self> {
        for path in paths {
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "temporary directory collisions exhausted",
        ))
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collision_never_claims_or_removes_foreign_directory() {
        let parent = TempDir::new().expect("parent");
        let foreign = parent.path().join("foreign");
        fs::create_dir(&foreign).expect("foreign");
        fs::write(foreign.join("sentinel"), b"unchanged").expect("sentinel");
        assert!(TempDir::acquire([foreign.clone()]).is_err());
        let fresh = parent.path().join("fresh");
        let owned = TempDir::acquire([foreign.clone(), fresh.clone()]).expect("retry collision");
        assert_eq!(owned.path(), fresh);
        drop(owned);
        assert!(!fresh.exists());
        assert_eq!(
            fs::read(foreign.join("sentinel")).expect("foreign preserved"),
            b"unchanged"
        );
    }

    #[test]
    fn setup_error_and_unwind_remove_only_acquired_directory() {
        let parent = TempDir::new().expect("parent");
        let failed = parent.path().join("failed");
        let setup = || -> io::Result<()> {
            let owned = TempDir::acquire([failed.clone()])?;
            fs::write(owned.path().join("missing/script.vela"), b"source")
        };
        assert!(setup().is_err());
        assert!(!failed.exists());
        let unwound = parent.path().join("unwound");
        assert!(
            std::panic::catch_unwind(|| {
                let _owned = TempDir::acquire([unwound.clone()]).expect("acquire");
                panic!("setup failed after acquisition");
            })
            .is_err()
        );
        assert!(!unwound.exists());
        assert!(parent.path().exists());
    }

    #[test]
    fn concurrent_owners_have_distinct_roots() {
        let a = TempDir::new().expect("a");
        let b = TempDir::new().expect("b");
        assert_ne!(a.path(), b.path());
        drop(a);
        assert!(b.path().exists());
    }
}

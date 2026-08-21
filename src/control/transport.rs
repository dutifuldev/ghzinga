use std::{
    fs::{self, File, OpenOptions},
    io,
    path::Path,
};

use fs2::FileExt;

#[cfg(unix)]
#[path = "transport/unix.rs"]
mod platform;

#[cfg(windows)]
#[path = "transport/windows.rs"]
mod platform;

pub(crate) use platform::*;

pub(crate) struct SessionLock(File);

impl SessionLock {
    pub(crate) fn acquire(path: &Path) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(path)?;
        file.try_lock_exclusive().map_err(|error| {
            io::Error::new(
                io::ErrorKind::AddrInUse,
                format!("ghzinga session control endpoint is already active: {error}"),
            )
        })?;
        Ok(Self(file))
    }
}

impl Drop for SessionLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}

#[cfg(all(test, windows))]
mod tests {
    use std::io;

    use super::platform::endpoint_path;

    #[test]
    fn endpoint_rejects_invalid_session_ids() {
        let separator = endpoint_path(r"work\other").unwrap_err();
        let oversized = endpoint_path(&"a".repeat(65)).unwrap_err();

        assert_eq!(separator.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(oversized.kind(), io::ErrorKind::InvalidInput);
    }
}

#[cfg(unix)]
#[path = "transport/unix.rs"]
mod platform;

#[cfg(windows)]
#[path = "transport/windows.rs"]
mod platform;

pub(crate) use platform::*;

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

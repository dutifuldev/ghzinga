use std::{
    fs, io,
    os::unix::{
        fs::{MetadataExt, PermissionsExt},
        net::UnixStream as StdUnixStream,
    },
    path::PathBuf,
};

use tokio::net::{UnixListener, UnixStream};

use super::super::socket_path;

pub(crate) struct Listener {
    inner: UnixListener,
    cleanup_path: PathBuf,
    cleanup_device: u64,
    cleanup_inode: u64,
}

#[derive(Clone)]
pub(crate) struct Cleanup {
    path: PathBuf,
    device: u64,
    inode: u64,
}

impl Cleanup {
    pub(crate) fn remove(&self) {
        let Ok(metadata) = fs::metadata(&self.path) else {
            return;
        };
        if metadata.dev() == self.device && metadata.ino() == self.inode {
            let _ = fs::remove_file(&self.path);
        }
    }
}

impl Listener {
    pub(crate) fn bind(session_id: &str) -> io::Result<Self> {
        let path = socket_path(session_id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        if path.exists() {
            match StdUnixStream::connect(&path) {
                Ok(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::AddrInUse,
                        format!("ghzinga session socket already active: {}", path.display()),
                    ));
                }
                Err(_) => fs::remove_file(&path)?,
            }
        }
        let inner = UnixListener::bind(&path)?;
        if let Ok(metadata) = fs::metadata(&path) {
            let mut permissions = metadata.permissions();
            permissions.set_mode(0o600);
            let _ = fs::set_permissions(&path, permissions);
        }
        let metadata = fs::metadata(&path)?;
        Ok(Self {
            inner,
            cleanup_path: path,
            cleanup_device: metadata.dev(),
            cleanup_inode: metadata.ino(),
        })
    }

    pub(crate) async fn accept(&mut self) -> io::Result<UnixStream> {
        self.inner.accept().await.map(|(stream, _)| stream)
    }

    pub(crate) fn cleanup(&self) -> Cleanup {
        Cleanup {
            path: self.cleanup_path.clone(),
            device: self.cleanup_device,
            inode: self.cleanup_inode,
        }
    }

    pub(crate) fn auth_token(&self) -> Option<&str> {
        None
    }
}

pub(crate) async fn connect(session_id: &str) -> io::Result<(UnixStream, Option<String>)> {
    UnixStream::connect(socket_path(session_id))
        .await
        .map(|stream| (stream, None))
}

pub(crate) fn is_session_live(session_id: &str) -> bool {
    StdUnixStream::connect(socket_path(session_id)).is_ok()
}

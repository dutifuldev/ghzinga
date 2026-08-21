#[cfg(unix)]
mod platform {
    use std::{
        fs, io,
        os::unix::{fs::PermissionsExt, net::UnixStream as StdUnixStream},
    };

    use tokio::net::{UnixListener, UnixStream};

    use super::super::socket_path;

    pub(crate) struct Listener {
        inner: UnixListener,
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
                let _ = fs::set_permissions(path, permissions);
            }
            Ok(Self { inner })
        }

        pub(crate) async fn accept(&mut self) -> io::Result<UnixStream> {
            self.inner.accept().await.map(|(stream, _)| stream)
        }
    }

    pub(crate) async fn connect(session_id: &str) -> io::Result<UnixStream> {
        UnixStream::connect(socket_path(session_id)).await
    }

    pub(crate) fn is_session_live(session_id: &str) -> bool {
        StdUnixStream::connect(socket_path(session_id)).is_ok()
    }
}

#[cfg(windows)]
mod platform {
    use std::{io, mem, time::Duration};

    use super::super::runtime_dir;
    use tokio::{
        net::windows::named_pipe::{
            ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
        },
        time,
    };

    const ERROR_PIPE_BUSY_CODE: i32 = 231;
    const PIPE_RETRY_DELAY: Duration = Duration::from_millis(50);

    pub(crate) struct Listener {
        name: String,
        pending: NamedPipeServer,
    }

    impl Listener {
        pub(crate) fn bind(session_id: &str) -> io::Result<Self> {
            let name = pipe_name(session_id);
            let pending = ServerOptions::new()
                .first_pipe_instance(true)
                .create(&name)?;
            Ok(Self { name, pending })
        }

        pub(crate) async fn accept(&mut self) -> io::Result<NamedPipeServer> {
            self.pending.connect().await?;
            let next = ServerOptions::new().create(&self.name)?;
            Ok(mem::replace(&mut self.pending, next))
        }
    }

    pub(crate) async fn connect(session_id: &str) -> io::Result<NamedPipeClient> {
        let name = pipe_name(session_id);
        loop {
            match ClientOptions::new().open(&name) {
                Ok(client) => return Ok(client),
                Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY_CODE) => {
                    time::sleep(PIPE_RETRY_DELAY).await;
                }
                Err(error) => return Err(error),
            }
        }
    }

    pub(crate) fn is_session_live(session_id: &str) -> bool {
        ClientOptions::new().open(pipe_name(session_id)).is_ok()
    }

    fn pipe_name(session_id: &str) -> String {
        let mut runtime_hash = 0xcbf2_9ce4_8422_2325_u64;
        for byte in runtime_dir().as_os_str().to_string_lossy().bytes() {
            runtime_hash ^= u64::from(byte);
            runtime_hash = runtime_hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        format!(r"\\.\pipe\ghzinga-{runtime_hash:016x}-{session_id}")
    }
}

pub(crate) use platform::*;

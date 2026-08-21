use std::{
    ffi::{c_void, OsStr},
    fmt::Write as _,
    fs, io, iter, mem,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    ptr,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use tokio::{
    net::windows::named_pipe::{ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions},
    time,
};
use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::{
        Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW, PSECURITY_DESCRIPTOR,
        SECURITY_ATTRIBUTES,
    },
};

use super::{super::runtime_dir, SessionLock};

const ERROR_PIPE_BUSY_CODE: i32 = 231;
const SDDL_REVISION_1: u32 = 1;
const PIPE_RETRY_DELAY: Duration = Duration::from_millis(50);
const OWNER_ONLY_PIPE_SDDL: &str = "D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;OW)";

#[derive(Serialize, Deserialize)]
struct EndpointInfo {
    pipe_name: String,
    auth_token: String,
}

pub(crate) struct Listener {
    name: String,
    pending: NamedPipeServer,
    auth_token: String,
    cleanup: Option<Cleanup>,
}

pub(crate) struct Cleanup {
    path: PathBuf,
    auth_token: String,
    _lock: SessionLock,
}

impl Cleanup {
    pub(crate) fn remove(&self) {
        let Ok(endpoint) = read_endpoint_path(&self.path) else {
            return;
        };
        if endpoint.auth_token == self.auth_token {
            let _ = fs::remove_file(&self.path);
        }
    }
}

impl Listener {
    pub(crate) fn bind(session_id: &str) -> io::Result<Self> {
        let cleanup_path = endpoint_path(session_id)?;
        if let Some(parent) = cleanup_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let lock = SessionLock::acquire(&cleanup_path.with_extension("lock"))?;
        let name = format!(r"\\.\pipe\ghzinga-{}", random_hex()?);
        let auth_token = random_hex()?;
        let pending = create_server(&name, true)?;
        let endpoint = EndpointInfo {
            pipe_name: name.clone(),
            auth_token: auth_token.clone(),
        };
        publish_endpoint(&cleanup_path, &endpoint)?;
        Ok(Self {
            name,
            pending,
            auth_token,
            cleanup: Some(Cleanup {
                path: cleanup_path,
                auth_token: endpoint.auth_token,
                _lock: lock,
            }),
        })
    }

    pub(crate) async fn accept(&mut self) -> io::Result<NamedPipeServer> {
        self.pending.connect().await?;
        let next = create_server(&self.name, false)?;
        Ok(mem::replace(&mut self.pending, next))
    }

    pub(crate) fn take_cleanup(&mut self) -> io::Result<Cleanup> {
        self.cleanup
            .take()
            .ok_or_else(|| io::Error::other("control cleanup already taken"))
    }

    pub(crate) fn auth_token(&self) -> Option<&str> {
        Some(&self.auth_token)
    }
}

pub(crate) async fn connect(session_id: &str) -> io::Result<(NamedPipeClient, Option<String>)> {
    let endpoint = read_endpoint(session_id)?;
    loop {
        match ClientOptions::new().open(&endpoint.pipe_name) {
            Ok(client) => return Ok((client, Some(endpoint.auth_token))),
            Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY_CODE) => {
                time::sleep(PIPE_RETRY_DELAY).await;
            }
            Err(error) => return Err(error),
        }
    }
}

pub(crate) fn is_session_live(session_id: &str) -> bool {
    let Ok(endpoint) = read_endpoint(session_id) else {
        return false;
    };
    match ClientOptions::new().open(endpoint.pipe_name) {
        Ok(_) => true,
        Err(error) => error.raw_os_error() == Some(ERROR_PIPE_BUSY_CODE),
    }
}

pub(super) fn endpoint_path(session_id: &str) -> io::Result<PathBuf> {
    if session_id.is_empty()
        || session_id.len() > 64
        || !session_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "session id must be 1-64 ASCII letters, digits, underscores, or hyphens",
        ));
    }
    Ok(runtime_dir().join(format!("{session_id}.pipe")))
}

fn read_endpoint(session_id: &str) -> io::Result<EndpointInfo> {
    read_endpoint_path(&endpoint_path(session_id)?)
}

fn read_endpoint_path(path: &Path) -> io::Result<EndpointInfo> {
    let endpoint =
        serde_json::from_slice::<EndpointInfo>(&fs::read(path)?).map_err(io::Error::other)?;
    if !endpoint.pipe_name.starts_with(r"\\.\pipe\ghzinga-")
        || endpoint.auth_token.len() != 32
        || !endpoint
            .auth_token
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid ghzinga control endpoint",
        ));
    }
    Ok(endpoint)
}

fn publish_endpoint(path: &Path, endpoint: &EndpointInfo) -> io::Result<()> {
    let temp_path = path.with_extension(format!("{}.tmp", endpoint.auth_token));
    let _pending = PendingEndpoint(temp_path.clone());
    fs::write(
        &temp_path,
        serde_json::to_vec(endpoint).map_err(io::Error::other)?,
    )?;
    for _ in 0..3 {
        match fs::hard_link(&temp_path, path) {
            Ok(()) => return Ok(()),
            Err(_error) if path.exists() => {
                if endpoint_path_is_live(path) {
                    return Err(io::Error::new(
                        io::ErrorKind::AddrInUse,
                        "ghzinga session control endpoint is already active",
                    ));
                }
                match fs::remove_file(path) {
                    Ok(()) => {}
                    Err(remove_error) if remove_error.kind() == io::ErrorKind::NotFound => {}
                    Err(remove_error) => return Err(remove_error),
                }
            }
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AddrInUse,
        "ghzinga session control endpoint changed repeatedly",
    ))
}

fn endpoint_path_is_live(path: &Path) -> bool {
    let Ok(endpoint) = read_endpoint_path(path) else {
        return false;
    };
    match ClientOptions::new().open(endpoint.pipe_name) {
        Ok(_) => true,
        Err(error) => error.raw_os_error() == Some(ERROR_PIPE_BUSY_CODE),
    }
}

struct PendingEndpoint(PathBuf);

impl Drop for PendingEndpoint {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn random_hex() -> io::Result<String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|error| io::Error::other(error.to_string()))?;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").map_err(io::Error::other)?;
    }
    Ok(output)
}

fn create_server(name: &str, first_instance: bool) -> io::Result<NamedPipeServer> {
    let mut descriptor = OwnerOnlySecurityDescriptor::new()?;
    let mut attributes = descriptor.attributes();
    let mut options = ServerOptions::new();
    options
        .first_pipe_instance(first_instance)
        .reject_remote_clients(true);
    // SAFETY: [Category 8 — FFI boundary, Category 13 — unsafe contract]
    // `attributes` points to a live SECURITY_ATTRIBUTES value whose security
    // descriptor remains owned by `descriptor` for the duration of this call.
    unsafe {
        options.create_with_security_attributes_raw(
            name,
            ptr::from_mut(&mut attributes).cast::<c_void>(),
        )
    }
}

struct OwnerOnlySecurityDescriptor {
    raw: PSECURITY_DESCRIPTOR,
}

impl OwnerOnlySecurityDescriptor {
    fn new() -> io::Result<Self> {
        let sddl = OsStr::new(OWNER_ONLY_PIPE_SDDL)
            .encode_wide()
            .chain(iter::once(0))
            .collect::<Vec<_>>();
        let mut raw = ptr::null_mut();
        // SAFETY: [Category 8 — FFI boundary]
        // `sddl` is NUL-terminated and remains live for the call, while `raw`
        // points to writable storage for the descriptor allocated by Windows.
        let converted = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut raw,
                ptr::null_mut(),
            )
        };
        if converted == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { raw })
    }

    fn attributes(&mut self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.raw,
            bInheritHandle: 0,
        }
    }
}

impl Drop for OwnerOnlySecurityDescriptor {
    fn drop(&mut self) {
        // SAFETY: [Category 8 — FFI boundary, Category 12 — invalid free]
        // `raw` is the unique descriptor returned by LocalAlloc through
        // ConvertStringSecurityDescriptorToSecurityDescriptorW and is freed once.
        unsafe {
            LocalFree(self.raw.cast());
        }
    }
}

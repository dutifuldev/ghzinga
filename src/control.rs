use std::{
    env, fs, io,
    path::{Path, PathBuf},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::{mpsc::UnboundedSender, oneshot},
    task::JoinHandle,
};

use crate::domain::ResourceId;

pub const GZG_RUNTIME_HOME_ENV: &str = "GZG_RUNTIME_HOME";
const CONTROL_SCHEMA_VERSION: u32 = 1;
const CONTROL_TIMEOUT: Duration = Duration::from_millis(1200);

#[derive(Debug)]
pub struct RuntimeRequest {
    pub command: RuntimeCommand,
    pub reply: oneshot::Sender<ControlReply>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeCommand {
    Open {
        request_id: String,
        resource: ResourceId,
    },
    Set {
        request_id: String,
        key: String,
        value: String,
    },
}

impl RuntimeCommand {
    fn request_id(&self) -> &str {
        match self {
            Self::Open { request_id, .. } | Self::Set { request_id, .. } => request_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlReply {
    pub ok: bool,
    pub result: Option<String>,
    pub error: Option<String>,
}

impl ControlReply {
    pub fn ok(result: impl Into<String>) -> Self {
        Self {
            ok: true,
            result: Some(result.into()),
            error: None,
        }
    }

    pub fn error(error: impl Into<String>) -> Self {
        Self {
            ok: false,
            result: None,
            error: Some(error.into()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct WireCommand {
    schema_version: u32,
    id: String,
    method: String,
    #[serde(default)]
    resource: Option<String>,
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    value: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct WireReply {
    schema_version: u32,
    id: String,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

pub fn runtime_dir() -> PathBuf {
    runtime_dir_from_env(
        env::var_os(GZG_RUNTIME_HOME_ENV),
        env::var_os("XDG_RUNTIME_DIR"),
        env::var_os("UID"),
    )
}

fn runtime_dir_from_env(
    override_dir: Option<std::ffi::OsString>,
    xdg_runtime: Option<std::ffi::OsString>,
    uid: Option<std::ffi::OsString>,
) -> PathBuf {
    if let Some(path) = override_dir {
        return PathBuf::from(path);
    }
    if let Some(path) = xdg_runtime {
        return PathBuf::from(path).join("ghzinga");
    }
    let suffix = uid
        .and_then(|value| value.into_string().ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(stable_runtime_suffix);
    PathBuf::from("/tmp").join(format!("ghzinga-{suffix}"))
}

fn stable_runtime_suffix() -> String {
    #[cfg(unix)]
    {
        effective_user_id().to_string()
    }
    #[cfg(not(unix))]
    {
        std::process::id().to_string()
    }
}

#[cfg(unix)]
fn effective_user_id() -> u32 {
    use std::os::raw::c_uint;

    extern "C" {
        fn geteuid() -> c_uint;
    }

    // `geteuid` has no preconditions and returns the effective uid for this process.
    unsafe { geteuid() }
}

pub fn socket_path(session_id: &str) -> PathBuf {
    runtime_dir().join(format!("{session_id}.sock"))
}

pub struct ControlServer {
    path: PathBuf,
    task: JoinHandle<()>,
}

impl Drop for ControlServer {
    fn drop(&mut self) {
        self.task.abort();
        let _ = fs::remove_file(&self.path);
    }
}

pub fn start_server(
    session_id: &str,
    tx: UnboundedSender<RuntimeRequest>,
) -> io::Result<ControlServer> {
    let path = socket_path(session_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    remove_stale_socket(&path)?;
    let listener = UnixListener::bind(&path)?;
    set_owner_only_permissions(&path);
    let task = tokio::spawn(server_loop(listener, tx));
    Ok(ControlServer { path, task })
}

fn remove_stale_socket(path: &Path) -> io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    match std::os::unix::net::UnixStream::connect(path) {
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AddrInUse,
            format!("ghzinga session socket already active: {}", path.display()),
        )),
        Err(_) => fs::remove_file(path),
    }
}

fn set_owner_only_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if let Ok(metadata) = fs::metadata(path) {
            let mut permissions = metadata.permissions();
            permissions.set_mode(0o600);
            let _ = fs::set_permissions(path, permissions);
        }
    }
}

async fn server_loop(listener: UnixListener, tx: UnboundedSender<RuntimeRequest>) {
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            break;
        };
        let tx = tx.clone();
        tokio::spawn(async move {
            let _ = handle_connection(stream, tx).await;
        });
    }
}

async fn handle_connection(
    stream: UnixStream,
    tx: UnboundedSender<RuntimeRequest>,
) -> io::Result<()> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    let command = parse_wire_command(&line);
    let reply = match command {
        Ok(command) => {
            let request_id = command.request_id().to_string();
            let (reply_tx, reply_rx) = oneshot::channel();
            if tx
                .send(RuntimeRequest {
                    command,
                    reply: reply_tx,
                })
                .is_err()
            {
                wire_reply(
                    &request_id,
                    ControlReply::error("ghzinga UI is not accepting commands"),
                )
            } else {
                match tokio::time::timeout(CONTROL_TIMEOUT, reply_rx).await {
                    Ok(Ok(reply)) => wire_reply(&request_id, reply),
                    Ok(Err(_)) => wire_reply(
                        &request_id,
                        ControlReply::error("ghzinga UI dropped command"),
                    ),
                    Err(_) => wire_reply(
                        &request_id,
                        ControlReply::error("ghzinga UI command timed out"),
                    ),
                }
            }
        }
        Err((request_id, error)) => wire_reply(&request_id, ControlReply::error(error)),
    };
    let stream = reader.get_mut();
    stream
        .write_all(serde_json::to_string(&reply)?.as_bytes())
        .await?;
    stream.write_all(b"\n").await?;
    stream.flush().await?;
    Ok(())
}

fn parse_wire_command(line: &str) -> Result<RuntimeCommand, (String, String)> {
    let value = serde_json::from_str::<WireCommand>(line).map_err(|error| {
        (
            "unknown".to_string(),
            format!("invalid control command: {error}"),
        )
    })?;
    if value.schema_version != CONTROL_SCHEMA_VERSION {
        return Err((
            value.id,
            format!("unsupported control schema {}", value.schema_version),
        ));
    }
    match value.method.as_str() {
        "open" => {
            let resource = value.resource.ok_or_else(|| {
                (
                    value.id.clone(),
                    "open command requires resource".to_string(),
                )
            })?;
            let resource = ResourceId::parse(&resource)
                .map_err(|error| (value.id.clone(), error.to_string()))?;
            Ok(RuntimeCommand::Open {
                request_id: value.id,
                resource,
            })
        }
        "set" => {
            let key = value
                .key
                .ok_or_else(|| (value.id.clone(), "set command requires key".to_string()))?;
            let value_text = value
                .value
                .ok_or_else(|| (value.id.clone(), "set command requires value".to_string()))?;
            Ok(RuntimeCommand::Set {
                request_id: value.id,
                key,
                value: value_text,
            })
        }
        other => Err((value.id, format!("unknown control method `{other}`"))),
    }
}

fn wire_reply(id: &str, reply: ControlReply) -> WireReply {
    WireReply {
        schema_version: CONTROL_SCHEMA_VERSION,
        id: id.to_string(),
        ok: reply.ok,
        result: reply.result,
        error: reply.error,
    }
}

pub async fn send_open(session_id: &str, resource: &ResourceId) -> io::Result<ControlReply> {
    send_command(
        session_id,
        WireCommand {
            schema_version: CONTROL_SCHEMA_VERSION,
            id: command_id(),
            method: "open".into(),
            resource: Some(resource.canonical_name()),
            key: None,
            value: None,
        },
    )
    .await
}

pub async fn send_set(session_id: &str, key: &str, value: &str) -> io::Result<ControlReply> {
    send_command(
        session_id,
        WireCommand {
            schema_version: CONTROL_SCHEMA_VERSION,
            id: command_id(),
            method: "set".into(),
            resource: None,
            key: Some(key.into()),
            value: Some(value.into()),
        },
    )
    .await
}

async fn send_command(session_id: &str, command: WireCommand) -> io::Result<ControlReply> {
    let path = socket_path(session_id);
    let mut stream = tokio::time::timeout(CONTROL_TIMEOUT, UnixStream::connect(&path))
        .await
        .map_err(|_| {
            io::Error::new(io::ErrorKind::TimedOut, "control socket connect timed out")
        })??;
    stream
        .write_all(serde_json::to_string(&command)?.as_bytes())
        .await?;
    stream.write_all(b"\n").await?;
    stream.flush().await?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    tokio::time::timeout(CONTROL_TIMEOUT, reader.read_line(&mut line))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "control reply timed out"))??;
    let reply = serde_json::from_str::<WireReply>(&line)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(ControlReply {
        ok: reply.ok,
        result: reply.result,
        error: reply.error,
    })
}

fn command_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("c_{}_{}", std::process::id(), nanos)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;
    use tokio::sync::Mutex;

    use super::*;

    static ENV_LOCK: Mutex<()> = Mutex::const_new(());

    #[test]
    fn runtime_dir_uses_override_then_xdg_then_tmp() {
        assert_eq!(
            runtime_dir_from_env(Some("/tmp/custom".into()), None, None),
            PathBuf::from("/tmp/custom")
        );
        assert_eq!(
            runtime_dir_from_env(None, Some("/run/user/1000".into()), None),
            PathBuf::from("/run/user/1000/ghzinga")
        );
        assert_eq!(
            runtime_dir_from_env(None, None, Some("1000".into())),
            PathBuf::from("/tmp/ghzinga-1000")
        );
    }

    #[test]
    #[cfg(unix)]
    fn runtime_dir_uses_stable_effective_uid_when_uid_env_is_missing() {
        assert_eq!(
            runtime_dir_from_env(None, None, None),
            PathBuf::from("/tmp").join(format!("ghzinga-{}", effective_user_id()))
        );
    }

    #[test]
    fn parses_open_wire_command() {
        let parsed = parse_wire_command(
            r#"{"schema_version":1,"id":"c_1","method":"open","resource":"owner/repo#12"}"#,
        )
        .unwrap();

        assert!(matches!(
            parsed,
            RuntimeCommand::Open {
                request_id,
                resource
            } if request_id == "c_1" && resource.canonical_name() == "owner/repo#12"
        ));
    }

    #[test]
    fn rejects_unknown_schema() {
        let error =
            parse_wire_command(r#"{"schema_version":2,"id":"c_1","method":"open"}"#).unwrap_err();

        assert_eq!(error.0, "c_1");
        assert!(error.1.contains("unsupported control schema"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn server_receives_open_command_and_replies() {
        let _guard = ENV_LOCK.lock().await;
        let dir = tempdir().unwrap();
        let previous_runtime = env::var_os(GZG_RUNTIME_HOME_ENV);
        env::set_var(GZG_RUNTIME_HOME_ENV, dir.path());
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let _server = start_server("work", tx).unwrap();
        let id = ResourceId::parse("owner/repo#12").unwrap();
        let client = tokio::spawn(async move { send_open("work", &id).await.unwrap() });

        let request = rx.recv().await.unwrap();
        assert!(matches!(
            request.command,
            RuntimeCommand::Open { resource, .. } if resource.canonical_name() == "owner/repo#12"
        ));
        request.reply.send(ControlReply::ok("opened")).unwrap();

        let reply = client.await.unwrap();
        assert!(reply.ok);
        assert_eq!(reply.result.as_deref(), Some("opened"));

        if let Some(value) = previous_runtime {
            env::set_var(GZG_RUNTIME_HOME_ENV, value);
        } else {
            env::remove_var(GZG_RUNTIME_HOME_ENV);
        }
    }
}

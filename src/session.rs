use std::{
    collections::{BTreeMap, HashSet},
    env, fs, io,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{
    app::{AppState, BlockId, ResourceTabState, Tab},
    domain::{Resource, ResourceId, ResourceKind},
    render::{
        normalize_fixed_width, ContentWidthMode, ScrollbarMode, SpacingMode, SymbolMode, ThemeName,
    },
};

pub const GZG_SESSION_ENV: &str = "GZG_SESSION";
pub const GZG_STATE_HOME_ENV: &str = "GZG_STATE_HOME";
pub const GZG_CACHE_HOME_ENV: &str = "GZG_CACHE_HOME";
const SESSION_SCHEMA_VERSION: u32 = 1;
const INDEX_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreMode {
    Auto,
    New,
    NoRestore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreRequest {
    pub mode: RestoreMode,
    pub explicit_session: Option<String>,
    pub has_resource_arg: bool,
    pub argv: Vec<String>,
    pub cwd: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestorePlan {
    pub handle: Option<SessionHandle>,
    pub snapshot: Option<SessionSnapshot>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionHandle {
    pub id: String,
    pub state_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub contexts: Vec<LaunchContext>,
    pub ephemeral: bool,
}

impl SessionHandle {
    pub fn session_path(&self) -> PathBuf {
        self.state_dir
            .join("sessions")
            .join(&self.id)
            .join("session.json")
    }

    pub fn index_path(&self) -> PathBuf {
        self.state_dir.join("session-index.json")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub schema_version: u32,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub launch: LaunchSnapshot,
    pub ui: UiSnapshot,
    pub resources: ResourcesSnapshot,
}

impl SessionSnapshot {
    pub fn from_state(
        id: impl Into<String>,
        previous: Option<&Self>,
        contexts: Vec<LaunchContext>,
        argv: Vec<String>,
        cwd: PathBuf,
        state: &mut AppState,
    ) -> Self {
        let id = id.into();
        let now = timestamp_label();
        Self {
            schema_version: SESSION_SCHEMA_VERSION,
            id,
            name: previous.and_then(|snapshot| snapshot.name.clone()),
            created_at: previous
                .map(|snapshot| snapshot.created_at.clone())
                .unwrap_or_else(|| now.clone()),
            updated_at: now,
            launch: LaunchSnapshot {
                argv,
                cwd,
                contexts: dedupe_contexts(contexts),
            },
            ui: UiSnapshot::from_state(state),
            resources: ResourcesSnapshot::from_state(state),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchSnapshot {
    pub argv: Vec<String>,
    pub cwd: PathBuf,
    #[serde(default)]
    pub contexts: Vec<LaunchContext>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSnapshot {
    pub theme: String,
    pub symbols: String,
    pub spacing: String,
    pub width_mode: String,
    pub fixed_width: u16,
    pub scrollbar: String,
}

impl UiSnapshot {
    fn from_state(state: &AppState) -> Self {
        Self {
            theme: state.theme.to_string(),
            symbols: state.symbols.to_string(),
            spacing: state.spacing.to_string(),
            width_mode: state.width_mode.to_string(),
            fixed_width: state.fixed_width,
            scrollbar: state.scrollbar.to_string(),
        }
    }

    pub fn apply_to_state(&self, state: &mut AppState) {
        if let Ok(theme) = ThemeName::from_str(&self.theme) {
            state.theme = theme;
        }
        if let Ok(symbols) = SymbolMode::from_str(&self.symbols) {
            state.symbols = symbols;
        }
        if let Ok(spacing) = SpacingMode::from_str(&self.spacing) {
            state.spacing = spacing;
        }
        if let Ok(width_mode) = ContentWidthMode::from_str(&self.width_mode) {
            state.width_mode = width_mode;
        }
        state.fixed_width = normalize_fixed_width(self.fixed_width);
        if let Ok(scrollbar) = ScrollbarMode::from_str(&self.scrollbar) {
            state.scrollbar = scrollbar;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourcesSnapshot {
    pub active_index: usize,
    #[serde(default)]
    pub tabs: Vec<ResourceTabSnapshot>,
}

impl ResourcesSnapshot {
    fn from_state(state: &mut AppState) -> Self {
        let tabs = state
            .session_resource_tabs()
            .into_iter()
            .map(ResourceTabSnapshot::from_tab)
            .collect();
        Self {
            active_index: state.active_resource_tab,
            tabs,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceTabSnapshot {
    pub id: String,
    pub resource: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind_hint: Option<ResourceKind>,
    pub view: String,
    pub scroll: u16,
    pub reverse_chronological: bool,
    #[serde(default)]
    pub expanded_blocks: Vec<BlockIdSnapshot>,
}

impl ResourceTabSnapshot {
    fn from_tab(tab: ResourceTabState) -> Self {
        let mut expanded_blocks = tab
            .expanded_blocks
            .iter()
            .map(BlockIdSnapshot::from)
            .collect::<Vec<_>>();
        expanded_blocks.sort();
        Self {
            id: format!("r_{}", tab.id),
            resource: tab.resource.id.canonical_name(),
            kind_hint: tab
                .resource
                .id
                .kind_hint
                .or_else(|| Some(tab.resource.kind())),
            view: tab.active_tab.to_string(),
            scroll: tab.scroll,
            reverse_chronological: tab.reverse_chronological,
            expanded_blocks,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum BlockIdSnapshot {
    Body,
    Activity(String),
    Commit(String),
    Check(String),
    File(String),
    Patch(String),
}

impl From<&BlockId> for BlockIdSnapshot {
    fn from(id: &BlockId) -> Self {
        match id {
            BlockId::Body => Self::Body,
            BlockId::Activity(value) => Self::Activity(value.clone()),
            BlockId::Commit(value) => Self::Commit(value.clone()),
            BlockId::Check(value) => Self::Check(value.clone()),
            BlockId::File(value) => Self::File(value.clone()),
            BlockId::Patch(value) => Self::Patch(value.clone()),
        }
    }
}

impl From<BlockIdSnapshot> for BlockId {
    fn from(id: BlockIdSnapshot) -> Self {
        match id {
            BlockIdSnapshot::Body => Self::Body,
            BlockIdSnapshot::Activity(value) => Self::Activity(value),
            BlockIdSnapshot::Commit(value) => Self::Commit(value),
            BlockIdSnapshot::Check(value) => Self::Check(value),
            BlockIdSnapshot::File(value) => Self::File(value),
            BlockIdSnapshot::Patch(value) => Self::Patch(value),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextConfidence {
    Explicit,
    Strong,
    Medium,
    Weak,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchContext {
    pub provider: String,
    pub key: String,
    pub confidence: ContextConfidence,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

impl LaunchContext {
    fn new(
        provider: impl Into<String>,
        key: impl Into<String>,
        confidence: ContextConfidence,
    ) -> Self {
        Self {
            provider: provider.into(),
            key: key.into(),
            confidence,
            metadata: BTreeMap::new(),
        }
    }

    fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionIndex {
    pub schema_version: u32,
    #[serde(default)]
    pub anchors: Vec<SessionAnchor>,
}

impl Default for SessionIndex {
    fn default() -> Self {
        Self {
            schema_version: INDEX_SCHEMA_VERSION,
            anchors: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionAnchor {
    pub provider: String,
    pub key: String,
    pub session_id: String,
    pub confidence: ContextConfidence,
    pub last_seen_at: String,
}

pub fn state_dir() -> PathBuf {
    app_state_dir_from_env(
        env::var_os(GZG_STATE_HOME_ENV),
        env::var_os("XDG_STATE_HOME"),
        env::var_os("HOME"),
    )
}

pub fn cache_dir() -> PathBuf {
    app_cache_dir_from_env(
        env::var_os(GZG_CACHE_HOME_ENV),
        env::var_os("XDG_CACHE_HOME"),
        env::var_os("HOME"),
    )
}

fn app_state_dir_from_env(
    override_dir: Option<std::ffi::OsString>,
    xdg_state: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> PathBuf {
    if let Some(path) = override_dir {
        return PathBuf::from(path);
    }
    if let Some(path) = xdg_state {
        return PathBuf::from(path).join("ghzinga");
    }
    home.map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local")
        .join("state")
        .join("ghzinga")
}

fn app_cache_dir_from_env(
    override_dir: Option<std::ffi::OsString>,
    xdg_cache: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> PathBuf {
    if let Some(path) = override_dir {
        return PathBuf::from(path);
    }
    if let Some(path) = xdg_cache {
        return PathBuf::from(path).join("ghzinga");
    }
    home.map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cache")
        .join("ghzinga")
}

pub fn collect_launch_contexts(explicit_session: Option<&str>, cwd: &Path) -> Vec<LaunchContext> {
    let mut contexts = Vec::new();
    let explicit = explicit_session
        .map(str::to_string)
        .or_else(|| env::var(GZG_SESSION_ENV).ok());
    if let Some(session) = explicit {
        if !session.trim().is_empty() {
            contexts.push(LaunchContext::new(
                "explicit",
                normalize_session_id(&session),
                ContextConfidence::Explicit,
            ));
        }
    }

    if env::var("HERDR_ENV").ok().as_deref() == Some("1") {
        if let (Ok(socket), Ok(pane)) = (env::var("HERDR_SOCKET_PATH"), env::var("HERDR_PANE_ID")) {
            if let Some(session_id) = read_herdr_session_marker(&socket, &pane) {
                contexts.push(
                    LaunchContext::new(
                        "herdr-label",
                        format!("session={session_id}"),
                        ContextConfidence::Strong,
                    )
                    .with_metadata("session_id", session_id),
                );
            }
            contexts.push(
                LaunchContext::new(
                    "herdr",
                    format!("socket={socket};pane={pane}"),
                    ContextConfidence::Strong,
                )
                .with_metadata("socket_path", socket)
                .with_metadata("pane_id", pane),
            );
        }
    }

    if let (Ok(tmux), Ok(pane)) = (env::var("TMUX"), env::var("TMUX_PANE")) {
        contexts.push(
            LaunchContext::new(
                "tmux",
                format!("tmux={tmux};pane={pane}"),
                ContextConfidence::Strong,
            )
            .with_metadata("tmux", tmux)
            .with_metadata("pane", pane),
        );
    }

    if let Ok(sty) = env::var("STY") {
        let window = env::var("WINDOW").unwrap_or_default();
        contexts.push(
            LaunchContext::new(
                "screen",
                format!("sty={sty};window={window}"),
                ContextConfidence::Medium,
            )
            .with_metadata("sty", sty)
            .with_metadata("window", window),
        );
    }

    if let Some(remote) = git_remote_context(cwd) {
        contexts.push(remote);
    }

    let cwd_key = fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    contexts.push(
        LaunchContext::new(
            "cwd",
            cwd_key.display().to_string(),
            ContextConfidence::Weak,
        )
        .with_metadata("cwd", cwd_key.display().to_string()),
    );

    if let Some(tty) = current_tty() {
        contexts.push(
            LaunchContext::new("tty", tty.clone(), ContextConfidence::Weak)
                .with_metadata("tty", tty),
        );
    }

    dedupe_contexts(contexts)
}

fn git_remote_context(cwd: &Path) -> Option<LaunchContext> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let remote = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let key = github_repo_key_from_remote(&remote)?;
    Some(LaunchContext::new("git", key, ContextConfidence::Weak).with_metadata("remote", remote))
}

fn github_repo_key_from_remote(remote: &str) -> Option<String> {
    let mut value = remote.trim().trim_end_matches(".git").to_string();
    if let Some(rest) = value.strip_prefix("git@github.com:") {
        value = rest.to_string();
    } else if let Some(rest) = value.strip_prefix("https://github.com/") {
        value = rest.to_string();
    } else if let Some(rest) = value.strip_prefix("ssh://git@github.com/") {
        value = rest.to_string();
    } else {
        return None;
    }
    let parts = value.split('/').collect::<Vec<_>>();
    (parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty())
        .then(|| format!("github.com/{}/{}", parts[0], parts[1]))
}

fn current_tty() -> Option<String> {
    let output = Command::new("tty").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let tty = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!tty.is_empty() && tty != "not a tty").then_some(tty)
}

pub fn resolve_restore_plan(request: RestoreRequest) -> RestorePlan {
    let state_dir = state_dir();
    let cache_dir = cache_dir();
    let configured_explicit = request
        .explicit_session
        .clone()
        .or_else(|| env::var(GZG_SESSION_ENV).ok());
    let contexts = collect_launch_contexts(configured_explicit.as_deref(), &request.cwd);
    let explicit = configured_explicit.or_else(|| herdr_label_session_id(&contexts));
    let mut warnings = Vec::new();

    if request.mode == RestoreMode::NoRestore {
        return RestorePlan {
            handle: None,
            snapshot: None,
            warnings,
        };
    }

    let index_path = state_dir.join("session-index.json");
    let mut index = match load_index(&index_path) {
        Ok(index) => index,
        Err(error) => {
            warnings.push(error);
            SessionIndex::default()
        }
    };

    let session_id = if let Some(explicit) = explicit {
        normalize_session_id(&explicit)
    } else if request.mode == RestoreMode::New {
        new_session_id()
    } else if let Some(session_id) =
        resolve_index_match(&index, &contexts, request.has_resource_arg)
    {
        session_id
    } else {
        new_session_id()
    };

    let handle = SessionHandle {
        id: session_id.clone(),
        state_dir: state_dir.clone(),
        cache_dir,
        contexts: contexts.clone(),
        ephemeral: false,
    };
    let snapshot = if request.mode == RestoreMode::New {
        None
    } else {
        match load_snapshot(&handle.session_path()) {
            Ok(snapshot) => Some(snapshot),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => {
                warnings.push(format!(
                    "failed to load session {}: {error}",
                    handle.session_path().display()
                ));
                None
            }
        }
    };

    bind_contexts(&mut index, &contexts, &session_id);
    if let Err(error) = save_index(&index_path, &index) {
        warnings.push(format!(
            "failed to save session index {}: {error}",
            index_path.display()
        ));
    }

    RestorePlan {
        handle: Some(handle),
        snapshot,
        warnings,
    }
}

fn herdr_label_session_id(contexts: &[LaunchContext]) -> Option<String> {
    contexts
        .iter()
        .find(|context| context.provider == "herdr-label")
        .and_then(|context| context.metadata.get("session_id"))
        .cloned()
}

fn resolve_index_match(
    index: &SessionIndex,
    contexts: &[LaunchContext],
    has_resource_arg: bool,
) -> Option<String> {
    for confidence in [
        ContextConfidence::Explicit,
        ContextConfidence::Strong,
        ContextConfidence::Medium,
        ContextConfidence::Weak,
    ] {
        if has_resource_arg && confidence == ContextConfidence::Weak {
            continue;
        }
        let matches = matching_sessions(index, contexts, confidence);
        if matches.len() == 1 {
            return matches.into_iter().next();
        }
    }
    None
}

fn matching_sessions(
    index: &SessionIndex,
    contexts: &[LaunchContext],
    confidence: ContextConfidence,
) -> HashSet<String> {
    let context_keys = contexts
        .iter()
        .filter(|context| context.confidence == confidence)
        .map(|context| (context.provider.as_str(), context.key.as_str()))
        .collect::<HashSet<_>>();
    index
        .anchors
        .iter()
        .filter(|anchor| {
            anchor.confidence == confidence
                && context_keys.contains(&(anchor.provider.as_str(), anchor.key.as_str()))
        })
        .map(|anchor| anchor.session_id.clone())
        .collect()
}

fn bind_contexts(index: &mut SessionIndex, contexts: &[LaunchContext], session_id: &str) {
    let now = timestamp_label();
    for context in contexts {
        if let Some(anchor) = index
            .anchors
            .iter_mut()
            .find(|anchor| anchor.provider == context.provider && anchor.key == context.key)
        {
            anchor.session_id = session_id.to_string();
            anchor.confidence = context.confidence;
            anchor.last_seen_at = now.clone();
        } else {
            index.anchors.push(SessionAnchor {
                provider: context.provider.clone(),
                key: context.key.clone(),
                session_id: session_id.to_string(),
                confidence: context.confidence,
                last_seen_at: now.clone(),
            });
        }
    }
}

pub fn save_session(
    handle: &SessionHandle,
    previous: Option<&SessionSnapshot>,
    argv: Vec<String>,
    cwd: PathBuf,
    state: &mut AppState,
) -> io::Result<SessionSnapshot> {
    if handle.ephemeral {
        return Err(io::Error::other("cannot save ephemeral session"));
    }
    let snapshot = SessionSnapshot::from_state(
        handle.id.clone(),
        previous,
        handle.contexts.clone(),
        argv,
        cwd,
        state,
    );
    save_snapshot(&handle.session_path(), &snapshot)?;
    save_resource_cache_for_state(handle, state)?;
    best_effort_mark_provider_session(handle, state);
    Ok(snapshot)
}

fn best_effort_mark_provider_session(handle: &SessionHandle, state: &mut AppState) {
    #[cfg(unix)]
    {
        let Some(context) = handle
            .contexts
            .iter()
            .find(|context| context.provider == "herdr")
        else {
            return;
        };
        let (Some(socket), Some(pane)) = (
            context.metadata.get("socket_path"),
            context.metadata.get("pane_id"),
        ) else {
            return;
        };
        let active = state
            .session_resource_tabs()
            .get(state.active_resource_tab)
            .map(|tab| tab.resource.id.canonical_name())
            .unwrap_or_else(|| "empty".into());
        let label = format!("gzg:{} {active}", handle.id);
        let _ = write_herdr_pane_label(socket, pane, &label);
    }
}

#[cfg(unix)]
fn write_herdr_pane_label(socket: &str, pane_id: &str, label: &str) -> io::Result<()> {
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    let mut stream = UnixStream::connect(socket)?;
    stream.set_write_timeout(Some(Duration::from_millis(100)))?;
    let request = serde_json::json!({
        "id": "gzg:session-marker",
        "method": "pane.rename",
        "params": {
            "pane_id": pane_id,
            "label": label
        }
    });
    writeln!(stream, "{request}")?;
    Ok(())
}

fn read_herdr_session_marker(socket: &str, pane_id: &str) -> Option<String> {
    #[cfg(unix)]
    {
        read_herdr_pane_label(socket, pane_id).and_then(|label| parse_herdr_session_marker(&label))
    }
    #[cfg(not(unix))]
    {
        let _ = (socket, pane_id);
        None
    }
}

#[cfg(unix)]
fn read_herdr_pane_label(socket: &str, pane_id: &str) -> Option<String> {
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    let mut stream = UnixStream::connect(socket).ok()?;
    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let request = serde_json::json!({
        "id": "gzg:session-marker-read",
        "method": "pane.get",
        "params": {
            "pane_id": pane_id
        }
    });
    writeln!(stream, "{request}").ok()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&line).ok()?;
    value
        .get("result")?
        .get("pane")?
        .get("label")?
        .as_str()
        .map(str::to_string)
}

fn parse_herdr_session_marker(label: &str) -> Option<String> {
    label
        .strip_prefix("gzg:")
        .and_then(|rest| rest.split_whitespace().next())
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

pub fn load_snapshot(path: &Path) -> io::Result<SessionSnapshot> {
    let raw = fs::read_to_string(path)?;
    let snapshot = serde_json::from_str::<SessionSnapshot>(&raw)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if snapshot.schema_version != SESSION_SCHEMA_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported session schema {}", snapshot.schema_version),
        ));
    }
    Ok(snapshot)
}

pub fn save_snapshot(path: &Path, snapshot: &SessionSnapshot) -> io::Result<()> {
    save_json_atomic(path, snapshot)
}

pub fn load_index(path: &Path) -> Result<SessionIndex, String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(SessionIndex::default());
        }
        Err(error) => {
            return Err(format!(
                "failed to read session index {}: {error}",
                path.display()
            ))
        }
    };
    let index = serde_json::from_str::<SessionIndex>(&raw)
        .map_err(|error| format!("failed to parse session index {}: {error}", path.display()))?;
    if index.schema_version != INDEX_SCHEMA_VERSION {
        return Err(format!(
            "unsupported session index schema {} in {}",
            index.schema_version,
            path.display()
        ));
    }
    Ok(index)
}

pub fn save_index(path: &Path, index: &SessionIndex) -> io::Result<()> {
    save_json_atomic(path, index)
}

pub fn resource_cache_path(cache_dir: &Path, id: &ResourceId) -> PathBuf {
    cache_dir
        .join("resources")
        .join(&id.owner)
        .join(&id.repo)
        .join(format!("{}.json", id.number))
}

pub fn load_resource_cache(cache_dir: &Path, id: &ResourceId) -> io::Result<Resource> {
    let path = resource_cache_path(cache_dir, id);
    let raw = fs::read_to_string(&path)?;
    serde_json::from_str::<Resource>(&raw)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub fn save_resource_cache(cache_dir: &Path, resource: &Resource) -> io::Result<()> {
    save_json_atomic(&resource_cache_path(cache_dir, &resource.id), resource)
}

fn save_resource_cache_for_state(handle: &SessionHandle, state: &mut AppState) -> io::Result<()> {
    let tabs = state.session_resource_tabs();
    for tab in tabs {
        if tab.resource.state != "LOADING" {
            save_resource_cache(&handle.cache_dir, &tab.resource)?;
        }
    }
    Ok(())
}

fn save_json_atomic<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_vec_pretty(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, raw)?;
    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&tmp);
            Err(error)
        }
    }
}

pub fn restore_state_from_snapshot(
    snapshot: &SessionSnapshot,
    cache_dir: &Path,
) -> Option<AppState> {
    let mut tabs = Vec::new();
    for (index, tab) in snapshot.resources.tabs.iter().enumerate() {
        let id = ResourceId::parse(&tab.resource).ok().map(|mut id| {
            id.kind_hint = tab.kind_hint;
            id
        })?;
        let resource = match load_resource_cache(cache_dir, &id) {
            Ok(resource) => resource,
            Err(_) => crate::app::loading_resource_placeholder(id),
        };
        let active_tab = Tab::from_str(&tab.view).unwrap_or(Tab::Overview);
        let expanded_blocks = tab
            .expanded_blocks
            .clone()
            .into_iter()
            .map(BlockId::from)
            .collect();
        tabs.push(ResourceTabState::from_session_parts(
            index as u64 + 1,
            resource,
            active_tab,
            tab.scroll,
            tab.reverse_chronological,
            expanded_blocks,
        ));
    }
    if tabs.is_empty() {
        return None;
    }
    let active_index = snapshot
        .resources
        .active_index
        .min(tabs.len().saturating_sub(1));
    let mut state = AppState::from_session_tabs(tabs, active_index);
    snapshot.ui.apply_to_state(&mut state);
    Some(state)
}

pub fn first_refresh_action(snapshot: &SessionSnapshot) -> Option<ResourceId> {
    let active = snapshot
        .resources
        .tabs
        .get(snapshot.resources.active_index)
        .or_else(|| snapshot.resources.tabs.first())?;
    ResourceId::parse(&active.resource).ok().map(|mut id| {
        id.kind_hint = active.kind_hint;
        id
    })
}

fn dedupe_contexts(contexts: Vec<LaunchContext>) -> Vec<LaunchContext> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for context in contexts {
        let key = (context.provider.clone(), context.key.clone());
        if seen.insert(key) {
            out.push(context);
        }
    }
    out
}

fn new_session_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("s_{:x}_{:x}", std::process::id(), nanos)
}

fn normalize_session_id(input: &str) -> String {
    let mut output = input
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    while output.contains("--") {
        output = output.replace("--", "-");
    }
    output = output.trim_matches('-').to_string();
    if output.is_empty() {
        new_session_id()
    } else {
        output
    }
}

fn timestamp_label() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

pub fn prune_session_anchors(state_dir: &Path, session_id: &str) -> Result<usize, String> {
    let index_path = state_dir.join("session-index.json");
    let mut index = load_index(&index_path)?;
    let before = index.anchors.len();
    index
        .anchors
        .retain(|anchor| anchor.session_id != session_id);
    let removed = before.saturating_sub(index.anchors.len());
    if removed > 0 {
        save_index(&index_path, &index).map_err(|error| {
            format!(
                "failed to save session index {}: {error}",
                index_path.display()
            )
        })?;
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use tempfile::tempdir;

    use super::*;
    use crate::domain::{ReactionCounts, ResourceKind};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn resource(number: u64) -> Resource {
        Resource {
            id: ResourceId {
                owner: "owner".into(),
                repo: "repo".into(),
                number,
                kind_hint: Some(ResourceKind::Issue),
            },
            title: format!("Issue {number}"),
            url: format!("https://github.com/owner/repo/issues/{number}"),
            state: "OPEN".into(),
            author: "alice".into(),
            created_at: "now".into(),
            updated_at: "now".into(),
            labels: vec![],
            assignees: vec![],
            reactions: ReactionCounts::default(),
            body: "body".into(),
            activity: vec![],
            related_resources: vec![],
            metadata: vec![],
            warnings: vec![],
            pull_request: None,
        }
    }

    #[test]
    fn state_and_cache_dirs_use_overrides() {
        assert_eq!(
            app_state_dir_from_env(Some("/tmp/state".into()), None, None),
            PathBuf::from("/tmp/state")
        );
        assert_eq!(
            app_cache_dir_from_env(Some("/tmp/cache".into()), None, None),
            PathBuf::from("/tmp/cache")
        );
    }

    #[test]
    fn github_remote_context_parses_common_urls() {
        assert_eq!(
            github_repo_key_from_remote("https://github.com/dutifuldev/ghzinga.git"),
            Some("github.com/dutifuldev/ghzinga".into())
        );
        assert_eq!(
            github_repo_key_from_remote("git@github.com:dutifuldev/ghzinga.git"),
            Some("github.com/dutifuldev/ghzinga".into())
        );
    }

    #[test]
    fn strong_context_restores_matching_session() {
        let index = SessionIndex {
            schema_version: INDEX_SCHEMA_VERSION,
            anchors: vec![SessionAnchor {
                provider: "tmux".into(),
                key: "tmux=/tmp/tmux;pane=%1".into(),
                session_id: "work".into(),
                confidence: ContextConfidence::Strong,
                last_seen_at: "1".into(),
            }],
        };
        let contexts = vec![LaunchContext::new(
            "tmux",
            "tmux=/tmp/tmux;pane=%1",
            ContextConfidence::Strong,
        )];

        assert_eq!(
            resolve_index_match(&index, &contexts, true),
            Some("work".into())
        );
    }

    #[test]
    fn weak_context_does_not_restore_when_resource_arg_is_present() {
        let index = SessionIndex {
            schema_version: INDEX_SCHEMA_VERSION,
            anchors: vec![SessionAnchor {
                provider: "cwd".into(),
                key: "/repo".into(),
                session_id: "work".into(),
                confidence: ContextConfidence::Weak,
                last_seen_at: "1".into(),
            }],
        };
        let contexts = vec![LaunchContext::new("cwd", "/repo", ContextConfidence::Weak)];

        assert_eq!(resolve_index_match(&index, &contexts, true), None);
        assert_eq!(
            resolve_index_match(&index, &contexts, false),
            Some("work".into())
        );
    }

    #[test]
    fn new_mode_keeps_explicit_session_name() {
        let _guard = ENV_LOCK.lock().unwrap();
        let state = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let previous_state = env::var_os(GZG_STATE_HOME_ENV);
        let previous_cache = env::var_os(GZG_CACHE_HOME_ENV);
        let previous_session = env::var_os(GZG_SESSION_ENV);
        env::set_var(GZG_STATE_HOME_ENV, state.path());
        env::set_var(GZG_CACHE_HOME_ENV, cache.path());
        env::remove_var(GZG_SESSION_ENV);

        let plan = resolve_restore_plan(RestoreRequest {
            mode: RestoreMode::New,
            explicit_session: Some("Work Session".into()),
            has_resource_arg: true,
            argv: vec!["gzg".into()],
            cwd: PathBuf::from("/repo"),
        });

        if let Some(value) = previous_state {
            env::set_var(GZG_STATE_HOME_ENV, value);
        } else {
            env::remove_var(GZG_STATE_HOME_ENV);
        }
        if let Some(value) = previous_cache {
            env::set_var(GZG_CACHE_HOME_ENV, value);
        } else {
            env::remove_var(GZG_CACHE_HOME_ENV);
        }
        if let Some(value) = previous_session {
            env::set_var(GZG_SESSION_ENV, value);
        } else {
            env::remove_var(GZG_SESSION_ENV);
        }

        assert_eq!(plan.handle.unwrap().id, "Work-Session");
        assert!(plan.snapshot.is_none());
    }

    #[test]
    fn session_snapshot_round_trips_tabs() {
        let mut state = AppState::new(resource(1));
        state.open_resource_in_tab(resource(2));
        state.set_tab(Tab::Activity);
        state.scroll_down(5);

        let snapshot = SessionSnapshot::from_state(
            "work",
            None,
            vec![],
            vec!["gzg".into()],
            PathBuf::from("/repo"),
            &mut state,
        );

        assert_eq!(snapshot.resources.tabs.len(), 2);
        assert_eq!(snapshot.resources.active_index, 1);
        assert_eq!(snapshot.resources.tabs[1].view, "activity");
        assert_eq!(snapshot.resources.tabs[1].scroll, 5);
    }

    #[test]
    fn session_snapshot_round_trips_reverse_order() {
        let dir = tempdir().unwrap();
        save_resource_cache(dir.path(), &resource(1)).unwrap();
        let mut state = AppState::new(resource(1));
        state.toggle_feed_order();

        let snapshot = SessionSnapshot::from_state(
            "work",
            None,
            vec![],
            vec!["gzg".into()],
            PathBuf::from("/repo"),
            &mut state,
        );

        assert!(snapshot.resources.tabs[0].reverse_chronological);

        let restored = restore_state_from_snapshot(&snapshot, dir.path()).unwrap();

        assert!(restored.reverse_chronological);
    }

    #[test]
    fn restore_state_uses_cached_resources() {
        let dir = tempdir().unwrap();
        save_resource_cache(dir.path(), &resource(1)).unwrap();
        let snapshot = SessionSnapshot {
            schema_version: SESSION_SCHEMA_VERSION,
            id: "work".into(),
            name: None,
            created_at: "1".into(),
            updated_at: "1".into(),
            launch: LaunchSnapshot {
                argv: vec![],
                cwd: PathBuf::from("/repo"),
                contexts: vec![],
            },
            ui: UiSnapshot {
                theme: "default".into(),
                symbols: "emoji".into(),
                spacing: "comfortable".into(),
                width_mode: "fixed".into(),
                fixed_width: 118,
                scrollbar: "on-scroll".into(),
            },
            resources: ResourcesSnapshot {
                active_index: 0,
                tabs: vec![ResourceTabSnapshot {
                    id: "r_1".into(),
                    resource: "owner/repo#1".into(),
                    kind_hint: Some(ResourceKind::Issue),
                    view: "overview".into(),
                    scroll: 0,
                    reverse_chronological: false,
                    expanded_blocks: vec![],
                }],
            },
        };

        let state = restore_state_from_snapshot(&snapshot, dir.path()).unwrap();

        assert_eq!(state.resource.title, "Issue 1");
        assert_eq!(state.resource.state, "OPEN");
    }

    #[test]
    fn herdr_session_marker_parses_from_pane_label() {
        assert_eq!(
            parse_herdr_session_marker("gzg:s_abc123 owner/repo#1"),
            Some("s_abc123".into())
        );
        assert_eq!(parse_herdr_session_marker("other label"), None);
    }

    #[test]
    fn pruning_session_anchors_removes_deleted_session_bindings() {
        let dir = tempdir().unwrap();
        let index_path = dir.path().join("session-index.json");
        save_index(
            &index_path,
            &SessionIndex {
                schema_version: INDEX_SCHEMA_VERSION,
                anchors: vec![
                    SessionAnchor {
                        provider: "tmux".into(),
                        key: "tmux=/tmp/tmux;pane=%1".into(),
                        session_id: "deleted".into(),
                        confidence: ContextConfidence::Strong,
                        last_seen_at: "1".into(),
                    },
                    SessionAnchor {
                        provider: "cwd".into(),
                        key: "/repo".into(),
                        session_id: "kept".into(),
                        confidence: ContextConfidence::Weak,
                        last_seen_at: "1".into(),
                    },
                ],
            },
        )
        .unwrap();

        assert_eq!(prune_session_anchors(dir.path(), "deleted").unwrap(), 1);

        let index = load_index(&index_path).unwrap();
        assert_eq!(index.anchors.len(), 1);
        assert_eq!(index.anchors[0].session_id, "kept");
    }
}

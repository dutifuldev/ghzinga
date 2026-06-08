use std::{
    process::Stdio,
    str::FromStr,
    time::{Duration, Instant},
};

use crate::{
    app::{apply_event, loading_resource_placeholder, AppEvent, AppIntent, AppState, Tab},
    cli::Cli,
    config::{self, AppConfig},
    control::{self, ControlReply, RuntimeCommand, RuntimeRequest},
    domain::{Resource, ResourceId, FILE_PATCH_CONTEXT_UNAVAILABLE_WARNING},
    fetch::{
        apply_completed_fetches, start_background_fetch, FetchAction, FetchOutcome, FetchSource,
        OfflineFixtureSource,
    },
    github::{api::GithubApiGateway, load_fixture},
    render::render_app,
    session::{self, RestorePlan, RestoreRequest, SessionHandle, SessionSnapshot},
    terminal::TerminalGuard,
};
use anyhow::Context;
use clap::Parser;
use crossterm::event::{self, Event};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

const EVENT_POLL_TIMEOUT: Duration = Duration::from_millis(250);
const MAX_PENDING_EVENTS_PER_FRAME: usize = 64;
const SESSION_SAVE_DEBOUNCE: Duration = Duration::from_millis(750);

async fn maybe_run_session_command(args: &[String]) -> anyhow::Result<Option<i32>> {
    let Some(command) = args.get(1).map(String::as_str) else {
        return Ok(None);
    };
    match command {
        "sessions" => {
            if args.get(2).is_some_and(|value| is_help_arg(value)) && args.len() == 3 {
                print_sessions_usage();
                return Ok(Some(0));
            }
            if args.len() != 2 {
                print_sessions_usage_to_stderr();
                return Ok(Some(2));
            }
            print_sessions()?;
            Ok(Some(0))
        }
        "session" => run_session_subcommand(&args[2..]).map(Some),
        "open" => run_open_command(args, &args[2..]).await.map(Some),
        "set" => run_set_command(args, &args[2..]).await.map(Some),
        _ => Ok(None),
    }
}

async fn run_open_command(raw_args: &[String], args: &[String]) -> anyhow::Result<i32> {
    if args.iter().any(|value| is_help_arg(value)) {
        print_open_usage();
        return Ok(0);
    }
    let (session_ref, resource_args) = parse_control_session_option(args)?;
    if resource_args.is_empty() {
        print_open_usage_to_stderr();
        return Ok(2);
    }
    let resources = parse_resource_args(&resource_args)?;
    let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());
    let plan = match session::resolve_control_plan(session_ref, raw_args.to_vec(), cwd.clone()) {
        Ok(plan) => plan,
        Err(session::ControlResolveError::Ambiguous(matches)) => {
            eprintln!(
                "ambiguous ghzinga session; pass --session. matches: {}",
                matches.join(", ")
            );
            return Ok(2);
        }
    };
    print_control_warnings(&plan.warnings);
    let Some(handle) = plan.handle else {
        eprintln!("no persistent ghzinga session available");
        return Ok(1);
    };
    for resource in &resources {
        match control::send_open(&handle.id, resource).await {
            Ok(reply) if reply.ok => {
                println!(
                    "{}",
                    reply
                        .result
                        .unwrap_or_else(|| format!("opened {}", resource.canonical_name()))
                );
            }
            Ok(reply) => {
                eprintln!(
                    "{}",
                    reply
                        .error
                        .unwrap_or_else(|| "control command failed".into())
                );
                return Ok(1);
            }
            Err(error) => {
                save_open_commands_to_session(
                    &handle,
                    plan.snapshot,
                    raw_args.to_vec(),
                    cwd,
                    &resources,
                )?;
                for resource in &resources {
                    println!(
                        "saved {} for next restore in session {} ({error})",
                        resource.canonical_name(),
                        handle.id
                    );
                }
                return Ok(0);
            }
        }
    }
    Ok(0)
}

async fn run_set_command(raw_args: &[String], args: &[String]) -> anyhow::Result<i32> {
    if args.iter().any(|value| is_help_arg(value)) {
        print_set_usage();
        return Ok(0);
    }
    let (session_ref, rest) = parse_control_session_option(args)?;
    if rest.len() != 2 {
        print_set_usage_to_stderr();
        return Ok(2);
    }
    let key = &rest[0];
    let value = &rest[1];
    let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());
    let plan = match session::resolve_control_plan(session_ref, raw_args.to_vec(), cwd) {
        Ok(plan) => plan,
        Err(session::ControlResolveError::Ambiguous(matches)) => {
            eprintln!(
                "ambiguous ghzinga session; pass --session. matches: {}",
                matches.join(", ")
            );
            return Ok(2);
        }
    };
    print_control_warnings(&plan.warnings);
    let Some(handle) = plan.handle else {
        eprintln!("no persistent ghzinga session available");
        return Ok(1);
    };
    match control::send_set(&handle.id, key, value).await {
        Ok(reply) if reply.ok => {
            println!(
                "{}",
                reply
                    .result
                    .unwrap_or_else(|| format!("set {key} for session {}", handle.id))
            );
            Ok(0)
        }
        Ok(reply) => {
            eprintln!(
                "{}",
                reply
                    .error
                    .unwrap_or_else(|| "control command failed".into())
            );
            Ok(1)
        }
        Err(error) => {
            let Some(mut snapshot) = plan.snapshot else {
                eprintln!(
                    "session {} is not running and has no saved state; open a resource first ({error})",
                    handle.id
                );
                return Ok(1);
            };
            match session::apply_ui_setting_to_snapshot(&mut snapshot, key, value) {
                Ok(()) => {
                    session::save_snapshot(&handle.session_path(), &snapshot)?;
                    println!(
                        "saved {key}={value} for next restore in session {} ({error})",
                        handle.id
                    );
                    Ok(0)
                }
                Err(error) => {
                    eprintln!("{error}");
                    Ok(2)
                }
            }
        }
    }
}

fn parse_control_session_option(args: &[String]) -> anyhow::Result<(Option<String>, Vec<String>)> {
    let mut session_ref = None;
    let mut rest = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--session" => {
                let Some(value) = args.get(index + 1) else {
                    anyhow::bail!("--session requires an id or name");
                };
                session_ref = Some(value.clone());
                index += 2;
            }
            value if value.starts_with("--session=") => {
                session_ref = Some(value.trim_start_matches("--session=").to_string());
                index += 1;
            }
            value if value.starts_with("--") => {
                anyhow::bail!("unknown control option `{value}`");
            }
            _ => {
                rest.push(args[index].clone());
                index += 1;
            }
        }
    }
    Ok((session_ref, rest))
}

fn parse_resource_args(args: &[String]) -> anyhow::Result<Vec<ResourceId>> {
    match args {
        [] => anyhow::bail!("expected at least one GitHub PR/issue resource"),
        [single] => Ok(vec![ResourceId::parse(single).map_err(anyhow::Error::new)?]),
        [owner_repo, number]
            if ResourceId::parse(owner_repo).is_err() && number.parse::<u64>().is_ok() =>
        {
            Ok(vec![ResourceId::from_owner_repo_number(owner_repo, number)
                .map_err(anyhow::Error::new)?])
        }
        many => many
            .iter()
            .map(|value| ResourceId::parse(value).map_err(anyhow::Error::new))
            .collect(),
    }
}

fn is_help_arg(value: &str) -> bool {
    matches!(value, "help" | "--help" | "-h")
}

fn print_sessions_usage() {
    println!("usage: gzg sessions");
    println!();
    println!("List saved ghzinga sessions with live/saved status, active resource, and tab count.");
}

fn print_sessions_usage_to_stderr() {
    eprintln!("usage: gzg sessions");
}

fn print_open_usage() {
    println!("usage: gzg open [--session <id-or-name>] <resource> [resource...]");
    println!();
    println!("Resources may be GitHub URLs or owner/repo#number values.");
    println!("The owner/repo number form is supported for one resource at a time.");
}

fn print_open_usage_to_stderr() {
    eprintln!("usage: gzg open [--session <id-or-name>] <resource> [resource...]");
}

fn print_set_usage() {
    println!("usage: gzg set [--session <id-or-name>] <setting> <value>");
    println!();
    println!("Settings: theme, symbols, spacing, width-mode, fixed-width, scrollbar");
}

fn print_set_usage_to_stderr() {
    eprintln!("usage: gzg set [--session <id-or-name>] <setting> <value>");
}

fn print_control_warnings(warnings: &[String]) {
    for warning in warnings {
        eprintln!("warning: {warning}");
    }
}

fn save_open_commands_to_session(
    handle: &SessionHandle,
    snapshot: Option<SessionSnapshot>,
    argv: Vec<String>,
    cwd: std::path::PathBuf,
    resources: &[ResourceId],
) -> anyhow::Result<()> {
    let Some(first_resource) = resources.first() else {
        anyhow::bail!("expected at least one resource to save");
    };
    let mut snapshot = if let Some(snapshot) = snapshot {
        snapshot
    } else {
        let mut state = AppState::new(loading_resource_placeholder(first_resource.clone()));
        SessionSnapshot::from_state(
            handle.id.clone(),
            None,
            handle.contexts.clone(),
            argv,
            cwd,
            &mut state,
        )
    };
    for resource in resources {
        session::upsert_resource_in_snapshot(&mut snapshot, resource);
    }
    session::save_snapshot(&handle.session_path(), &snapshot)?;
    Ok(())
}

fn run_session_subcommand(args: &[String]) -> anyhow::Result<i32> {
    let Some(command) = args.first().map(String::as_str) else {
        eprintln!("usage: gzg session <show|delete|rename> ...");
        return Ok(2);
    };
    match command {
        "show" => {
            let Some(reference) = args.get(1) else {
                eprintln!("usage: gzg session show <id-or-name>");
                return Ok(2);
            };
            let state_root = session::state_dir();
            let id = session::resolve_session_reference(&state_root, reference);
            let path = session::session_json_path(&state_root, &id);
            let raw = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            println!("{raw}");
            Ok(0)
        }
        "delete" => {
            let Some(reference) = args.get(1) else {
                eprintln!("usage: gzg session delete <id-or-name>");
                return Ok(2);
            };
            let state_root = session::state_dir();
            let id = session::resolve_session_reference(&state_root, reference);
            let path = state_root.join("sessions").join(&id);
            match std::fs::remove_dir_all(&path) {
                Ok(()) => {
                    session::prune_session_anchors(&state_root, &id)
                        .map_err(|error| anyhow::anyhow!(error))?;
                    println!("deleted session {id}");
                    Ok(0)
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    eprintln!("session not found: {id}");
                    Ok(1)
                }
                Err(error) => Err(error).with_context(|| format!("failed to delete {id}")),
            }
        }
        "rename" => {
            let (Some(reference), Some(name)) = (args.get(1), args.get(2)) else {
                eprintln!("usage: gzg session rename <id-or-name> <name>");
                return Ok(2);
            };
            let state_root = session::state_dir();
            let id = session::resolve_session_reference(&state_root, reference);
            let path = session::session_json_path(&state_root, &id);
            let mut snapshot = session::load_snapshot(&path)
                .with_context(|| format!("failed to load {}", path.display()))?;
            snapshot.name = Some(name.to_string());
            session::save_snapshot(&path, &snapshot)
                .with_context(|| format!("failed to save {}", path.display()))?;
            println!("renamed session {id} to {name}");
            Ok(0)
        }
        "help" | "--help" | "-h" => {
            eprintln!("usage: gzg session <show|delete|rename> ...");
            Ok(0)
        }
        _ => {
            eprintln!("usage: gzg session <show|delete|rename> ...");
            Ok(2)
        }
    }
}

fn print_sessions() -> anyhow::Result<()> {
    let root = session::state_dir().join("sessions");
    let entries = match std::fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            println!("no ghzinga sessions");
            return Ok(());
        }
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", root.display()))
        }
    };
    let mut rows = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().to_string();
        let snapshot_path = entry.path().join("session.json");
        let snapshot = match session::load_snapshot(&snapshot_path) {
            Ok(snapshot) => snapshot,
            Err(_) => continue,
        };
        let active = snapshot
            .resources
            .tabs
            .get(snapshot.resources.active_index)
            .or_else(|| snapshot.resources.tabs.first())
            .map(|tab| tab.resource.as_str())
            .unwrap_or("-");
        rows.push((
            id.clone(),
            session_live_status(&id),
            snapshot.name.unwrap_or_default(),
            active.to_string(),
            resource_count_label(snapshot.resources.tabs.len()),
        ));
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    if rows.is_empty() {
        println!("no ghzinga sessions");
    } else {
        for (id, status, name, active, resource_count) in rows {
            if name.is_empty() {
                println!("{id}\t{status}\t{active}\t{resource_count}");
            } else {
                println!("{id}\t{status}\t{name}\t{active}\t{resource_count}");
            }
        }
    }
    Ok(())
}

fn session_live_status(id: &str) -> &'static str {
    if std::os::unix::net::UnixStream::connect(control::socket_path(id)).is_ok() {
        "running"
    } else {
        "saved"
    }
}

fn resource_count_label(count: usize) -> String {
    match count {
        1 => "1 resource".into(),
        count => format!("{count} resources"),
    }
}

pub async fn run_from_cli() -> anyhow::Result<()> {
    let raw_args = std::env::args().collect::<Vec<_>>();
    if let Some(code) = maybe_run_session_command(&raw_args).await? {
        std::process::exit(code);
    }
    let cli = Cli::parse();
    let loaded_config = config::load();
    let resource_id = cli.parse_optional_resource_id()?;
    let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());
    let restore_plan = if cli.once {
        RestorePlan {
            handle: None,
            snapshot: None,
            warnings: Vec::new(),
        }
    } else {
        session::resolve_restore_plan(RestoreRequest {
            mode: cli.restore_mode(),
            explicit_session: cli.session.clone(),
            has_resource_arg: cli.has_resource_arg(),
            argv: raw_args.clone(),
            cwd: cwd.clone(),
        })
    };
    let api_depth = cli
        .api_depth
        .unwrap_or_else(crate::github::api::ApiDepth::from_env);
    let (mut state, fetch_source, initial_fetch, restored_ui) =
        if let Some(path) = &cli.offline_fixture {
            let resource = load_fixture(path)?;
            let fixture_source = OfflineFixtureSource::from_primary_and_paths(
                resource.clone(),
                &cli.offline_resource_fixture,
            )?;
            (
                AppState::new(resource),
                FetchSource::OfflineFixtures(fixture_source),
                None,
                false,
            )
        } else {
            let gateway = GithubApiGateway::new(api_depth);
            let fetch_source = FetchSource::Github(gateway);
            if cli.once {
                let Some(resource_id) = resource_id.clone() else {
                    anyhow::bail!(
                        "expected a GitHub PR/issue URL, owner/repo#number, or owner/repo number"
                    );
                };
                (
                    AppState::new(fetch_source.fetch_resource(&resource_id).await?),
                    fetch_source,
                    None,
                    false,
                )
            } else if let Some(snapshot) = &restore_plan.snapshot {
                let mut state = session::restore_state_from_snapshot(
                    snapshot,
                    &restore_plan
                        .handle
                        .as_ref()
                        .map(|handle| handle.cache_dir.clone())
                        .unwrap_or_else(session::cache_dir),
                )
                .unwrap_or_else(empty_launch_state);
                let initial_fetch =
                    prepare_restored_initial_fetch(&mut state, snapshot, resource_id.clone());
                (state, fetch_source, initial_fetch, true)
            } else {
                let initial_resource = resource_id
                    .clone()
                    .map(loading_resource_placeholder)
                    .unwrap_or_else(empty_launch_resource);
                let mut state = AppState::new(initial_resource);
                if resource_id.is_none() {
                    state.open_add_resource_prompt();
                }
                (
                    state,
                    fetch_source,
                    resource_id.map(|id| FetchAction::Initial { id }),
                    false,
                )
            }
        };

    state.config_path = loaded_config.path.clone();
    if !restored_ui {
        state.theme = loaded_config.config.ui.theme;
        state.symbols = loaded_config.config.ui.symbols;
        state.spacing = loaded_config.config.ui.spacing;
        state.width_mode = loaded_config.config.ui.width_mode;
        state.fixed_width = loaded_config.config.ui.fixed_width;
        state.scrollbar = loaded_config.config.ui.scrollbar;
    }
    if !loaded_config.diagnostics.is_empty() {
        state.last_error = Some(loaded_config.diagnostics.join("; "));
    }
    if !restore_plan.warnings.is_empty() {
        state.last_error = Some(restore_plan.warnings.join("; "));
    }
    if let Some(theme) = cli.theme {
        state.theme = theme;
    }
    if let Some(symbols) = cli.symbols {
        state.symbols = symbols;
    }
    if let Some(spacing) = cli.spacing {
        state.spacing = spacing;
    }
    if let Some(width_mode) = cli.width_mode {
        state.width_mode = width_mode;
    }
    if let Some(fixed_width) = cli.fixed_width {
        state.fixed_width = crate::render::normalize_fixed_width(fixed_width);
    }
    if let Some(scrollbar) = cli.scrollbar {
        state.scrollbar = scrollbar;
    }
    if let Some(tab) = cli.tab {
        state.set_tab(tab);
    }
    if cli.once {
        print_once(&mut state)?;
        return Ok(());
    }

    let mut session_runtime = restore_plan.handle.map(|handle| SessionRuntime {
        handle,
        snapshot: restore_plan.snapshot,
        argv: raw_args,
        cwd,
        dirty: false,
        dirty_since: None,
    });
    if let Some(runtime) = &mut session_runtime {
        persist_session_now(&mut state, runtime);
    }

    run_tui(
        &mut state,
        !cli.no_mouse,
        fetch_source,
        Duration::from_secs(cli.refresh_seconds),
        initial_fetch,
        session_runtime,
    )
    .await
    .context("failed to run terminal UI")
}

struct SessionRuntime {
    handle: SessionHandle,
    snapshot: Option<SessionSnapshot>,
    argv: Vec<String>,
    cwd: std::path::PathBuf,
    dirty: bool,
    dirty_since: Option<Instant>,
}

impl SessionRuntime {
    fn mark_dirty(&mut self, now: Instant) {
        self.dirty = true;
        self.dirty_since = Some(now);
    }
}

fn print_once(state: &mut AppState) -> anyhow::Result<()> {
    let backend = ratatui::backend::TestBackend::new(120, 36);
    let mut terminal = ratatui::Terminal::new(backend)?;
    terminal.draw(|frame| render_app(frame, state))?;
    println!("{:?}", terminal.backend().buffer());
    Ok(())
}

async fn run_tui(
    state: &mut AppState,
    mouse_enabled: bool,
    fetch_source: FetchSource,
    refresh_interval: Duration,
    initial_fetch: Option<FetchAction>,
    mut session_runtime: Option<SessionRuntime>,
) -> anyhow::Result<()> {
    let (_guard, mut terminal) = TerminalGuard::enter(mouse_enabled)?;
    let (fetch_tx, mut fetch_rx) = mpsc::unbounded_channel();
    let (control_tx, mut control_rx) = mpsc::unbounded_channel();
    let _control_server = match &session_runtime {
        Some(runtime) => match control::start_server(&runtime.handle.id, control_tx) {
            Ok(server) => Some(server),
            Err(error) => {
                state.last_error = Some(format!("failed to start ghzinga control socket: {error}"));
                None
            }
        },
        None => None,
    };
    let mut last_refresh = Instant::now();
    if let Some(action) = initial_fetch {
        if start_background_fetch(state, action, fetch_source.clone(), &fetch_tx) {
            last_refresh = Instant::now();
        }
    }
    loop {
        if apply_completed_fetches(state, &mut fetch_rx) {
            if let Some(runtime) = &mut session_runtime {
                persist_session_now(state, runtime);
            }
        }
        maybe_load_file_patches_for_active_files_tab(
            state,
            fetch_source.clone(),
            &fetch_tx,
            &mut last_refresh,
        );
        handle_pending_control_requests(
            state,
            fetch_source.clone(),
            &fetch_tx,
            &mut control_rx,
            &mut session_runtime,
            &mut last_refresh,
        );
        state.advance_loading_frame();
        terminal.draw(|frame| render_app(frame, state))?;
        if state.should_quit {
            if let Some(runtime) = &mut session_runtime {
                persist_session_now(state, runtime);
            }
            return Ok(());
        }
        maybe_refresh_loading_active_resource(
            state,
            fetch_source.clone(),
            &fetch_tx,
            &mut last_refresh,
        );
        maybe_auto_refresh(
            state,
            fetch_source.is_live_github(),
            refresh_interval,
            &mut last_refresh,
            Instant::now(),
            fetch_source.clone(),
            &fetch_tx,
        );
        for app_event in read_pending_app_events()? {
            let intent = apply_event(state, app_event);
            if handle_intent(
                state,
                intent,
                fetch_source.clone(),
                &mut last_refresh,
                &fetch_tx,
            )
            .await
            {
                if let Some(runtime) = &mut session_runtime {
                    persist_session_now(state, runtime);
                }
                return Ok(());
            }
            maybe_refresh_loading_active_resource(
                state,
                fetch_source.clone(),
                &fetch_tx,
                &mut last_refresh,
            );
            maybe_load_file_patches_for_active_files_tab(
                state,
                fetch_source.clone(),
                &fetch_tx,
                &mut last_refresh,
            );
            if let Some(runtime) = &mut session_runtime {
                runtime.mark_dirty(Instant::now());
            }
        }
        if let Some(runtime) = &mut session_runtime {
            persist_session_when_due(state, runtime, Instant::now());
        }
    }
}

fn handle_pending_control_requests(
    state: &mut AppState,
    fetch_source: FetchSource,
    fetch_tx: &UnboundedSender<FetchOutcome>,
    control_rx: &mut UnboundedReceiver<RuntimeRequest>,
    session_runtime: &mut Option<SessionRuntime>,
    last_refresh: &mut Instant,
) {
    while let Ok(request) = control_rx.try_recv() {
        let reply = handle_control_command(
            state,
            request.command,
            fetch_source.clone(),
            fetch_tx,
            session_runtime.as_ref(),
            last_refresh,
        );
        let changed = reply.ok;
        let _ = request.reply.send(reply);
        if changed {
            if let Some(runtime) = session_runtime {
                persist_session_now(state, runtime);
            }
        }
    }
}

fn handle_control_command(
    state: &mut AppState,
    command: RuntimeCommand,
    fetch_source: FetchSource,
    fetch_tx: &UnboundedSender<FetchOutcome>,
    session_runtime: Option<&SessionRuntime>,
    last_refresh: &mut Instant,
) -> ControlReply {
    match command {
        RuntimeCommand::Open { resource, .. } => handle_control_open(
            state,
            resource,
            fetch_source,
            fetch_tx,
            session_runtime,
            last_refresh,
        ),
        RuntimeCommand::Set { key, value, .. } => {
            match apply_control_setting(state, &key, &value) {
                Ok(changed) => {
                    if changed {
                        ControlReply::ok(format!("set {key}={value}"))
                    } else {
                        ControlReply::ok(format!("{key} already {value}"))
                    }
                }
                Err(error) => ControlReply::error(error),
            }
        }
    }
}

fn handle_control_open(
    state: &mut AppState,
    resource: ResourceId,
    fetch_source: FetchSource,
    fetch_tx: &UnboundedSender<FetchOutcome>,
    session_runtime: Option<&SessionRuntime>,
    last_refresh: &mut Instant,
) -> ControlReply {
    let canonical = resource.canonical_name();
    if state.focus_resource_tab(&resource) {
        if start_background_fetch(
            state,
            FetchAction::Refresh {
                id: resource.clone(),
            },
            fetch_source,
            fetch_tx,
        ) {
            *last_refresh = Instant::now();
        }
        return ControlReply::ok(format!("focused {canonical}"));
    }

    let cached = session_runtime.and_then(|runtime| {
        session::load_resource_cache(&runtime.handle.cache_dir, &resource).ok()
    });
    let action = FetchAction::Refresh {
        id: resource.clone(),
    };
    if should_replace_empty_launch_tab(state) {
        state.replace_resource_preserve_tab(
            cached.unwrap_or_else(|| loading_resource_placeholder(resource.clone())),
        );
    } else {
        state.open_resource_in_tab(
            cached.unwrap_or_else(|| loading_resource_placeholder(resource.clone())),
        );
    }
    if start_background_fetch(state, action, fetch_source, fetch_tx) {
        *last_refresh = Instant::now();
    }
    ControlReply::ok(format!("opening {canonical}"))
}

fn apply_control_setting(state: &mut AppState, key: &str, value: &str) -> Result<bool, String> {
    match key.trim().to_ascii_lowercase().replace('_', "-").as_str() {
        "theme" => Ok(state.set_theme(crate::render::ThemeName::from_str(value)?)),
        "symbols" => Ok(state.set_symbols(crate::render::SymbolMode::from_str(value)?)),
        "spacing" => Ok(state.set_spacing(crate::render::SpacingMode::from_str(value)?)),
        "width-mode" => Ok(state.set_width_mode(crate::render::ContentWidthMode::from_str(
            value,
        )?)),
        "fixed-width" => {
            let width = value
                .trim()
                .parse::<u16>()
                .map_err(|_| "fixed-width expects a number".to_string())?;
            Ok(state.set_fixed_width(width))
        }
        "scrollbar" => Ok(state.set_scrollbar(crate::render::ScrollbarMode::from_str(
            value,
        )?)),
        other => Err(format!(
            "unknown setting `{other}`; expected theme, symbols, spacing, width-mode, fixed-width, or scrollbar"
        )),
    }
}

fn persist_session_now(state: &mut AppState, runtime: &mut SessionRuntime) {
    if !session_state_persistable(state) {
        return;
    }
    match session::save_session(
        &runtime.handle,
        runtime.snapshot.as_ref(),
        runtime.argv.clone(),
        runtime.cwd.clone(),
        state,
    ) {
        Ok(snapshot) => {
            runtime.snapshot = Some(snapshot);
            runtime.dirty = false;
            runtime.dirty_since = None;
        }
        Err(error) => {
            state.last_error = Some(format!("failed to save ghzinga session: {error}"));
        }
    }
}

fn persist_session_when_due(state: &mut AppState, runtime: &mut SessionRuntime, now: Instant) {
    if !runtime.dirty {
        return;
    }
    let Some(dirty_since) = runtime.dirty_since else {
        return;
    };
    if now.duration_since(dirty_since) >= SESSION_SAVE_DEBOUNCE {
        persist_session_now(state, runtime);
    }
}

fn session_state_persistable(state: &AppState) -> bool {
    !is_empty_launch_resource(&state.resource)
}

fn empty_launch_state() -> AppState {
    let mut state = AppState::new(empty_launch_resource());
    state.open_add_resource_prompt();
    state
}

fn prepare_restored_initial_fetch(
    state: &mut AppState,
    snapshot: &session::SessionSnapshot,
    resource_id: Option<crate::domain::ResourceId>,
) -> Option<FetchAction> {
    if let Some(resource_id) = resource_id {
        if state.focus_resource_tab(&resource_id) {
            Some(FetchAction::Refresh { id: resource_id })
        } else {
            state.open_resource_in_tab(loading_resource_placeholder(resource_id.clone()));
            Some(FetchAction::Initial { id: resource_id })
        }
    } else {
        session::first_refresh_action(snapshot).map(|id| FetchAction::Refresh { id })
    }
}

fn empty_launch_resource() -> crate::domain::Resource {
    use crate::domain::{MetadataItem, ReactionCounts, Resource, ResourceId, ResourceKind};

    Resource {
        id: ResourceId {
            owner: "dutifuldev".into(),
            repo: "ghzinga".into(),
            number: 1,
            kind_hint: Some(ResourceKind::Issue),
        },
        title: "Open a PR or issue".into(),
        url: "https://github.com/dutifuldev/ghzinga/issues/1".into(),
        state: "READY".into(),
        author: "ghzinga".into(),
        created_at: "now".into(),
        updated_at: "now".into(),
        labels: vec![],
        assignees: vec![],
        reactions: ReactionCounts::default(),
        body: "Use the open-resource prompt to add a GitHub PR or issue.".into(),
        activity: vec![],
        related_resources: vec![],
        metadata: vec![MetadataItem {
            label: "session".into(),
            value: "no restored resource yet".into(),
        }],
        warnings: vec![],
        pull_request: None,
    }
}

fn is_empty_launch_resource(resource: &crate::domain::Resource) -> bool {
    resource.id.owner == "dutifuldev"
        && resource.id.repo == "ghzinga"
        && resource.id.number == 1
        && resource.title == "Open a PR or issue"
        && resource.state == "READY"
        && resource.author == "ghzinga"
        && resource.pull_request.is_none()
}

fn is_loading_resource(resource: &crate::domain::Resource) -> bool {
    resource.state == "LOADING"
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OpenResourceMode {
    NewTab,
    ReplaceCurrent,
}

fn read_pending_app_events() -> anyhow::Result<Vec<AppEvent>> {
    if !event::poll(EVENT_POLL_TIMEOUT)? {
        return Ok(Vec::new());
    }

    let mut events = Vec::with_capacity(MAX_PENDING_EVENTS_PER_FRAME);
    events.push(event_to_app_event(event::read()?));
    while events.len() < MAX_PENDING_EVENTS_PER_FRAME && event::poll(Duration::ZERO)? {
        events.push(event_to_app_event(event::read()?));
    }
    Ok(events.into_iter().flatten().collect())
}

fn event_to_app_event(event: Event) -> Option<AppEvent> {
    match event {
        Event::Key(key) => Some(AppEvent::Key(key)),
        Event::Mouse(mouse) => Some(AppEvent::Mouse(mouse)),
        _ => None,
    }
}

async fn handle_intent(
    state: &mut AppState,
    intent: AppIntent,
    fetch_source: FetchSource,
    last_refresh: &mut Instant,
    fetch_tx: &UnboundedSender<FetchOutcome>,
) -> bool {
    match intent {
        AppIntent::Quit => true,
        AppIntent::Refresh => {
            if fetch_source.is_live_github() {
                let id = state.resource.id.clone();
                if start_background_fetch(
                    state,
                    FetchAction::Refresh { id },
                    fetch_source,
                    fetch_tx,
                ) {
                    *last_refresh = Instant::now();
                }
            } else {
                state.status_message = Some("offline fixture mode: refresh skipped".into());
            }
            false
        }
        AppIntent::LoadFullDepth => {
            if !state.resource.has_partial_depth_warning() {
                state.status_message = Some("full GitHub data is already loaded".into());
            } else if fetch_source.is_live_github() {
                let id = state.resource.id.clone();
                if start_background_fetch(
                    state,
                    FetchAction::LoadFull { id },
                    fetch_source,
                    fetch_tx,
                ) {
                    *last_refresh = Instant::now();
                }
            } else {
                state.status_message = Some("offline fixture mode: full-depth load skipped".into());
            }
            false
        }
        AppIntent::OpenResource(id) => {
            handle_open_resource_intent(
                state,
                id,
                OpenResourceMode::NewTab,
                fetch_source,
                fetch_tx,
                last_refresh,
            );
            false
        }
        AppIntent::ReplaceResource(id) => {
            handle_open_resource_intent(
                state,
                id,
                OpenResourceMode::ReplaceCurrent,
                fetch_source,
                fetch_tx,
                last_refresh,
            );
            false
        }
        AppIntent::Navigate(id) => {
            let from = state.resource.id.clone();
            if start_background_fetch(
                state,
                FetchAction::Navigate { from, to: id },
                fetch_source,
                fetch_tx,
            ) {
                *last_refresh = Instant::now();
            }
            false
        }
        AppIntent::OpenUrl(url) => {
            open_url(state, &url).await;
            false
        }
        AppIntent::CopyUrl(url) => {
            copy_to_clipboard(state, &url).await;
            false
        }
        AppIntent::Back => {
            if fetch_source.is_live_github() || fetch_source.is_offline_fixture() {
                let Some(id) = state.history.last().cloned() else {
                    state.status_message = Some("no previous resource".into());
                    return false;
                };
                if start_background_fetch(
                    state,
                    FetchAction::Back { to: id },
                    fetch_source,
                    fetch_tx,
                ) {
                    let _ = state.pop_history();
                    *last_refresh = Instant::now();
                }
            } else {
                state.status_message = Some("offline fixture mode: no live history".into());
            }
            false
        }
        AppIntent::SaveSettings => {
            save_settings(state);
            false
        }
        AppIntent::None => false,
    }
}

fn handle_open_resource_intent(
    state: &mut AppState,
    id: ResourceId,
    mode: OpenResourceMode,
    fetch_source: FetchSource,
    fetch_tx: &UnboundedSender<FetchOutcome>,
    last_refresh: &mut Instant,
) {
    if !(fetch_source.is_live_github() || fetch_source.is_offline_fixture()) {
        state.status_message = Some("offline fixture mode: open resource skipped".into());
        state.set_add_resource_error("offline fixture mode: open resource skipped");
        return;
    }
    if let Some(message) = state.loading_message() {
        let message = format!("still loading: {message}");
        state.status_message = Some(message.clone());
        state.set_add_resource_error(message);
        return;
    }

    let replace_current =
        mode == OpenResourceMode::ReplaceCurrent || should_replace_empty_launch_tab(state);
    let action = if replace_current {
        state.replace_resource_reset_view(loading_resource_placeholder(id.clone()));
        FetchAction::Initial { id }
    } else {
        state.open_resource_in_tab(loading_resource_placeholder(id.clone()));
        FetchAction::OpenTab { id }
    };
    if start_background_fetch(state, action, fetch_source, fetch_tx) {
        state.close_add_resource_prompt();
        *last_refresh = Instant::now();
    } else if let Some(message) = state.status_message.clone() {
        state.set_add_resource_error(message);
    }
}

fn save_settings(state: &mut AppState) {
    let config = AppConfig::default()
        .with_theme(state.theme)
        .with_symbols(state.symbols)
        .with_spacing(state.spacing)
        .with_width_mode(state.width_mode)
        .with_fixed_width(state.fixed_width)
        .with_scrollbar(state.scrollbar);
    match config::save_to_path(&state.config_path, config) {
        Ok(()) => {
            state.last_error = None;
            state.status_message =
                Some(format!("saved settings to {}", state.config_path.display()));
        }
        Err(error) => {
            state.last_error = Some(format!(
                "failed to save settings to {}: {error}",
                state.config_path.display()
            ));
        }
    }
}

fn maybe_auto_refresh(
    state: &mut AppState,
    live_refresh: bool,
    refresh_interval: Duration,
    last_refresh: &mut Instant,
    now: Instant,
    fetch_source: FetchSource,
    fetch_tx: &UnboundedSender<FetchOutcome>,
) -> bool {
    maybe_auto_refresh_with_start(
        state,
        live_refresh,
        refresh_interval,
        last_refresh,
        now,
        |state, action| start_background_fetch(state, action, fetch_source.clone(), fetch_tx),
    )
}

pub(crate) fn maybe_auto_refresh_with_start(
    state: &mut AppState,
    live_refresh: bool,
    refresh_interval: Duration,
    last_refresh: &mut Instant,
    now: Instant,
    mut start: impl FnMut(&mut AppState, FetchAction) -> bool,
) -> bool {
    if is_loading_resource(&state.resource) {
        return false;
    }
    if !auto_refresh_due(
        live_refresh,
        refresh_interval,
        now.duration_since(*last_refresh),
    ) {
        return false;
    }

    let id = state.resource.id.clone();
    *last_refresh = now;
    start(state, FetchAction::Refresh { id })
}

fn auto_refresh_due(live_refresh: bool, refresh_interval: Duration, elapsed: Duration) -> bool {
    live_refresh && refresh_interval.as_secs() > 0 && elapsed >= refresh_interval
}

fn maybe_refresh_loading_active_resource(
    state: &mut AppState,
    fetch_source: FetchSource,
    fetch_tx: &UnboundedSender<FetchOutcome>,
    last_refresh: &mut Instant,
) -> bool {
    if !is_loading_resource(&state.resource) || is_empty_launch_resource(&state.resource) {
        return false;
    }
    if state.loading_message().is_some() {
        return false;
    }
    if state.last_error.is_some() {
        return false;
    }
    if !(fetch_source.is_live_github() || fetch_source.is_offline_fixture()) {
        return false;
    }
    let id = state.resource.id.clone();
    if start_background_fetch(state, FetchAction::Refresh { id }, fetch_source, fetch_tx) {
        *last_refresh = Instant::now();
        true
    } else {
        false
    }
}

fn maybe_load_file_patches_for_active_files_tab(
    state: &mut AppState,
    fetch_source: FetchSource,
    fetch_tx: &UnboundedSender<FetchOutcome>,
    last_refresh: &mut Instant,
) -> bool {
    maybe_load_file_patches_with_start(
        state,
        fetch_source.is_live_github(),
        last_refresh,
        |state, action| start_background_fetch(state, action, fetch_source.clone(), fetch_tx),
    )
}

pub(crate) fn maybe_load_file_patches_with_start(
    state: &mut AppState,
    live_github: bool,
    last_refresh: &mut Instant,
    mut start: impl FnMut(&mut AppState, FetchAction) -> bool,
) -> bool {
    if !live_github
        || state.active_tab != Tab::Files
        || state.loading_message().is_some()
        || state.file_patch_loading_message().is_some()
    {
        return false;
    }
    if !resource_needs_file_patches(&state.resource) {
        return false;
    }
    let resource = state.resource.clone();
    if start(
        state,
        FetchAction::LoadFilePatches {
            resource: Box::new(resource),
        },
    ) {
        *last_refresh = Instant::now();
        true
    } else {
        false
    }
}

fn resource_needs_file_patches(resource: &Resource) -> bool {
    if resource
        .warnings
        .iter()
        .any(|warning| warning.starts_with(FILE_PATCH_CONTEXT_UNAVAILABLE_WARNING))
    {
        return false;
    }
    resource
        .pull_request
        .as_ref()
        .is_some_and(|pr| pr.files.iter().any(|file| file.patch.is_none()))
}

fn should_replace_empty_launch_tab(state: &AppState) -> bool {
    state.resource_tabs.len() == 1 && is_empty_launch_resource(&state.resource)
}

async fn open_url(state: &mut AppState, url: &str) {
    let (program, args) = url_open_command(url, std::env::var("BROWSER").ok().as_deref());
    let mut command = Command::new(&program);
    command.args(&args);

    match command.stderr(Stdio::piped()).output().await {
        Ok(output) if output.status.success() => {
            state.last_error = None;
            state.status_message = Some(format!("opened {url}"));
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let details = stderr.trim();
            state.last_error = Some(if details.is_empty() {
                format!("`{program}` failed to open URL without an error message")
            } else {
                format!("`{program}` failed to open URL: {details}")
            });
        }
        Err(error) => {
            state.last_error = Some(format!(
                "failed to execute `{program}` for URL open: {error}"
            ));
        }
    }
}

async fn copy_to_clipboard(state: &mut AppState, text: &str) {
    let Some((program, args)) = current_clipboard_command() else {
        state.last_error = Some(
            "no clipboard command available; set GZG_COPY_COMMAND to a command that reads stdin"
                .into(),
        );
        return;
    };

    let mut child = match Command::new(&program)
        .args(&args)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            state.last_error = Some(format!(
                "failed to execute `{program}` for clipboard copy: {error}"
            ));
            return;
        }
    };

    let mut stdin = child.stdin.take().expect("stdin was configured as piped");
    if let Err(error) = stdin.write_all(text.as_bytes()).await {
        let _ = child.kill().await;
        state.last_error = Some(format!(
            "failed to write clipboard text to `{program}`: {error}"
        ));
        return;
    }
    drop(stdin);

    match child.wait_with_output().await {
        Ok(output) if output.status.success() => {
            state.last_error = None;
            state.status_message = Some(format!("copied {text}"));
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let details = stderr.trim();
            state.last_error = Some(if details.is_empty() {
                format!("`{program}` failed to copy without an error message")
            } else {
                format!("`{program}` failed to copy: {details}")
            });
        }
        Err(error) => {
            state.last_error = Some(format!(
                "failed to wait for `{program}` during clipboard copy: {error}"
            ));
        }
    }
}

fn url_open_command(url: &str, browser: Option<&str>) -> (String, Vec<String>) {
    if let Some(browser) = browser.map(str::trim).filter(|value| !value.is_empty()) {
        let mut parts = browser
            .split_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let program = parts.remove(0);
        parts.push(url.to_string());
        return (program, parts);
    }

    #[cfg(target_os = "macos")]
    {
        ("open".into(), vec![url.into()])
    }

    #[cfg(target_os = "windows")]
    {
        return (
            "cmd".into(),
            vec!["/C".into(), "start".into(), "".into(), url.into()],
        );
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        ("xdg-open".into(), vec![url.into()])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipboardPlatform {
    #[cfg(any(target_os = "macos", test))]
    Macos,
    #[cfg(any(target_os = "windows", test))]
    Windows,
    #[cfg(any(not(any(target_os = "macos", target_os = "windows")), test))]
    Unix,
}

fn current_clipboard_command() -> Option<(String, Vec<String>)> {
    clipboard_command(
        current_clipboard_platform(),
        std::env::var("GZG_COPY_COMMAND").ok().as_deref(),
        std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
        std::env::var("DISPLAY").ok().as_deref(),
    )
}

fn current_clipboard_platform() -> ClipboardPlatform {
    #[cfg(target_os = "macos")]
    {
        ClipboardPlatform::Macos
    }

    #[cfg(target_os = "windows")]
    {
        return ClipboardPlatform::Windows;
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        ClipboardPlatform::Unix
    }
}

fn clipboard_command(
    platform: ClipboardPlatform,
    explicit_command: Option<&str>,
    _wayland_display: Option<&str>,
    _x11_display: Option<&str>,
) -> Option<(String, Vec<String>)> {
    if let Some(command) = explicit_command
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let mut parts = command
            .split_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let program = parts.remove(0);
        return Some((program, parts));
    }

    match platform {
        #[cfg(any(target_os = "macos", test))]
        ClipboardPlatform::Macos => Some(("pbcopy".into(), vec![])),
        #[cfg(any(target_os = "windows", test))]
        ClipboardPlatform::Windows => Some(("clip".into(), vec![])),
        #[cfg(any(not(any(target_os = "macos", target_os = "windows")), test))]
        ClipboardPlatform::Unix => {
            if _wayland_display
                .map(str::trim)
                .is_some_and(|value| !value.is_empty())
            {
                Some(("wl-copy".into(), vec![]))
            } else if _x11_display
                .map(str::trim)
                .is_some_and(|value| !value.is_empty())
            {
                Some((
                    "xclip".into(),
                    vec!["-selection".into(), "clipboard".into()],
                ))
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
async fn navigate_to_resource<G: crate::github::api::GithubGateway>(
    state: &mut AppState,
    id: crate::domain::ResourceId,
    gateway: &G,
) {
    match gateway.fetch_resource(&id).await {
        Ok(resource) => {
            state.push_current_to_history();
            state.replace_resource_reset_view(resource);
            state.last_error = None;
            state.status_message = None;
        }
        Err(error) => {
            state.last_error = Some(error.to_string());
        }
    }
}

#[cfg(test)]
async fn navigate_back<G: crate::github::api::GithubGateway>(state: &mut AppState, gateway: &G) {
    let Some(id) = state.pop_history() else {
        state.status_message = Some("no previous resource".into());
        return;
    };
    match gateway.fetch_resource(&id).await {
        Ok(resource) => {
            state.replace_resource_reset_view(resource);
            state.last_error = None;
            state.status_message = None;
        }
        Err(error) => {
            state.last_error = Some(error.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::Mutex,
        time::{Duration, Instant},
    };

    use crate::{
        app::{loading_resource_placeholder, AppIntent, AppState, Tab},
        domain::{
            ChangedFile, PullRequest, ReactionCounts, Resource, ResourceId, ResourceKind,
            FILE_PATCH_CONTEXT_UNAVAILABLE_WARNING, FULL_DEPTH_WARNING_HINT,
        },
        fetch::{apply_completed_fetches, FetchAction, FetchSource, OfflineFixtureSource},
        github::api::GithubGateway,
        session::{
            self, BlockIdSnapshot, LaunchContext, LaunchSnapshot, ResourceTabSnapshot,
            ResourcesSnapshot, SessionHandle, SessionSnapshot, UiSnapshot,
        },
    };

    use super::{
        apply_control_setting, auto_refresh_due, clipboard_command, empty_launch_resource,
        handle_control_open, handle_intent, maybe_auto_refresh_with_start,
        maybe_load_file_patches_with_start, maybe_refresh_loading_active_resource, navigate_back,
        navigate_to_resource, parse_resource_args, prepare_restored_initial_fetch,
        resource_count_label, save_open_commands_to_session, session_state_persistable,
        should_replace_empty_launch_tab, url_open_command, ClipboardPlatform,
    };

    struct FakeGateway {
        resources: Mutex<VecDeque<Resource>>,
        requested: Mutex<Vec<ResourceId>>,
    }

    impl FakeGateway {
        fn new(resources: Vec<Resource>) -> Self {
            Self {
                resources: Mutex::new(resources.into()),
                requested: Mutex::new(Vec::new()),
            }
        }

        fn requested(&self) -> Vec<ResourceId> {
            self.requested.lock().unwrap().clone()
        }
    }

    impl GithubGateway for FakeGateway {
        async fn fetch_resource(&self, id: &ResourceId) -> anyhow::Result<Resource> {
            self.requested.lock().unwrap().push(id.clone());
            self.resources
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| anyhow::anyhow!("no fake resource queued"))
        }

        async fn fetch_resource_base(&self, id: &ResourceId) -> anyhow::Result<Resource> {
            self.fetch_resource(id).await
        }

        async fn enrich_resource(&self, resource: Resource) -> anyhow::Result<Resource> {
            Ok(resource)
        }

        async fn enrich_file_patches(&self, resource: Resource) -> anyhow::Result<Resource> {
            Ok(resource)
        }
    }

    fn issue_resource(number: u64, title: &str) -> Resource {
        Resource {
            id: ResourceId {
                owner: "owner".into(),
                repo: "repo".into(),
                number,
                kind_hint: Some(ResourceKind::Issue),
            },
            title: title.into(),
            url: format!("https://github.com/owner/repo/issues/{number}"),
            state: "OPEN".into(),
            author: "alice".into(),
            created_at: "now".into(),
            updated_at: "now".into(),
            labels: vec![],
            assignees: vec![],
            reactions: ReactionCounts::default(),
            body: "Body".into(),
            activity: vec![],
            related_resources: vec![],
            metadata: vec![],
            warnings: vec![],
            pull_request: None,
        }
    }

    fn pr_resource_with_patch(patch: Option<&str>) -> Resource {
        let mut resource = issue_resource(1, "Pull request");
        resource.id.kind_hint = Some(ResourceKind::PullRequest);
        resource.url = "https://github.com/owner/repo/pull/1".into();
        resource.pull_request = Some(PullRequest {
            base_ref: "main".into(),
            head_ref: "feature".into(),
            requested_reviewers: vec![],
            review_decision: None,
            merge_state: None,
            additions: 1,
            deletions: 0,
            commits: vec![],
            checks: vec![],
            files: vec![ChangedFile {
                path: "src/lib.rs".into(),
                additions: 1,
                deletions: 0,
                change_type: "MODIFIED".into(),
                patch: patch.map(str::to_string),
            }],
            metadata: vec![],
        });
        resource
    }

    fn session_snapshot_for(resource: &Resource) -> SessionSnapshot {
        SessionSnapshot {
            schema_version: 1,
            id: "work".into(),
            name: None,
            created_at: "1".into(),
            updated_at: "1".into(),
            launch: LaunchSnapshot {
                argv: vec!["gzg".into()],
                cwd: std::path::PathBuf::from("/repo"),
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
                    resource: resource.id.canonical_name(),
                    kind_hint: resource.id.kind_hint,
                    view: "activity".into(),
                    scroll: 5,
                    reverse_chronological: false,
                    expanded_blocks: vec![BlockIdSnapshot::Body],
                }],
            },
        }
    }

    #[test]
    fn url_open_command_uses_browser_env_when_available() {
        assert_eq!(
            url_open_command(
                "https://github.com/openclaw/openclaw/actions/runs/1",
                Some("echo")
            ),
            (
                "echo".into(),
                vec!["https://github.com/openclaw/openclaw/actions/runs/1".into()]
            )
        );
    }

    #[test]
    fn url_open_command_preserves_browser_arguments() {
        assert_eq!(
            url_open_command(
                "https://github.com/openclaw/openclaw/actions/runs/1",
                Some("browser --new-window")
            ),
            (
                "browser".into(),
                vec![
                    "--new-window".into(),
                    "https://github.com/openclaw/openclaw/actions/runs/1".into()
                ]
            )
        );
    }

    #[test]
    fn clipboard_command_uses_explicit_env_command() {
        assert_eq!(
            clipboard_command(
                ClipboardPlatform::Unix,
                Some("tmux load-buffer -"),
                Some("wayland-1"),
                Some(":0"),
            ),
            Some(("tmux".into(), vec!["load-buffer".into(), "-".into()]))
        );
    }

    #[test]
    fn clipboard_command_prefers_wayland_when_available() {
        assert_eq!(
            clipboard_command(ClipboardPlatform::Unix, None, Some("wayland-1"), Some(":0")),
            Some(("wl-copy".into(), vec![]))
        );
    }

    #[test]
    fn clipboard_command_uses_xclip_for_x11() {
        assert_eq!(
            clipboard_command(ClipboardPlatform::Unix, None, None, Some(":0")),
            Some((
                "xclip".into(),
                vec!["-selection".into(), "clipboard".into()]
            ))
        );
    }

    #[test]
    fn clipboard_command_is_unavailable_without_unix_display() {
        assert_eq!(
            clipboard_command(ClipboardPlatform::Unix, None, None, None),
            None
        );
    }

    #[test]
    fn clipboard_command_uses_platform_defaults() {
        assert_eq!(
            clipboard_command(ClipboardPlatform::Macos, None, None, None),
            Some(("pbcopy".into(), vec![]))
        );
        assert_eq!(
            clipboard_command(ClipboardPlatform::Windows, None, None, None),
            Some(("clip".into(), vec![]))
        );
    }

    #[test]
    fn empty_launch_placeholder_is_not_persistable() {
        let state = AppState::new(empty_launch_resource());

        assert!(!session_state_persistable(&state));
    }

    #[test]
    fn empty_launch_placeholder_first_open_replaces_tab() {
        let mut state = AppState::new(empty_launch_resource());

        assert!(should_replace_empty_launch_tab(&state));

        state.replace_resource_preserve_tab(issue_resource(2, "Real issue"));

        assert_eq!(state.resource_tabs.len(), 1);
        assert_eq!(state.resource.id.number, 2);
        assert!(session_state_persistable(&state));
    }

    #[test]
    fn restored_cached_resource_arg_refreshes_existing_tab() {
        let resource = issue_resource(2, "Cached issue");
        let snapshot = session_snapshot_for(&resource);
        let mut state = AppState::new(resource.clone());
        state.set_tab(crate::app::Tab::Activity);
        state.scroll_down(5);

        let action =
            prepare_restored_initial_fetch(&mut state, &snapshot, Some(resource.id.clone()));

        assert!(matches!(action, Some(FetchAction::Refresh { .. })));
        assert_eq!(state.resource.title, "Cached issue");
        assert_eq!(state.active_tab, crate::app::Tab::Activity);
        assert_eq!(state.resource_tabs.len(), 1);
    }

    #[test]
    fn restored_missing_resource_arg_opens_loading_placeholder() {
        let resource = issue_resource(1, "Cached issue");
        let missing = ResourceId {
            owner: "owner".into(),
            repo: "repo".into(),
            number: 2,
            kind_hint: None,
        };
        let snapshot = session_snapshot_for(&resource);
        let mut state = AppState::new(resource);

        let action = prepare_restored_initial_fetch(&mut state, &snapshot, Some(missing));

        assert!(matches!(action, Some(FetchAction::Initial { .. })));
        assert_eq!(state.resource.title, "Loading owner/repo#2");
        assert_eq!(state.resource_tabs.len(), 2);
    }

    #[test]
    fn restored_session_without_resource_arg_refreshes_active_tab() {
        let resource = issue_resource(1, "Cached issue");
        let snapshot = session_snapshot_for(&resource);
        let mut state = AppState::new(resource);

        let action = prepare_restored_initial_fetch(&mut state, &snapshot, None);

        assert!(matches!(action, Some(FetchAction::Refresh { .. })));
    }

    #[test]
    fn saved_open_command_creates_or_updates_session_snapshot() {
        let state_dir = tempfile::tempdir().unwrap();
        let cache_dir = tempfile::tempdir().unwrap();
        let handle = SessionHandle {
            id: "work".into(),
            state_dir: state_dir.path().into(),
            cache_dir: cache_dir.path().into(),
            contexts: Vec::<LaunchContext>::new(),
            ephemeral: false,
        };
        let id = ResourceId {
            owner: "owner".into(),
            repo: "repo".into(),
            number: 2,
            kind_hint: Some(ResourceKind::Issue),
        };

        save_open_commands_to_session(
            &handle,
            None,
            vec!["gzg".into(), "open".into(), "owner/repo#2".into()],
            std::path::PathBuf::from("/repo"),
            &[id],
        )
        .unwrap();

        let snapshot = session::load_snapshot(&handle.session_path()).unwrap();
        assert_eq!(snapshot.resources.tabs.len(), 1);
        assert_eq!(snapshot.resources.tabs[0].resource, "owner/repo#2");
        assert_eq!(snapshot.resources.active_index, 0);
    }

    #[test]
    fn open_parser_accepts_multiple_single_token_resources() {
        let resources = parse_resource_args(&[
            "owner/repo#2".to_string(),
            "https://github.com/owner/repo/issues/3".to_string(),
        ])
        .unwrap();

        assert_eq!(resources.len(), 2);
        assert_eq!(resources[0].canonical_name(), "owner/repo#2");
        assert_eq!(resources[1].canonical_name(), "owner/repo#3");
    }

    #[test]
    fn open_parser_keeps_owner_repo_number_single_resource_form() {
        let resources = parse_resource_args(&["owner/repo".to_string(), "4".to_string()]).unwrap();

        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0].canonical_name(), "owner/repo#4");
    }

    #[test]
    fn session_resource_count_label_uses_singular_and_plural() {
        assert_eq!(resource_count_label(1), "1 resource");
        assert_eq!(resource_count_label(2), "2 resources");
    }

    #[test]
    fn control_open_focuses_existing_tab_while_fetch_is_loading() {
        let initial = issue_resource(1, "Initial issue");
        let second = issue_resource(2, "Second issue");
        let second_id = second.id.clone();
        let mut state = AppState::new(initial.clone());
        state.open_resource_in_tab(second.clone());
        state.switch_resource_tab(0);
        state.begin_loading(initial.id.clone(), "refreshing owner/repo#1 from GitHub");
        let (fetch_tx, mut fetch_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut last_refresh = Instant::now();

        let reply = handle_control_open(
            &mut state,
            second_id.clone(),
            FetchSource::OfflineFixtures(OfflineFixtureSource::new([initial, second])),
            &fetch_tx,
            None,
            &mut last_refresh,
        );

        assert!(reply.ok);
        assert_eq!(reply.result.as_deref(), Some("focused owner/repo#2"));
        assert_eq!(
            state.resource.id.canonical_name(),
            second_id.canonical_name()
        );
        assert_eq!(state.resource_tabs.len(), 2);
        assert_eq!(
            state.loading_message(),
            Some("refreshing owner/repo#1 from GitHub")
        );
        assert_eq!(
            state.status_message.as_deref(),
            Some("still loading: refreshing owner/repo#1 from GitHub")
        );
        assert!(fetch_rx.try_recv().is_err());
    }

    #[test]
    fn control_open_adds_tab_while_fetch_is_loading() {
        let initial = issue_resource(1, "Initial issue");
        let second = issue_resource(2, "Second issue");
        let second_id = second.id.clone();
        let mut state = AppState::new(initial.clone());
        state.begin_loading(initial.id.clone(), "refreshing owner/repo#1 from GitHub");
        let (fetch_tx, mut fetch_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut last_refresh = Instant::now();

        let reply = handle_control_open(
            &mut state,
            second_id.clone(),
            FetchSource::OfflineFixtures(OfflineFixtureSource::new([initial, second])),
            &fetch_tx,
            None,
            &mut last_refresh,
        );

        assert!(reply.ok);
        assert_eq!(reply.result.as_deref(), Some("opening owner/repo#2"));
        assert_eq!(
            state.resource.id.canonical_name(),
            second_id.canonical_name()
        );
        assert_eq!(state.resource.state, "LOADING");
        assert_eq!(state.resource_tabs.len(), 2);
        assert_eq!(
            state.loading_message(),
            Some("refreshing owner/repo#1 from GitHub")
        );
        assert_eq!(
            state.status_message.as_deref(),
            Some("still loading: refreshing owner/repo#1 from GitHub")
        );
        assert!(fetch_rx.try_recv().is_err());
    }

    #[test]
    fn control_setting_helper_uses_same_state_setters() {
        let mut state = AppState::new(issue_resource(1, "Issue"));

        assert!(apply_control_setting(&mut state, "spacing", "compact").unwrap());
        assert_eq!(state.spacing, crate::render::SpacingMode::Compact);
        assert!(apply_control_setting(&mut state, "width-mode", "full").unwrap());
        assert_eq!(state.width_mode, crate::render::ContentWidthMode::Full);
        assert!(apply_control_setting(&mut state, "fixed-width", "999").unwrap());
        assert_eq!(state.fixed_width, crate::render::MAX_FIXED_CONTENT_WIDTH);
        assert!(apply_control_setting(&mut state, "unknown", "value").is_err());
    }

    #[test]
    fn auto_refresh_due_requires_live_mode_positive_interval_and_elapsed_time() {
        assert!(auto_refresh_due(
            true,
            Duration::from_secs(30),
            Duration::from_secs(30)
        ));
        assert!(!auto_refresh_due(
            false,
            Duration::from_secs(30),
            Duration::from_secs(30)
        ));
        assert!(!auto_refresh_due(
            true,
            Duration::from_secs(0),
            Duration::from_secs(30)
        ));
        assert!(!auto_refresh_due(
            true,
            Duration::from_secs(30),
            Duration::from_secs(29)
        ));
    }

    #[tokio::test]
    async fn full_depth_intent_skips_offline_fixture_mode() {
        let mut fixture = issue_resource(1, "Initial issue");
        fixture.warnings.push(format!(
            "normal API depth shows the first 100 only for comments; {FULL_DEPTH_WARNING_HINT} for exhaustive pagination"
        ));
        let mut state = AppState::new(fixture.clone());
        let (fetch_tx, mut fetch_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut last_refresh = Instant::now();

        let should_quit = handle_intent(
            &mut state,
            AppIntent::LoadFullDepth,
            FetchSource::OfflineFixtures(OfflineFixtureSource::new([fixture])),
            &mut last_refresh,
            &fetch_tx,
        )
        .await;

        assert!(!should_quit);
        assert_eq!(
            state.status_message.as_deref(),
            Some("offline fixture mode: full-depth load skipped")
        );
        assert!(fetch_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn open_resource_prompt_stays_open_when_fetch_cannot_start() {
        let fixture = issue_resource(1, "Initial issue");
        let mut state = AppState::new(fixture.clone());
        state.open_add_resource_prompt();
        state
            .add_resource_input_mut()
            .unwrap()
            .push_str("owner/repo#2");
        state.begin_loading(
            state.resource.id.clone(),
            "refreshing owner/repo#1 from GitHub",
        );
        let (fetch_tx, mut fetch_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut last_refresh = Instant::now();

        let should_quit = handle_intent(
            &mut state,
            AppIntent::OpenResource(ResourceId {
                owner: "owner".into(),
                repo: "repo".into(),
                number: 2,
                kind_hint: None,
            }),
            FetchSource::OfflineFixtures(OfflineFixtureSource::new([fixture])),
            &mut last_refresh,
            &fetch_tx,
        )
        .await;

        assert!(!should_quit);
        let prompt = state.add_resource_prompt.as_ref().unwrap();
        assert_eq!(prompt.input, "owner/repo#2");
        assert_eq!(
            prompt.error.as_deref(),
            Some("still loading: refreshing owner/repo#1 from GitHub")
        );
        assert!(fetch_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn open_resource_new_tab_shows_loading_tab_immediately() {
        let fixture = issue_resource(1, "Initial issue");
        let fetched = issue_resource(2, "Fetched issue");
        let id = fetched.id.clone();
        let mut state = AppState::new(fixture.clone());
        state.open_add_resource_prompt();
        let (fetch_tx, _fetch_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut last_refresh = Instant::now();

        let should_quit = handle_intent(
            &mut state,
            AppIntent::OpenResource(id.clone()),
            FetchSource::OfflineFixtures(OfflineFixtureSource::new([fixture, fetched])),
            &mut last_refresh,
            &fetch_tx,
        )
        .await;

        assert!(!should_quit);
        assert_eq!(state.resource.id.canonical_name(), id.canonical_name());
        assert_eq!(state.resource.state, "LOADING");
        assert_eq!(state.resource_tabs.len(), 2);
        assert!(state.add_resource_prompt.is_none());
        assert!(state.loading_message().is_some());
    }

    #[tokio::test]
    async fn replace_resource_shows_loading_placeholder_immediately() {
        let fixture = issue_resource(1, "Initial issue");
        let fetched = issue_resource(2, "Fetched issue");
        let id = fetched.id.clone();
        let mut state = AppState::new(fixture.clone());
        state.open_replace_resource_prompt();
        let (fetch_tx, _fetch_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut last_refresh = Instant::now();

        let should_quit = handle_intent(
            &mut state,
            AppIntent::ReplaceResource(id.clone()),
            FetchSource::OfflineFixtures(OfflineFixtureSource::new([fixture, fetched])),
            &mut last_refresh,
            &fetch_tx,
        )
        .await;

        assert!(!should_quit);
        assert_eq!(state.resource.id.canonical_name(), id.canonical_name());
        assert_eq!(state.resource.state, "LOADING");
        assert_eq!(state.resource_tabs.len(), 1);
        assert!(state.add_resource_prompt.is_none());
        assert!(state.loading_message().is_some());
    }

    #[tokio::test]
    async fn focused_loading_placeholder_starts_refresh() {
        let id = ResourceId {
            owner: "owner".into(),
            repo: "repo".into(),
            number: 2,
            kind_hint: Some(ResourceKind::Issue),
        };
        let fetched = issue_resource(2, "Fetched issue");
        let mut state = AppState::new(loading_resource_placeholder(id.clone()));
        let (fetch_tx, mut fetch_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut last_refresh = Instant::now();

        assert!(maybe_refresh_loading_active_resource(
            &mut state,
            FetchSource::OfflineFixtures(OfflineFixtureSource::new([fetched])),
            &fetch_tx,
            &mut last_refresh,
        ));

        for _ in 0..10 {
            if apply_completed_fetches(&mut state, &mut fetch_rx) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        assert_eq!(state.resource.title, "Fetched issue");
        assert_eq!(state.resource.state, "OPEN");
    }

    #[test]
    fn failed_loading_placeholder_waits_for_explicit_retry() {
        let id = ResourceId {
            owner: "owner".into(),
            repo: "repo".into(),
            number: 2,
            kind_hint: Some(ResourceKind::Issue),
        };
        let mut state = AppState::new(loading_resource_placeholder(id));
        state.last_error = Some("network down".into());
        let (fetch_tx, mut fetch_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut last_refresh = Instant::now();

        assert!(!maybe_refresh_loading_active_resource(
            &mut state,
            FetchSource::OfflineFixtures(OfflineFixtureSource::new([issue_resource(
                2,
                "Fetched issue"
            )])),
            &fetch_tx,
            &mut last_refresh,
        ));

        assert!(fetch_rx.try_recv().is_err());
        assert_eq!(state.last_error.as_deref(), Some("network down"));
        assert!(state.loading.is_none());
    }

    #[test]
    fn automatic_refresh_waits_until_interval_is_due() {
        let initial = issue_resource(1, "Initial issue");
        let mut state = AppState::new(initial.clone());
        let mut last_refresh = Instant::now();
        let now = last_refresh + Duration::from_secs(29);
        let mut started = false;

        let refreshed = maybe_auto_refresh_with_start(
            &mut state,
            true,
            Duration::from_secs(30),
            &mut last_refresh,
            now,
            |_, _| {
                started = true;
                true
            },
        );

        assert!(!refreshed);
        assert_eq!(state.resource.title, initial.title);
        assert!(!started);
    }

    #[test]
    fn automatic_refresh_throttles_due_attempt_while_fetch_is_in_progress() {
        let initial = issue_resource(1, "Initial issue");
        let mut state = AppState::new(initial);
        state.begin_loading(
            state.resource.id.clone(),
            "refreshing owner/repo#1 from GitHub",
        );
        let mut last_refresh = Instant::now();
        let now = last_refresh + Duration::from_secs(30);
        let mut attempts = 0;

        let refreshed = maybe_auto_refresh_with_start(
            &mut state,
            true,
            Duration::from_secs(30),
            &mut last_refresh,
            now,
            |state, _| {
                attempts += 1;
                state.status_message = Some(format!(
                    "still loading: {}",
                    state.loading_message().unwrap()
                ));
                false
            },
        );

        assert!(!refreshed);
        assert_eq!(attempts, 1);
        assert_eq!(last_refresh, now);
        assert_eq!(
            state.status_message.as_deref(),
            Some("still loading: refreshing owner/repo#1 from GitHub")
        );

        let next_frame = now + Duration::from_millis(250);
        let refreshed = maybe_auto_refresh_with_start(
            &mut state,
            true,
            Duration::from_secs(30),
            &mut last_refresh,
            next_frame,
            |_, _| {
                attempts += 1;
                false
            },
        );

        assert!(!refreshed);
        assert_eq!(attempts, 1);
        assert_eq!(last_refresh, now);
    }

    #[test]
    fn files_tab_schedules_missing_file_patches_on_demand() {
        let mut state = AppState::new(pr_resource_with_patch(None));
        state.set_tab(Tab::Files);
        let mut last_refresh = Instant::now();
        let mut started = Vec::new();

        let scheduled =
            maybe_load_file_patches_with_start(&mut state, true, &mut last_refresh, |_, action| {
                started.push(action);
                true
            });

        assert!(scheduled);
        assert_eq!(started.len(), 1);
        match &started[0] {
            FetchAction::LoadFilePatches { resource } => {
                assert_eq!(resource.id.canonical_name(), "owner/repo#1");
                assert!(resource
                    .pull_request
                    .as_ref()
                    .unwrap()
                    .files
                    .iter()
                    .any(|file| file.patch.is_none()));
            }
            other => panic!("expected file patch load, got {other:?}"),
        }
    }

    #[test]
    fn file_patch_loading_waits_for_files_tab_missing_patches_and_idle_load_state() {
        let mut not_files = AppState::new(pr_resource_with_patch(None));
        let mut last_refresh = Instant::now();
        let mut started = Vec::new();

        assert!(!maybe_load_file_patches_with_start(
            &mut not_files,
            true,
            &mut last_refresh,
            |_, action| {
                started.push(action);
                true
            }
        ));

        let mut already_loaded = AppState::new(pr_resource_with_patch(Some("@@ patch")));
        already_loaded.set_tab(Tab::Files);
        assert!(!maybe_load_file_patches_with_start(
            &mut already_loaded,
            true,
            &mut last_refresh,
            |_, action| {
                started.push(action);
                true
            }
        ));

        let mut already_loading_patches = AppState::new(pr_resource_with_patch(None));
        already_loading_patches.set_tab(Tab::Files);
        already_loading_patches.begin_file_patch_loading();
        assert!(!maybe_load_file_patches_with_start(
            &mut already_loading_patches,
            true,
            &mut last_refresh,
            |_, action| {
                started.push(action);
                true
            }
        ));

        let mut unavailable = AppState::new(pr_resource_with_patch(None));
        unavailable.set_tab(Tab::Files);
        unavailable.resource.warnings.push(format!(
            "{FILE_PATCH_CONTEXT_UNAVAILABLE_WARNING}: rate limited"
        ));
        assert!(!maybe_load_file_patches_with_start(
            &mut unavailable,
            true,
            &mut last_refresh,
            |_, action| {
                started.push(action);
                true
            }
        ));

        assert!(started.is_empty());
    }

    #[test]
    fn automatic_refresh_skips_loading_placeholders() {
        let id = ResourceId {
            owner: "owner".into(),
            repo: "repo".into(),
            number: 2,
            kind_hint: Some(ResourceKind::Issue),
        };
        let mut state = AppState::new(loading_resource_placeholder(id));
        let mut last_refresh = Instant::now();
        let now = last_refresh + Duration::from_secs(30);
        let mut attempts = 0;

        let refreshed = maybe_auto_refresh_with_start(
            &mut state,
            true,
            Duration::from_secs(30),
            &mut last_refresh,
            now,
            |_, _| {
                attempts += 1;
                true
            },
        );

        assert!(!refreshed);
        assert_eq!(attempts, 0);
        assert_ne!(last_refresh, now);
    }

    #[tokio::test]
    async fn navigation_loads_target_and_back_restores_previous_resource() {
        let initial = issue_resource(1, "Initial issue");
        let target = issue_resource(2, "Linked issue");
        let gateway = FakeGateway::new(vec![target, initial.clone()]);
        let mut state = AppState::new(initial.clone());
        let target_id = ResourceId {
            owner: "owner".into(),
            repo: "repo".into(),
            number: 2,
            kind_hint: Some(ResourceKind::Issue),
        };

        navigate_to_resource(&mut state, target_id.clone(), &gateway).await;

        assert_eq!(state.resource.id, target_id);
        assert_eq!(state.resource.title, "Linked issue");
        assert_eq!(state.history.as_slice(), std::slice::from_ref(&initial.id));
        assert!(state.status_message.is_none());

        navigate_back(&mut state, &gateway).await;

        assert_eq!(state.resource.id, initial.id);
        assert_eq!(state.resource.title, "Initial issue");
        assert!(state.history.is_empty());
        assert!(state.status_message.is_none());
        assert_eq!(
            gateway
                .requested()
                .into_iter()
                .map(|id| id.number)
                .collect::<Vec<_>>(),
            [2, 1]
        );
    }
}

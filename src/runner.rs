use std::{
    process::Stdio,
    time::{Duration, Instant},
};

use crate::{
    app::{apply_event, loading_resource_placeholder, AppEvent, AppIntent, AppState},
    cli::Cli,
    config::{self, AppConfig},
    fetch::{
        apply_completed_fetches, start_background_fetch, FetchAction, FetchOutcome, FetchSource,
        OfflineFixtureSource,
    },
    github::{api::GithubApiGateway, load_fixture},
    render::render_app,
    terminal::TerminalGuard,
};
use anyhow::Context;
use clap::Parser;
use crossterm::event::{self, Event};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::mpsc::{self, UnboundedSender};

const EVENT_POLL_TIMEOUT: Duration = Duration::from_millis(250);
const MAX_PENDING_EVENTS_PER_FRAME: usize = 64;

pub async fn run_from_cli() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let loaded_config = config::load();
    let resource_id = cli.parse_resource_id()?;
    let api_depth = cli
        .api_depth
        .unwrap_or_else(crate::github::api::ApiDepth::from_env);
    let (mut state, fetch_source, initial_fetch) = if let Some(path) = &cli.offline_fixture {
        let resource = load_fixture(path)?;
        let fixture_source = OfflineFixtureSource::from_primary_and_paths(
            resource.clone(),
            &cli.offline_resource_fixture,
        )?;
        (
            AppState::new(resource),
            FetchSource::OfflineFixtures(fixture_source),
            None,
        )
    } else {
        let gateway = GithubApiGateway::new(api_depth);
        let fetch_source = FetchSource::Github(gateway);
        if cli.once {
            (
                AppState::new(fetch_source.fetch_resource(&resource_id).await?),
                fetch_source,
                None,
            )
        } else {
            (
                AppState::new(loading_resource_placeholder(resource_id.clone())),
                fetch_source,
                Some(FetchAction::Initial { id: resource_id }),
            )
        }
    };

    state.config_path = loaded_config.path.clone();
    state.theme = loaded_config.config.ui.theme;
    state.symbols = loaded_config.config.ui.symbols;
    state.spacing = loaded_config.config.ui.spacing;
    state.width_mode = loaded_config.config.ui.width_mode;
    state.fixed_width = loaded_config.config.ui.fixed_width;
    state.scrollbar = loaded_config.config.ui.scrollbar;
    if !loaded_config.diagnostics.is_empty() {
        state.last_error = Some(loaded_config.diagnostics.join("; "));
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

    run_tui(
        &mut state,
        !cli.no_mouse,
        fetch_source,
        Duration::from_secs(cli.refresh_seconds),
        initial_fetch,
    )
    .await
    .context("failed to run terminal UI")
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
) -> anyhow::Result<()> {
    let (_guard, mut terminal) = TerminalGuard::enter(mouse_enabled)?;
    let (fetch_tx, mut fetch_rx) = mpsc::unbounded_channel();
    let mut last_refresh = Instant::now();
    if let Some(action) = initial_fetch {
        if start_background_fetch(state, action, fetch_source.clone(), &fetch_tx) {
            last_refresh = Instant::now();
        }
    }
    loop {
        apply_completed_fetches(state, &mut fetch_rx);
        state.advance_loading_frame();
        terminal.draw(|frame| render_app(frame, state))?;
        if state.should_quit {
            return Ok(());
        }
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
                return Ok(());
            }
        }
    }
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
            if fetch_source.is_live_github() || fetch_source.is_offline_fixture() {
                if start_background_fetch(
                    state,
                    FetchAction::OpenTab { id },
                    fetch_source,
                    fetch_tx,
                ) {
                    state.close_add_resource_prompt();
                    *last_refresh = Instant::now();
                } else if let Some(message) = state.status_message.clone() {
                    state.set_add_resource_error(message);
                }
            } else {
                state.status_message = Some("offline fixture mode: open resource skipped".into());
                state.set_add_resource_error("offline fixture mode: open resource skipped");
            }
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
        app::{AppIntent, AppState},
        domain::{ReactionCounts, Resource, ResourceId, ResourceKind, FULL_DEPTH_WARNING_HINT},
        fetch::{FetchSource, OfflineFixtureSource},
        github::api::GithubGateway,
    };

    use super::{
        auto_refresh_due, clipboard_command, handle_intent, maybe_auto_refresh_with_start,
        navigate_back, navigate_to_resource, url_open_command, ClipboardPlatform,
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

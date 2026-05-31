use std::{
    process::Stdio,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use clap::Parser;
use crossterm::event::{self, Event};
use ghzinga::{
    app::{apply_event, AppEvent, AppIntent, AppState},
    cli::Cli,
    config::{self, AppConfig},
    domain::ResourceId,
    github::{
        api::{GithubApiGateway, GithubGateway},
        load_fixture,
    },
    render::render_app,
    terminal::TerminalGuard,
};
use tokio::process::Command;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

const EVENT_POLL_TIMEOUT: Duration = Duration::from_millis(250);
const MAX_PENDING_EVENTS_PER_FRAME: usize = 64;

#[derive(Debug, Clone)]
enum FetchAction {
    Refresh { id: ResourceId },
    Navigate { from: ResourceId, to: ResourceId },
    Back { to: ResourceId },
}

impl FetchAction {
    fn target(&self) -> &ResourceId {
        match self {
            Self::Refresh { id } => id,
            Self::Navigate { to, .. } | Self::Back { to } => to,
        }
    }

    fn loading_message(&self) -> String {
        match self {
            Self::Refresh { id } => format!("refreshing {} from GitHub", id.canonical_name()),
            Self::Navigate { to, .. } => format!("opening {} from GitHub", to.canonical_name()),
            Self::Back { to } => format!("returning to {} from GitHub", to.canonical_name()),
        }
    }
}

struct FetchOutcome {
    action: FetchAction,
    result: anyhow::Result<ghzinga::domain::Resource>,
    refreshed_at: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let loaded_config = config::load();
    let resource_id = cli.parse_resource_id()?;
    let resource = if let Some(path) = &cli.offline_fixture {
        load_fixture(path)?
    } else {
        let gateway = GithubApiGateway;
        gateway.fetch_resource(&resource_id).await?
    };

    let mut state = AppState::new(resource);
    state.config_path = loaded_config.path.clone();
    state.theme = loaded_config.config.ui.theme;
    state.symbols = loaded_config.config.ui.symbols;
    state.spacing = loaded_config.config.ui.spacing;
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
        cli.offline_fixture.is_none(),
        Duration::from_secs(cli.refresh_seconds),
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
    live_refresh: bool,
    refresh_interval: Duration,
) -> anyhow::Result<()> {
    let (_guard, mut terminal) = TerminalGuard::enter(mouse_enabled)?;
    let (fetch_tx, mut fetch_rx) = mpsc::unbounded_channel();
    let mut last_refresh = Instant::now();
    loop {
        apply_completed_fetches(state, &mut fetch_rx);
        state.advance_loading_frame();
        terminal.draw(|frame| render_app(frame, state))?;
        if state.should_quit {
            return Ok(());
        }
        maybe_auto_refresh(
            state,
            live_refresh,
            refresh_interval,
            &mut last_refresh,
            Instant::now(),
            &fetch_tx,
        );
        for app_event in read_pending_app_events()? {
            let intent = apply_event(state, app_event);
            if handle_intent(state, intent, live_refresh, &mut last_refresh, &fetch_tx).await {
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
    live_refresh: bool,
    last_refresh: &mut Instant,
    fetch_tx: &UnboundedSender<FetchOutcome>,
) -> bool {
    match intent {
        AppIntent::Quit => true,
        AppIntent::Refresh => {
            if live_refresh {
                let id = state.resource.id.clone();
                if start_background_fetch(state, FetchAction::Refresh { id }, fetch_tx) {
                    *last_refresh = Instant::now();
                }
            } else {
                state.status_message = Some("offline fixture mode: refresh skipped".into());
            }
            false
        }
        AppIntent::Navigate(id) => {
            if live_refresh {
                let from = state.resource.id.clone();
                if start_background_fetch(state, FetchAction::Navigate { from, to: id }, fetch_tx) {
                    *last_refresh = Instant::now();
                }
            } else {
                state.last_error = Some(format!(
                    "offline fixture mode: cannot navigate to {}",
                    id.canonical_name()
                ));
            }
            false
        }
        AppIntent::OpenResource(id) => {
            open_resource(state, &id).await;
            false
        }
        AppIntent::OpenUrl(url) => {
            open_url(state, &url).await;
            false
        }
        AppIntent::Back => {
            if live_refresh {
                let Some(id) = state.history.last().cloned() else {
                    state.status_message = Some("no previous resource".into());
                    return false;
                };
                if start_background_fetch(state, FetchAction::Back { to: id }, fetch_tx) {
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
        .with_spacing(state.spacing);
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
    fetch_tx: &UnboundedSender<FetchOutcome>,
) -> bool {
    maybe_auto_refresh_with_start(
        state,
        live_refresh,
        refresh_interval,
        last_refresh,
        now,
        |state, action| start_background_fetch(state, action, fetch_tx),
    )
}

fn maybe_auto_refresh_with_start(
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
    if start(state, FetchAction::Refresh { id }) {
        *last_refresh = now;
        true
    } else {
        false
    }
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
        return ("open".into(), vec![url.into()]);
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

async fn open_resource(state: &mut AppState, id: &ResourceId) {
    open_url(state, &id.web_url()).await;
}

fn start_background_fetch(
    state: &mut AppState,
    action: FetchAction,
    fetch_tx: &UnboundedSender<FetchOutcome>,
) -> bool {
    if let Some(message) = state.loading_message() {
        state.status_message = Some(format!("still loading: {message}"));
        return false;
    }

    let target = action.target().clone();
    let message = action.loading_message();
    state.begin_loading(target.clone(), message);
    let tx = fetch_tx.clone();
    tokio::spawn(async move {
        let gateway = GithubApiGateway;
        let result = gateway.fetch_resource(&target).await;
        let _ = tx.send(FetchOutcome {
            action,
            result,
            refreshed_at: current_refresh_label(),
        });
    });
    true
}

fn apply_completed_fetches(state: &mut AppState, fetch_rx: &mut UnboundedReceiver<FetchOutcome>) {
    while let Ok(outcome) = fetch_rx.try_recv() {
        apply_fetch_outcome(state, outcome);
    }
}

fn apply_fetch_outcome(state: &mut AppState, outcome: FetchOutcome) {
    state.finish_loading();
    match (outcome.action, outcome.result) {
        (FetchAction::Refresh { .. }, Ok(resource)) => {
            state.apply_refreshed_resource(resource, outcome.refreshed_at);
        }
        (FetchAction::Navigate { from, .. }, Ok(resource)) => {
            state.history.push(from);
            let name = resource.id.canonical_name();
            state.replace_resource(resource);
            state.last_error = None;
            state.status_message = Some(format!("opened {name}"));
        }
        (FetchAction::Back { .. }, Ok(resource)) => {
            let name = resource.id.canonical_name();
            state.replace_resource(resource);
            state.last_error = None;
            state.status_message = Some(format!("returned to {name}"));
        }
        (FetchAction::Back { to }, Err(error)) => {
            state.history.push(to);
            state.last_error = Some(error.to_string());
        }
        (_, Err(error)) => {
            state.last_error = Some(error.to_string());
        }
    }
}

#[cfg(test)]
async fn navigate_to_resource<G: GithubGateway>(
    state: &mut AppState,
    id: ghzinga::domain::ResourceId,
    gateway: &G,
) {
    match gateway.fetch_resource(&id).await {
        Ok(resource) => {
            state.push_current_to_history();
            let name = resource.id.canonical_name();
            state.replace_resource(resource);
            state.last_error = None;
            state.status_message = Some(format!("opened {name}"));
        }
        Err(error) => {
            state.last_error = Some(error.to_string());
        }
    }
}

fn current_refresh_label() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        % 86_400;
    let hours = seconds / 3_600;
    let minutes = (seconds % 3_600) / 60;
    let seconds = seconds % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02} UTC")
}

#[cfg(test)]
async fn navigate_back<G: GithubGateway>(state: &mut AppState, gateway: &G) {
    let Some(id) = state.pop_history() else {
        state.status_message = Some("no previous resource".into());
        return;
    };
    match gateway.fetch_resource(&id).await {
        Ok(resource) => {
            let name = resource.id.canonical_name();
            state.replace_resource(resource);
            state.last_error = None;
            state.status_message = Some(format!("returned to {name}"));
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

    use ghzinga::{
        app::AppState,
        domain::{ReactionCounts, Resource, ResourceId, ResourceKind},
        github::api::GithubGateway,
    };

    use super::{
        apply_fetch_outcome, auto_refresh_due, maybe_auto_refresh_with_start, navigate_back,
        navigate_to_resource, url_open_command, FetchAction, FetchOutcome,
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

    #[test]
    fn automatic_refresh_starts_background_fetch_and_records_completed_changes() {
        let initial = issue_resource(1, "Initial issue");
        let mut refreshed_resource = issue_resource(1, "Updated issue");
        refreshed_resource.body = "Updated body".into();
        let mut state = AppState::new(initial);
        let mut last_refresh = Instant::now();
        let now = last_refresh + Duration::from_secs(30);
        let mut started = Vec::new();

        let refreshed = maybe_auto_refresh_with_start(
            &mut state,
            true,
            Duration::from_secs(30),
            &mut last_refresh,
            now,
            |state, action| {
                state.begin_loading(action.target().clone(), action.loading_message());
                started.push(action);
                true
            },
        );

        assert!(refreshed);
        assert_eq!(last_refresh, now);
        assert_eq!(
            state.loading_message(),
            Some("refreshing owner/repo#1 from GitHub")
        );
        assert_eq!(started.len(), 1);
        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action: started.pop().unwrap(),
                result: Ok(refreshed_resource),
                refreshed_at: "12:34:56 UTC".into(),
            },
        );

        assert_eq!(state.resource.title, "Updated issue");
        assert_eq!(state.resource.body, "Updated body");
        assert_eq!(state.last_refresh_had_changes, Some(true));
        assert_eq!(state.last_refresh_changed_sections, ["summary"]);
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
    fn failed_back_fetch_restores_history_target() {
        let initial = issue_resource(1, "Initial issue");
        let previous = issue_resource(2, "Previous issue");
        let mut state = AppState::new(initial);
        state.history.push(previous.id.clone());
        let popped = state.pop_history().unwrap();
        state.begin_loading(popped.clone(), "returning to owner/repo#2 from GitHub");

        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action: FetchAction::Back { to: popped },
                result: Err(anyhow::anyhow!("network down")),
                refreshed_at: "12:34:56 UTC".into(),
            },
        );

        assert_eq!(state.history, [previous.id]);
        assert_eq!(state.last_error.as_deref(), Some("network down"));
        assert!(state.loading.is_none());
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
        assert_eq!(state.status_message.as_deref(), Some("opened owner/repo#2"));

        navigate_back(&mut state, &gateway).await;

        assert_eq!(state.resource.id, initial.id);
        assert_eq!(state.resource.title, "Initial issue");
        assert!(state.history.is_empty());
        assert_eq!(
            state.status_message.as_deref(),
            Some("returned to owner/repo#1")
        );
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

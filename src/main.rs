use std::{
    process::Stdio,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use clap::Parser;
use crossterm::event::{self, Event};
use ghzoom::{
    app::{apply_event, AppEvent, AppIntent, AppState},
    cli::Cli,
    domain::{ResourceId, ResourceKind},
    github::{
        gh_cli::{GhCliGateway, GithubGateway},
        load_fixture,
    },
    render::render_app,
    terminal::TerminalGuard,
};
use tokio::process::Command;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let resource_id = cli.parse_resource_id()?;
    let resource = if let Some(path) = &cli.offline_fixture {
        load_fixture(path)?
    } else {
        let gateway = GhCliGateway;
        gateway.fetch_resource(&resource_id).await?
    };

    let mut state = AppState::new(resource);
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
    let mut last_refresh = Instant::now();
    loop {
        terminal.draw(|frame| render_app(frame, state))?;
        if state.should_quit {
            return Ok(());
        }
        if live_refresh
            && refresh_interval.as_secs() > 0
            && last_refresh.elapsed() >= refresh_interval
        {
            refresh_resource(state).await;
            last_refresh = Instant::now();
        }
        if event::poll(Duration::from_millis(250))? {
            let app_event = match event::read()? {
                Event::Key(key) => Some(AppEvent::Key(key)),
                Event::Mouse(mouse) => Some(AppEvent::Mouse(mouse)),
                _ => None,
            };
            if let Some(app_event) = app_event {
                match apply_event(state, app_event) {
                    AppIntent::Quit => return Ok(()),
                    AppIntent::Refresh => {
                        if live_refresh {
                            refresh_resource(state).await;
                            last_refresh = Instant::now();
                        } else {
                            state.status_message =
                                Some("offline fixture mode: refresh skipped".into());
                        }
                    }
                    AppIntent::Navigate(id) => {
                        if live_refresh {
                            navigate_to_resource(state, id).await;
                            last_refresh = Instant::now();
                        } else {
                            state.last_error = Some(format!(
                                "offline fixture mode: cannot navigate to {}",
                                id.canonical_name()
                            ));
                        }
                    }
                    AppIntent::OpenResource(id) => {
                        open_resource(state, &id).await;
                    }
                    AppIntent::Back => {
                        if live_refresh {
                            navigate_back(state).await;
                            last_refresh = Instant::now();
                        } else {
                            state.status_message =
                                Some("offline fixture mode: no live history".into());
                        }
                    }
                    AppIntent::None => {}
                }
            }
        }
    }
}

async fn open_resource(state: &mut AppState, id: &ResourceId) {
    let args = open_command_args(id);
    let mut command = Command::new("gh");
    command.args(&args);

    match command.stderr(Stdio::piped()).output().await {
        Ok(output) if output.status.success() => {
            state.last_error = None;
            state.status_message = Some(format!("opened {}", id.canonical_name()));
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let details = stderr.trim();
            state.last_error = Some(if details.is_empty() {
                "`gh` web open failed without an error message".into()
            } else {
                format!("`gh` web open failed: {details}")
            });
        }
        Err(error) => {
            state.last_error = Some(format!("failed to execute `gh` web open: {error}"));
        }
    }
}

fn open_command_args(id: &ResourceId) -> Vec<String> {
    let repo = id.repo_name_with_owner();
    let number = id.number.to_string();
    match id.kind_hint {
        Some(ResourceKind::PullRequest) => vec![
            "pr".into(),
            "view".into(),
            number,
            "-R".into(),
            repo,
            "--web".into(),
        ],
        Some(ResourceKind::Issue) => vec![
            "issue".into(),
            "view".into(),
            number,
            "-R".into(),
            repo,
            "--web".into(),
        ],
        None => vec!["browse".into(), number, "-R".into(), repo],
    }
}

async fn refresh_resource(state: &mut AppState) {
    let id = state.resource.id.clone();
    let gateway = GhCliGateway;
    match gateway.fetch_resource(&id).await {
        Ok(resource) => {
            state.apply_refreshed_resource(resource, current_refresh_label());
        }
        Err(error) => {
            state.last_error = Some(error.to_string());
        }
    }
}

async fn navigate_to_resource(state: &mut AppState, id: ghzoom::domain::ResourceId) {
    let gateway = GhCliGateway;
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

async fn navigate_back(state: &mut AppState) {
    let Some(id) = state.pop_history() else {
        state.status_message = Some("no previous resource".into());
        return;
    };
    let gateway = GhCliGateway;
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
    use ghzoom::domain::{ResourceId, ResourceKind};

    use super::open_command_args;

    #[test]
    fn open_command_uses_pr_web_view_for_pull_requests() {
        let id = ResourceId {
            owner: "openclaw".into(),
            repo: "openclaw".into(),
            number: 81834,
            kind_hint: Some(ResourceKind::PullRequest),
        };

        assert_eq!(
            open_command_args(&id),
            ["pr", "view", "81834", "-R", "openclaw/openclaw", "--web"]
        );
    }

    #[test]
    fn open_command_uses_issue_web_view_for_issues() {
        let id = ResourceId {
            owner: "openclaw".into(),
            repo: "openclaw".into(),
            number: 88499,
            kind_hint: Some(ResourceKind::Issue),
        };

        assert_eq!(
            open_command_args(&id),
            ["issue", "view", "88499", "-R", "openclaw/openclaw", "--web"]
        );
    }
}

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::{
    app::AppState,
    domain::{Resource, ResourceId},
    github::{
        api::{ApiDepth, GithubApiGateway, GithubGateway},
        load_fixture,
    },
};

#[derive(Debug, Clone)]
pub(crate) enum FetchAction {
    Initial { id: ResourceId },
    Refresh { id: ResourceId },
    LoadFull { id: ResourceId },
    Navigate { from: ResourceId, to: ResourceId },
    Back { to: ResourceId },
}

impl FetchAction {
    pub(crate) fn target(&self) -> &ResourceId {
        match self {
            Self::Initial { id } | Self::Refresh { id } | Self::LoadFull { id } => id,
            Self::Navigate { to, .. } | Self::Back { to } => to,
        }
    }

    pub(crate) fn loading_message(&self) -> String {
        match self {
            Self::Initial { id } => format!("opening {} from GitHub", id.canonical_name()),
            Self::Refresh { id } => format!("refreshing {} from GitHub", id.canonical_name()),
            Self::LoadFull { id } => {
                format!("loading full data for {} from GitHub", id.canonical_name())
            }
            Self::Navigate { to, .. } => format!("opening {} from GitHub", to.canonical_name()),
            Self::Back { to } => format!("returning to {} from GitHub", to.canonical_name()),
        }
    }
}

pub(crate) struct FetchOutcome {
    action: FetchAction,
    result: anyhow::Result<Resource>,
    refreshed_at: String,
}

#[derive(Clone)]
pub(crate) enum FetchSource {
    Github(GithubApiGateway),
    OfflineFixtures(OfflineFixtureSource),
}

impl FetchSource {
    pub(crate) async fn fetch_resource(&self, id: &ResourceId) -> anyhow::Result<Resource> {
        match self {
            Self::Github(gateway) => gateway.fetch_resource(id).await,
            Self::OfflineFixtures(fixtures) => fixtures.fetch_resource(id),
        }
    }

    async fn fetch_resource_full_depth(&self, id: &ResourceId) -> anyhow::Result<Resource> {
        match self {
            Self::Github(_) => {
                GithubApiGateway::new(ApiDepth::Full)
                    .fetch_resource(id)
                    .await
            }
            Self::OfflineFixtures(fixtures) => fixtures.fetch_resource(id),
        }
    }

    pub(crate) fn is_live_github(&self) -> bool {
        matches!(self, Self::Github(_))
    }

    pub(crate) fn is_offline_fixture(&self) -> bool {
        matches!(self, Self::OfflineFixtures(_))
    }
}

#[derive(Clone)]
pub(crate) struct OfflineFixtureSource {
    resources: Arc<HashMap<String, Resource>>,
}

impl OfflineFixtureSource {
    pub(crate) fn new(resources: impl IntoIterator<Item = Resource>) -> Self {
        Self {
            resources: Arc::new(
                resources
                    .into_iter()
                    .map(|resource| (resource.id.canonical_name(), resource))
                    .collect(),
            ),
        }
    }

    pub(crate) fn from_primary_and_paths(
        primary: Resource,
        extra_paths: &[PathBuf],
    ) -> anyhow::Result<Self> {
        let mut resources = vec![primary];
        for path in extra_paths {
            resources.push(load_fixture(path)?);
        }
        Ok(Self::new(resources))
    }

    pub(crate) fn fetch_resource(&self, id: &ResourceId) -> anyhow::Result<Resource> {
        let key = id.canonical_name();
        self.resources
            .get(&key)
            .cloned()
            .with_context(|| format!("offline fixture mode: no fixture loaded for {key}"))
    }
}

pub(crate) fn start_background_fetch(
    state: &mut AppState,
    action: FetchAction,
    fetch_source: FetchSource,
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
        let result = match &action {
            FetchAction::LoadFull { .. } => fetch_source.fetch_resource_full_depth(&target).await,
            _ => fetch_source.fetch_resource(&target).await,
        };
        let _ = tx.send(FetchOutcome {
            action,
            result,
            refreshed_at: current_refresh_label(),
        });
    });
    true
}

pub(crate) fn apply_completed_fetches(
    state: &mut AppState,
    fetch_rx: &mut UnboundedReceiver<FetchOutcome>,
) {
    while let Ok(outcome) = fetch_rx.try_recv() {
        apply_fetch_outcome(state, outcome);
    }
}

pub(crate) fn apply_fetch_outcome(state: &mut AppState, outcome: FetchOutcome) {
    state.finish_loading();
    match (outcome.action, outcome.result) {
        (FetchAction::Initial { .. }, Ok(resource)) => {
            let name = resource.id.canonical_name();
            state.replace_resource_preserve_tab(resource);
            state.last_error = None;
            state.status_message = Some(format!("loaded {name}"));
        }
        (FetchAction::Refresh { .. }, Ok(resource)) => {
            state.apply_refreshed_resource(resource, outcome.refreshed_at);
        }
        (FetchAction::LoadFull { .. }, Ok(resource)) => {
            let name = resource.id.canonical_name();
            state.apply_refreshed_resource(resource, outcome.refreshed_at);
            state.status_message = Some(format!("loaded full GitHub data for {name}"));
        }
        (FetchAction::Navigate { from, .. }, Ok(resource)) => {
            state.history.push(from);
            let name = resource.id.canonical_name();
            state.replace_resource_reset_view(resource);
            state.last_error = None;
            state.status_message = Some(format!("opened {name}"));
        }
        (FetchAction::Back { .. }, Ok(resource)) => {
            let name = resource.id.canonical_name();
            state.replace_resource_reset_view(resource);
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
mod tests {
    use std::time::{Duration, Instant};

    use crate::{
        app::{loading_resource_placeholder, AppState, Tab},
        domain::{ReactionCounts, Resource, ResourceId, ResourceKind, FULL_DEPTH_WARNING_HINT},
        github::api::GithubApiGateway,
        runner::maybe_auto_refresh_with_start,
    };

    use super::{
        apply_fetch_outcome, start_background_fetch, FetchAction, FetchOutcome, FetchSource,
        OfflineFixtureSource,
    };

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
    fn full_depth_fetch_outcome_preserves_view_and_reports_loaded_status() {
        let mut initial = issue_resource(1, "Initial issue");
        initial.warnings.push(format!(
            "normal API depth shows the first 100 only for comments; {FULL_DEPTH_WARNING_HINT} for exhaustive pagination"
        ));
        let mut full = issue_resource(1, "Initial issue");
        full.body = "full body with later comments".into();
        let mut state = AppState::new(initial);
        state.set_tab(Tab::Activity);
        state.set_scroll_limit(10);
        state.scroll_down(4);
        let id = state.resource.id.clone();

        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action: FetchAction::LoadFull { id },
                result: Ok(full),
                refreshed_at: "12:34:56 UTC".into(),
            },
        );

        assert_eq!(state.active_tab, Tab::Activity);
        assert_eq!(state.scroll, 4);
        assert_eq!(state.resource.body, "full body with later comments");
        assert!(!state.resource.has_partial_depth_warning());
        assert_eq!(
            state.status_message.as_deref(),
            Some("loaded full GitHub data for owner/repo#1")
        );
    }

    #[test]
    fn initial_fetch_outcome_replaces_placeholder_with_loaded_resource() {
        let id = ResourceId {
            owner: "owner".into(),
            repo: "repo".into(),
            number: 1,
            kind_hint: None,
        };
        let mut state = AppState::new(loading_resource_placeholder(id.clone()));
        state.set_tab(Tab::Files);
        state.begin_loading(id.clone(), "opening owner/repo#1 from GitHub");

        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action: FetchAction::Initial { id },
                result: Ok(issue_resource(1, "Loaded issue")),
                refreshed_at: "12:34:56 UTC".into(),
            },
        );

        assert_eq!(state.resource.title, "Loaded issue");
        assert_eq!(state.resource.state, "OPEN");
        assert_eq!(state.active_tab, Tab::Overview);
        assert_eq!(state.status_message.as_deref(), Some("loaded owner/repo#1"));
        assert!(state.loading.is_none());
    }

    #[tokio::test]
    async fn initial_live_fetch_starts_before_first_draw() {
        let id = ResourceId {
            owner: "owner".into(),
            repo: "repo".into(),
            number: 1,
            kind_hint: None,
        };
        let loaded = issue_resource(1, "Loaded issue");
        let mut state = AppState::new(loading_resource_placeholder(id.clone()));
        state.set_tab(Tab::Activity);
        let (fetch_tx, mut fetch_rx) = tokio::sync::mpsc::unbounded_channel();

        let started = start_background_fetch(
            &mut state,
            FetchAction::Initial { id },
            FetchSource::OfflineFixtures(OfflineFixtureSource::new([loaded])),
            &fetch_tx,
        );

        assert!(started);
        assert_eq!(
            state.loading_message(),
            Some("opening owner/repo#1 from GitHub")
        );
        assert_eq!(state.resource.title, "Loading owner/repo#1");

        let outcome = fetch_rx.recv().await.expect("initial fetch outcome");
        apply_fetch_outcome(&mut state, outcome);

        assert_eq!(state.resource.title, "Loaded issue");
        assert_eq!(state.active_tab, Tab::Activity);
        assert!(state.loading.is_none());
    }

    #[test]
    fn duplicate_fetch_start_reports_existing_loading_without_queueing() {
        let initial = issue_resource(1, "Initial issue");
        let mut state = AppState::new(initial);
        let (fetch_tx, mut fetch_rx) = tokio::sync::mpsc::unbounded_channel();
        state.begin_loading(
            state.resource.id.clone(),
            "opening owner/repo#2 from GitHub",
        );
        let id = state.resource.id.clone();

        let started = start_background_fetch(
            &mut state,
            FetchAction::Refresh { id },
            FetchSource::Github(GithubApiGateway::new(crate::github::api::ApiDepth::Partial)),
            &fetch_tx,
        );

        assert!(!started);
        assert_eq!(
            state.status_message.as_deref(),
            Some("still loading: opening owner/repo#2 from GitHub")
        );
        assert!(fetch_rx.try_recv().is_err());
    }

    #[test]
    fn offline_fixture_source_fetches_by_canonical_name_without_kind_hint() {
        let fixture = issue_resource(2, "Linked issue");
        let source = OfflineFixtureSource::new([fixture.clone()]);
        let id_without_kind = ResourceId {
            owner: "owner".into(),
            repo: "repo".into(),
            number: 2,
            kind_hint: None,
        };

        let loaded = source
            .fetch_resource(&id_without_kind)
            .expect("fixture resource");

        assert_eq!(loaded.id, fixture.id);
        assert_eq!(loaded.title, "Linked issue");
    }

    #[test]
    fn offline_fixture_source_reports_missing_navigation_target() {
        let source = OfflineFixtureSource::new([issue_resource(1, "Initial issue")]);
        let missing = ResourceId {
            owner: "owner".into(),
            repo: "repo".into(),
            number: 2,
            kind_hint: Some(ResourceKind::Issue),
        };

        let error = source
            .fetch_resource(&missing)
            .expect_err("missing fixture should fail");

        assert_eq!(
            error.to_string(),
            "offline fixture mode: no fixture loaded for owner/repo#2"
        );
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
}

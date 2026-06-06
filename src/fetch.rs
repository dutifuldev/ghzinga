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
    domain::FILE_PATCH_CONTEXT_UNAVAILABLE_WARNING,
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
    LoadFilePatches { resource: Box<Resource> },
    OpenTab { id: ResourceId },
    Navigate { from: ResourceId, to: ResourceId },
    Back { to: ResourceId },
}

impl FetchAction {
    pub(crate) fn target(&self) -> &ResourceId {
        match self {
            Self::Initial { id } | Self::Refresh { id } | Self::OpenTab { id } => id,
            Self::LoadFull { id } => id,
            Self::LoadFilePatches { resource } => &resource.id,
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
            Self::LoadFilePatches { resource } => {
                format!("loading file diffs for {}", resource.id.canonical_name())
            }
            Self::OpenTab { id } => format!("opening {} in a new tab", id.canonical_name()),
            Self::Navigate { to, .. } => format!("opening {} from GitHub", to.canonical_name()),
            Self::Back { to } => format!("returning to {} from GitHub", to.canonical_name()),
        }
    }

    fn can_load_progressively(&self) -> bool {
        !matches!(self, Self::LoadFull { .. } | Self::LoadFilePatches { .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FetchStage {
    Complete,
    Base,
    Enrichment,
}

pub(crate) struct FetchOutcome {
    action: FetchAction,
    result: anyhow::Result<Resource>,
    refreshed_at: String,
    request_id: u64,
    origin_tab_id: u64,
    stage: FetchStage,
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

    async fn fetch_resource_base(&self, id: &ResourceId) -> anyhow::Result<Resource> {
        match self {
            Self::Github(gateway) => gateway.fetch_resource_base(id).await,
            Self::OfflineFixtures(fixtures) => fixtures.fetch_resource(id),
        }
    }

    async fn enrich_resource(&self, resource: Resource) -> anyhow::Result<Resource> {
        match self {
            Self::Github(gateway) => gateway.enrich_resource(resource).await,
            Self::OfflineFixtures(_) => Ok(resource),
        }
    }

    async fn enrich_file_patches(&self, resource: Resource) -> anyhow::Result<Resource> {
        match self {
            Self::Github(gateway) => gateway.enrich_file_patches(resource).await,
            Self::OfflineFixtures(_) => Ok(resource),
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

    fn supports_progressive_loading(&self) -> bool {
        matches!(self, Self::Github(_))
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
    let origin_tab_id = state.active_resource_tab_id();
    let request_id = state.begin_loading(target.clone(), message);
    let tx = fetch_tx.clone();
    tokio::spawn(async move {
        if action.can_load_progressively() && fetch_source.supports_progressive_loading() {
            let base_result = fetch_source.fetch_resource_base(&target).await;
            let enrichment_seed = base_result.as_ref().ok().cloned();
            let _ = tx.send(FetchOutcome {
                action: action.clone(),
                result: base_result,
                refreshed_at: current_refresh_label(),
                request_id,
                origin_tab_id,
                stage: FetchStage::Base,
            });
            if let Some(resource) = enrichment_seed {
                let result = fetch_source.enrich_resource(resource).await;
                let _ = tx.send(FetchOutcome {
                    action,
                    result,
                    refreshed_at: current_refresh_label(),
                    request_id,
                    origin_tab_id,
                    stage: FetchStage::Enrichment,
                });
            }
        } else {
            let result = match &action {
                FetchAction::LoadFull { .. } => {
                    fetch_source.fetch_resource_full_depth(&target).await
                }
                FetchAction::LoadFilePatches { resource } => {
                    fetch_source
                        .enrich_file_patches(resource.as_ref().clone())
                        .await
                }
                _ => fetch_source.fetch_resource(&target).await,
            };
            let _ = tx.send(FetchOutcome {
                action,
                result,
                refreshed_at: current_refresh_label(),
                request_id,
                origin_tab_id,
                stage: FetchStage::Complete,
            });
        }
    });
    true
}

pub(crate) fn apply_completed_fetches(
    state: &mut AppState,
    fetch_rx: &mut UnboundedReceiver<FetchOutcome>,
) -> bool {
    let mut changed = false;
    while let Ok(outcome) = fetch_rx.try_recv() {
        apply_fetch_outcome(state, outcome);
        changed = true;
    }
    changed
}

pub(crate) fn apply_fetch_outcome(state: &mut AppState, outcome: FetchOutcome) {
    match outcome.stage {
        FetchStage::Complete => apply_blocking_fetch_outcome(state, outcome),
        FetchStage::Base => apply_base_fetch_outcome(state, outcome),
        FetchStage::Enrichment => apply_enrichment_fetch_outcome(state, outcome),
    }
}

fn apply_blocking_fetch_outcome(state: &mut AppState, outcome: FetchOutcome) {
    if !finish_matching_blocking_load(state, outcome.request_id) {
        return;
    }
    apply_loaded_resource_outcome(state, outcome, false);
}

fn apply_base_fetch_outcome(state: &mut AppState, outcome: FetchOutcome) {
    if !finish_matching_blocking_load(state, outcome.request_id) {
        return;
    }
    let origin_tab_id = outcome.origin_tab_id;
    let target = outcome.action.target().clone();
    let base_loaded = outcome.result.is_ok();
    apply_loaded_resource_outcome(state, outcome, false);
    if base_loaded {
        state.apply_to_resource_tab(origin_tab_id, |state| {
            if resource_matches_target(&state.resource, &target) {
                state.status_message = Some("loading additional GitHub details".into());
            }
        });
    }
}

fn finish_matching_blocking_load(state: &mut AppState, request_id: u64) -> bool {
    if !state.loading_request_matches(request_id) {
        return false;
    }
    state.finish_loading();
    state.clear_transient_loading_status_messages();
    true
}

fn apply_loaded_resource_outcome(state: &mut AppState, outcome: FetchOutcome, enrichment: bool) {
    let origin_tab_id = outcome.origin_tab_id;
    match (outcome.action, outcome.result) {
        (FetchAction::Initial { .. }, Ok(resource)) => {
            state.apply_to_resource_tab(origin_tab_id, |state| {
                state.replace_resource_preserve_tab(resource);
                state.last_error = None;
                state.status_message = None;
            });
        }
        (FetchAction::Refresh { .. }, Ok(resource)) => {
            state.apply_to_resource_tab(origin_tab_id, |state| {
                state.apply_refreshed_resource(resource, outcome.refreshed_at);
            });
        }
        (FetchAction::LoadFull { .. }, Ok(resource)) => {
            state.apply_to_resource_tab(origin_tab_id, |state| {
                state.apply_refreshed_resource(resource, outcome.refreshed_at);
                state.status_message = None;
            });
        }
        (FetchAction::LoadFilePatches { .. }, Ok(resource)) => {
            state.apply_to_resource_tab(origin_tab_id, |state| {
                if resource_matches_target(&state.resource, &resource.id) {
                    state.apply_refreshed_resource(resource, outcome.refreshed_at);
                    state.status_message = None;
                }
            });
        }
        (FetchAction::OpenTab { .. }, Ok(resource)) => {
            state.open_resource_in_tab(resource);
            state.last_error = None;
            state.status_message = None;
        }
        (FetchAction::Navigate { from, .. }, Ok(resource)) => {
            state.apply_to_resource_tab(origin_tab_id, |state| {
                state.history.push(from);
                state.replace_resource_reset_view(resource);
                state.last_error = None;
                state.status_message = None;
            });
        }
        (FetchAction::Back { .. }, Ok(resource)) => {
            state.apply_to_resource_tab(origin_tab_id, |state| {
                state.replace_resource_reset_view(resource);
                state.last_error = None;
                state.status_message = None;
            });
        }
        (FetchAction::Back { to }, Err(error)) => {
            state.apply_to_resource_tab(origin_tab_id, |state| {
                state.history.push(to);
                state.last_error = Some(error.to_string());
            });
        }
        (FetchAction::LoadFilePatches { resource }, Err(error)) => {
            let target = resource.id.clone();
            let error = error.to_string();
            state.apply_to_resource_tab(origin_tab_id, |state| {
                if resource_matches_target(&state.resource, &target) {
                    state.last_error = Some(error.clone());
                    state.status_message = None;
                    push_unique_warning(
                        &mut state.resource,
                        format!("{FILE_PATCH_CONTEXT_UNAVAILABLE_WARNING}: {error}"),
                    );
                }
            });
        }
        (_, Err(error)) => {
            state.apply_to_resource_tab(origin_tab_id, |state| {
                state.last_error = Some(error.to_string());
            });
        }
    }
    if enrichment {
        state.clear_transient_loading_status_messages();
    }
}

fn apply_enrichment_fetch_outcome(state: &mut AppState, outcome: FetchOutcome) {
    let origin_tab_id = outcome.origin_tab_id;
    let target = outcome.action.target().clone();
    let request_id = outcome.request_id;
    match outcome.result {
        Ok(resource) => {
            state.apply_to_resource_tab(origin_tab_id, |state| {
                if state.latest_fetch_request_matches(request_id)
                    && resource_matches_target(&state.resource, &target)
                {
                    state.apply_refreshed_resource(resource, outcome.refreshed_at);
                }
            });
        }
        Err(error) => {
            let warning = format!("background details unavailable: {error}");
            state.apply_to_resource_tab(origin_tab_id, |state| {
                if state.latest_fetch_request_matches(request_id)
                    && resource_matches_target(&state.resource, &target)
                    && !state.resource.warnings.iter().any(|item| item == &warning)
                {
                    state.resource.warnings.push(warning);
                    state.status_message = None;
                }
            });
        }
    }
}

fn resource_matches_target(resource: &Resource, target: &ResourceId) -> bool {
    resource.id.owner == target.owner
        && resource.id.repo == target.repo
        && resource.id.number == target.number
}

fn push_unique_warning(resource: &mut Resource, warning: String) {
    if !resource.warnings.iter().any(|item| item == &warning) {
        resource.warnings.push(warning);
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
        domain::{
            ChangedFile, PullRequest, ReactionCounts, Resource, ResourceId, ResourceKind,
            FILE_PATCH_CONTEXT_UNAVAILABLE_WARNING, FULL_DEPTH_WARNING_HINT,
        },
        github::api::GithubApiGateway,
        runner::maybe_auto_refresh_with_start,
    };

    use super::{
        apply_fetch_outcome, start_background_fetch, FetchAction, FetchOutcome, FetchSource,
        FetchStage, OfflineFixtureSource,
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

    fn begin_test_fetch(state: &mut AppState, action: &FetchAction) -> (u64, u64) {
        let origin_tab_id = state.active_resource_tab_id();
        let request_id = state.begin_loading(action.target().clone(), action.loading_message());
        (request_id, origin_tab_id)
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
                let (request_id, origin_tab_id) = begin_test_fetch(state, &action);
                started.push((action, request_id, origin_tab_id));
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
        let (action, request_id, origin_tab_id) = started.pop().unwrap();
        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action,
                result: Ok(refreshed_resource),
                refreshed_at: "12:34:56 UTC".into(),
                request_id,
                origin_tab_id,
                stage: FetchStage::Complete,
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
        let action = FetchAction::LoadFull { id };
        let (request_id, origin_tab_id) = begin_test_fetch(&mut state, &action);

        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action,
                result: Ok(full),
                refreshed_at: "12:34:56 UTC".into(),
                request_id,
                origin_tab_id,
                stage: FetchStage::Complete,
            },
        );

        assert_eq!(state.active_tab, Tab::Activity);
        assert_eq!(state.scroll, 4);
        assert_eq!(state.resource.body, "full body with later comments");
        assert!(!state.resource.has_partial_depth_warning());
        assert!(state.status_message.is_none());
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
        let action = FetchAction::Initial { id };
        let (request_id, origin_tab_id) = begin_test_fetch(&mut state, &action);

        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action,
                result: Ok(issue_resource(1, "Loaded issue")),
                refreshed_at: "12:34:56 UTC".into(),
                request_id,
                origin_tab_id,
                stage: FetchStage::Complete,
            },
        );

        assert_eq!(state.resource.title, "Loaded issue");
        assert_eq!(state.resource.state, "OPEN");
        assert_eq!(state.active_tab, Tab::Overview);
        assert!(state.status_message.is_none());
        assert!(state.loading.is_none());
    }

    #[test]
    fn progressive_base_outcome_finishes_blocking_load_before_enrichment() {
        let id = ResourceId {
            owner: "owner".into(),
            repo: "repo".into(),
            number: 1,
            kind_hint: None,
        };
        let mut base = issue_resource(1, "Base issue");
        base.body = "base body".into();
        let mut enriched = issue_resource(1, "Base issue");
        enriched.body = "enriched body".into();
        let mut state = AppState::new(loading_resource_placeholder(id.clone()));
        let action = FetchAction::Initial { id };
        let (request_id, origin_tab_id) = begin_test_fetch(&mut state, &action);

        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action: action.clone(),
                result: Ok(base),
                refreshed_at: "12:34:56 UTC".into(),
                request_id,
                origin_tab_id,
                stage: FetchStage::Base,
            },
        );

        assert_eq!(state.resource.title, "Base issue");
        assert_eq!(state.resource.body, "base body");
        assert!(state.loading.is_none());
        assert!(state.last_error.is_none());
        assert_eq!(
            state.status_message.as_deref(),
            Some("loading additional GitHub details")
        );

        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action,
                result: Ok(enriched),
                refreshed_at: "12:35:01 UTC".into(),
                request_id,
                origin_tab_id,
                stage: FetchStage::Enrichment,
            },
        );

        assert_eq!(state.resource.body, "enriched body");
        assert_eq!(state.last_refreshed_at.as_deref(), Some("12:35:01 UTC"));
        assert_eq!(state.last_refresh_changed_sections, ["summary"]);
        assert!(state.status_message.is_none());
    }

    #[test]
    fn progressive_base_error_does_not_leave_enrichment_status() {
        let id = ResourceId {
            owner: "owner".into(),
            repo: "repo".into(),
            number: 1,
            kind_hint: None,
        };
        let mut state = AppState::new(issue_resource(1, "Existing issue"));
        let action = FetchAction::Refresh { id };
        let (request_id, origin_tab_id) = begin_test_fetch(&mut state, &action);

        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action,
                result: Err(anyhow::anyhow!("network down")),
                refreshed_at: "12:35:01 UTC".into(),
                request_id,
                origin_tab_id,
                stage: FetchStage::Base,
            },
        );

        assert_eq!(state.resource.title, "Existing issue");
        assert_eq!(state.last_error.as_deref(), Some("network down"));
        assert!(state.status_message.is_none());
        assert!(state.loading.is_none());
    }

    #[test]
    fn progressive_enrichment_is_ignored_after_resource_replacement() {
        let id = ResourceId {
            owner: "owner".into(),
            repo: "repo".into(),
            number: 1,
            kind_hint: None,
        };
        let mut state = AppState::new(loading_resource_placeholder(id.clone()));
        let action = FetchAction::Initial { id };
        let (request_id, origin_tab_id) = begin_test_fetch(&mut state, &action);

        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action: action.clone(),
                result: Ok(issue_resource(1, "Base issue")),
                refreshed_at: "12:34:56 UTC".into(),
                request_id,
                origin_tab_id,
                stage: FetchStage::Base,
            },
        );
        state.replace_resource_reset_view(issue_resource(2, "Different issue"));

        let mut stale = issue_resource(1, "Stale enrichment");
        stale.body = "should not apply".into();
        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action,
                result: Ok(stale),
                refreshed_at: "12:35:01 UTC".into(),
                request_id,
                origin_tab_id,
                stage: FetchStage::Enrichment,
            },
        );

        assert_eq!(state.resource.id.number, 2);
        assert_eq!(state.resource.title, "Different issue");
        assert_ne!(state.resource.body, "should not apply");
    }

    #[test]
    fn progressive_enrichment_is_ignored_after_newer_request_for_same_resource() {
        let id = ResourceId {
            owner: "owner".into(),
            repo: "repo".into(),
            number: 1,
            kind_hint: None,
        };
        let mut state = AppState::new(loading_resource_placeholder(id.clone()));
        let action = FetchAction::Initial { id: id.clone() };
        let (request_id, origin_tab_id) = begin_test_fetch(&mut state, &action);

        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action: action.clone(),
                result: Ok(issue_resource(1, "Base issue")),
                refreshed_at: "12:34:56 UTC".into(),
                request_id,
                origin_tab_id,
                stage: FetchStage::Base,
            },
        );

        let refresh_action = FetchAction::Refresh { id };
        let (_newer_request_id, _same_origin_tab_id) =
            begin_test_fetch(&mut state, &refresh_action);

        let mut stale = issue_resource(1, "Stale enrichment");
        stale.body = "should not apply".into();
        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action,
                result: Ok(stale),
                refreshed_at: "12:35:01 UTC".into(),
                request_id,
                origin_tab_id,
                stage: FetchStage::Enrichment,
            },
        );

        assert_eq!(state.resource.title, "Base issue");
        assert_ne!(state.resource.body, "should not apply");
    }

    #[test]
    fn progressive_enrichment_error_adds_warning_without_failing_resource() {
        let id = ResourceId {
            owner: "owner".into(),
            repo: "repo".into(),
            number: 1,
            kind_hint: None,
        };
        let mut state = AppState::new(issue_resource(1, "Base issue"));
        let action = FetchAction::Refresh { id };
        let (request_id, origin_tab_id) = begin_test_fetch(&mut state, &action);

        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action,
                result: Err(anyhow::anyhow!("timeline timed out")),
                refreshed_at: "12:35:01 UTC".into(),
                request_id,
                origin_tab_id,
                stage: FetchStage::Enrichment,
            },
        );

        assert_eq!(state.resource.title, "Base issue");
        assert!(state.last_error.is_none());
        assert_eq!(
            state.resource.warnings,
            ["background details unavailable: timeline timed out"]
        );
    }

    #[test]
    fn file_patch_error_marks_resource_unavailable_without_retry_status() {
        let resource = pr_resource_with_patch(None);
        let mut state = AppState::new(resource.clone());
        state.set_tab(Tab::Files);
        let action = FetchAction::LoadFilePatches {
            resource: Box::new(resource),
        };
        let (request_id, origin_tab_id) = begin_test_fetch(&mut state, &action);

        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action,
                result: Err(anyhow::anyhow!("rate limited")),
                refreshed_at: "12:35:01 UTC".into(),
                request_id,
                origin_tab_id,
                stage: FetchStage::Complete,
            },
        );

        assert_eq!(state.last_error.as_deref(), Some("rate limited"));
        assert!(state.status_message.is_none());
        assert!(state.loading.is_none());
        assert!(state
            .resource
            .warnings
            .iter()
            .any(|warning| warning.starts_with(FILE_PATCH_CONTEXT_UNAVAILABLE_WARNING)));
    }

    #[test]
    fn open_tab_fetch_outcome_appends_and_activates_resource_tab() {
        let mut state = AppState::new(issue_resource(1, "Initial issue"));
        let second = issue_resource(2, "Second issue");
        let action = FetchAction::OpenTab {
            id: second.id.clone(),
        };
        let (request_id, origin_tab_id) = begin_test_fetch(&mut state, &action);

        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action,
                result: Ok(second),
                refreshed_at: "12:34:56 UTC".into(),
                request_id,
                origin_tab_id,
                stage: FetchStage::Complete,
            },
        );

        assert_eq!(state.resource.id.number, 2);
        assert_eq!(state.resource_tabs.len(), 2);
        assert_eq!(state.active_resource_tab, 1);
        assert!(state.loading.is_none());
    }

    #[test]
    fn refresh_fetch_outcome_updates_origin_tab_after_user_switches_tabs() {
        let mut state = AppState::new(issue_resource(1, "Initial issue"));
        let second = issue_resource(2, "Second issue");
        state.open_resource_in_tab(second);
        state.switch_resource_tab(0);

        let mut refreshed = issue_resource(1, "Updated issue");
        refreshed.body = "Updated body".into();
        let action = FetchAction::Refresh {
            id: refreshed.id.clone(),
        };
        let (request_id, origin_tab_id) = begin_test_fetch(&mut state, &action);
        state.switch_resource_tab(1);

        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action,
                result: Ok(refreshed),
                refreshed_at: "12:34:56 UTC".into(),
                request_id,
                origin_tab_id,
                stage: FetchStage::Complete,
            },
        );

        assert_eq!(state.resource.id.number, 2);
        assert_eq!(state.resource.title, "Second issue");
        assert!(state.loading.is_none());

        state.switch_resource_tab(0);

        assert_eq!(state.resource.id.number, 1);
        assert_eq!(state.resource.title, "Updated issue");
        assert_eq!(state.resource.body, "Updated body");
        assert_eq!(state.last_refreshed_at.as_deref(), Some("12:34:56 UTC"));
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
    fn completed_fetch_clears_blocked_loading_status_on_other_tabs() {
        let mut state = AppState::new(issue_resource(1, "Initial issue"));
        state.open_resource_in_tab(issue_resource(2, "Second issue"));
        state.switch_resource_tab(0);
        let action = FetchAction::Refresh {
            id: state.resource.id.clone(),
        };
        let (request_id, origin_tab_id) = begin_test_fetch(&mut state, &action);
        state.switch_resource_tab(1);
        let (fetch_tx, mut fetch_rx) = tokio::sync::mpsc::unbounded_channel();
        let blocked_id = state.resource.id.clone();

        let started = start_background_fetch(
            &mut state,
            FetchAction::Refresh { id: blocked_id },
            FetchSource::Github(GithubApiGateway::new(crate::github::api::ApiDepth::Partial)),
            &fetch_tx,
        );

        assert!(!started);
        assert_eq!(
            state.status_message.as_deref(),
            Some("still loading: refreshing owner/repo#1 from GitHub")
        );
        assert!(fetch_rx.try_recv().is_err());

        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action,
                result: Ok(issue_resource(1, "Updated issue")),
                refreshed_at: "12:34:56 UTC".into(),
                request_id,
                origin_tab_id,
                stage: FetchStage::Complete,
            },
        );

        assert_eq!(state.resource.id.number, 2);
        assert!(state.status_message.is_none());
        state.switch_resource_tab(0);
        assert_eq!(state.resource.title, "Updated issue");
        assert!(state.status_message.is_none());
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
        let action = FetchAction::Back { to: popped };
        let (request_id, origin_tab_id) = begin_test_fetch(&mut state, &action);

        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action,
                result: Err(anyhow::anyhow!("network down")),
                refreshed_at: "12:34:56 UTC".into(),
                request_id,
                origin_tab_id,
                stage: FetchStage::Complete,
            },
        );

        assert_eq!(state.history, [previous.id]);
        assert_eq!(state.last_error.as_deref(), Some("network down"));
        assert!(state.loading.is_none());
    }
}

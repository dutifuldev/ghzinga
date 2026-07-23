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
    domain::{Resource, ResourceAction, ResourceId},
    github::{
        api::{ApiDepth, GithubApiGateway, GithubGateway},
        load_fixture, mutations,
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
    FilePatches,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FetchOwner {
    request_id: u64,
    origin_tab_id: u64,
}

impl FetchOwner {
    fn new(request_id: u64, origin_tab_id: u64) -> Self {
        Self {
            request_id,
            origin_tab_id,
        }
    }
}

pub(crate) struct FetchOutcome {
    action: FetchAction,
    result: anyhow::Result<Resource>,
    refreshed_at: String,
    owner: FetchOwner,
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
    if let FetchAction::LoadFilePatches { resource } = &action {
        if state.file_patch_loading_message().is_some() {
            return false;
        }
        let origin_tab_id = state.active_resource_tab_id();
        let request_id = state.begin_file_patch_loading();
        let owner = FetchOwner::new(request_id, origin_tab_id);
        let action = action.clone();
        let resource = resource.as_ref().clone();
        let tx = fetch_tx.clone();
        tokio::spawn(async move {
            let result = fetch_source.enrich_file_patches(resource).await;
            let _ = tx.send(FetchOutcome {
                action,
                result,
                refreshed_at: current_refresh_label(),
                owner,
                stage: FetchStage::FilePatches,
            });
        });
        return true;
    }

    if let Some(message) = state.loading_message().map(str::to_string) {
        if matches!(action, FetchAction::Refresh { .. }) {
            state.refresh_requested = true;
        }
        state.status_message = Some(format!("still loading: {message}"));
        return false;
    }

    let target = action.target().clone();
    let message = action.loading_message();
    let origin_tab_id = state.active_resource_tab_id();
    let request_id = state.begin_loading(target.clone(), message);
    let owner = FetchOwner::new(request_id, origin_tab_id);
    let tx = fetch_tx.clone();
    tokio::spawn(async move {
        if action.can_load_progressively() && fetch_source.supports_progressive_loading() {
            let base_result = fetch_source.fetch_resource_base(&target).await;
            let enrichment_seed = base_result
                .as_ref()
                .ok()
                .filter(|resource| should_enqueue_enrichment(resource))
                .cloned();
            let _ = tx.send(FetchOutcome {
                action: action.clone(),
                result: base_result,
                refreshed_at: current_refresh_label(),
                owner,
                stage: FetchStage::Base,
            });
            if let Some(resource) = enrichment_seed {
                let result = fetch_source.enrich_resource(resource).await;
                let _ = tx.send(FetchOutcome {
                    action,
                    result,
                    refreshed_at: current_refresh_label(),
                    owner,
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
                owner,
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
        FetchStage::FilePatches => apply_file_patch_fetch_outcome(state, outcome),
    }
}

fn apply_blocking_fetch_outcome(state: &mut AppState, outcome: FetchOutcome) {
    if !finish_matching_blocking_load(state, outcome.owner) {
        return;
    }
    apply_loaded_resource_outcome(state, outcome);
}

fn apply_base_fetch_outcome(state: &mut AppState, outcome: FetchOutcome) {
    if !finish_matching_blocking_load(state, outcome.owner) {
        return;
    }
    let origin_tab_id = outcome.owner.origin_tab_id;
    let target = outcome.action.target().clone();
    let base_loaded = outcome.result.is_ok();
    apply_loaded_resource_outcome(state, outcome);
    if base_loaded {
        state.apply_to_resource_tab(origin_tab_id, |state| {
            if resource_matches_target(&state.resource, &target)
                && should_enqueue_enrichment(&state.resource)
            {
                state.status_message = Some("loading additional GitHub details".into());
            }
        });
    }
}

fn finish_matching_blocking_load(state: &mut AppState, owner: FetchOwner) -> bool {
    if !state.loading_request_matches(owner.request_id) {
        return false;
    }
    state.finish_loading();
    state.clear_transient_loading_status_messages();
    true
}

fn apply_loaded_resource_outcome(state: &mut AppState, outcome: FetchOutcome) {
    let origin_tab_id = outcome.owner.origin_tab_id;
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
                    state.apply_file_patch_resource(resource, outcome.refreshed_at);
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
}

fn apply_file_patch_fetch_outcome(state: &mut AppState, outcome: FetchOutcome) {
    let owner = outcome.owner;
    let origin_tab_id = owner.origin_tab_id;
    let target = outcome.action.target().clone();
    match outcome.result {
        Ok(resource) => {
            state.apply_to_resource_tab(origin_tab_id, |state| {
                if state.finish_file_patch_loading(owner.request_id)
                    && resource_matches_target(&state.resource, &target)
                {
                    state.apply_file_patch_resource(resource, outcome.refreshed_at);
                }
            });
        }
        Err(error) => {
            let error = error.to_string();
            state.apply_to_resource_tab(origin_tab_id, |state| {
                if state.finish_file_patch_loading(owner.request_id)
                    && resource_matches_target(&state.resource, &target)
                {
                    state.last_error = Some(error.clone());
                    push_unique_warning(
                        &mut state.resource,
                        format!("{FILE_PATCH_CONTEXT_UNAVAILABLE_WARNING}: {error}"),
                    );
                }
            });
        }
    }
}

fn apply_enrichment_fetch_outcome(state: &mut AppState, outcome: FetchOutcome) {
    let owner = outcome.owner;
    let origin_tab_id = owner.origin_tab_id;
    let target = outcome.action.target().clone();
    match outcome.result {
        Ok(resource) => {
            state.apply_to_resource_tab(origin_tab_id, |state| {
                if state.latest_fetch_request_matches(owner.request_id)
                    && resource_matches_target(&state.resource, &target)
                {
                    state.apply_enriched_resource(resource, outcome.refreshed_at);
                }
            });
        }
        Err(error) => {
            let warning = format!("background details unavailable: {error}");
            state.apply_to_resource_tab(origin_tab_id, |state| {
                if state.latest_fetch_request_matches(owner.request_id)
                    && resource_matches_target(&state.resource, &target)
                {
                    state.status_message = None;
                    push_unique_warning(&mut state.resource, warning);
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

fn should_enqueue_enrichment(resource: &Resource) -> bool {
    !resource.uses_public_rest_fallback()
}

/// Result of a background GitHub write action, delivered back to the main
/// loop over its own channel so it can never be confused with fetch results.
pub(crate) struct MutationOutcome {
    action: ResourceAction,
    target: ResourceId,
    origin_tab_id: u64,
    result: anyhow::Result<()>,
}

/// Spawn a GitHub write action. Mirrors `start_background_fetch`: state is
/// marked pending on the way out and only ever finalized on the main thread
/// by `apply_completed_mutations`.
pub(crate) fn start_background_mutation(
    state: &mut AppState,
    action: ResourceAction,
    mutation_tx: &UnboundedSender<MutationOutcome>,
) -> bool {
    if state.pending_action.is_some() {
        state.status_message = Some("an action is already in flight".into());
        return false;
    }
    let node_id = state.resource.actions.node_id.clone();
    if node_id.is_empty() {
        state.last_error = Some("this resource does not support actions".into());
        return false;
    }
    let kind = state.resource.kind();
    let target = state.resource.id.clone();
    let origin_tab_id = state.active_resource_tab_id();
    state.begin_action_submission(&action);
    let tx = mutation_tx.clone();
    tokio::spawn(async move {
        let result = mutations::submit_resource_action(&node_id, kind, &action).await;
        let _ = tx.send(MutationOutcome {
            action,
            target,
            origin_tab_id,
            result,
        });
    });
    true
}

pub(crate) struct MutationApplication {
    pub changed: bool,
    pub refresh_started: bool,
}

/// Apply finished write actions in their originating tab and refresh that
/// tab from GitHub on success. A tab that was closed or navigated to a
/// different resource mid-flight is left untouched.
pub(crate) fn apply_completed_mutations(
    state: &mut AppState,
    mutation_rx: &mut UnboundedReceiver<MutationOutcome>,
    fetch_source: &FetchSource,
    fetch_tx: &UnboundedSender<FetchOutcome>,
) -> MutationApplication {
    let mut application = MutationApplication {
        changed: false,
        refresh_started: false,
    };
    while let Ok(outcome) = mutation_rx.try_recv() {
        application.changed = true;
        let succeeded = outcome.result.is_ok();
        let error = outcome.result.err().map(|error| format!("{error:#}"));
        let action = outcome.action;
        let target = outcome.target;
        let mut refresh_started = false;
        let mut refresh_deferred = false;
        let mut applied = false;
        state.apply_to_resource_tab(outcome.origin_tab_id, |state| {
            if !resource_matches_target(&state.resource, &target) {
                // The tab navigated to another resource mid-flight; its
                // messages and drafts belong to that resource now.
                return;
            }
            applied = true;
            state.finish_action_submission(&action, error);
            if succeeded {
                refresh_started = start_background_fetch(
                    state,
                    FetchAction::Refresh { id: target.clone() },
                    fetch_source.clone(),
                    fetch_tx,
                );
                refresh_deferred = !refresh_started;
            }
        });
        if !applied {
            // The originating view is gone; never leave the app locked.
            state.pending_action = None;
        }
        if refresh_deferred {
            // An older fetch is still running and its completion wipes the
            // deferred-refresh flag, so remember this refresh explicitly.
            let entry = (outcome.origin_tab_id, target);
            if !state.pending_action_refreshes.contains(&entry) {
                state.pending_action_refreshes.push(entry);
            }
        }
        application.refresh_started |= refresh_started;
    }
    application
}

/// Start post-action refreshes that had to wait for an in-flight fetch.
/// Entries whose tab is gone or shows a different resource are dropped;
/// entries that still cannot start (another fetch raced in) are kept for
/// the next idle cycle. Returns true when a refresh was started.
pub(crate) fn start_pending_action_refreshes(
    state: &mut AppState,
    fetch_source: &FetchSource,
    fetch_tx: &UnboundedSender<FetchOutcome>,
) -> bool {
    if state.loading_message().is_some() {
        return false;
    }
    let mut started = false;
    let entries = std::mem::take(&mut state.pending_action_refreshes);
    let mut remaining = Vec::new();
    for (tab_id, target) in entries {
        if started {
            remaining.push((tab_id, target));
            continue;
        }
        let mut still_waiting = false;
        state.apply_to_resource_tab(tab_id, |state| {
            if resource_matches_target(&state.resource, &target) {
                started = start_background_fetch(
                    state,
                    FetchAction::Refresh { id: target.clone() },
                    fetch_source.clone(),
                    fetch_tx,
                );
                still_waiting = !started;
            }
        });
        if still_waiting {
            remaining.push((tab_id, target));
        }
    }
    state.pending_action_refreshes = remaining;
    started
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
            ResourceId, ResourceKind, FILE_PATCH_CONTEXT_UNAVAILABLE_WARNING,
            FULL_DEPTH_WARNING_HINT,
        },
        github::api::GithubApiGateway,
        runner::maybe_auto_refresh_with_start,
        test_fixtures::{issue_resource, pr_resource_with_patch},
    };

    use super::{
        apply_completed_mutations, apply_fetch_outcome, should_enqueue_enrichment,
        start_background_fetch, start_background_mutation, start_pending_action_refreshes,
        FetchAction, FetchOutcome, FetchOwner, FetchSource, FetchStage, MutationOutcome,
        OfflineFixtureSource,
    };
    use crate::domain::ResourceAction;

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
                owner: FetchOwner::new(request_id, origin_tab_id),
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
                owner: FetchOwner::new(request_id, origin_tab_id),
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
                owner: FetchOwner::new(request_id, origin_tab_id),
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
                owner: FetchOwner::new(request_id, origin_tab_id),
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
                owner: FetchOwner::new(request_id, origin_tab_id),
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
                owner: FetchOwner::new(request_id, origin_tab_id),
                stage: FetchStage::Base,
            },
        );

        assert_eq!(state.resource.title, "Existing issue");
        assert_eq!(state.last_error.as_deref(), Some("network down"));
        assert!(state.status_message.is_none());
        assert!(state.loading.is_none());
    }

    #[test]
    fn public_rest_fallback_base_does_not_show_enrichment_status() {
        let id = ResourceId {
            owner: "owner".into(),
            repo: "repo".into(),
            number: 1,
            kind_hint: None,
        };
        let mut fallback = issue_resource(1, "REST fallback issue");
        fallback
            .warnings
            .push("using public REST fallback after GitHub auth/API error: rate limited".into());
        let mut state = AppState::new(loading_resource_placeholder(id.clone()));
        let action = FetchAction::Initial { id };
        let (request_id, origin_tab_id) = begin_test_fetch(&mut state, &action);

        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action,
                result: Ok(fallback),
                refreshed_at: "12:34:56 UTC".into(),
                owner: FetchOwner::new(request_id, origin_tab_id),
                stage: FetchStage::Base,
            },
        );

        assert_eq!(state.resource.title, "REST fallback issue");
        assert!(state.loading.is_none());
        assert!(state.status_message.is_none());
    }

    #[test]
    fn enrichment_does_not_clear_other_tab_blocking_load() {
        let first = issue_resource(1, "First issue");
        let second = issue_resource(2, "Second issue");
        let mut state = AppState::new(first.clone());
        let first_action = FetchAction::Refresh {
            id: first.id.clone(),
        };
        let (first_request_id, first_origin_tab_id) = begin_test_fetch(&mut state, &first_action);

        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action: first_action.clone(),
                result: Ok(first.clone()),
                refreshed_at: "12:34:56 UTC".into(),
                owner: FetchOwner::new(first_request_id, first_origin_tab_id),
                stage: FetchStage::Base,
            },
        );

        state.open_resource_in_tab(second.clone());
        let second_action = FetchAction::Refresh {
            id: second.id.clone(),
        };
        let (second_request_id, second_origin_tab_id) =
            begin_test_fetch(&mut state, &second_action);
        let mut enriched_first = first.clone();
        enriched_first.body = "enriched first body".into();

        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action: first_action,
                result: Ok(enriched_first),
                refreshed_at: "12:35:01 UTC".into(),
                owner: FetchOwner::new(first_request_id, first_origin_tab_id),
                stage: FetchStage::Enrichment,
            },
        );

        assert_eq!(state.resource.id.number, 2);
        assert_eq!(
            state.loading_message(),
            Some("refreshing owner/repo#2 from GitHub")
        );
        assert!(state.loading_request_matches(second_request_id));

        let mut refreshed_second = second;
        refreshed_second.body = "refreshed second body".into();
        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action: second_action,
                result: Ok(refreshed_second),
                refreshed_at: "12:35:02 UTC".into(),
                owner: FetchOwner::new(second_request_id, second_origin_tab_id),
                stage: FetchStage::Complete,
            },
        );

        assert_eq!(state.resource.body, "refreshed second body");
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
                owner: FetchOwner::new(request_id, origin_tab_id),
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
                owner: FetchOwner::new(request_id, origin_tab_id),
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
                owner: FetchOwner::new(request_id, origin_tab_id),
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
                owner: FetchOwner::new(request_id, origin_tab_id),
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
                owner: FetchOwner::new(request_id, origin_tab_id),
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
    fn duplicate_progressive_enrichment_error_still_clears_status() {
        let id = ResourceId {
            owner: "owner".into(),
            repo: "repo".into(),
            number: 1,
            kind_hint: None,
        };
        let mut state = AppState::new(issue_resource(1, "Base issue"));
        state
            .resource
            .warnings
            .push("background details unavailable: timeline timed out".into());
        state.status_message = Some("loading additional GitHub details".into());
        let action = FetchAction::Refresh { id };
        let (request_id, origin_tab_id) = begin_test_fetch(&mut state, &action);

        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action,
                result: Err(anyhow::anyhow!("timeline timed out")),
                refreshed_at: "12:35:01 UTC".into(),
                owner: FetchOwner::new(request_id, origin_tab_id),
                stage: FetchStage::Enrichment,
            },
        );

        assert!(state.status_message.is_none());
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
        let origin_tab_id = state.active_resource_tab_id();
        let request_id = state.begin_file_patch_loading();

        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action,
                result: Err(anyhow::anyhow!("rate limited")),
                refreshed_at: "12:35:01 UTC".into(),
                owner: FetchOwner::new(request_id, origin_tab_id),
                stage: FetchStage::FilePatches,
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
    fn file_patch_outcome_merges_patches_without_overwriting_current_resource() {
        let mut current = pr_resource_with_patch(None);
        current.body = "current enriched body".into();
        current.warnings.push("current warning".into());
        let mut patch_resource = pr_resource_with_patch(Some("@@ -1 +1 @@\n-old\n+new"));
        patch_resource.body = "older background copy".into();
        patch_resource.warnings.push("patch warning".into());
        let mut state = AppState::new(current);
        state.set_tab(Tab::Files);
        let action = FetchAction::LoadFilePatches {
            resource: Box::new(pr_resource_with_patch(None)),
        };
        let origin_tab_id = state.active_resource_tab_id();
        let request_id = state.begin_file_patch_loading();

        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action,
                result: Ok(patch_resource),
                refreshed_at: "12:36:00 UTC".into(),
                owner: FetchOwner::new(request_id, origin_tab_id),
                stage: FetchStage::FilePatches,
            },
        );

        assert_eq!(state.resource.body, "current enriched body");
        assert_eq!(
            state.resource.pull_request.as_ref().unwrap().files[0]
                .patch
                .as_deref(),
            Some("@@ -1 +1 @@\n-old\n+new")
        );
        assert_eq!(
            state.resource.warnings,
            ["current warning", "patch warning"]
        );
        assert_eq!(state.last_refresh_changed_sections, ["warnings", "files"]);
        assert!(state.file_patch_loading_message().is_none());
        assert!(state.loading.is_none());
    }

    #[test]
    fn file_patch_loading_does_not_invalidate_pending_enrichment() {
        let id = ResourceId {
            owner: "owner".into(),
            repo: "repo".into(),
            number: 1,
            kind_hint: Some(ResourceKind::PullRequest),
        };
        let mut state = AppState::new(loading_resource_placeholder(id.clone()));
        let action = FetchAction::Initial { id };
        let (request_id, origin_tab_id) = begin_test_fetch(&mut state, &action);

        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action: action.clone(),
                result: Ok(pr_resource_with_patch(None)),
                refreshed_at: "12:34:56 UTC".into(),
                owner: FetchOwner::new(request_id, origin_tab_id),
                stage: FetchStage::Base,
            },
        );

        state.set_tab(Tab::Files);
        state.begin_file_patch_loading();
        let mut enriched = pr_resource_with_patch(None);
        enriched.body = "enriched body".into();

        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action,
                result: Ok(enriched),
                refreshed_at: "12:35:01 UTC".into(),
                owner: FetchOwner::new(request_id, origin_tab_id),
                stage: FetchStage::Enrichment,
            },
        );

        assert_eq!(state.resource.body, "enriched body");
        assert!(state.loading.is_none());
    }

    #[test]
    fn public_rest_fallback_resources_do_not_enqueue_graphql_enrichment() {
        let mut resource = issue_resource(1, "REST fallback issue");
        assert!(should_enqueue_enrichment(&resource));

        resource
            .warnings
            .push("using public REST fallback after GitHub auth/API error: rate limited".into());

        assert!(!should_enqueue_enrichment(&resource));
    }

    #[test]
    fn newer_blocking_fetch_cancels_pending_file_patch_result() {
        let resource = pr_resource_with_patch(None);
        let mut state = AppState::new(resource.clone());
        state.set_tab(Tab::Files);
        let patch_request_id = state.begin_file_patch_loading();
        let origin_tab_id = state.active_resource_tab_id();

        let refresh_action = FetchAction::Refresh {
            id: resource.id.clone(),
        };
        let (_refresh_request_id, _refresh_origin_tab_id) =
            begin_test_fetch(&mut state, &refresh_action);

        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action: FetchAction::LoadFilePatches {
                    resource: Box::new(resource),
                },
                result: Ok(pr_resource_with_patch(Some("@@ stale patch"))),
                refreshed_at: "12:36:00 UTC".into(),
                owner: FetchOwner::new(patch_request_id, origin_tab_id),
                stage: FetchStage::FilePatches,
            },
        );

        assert!(state.file_patch_loading_message().is_none());
        assert!(state.resource.pull_request.as_ref().unwrap().files[0]
            .patch
            .is_none());
    }

    #[test]
    fn enrichment_preserves_loaded_file_patches() {
        let resource = pr_resource_with_patch(None);
        let mut state = AppState::new(resource.clone());
        state.set_tab(Tab::Files);
        let action = FetchAction::Initial {
            id: resource.id.clone(),
        };
        let (request_id, origin_tab_id) = begin_test_fetch(&mut state, &action);

        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action: action.clone(),
                result: Ok(resource.clone()),
                refreshed_at: "12:34:56 UTC".into(),
                owner: FetchOwner::new(request_id, origin_tab_id),
                stage: FetchStage::Base,
            },
        );

        let patch_request_id = state.begin_file_patch_loading();
        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action: FetchAction::LoadFilePatches {
                    resource: Box::new(resource.clone()),
                },
                result: Ok(pr_resource_with_patch(Some("@@ loaded patch"))),
                refreshed_at: "12:35:00 UTC".into(),
                owner: FetchOwner::new(patch_request_id, origin_tab_id),
                stage: FetchStage::FilePatches,
            },
        );

        let mut enriched = pr_resource_with_patch(None);
        enriched.body = "enriched body".into();
        apply_fetch_outcome(
            &mut state,
            FetchOutcome {
                action,
                result: Ok(enriched),
                refreshed_at: "12:35:01 UTC".into(),
                owner: FetchOwner::new(request_id, origin_tab_id),
                stage: FetchStage::Enrichment,
            },
        );

        assert_eq!(state.resource.body, "enriched body");
        assert_eq!(
            state.resource.pull_request.as_ref().unwrap().files[0]
                .patch
                .as_deref(),
            Some("@@ loaded patch")
        );
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
                owner: FetchOwner::new(request_id, origin_tab_id),
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
                owner: FetchOwner::new(request_id, origin_tab_id),
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

    #[tokio::test]
    async fn file_patch_loading_does_not_block_open_fetches() {
        let resource = pr_resource_with_patch(None);
        let second = issue_resource(2, "Second issue");
        let mut state = AppState::new(resource.clone());
        state.set_tab(Tab::Files);
        let (fetch_tx, mut fetch_rx) = tokio::sync::mpsc::unbounded_channel();

        assert!(start_background_fetch(
            &mut state,
            FetchAction::LoadFilePatches {
                resource: Box::new(resource)
            },
            FetchSource::OfflineFixtures(OfflineFixtureSource::new([second.clone()])),
            &fetch_tx,
        ));
        assert!(state.loading.is_none());
        assert_eq!(
            state.file_patch_loading_message(),
            Some("loading file diffs")
        );

        assert!(start_background_fetch(
            &mut state,
            FetchAction::OpenTab {
                id: second.id.clone()
            },
            FetchSource::OfflineFixtures(OfflineFixtureSource::new([second])),
            &fetch_tx,
        ));
        assert_eq!(
            state.loading_message(),
            Some("opening owner/repo#2 in a new tab")
        );

        let mut outcomes = Vec::new();
        for _ in 0..2 {
            outcomes.push(fetch_rx.recv().await.expect("fetch outcome"));
        }
        assert!(outcomes
            .iter()
            .any(|outcome| outcome.stage == FetchStage::FilePatches));
        assert!(outcomes
            .iter()
            .any(|outcome| outcome.stage == FetchStage::Complete));
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
                owner: FetchOwner::new(request_id, origin_tab_id),
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
                owner: FetchOwner::new(request_id, origin_tab_id),
                stage: FetchStage::Complete,
            },
        );

        assert_eq!(state.history, [previous.id]);
        assert_eq!(state.last_error.as_deref(), Some("network down"));
        assert!(state.loading.is_none());
    }

    fn actionable_state() -> AppState {
        let mut resource = issue_resource(9, "Actionable");
        resource.actions.node_id = "I_node".into();
        resource.actions.viewer_can_update = true;
        AppState::new(resource)
    }

    #[test]
    fn mutation_is_rejected_while_another_is_pending() {
        let mut state = actionable_state();
        state.pending_action = Some(ResourceAction::Close);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        assert!(!start_background_mutation(
            &mut state,
            ResourceAction::Reopen,
            &tx
        ));
        assert_eq!(
            state.status_message.as_deref(),
            Some("an action is already in flight")
        );
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn mutation_is_rejected_without_a_node_id() {
        let mut state = AppState::new(issue_resource(9, "No node id"));
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        assert!(!start_background_mutation(
            &mut state,
            ResourceAction::Close,
            &tx
        ));
        assert_eq!(
            state.last_error.as_deref(),
            Some("this resource does not support actions")
        );
        assert!(rx.try_recv().is_err());
        assert!(state.pending_action.is_none());
    }

    fn offline_source(state: &AppState) -> FetchSource {
        FetchSource::OfflineFixtures(OfflineFixtureSource::new([state.resource.clone()]))
    }

    fn deliver_outcome(
        state: &mut AppState,
        outcome: MutationOutcome,
    ) -> super::MutationApplication {
        let (mutation_tx, mut mutation_rx) = tokio::sync::mpsc::unbounded_channel();
        let (fetch_tx, _fetch_rx) = tokio::sync::mpsc::unbounded_channel();
        mutation_tx.send(outcome).expect("send outcome");
        let source = offline_source(state);
        apply_completed_mutations(state, &mut mutation_rx, &source, &fetch_tx)
    }

    #[tokio::test]
    async fn successful_mutation_clears_pending_and_starts_a_refresh() {
        let mut state = actionable_state();
        state.begin_action_submission(&ResourceAction::Close);
        let outcome = MutationOutcome {
            action: ResourceAction::Close,
            target: state.resource.id.clone(),
            origin_tab_id: state.active_resource_tab_id(),
            result: Ok(()),
        };

        let application = deliver_outcome(&mut state, outcome);

        assert!(application.changed);
        assert!(application.refresh_started);
        assert!(state.pending_action.is_none());
        assert!(state.last_error.is_none());
        assert_eq!(
            state.loading.as_ref().map(|loading| loading.target.clone()),
            Some(state.resource.id.clone())
        );
    }

    #[tokio::test]
    async fn failed_mutation_surfaces_the_error_and_keeps_the_composer() {
        let mut state = actionable_state();
        let action = ResourceAction::Comment {
            body: "hello".into(),
        };
        state.begin_action_submission(&action);
        state.comment_composer = Some(crate::app::CommentComposer::new());
        let outcome = MutationOutcome {
            action,
            target: state.resource.id.clone(),
            origin_tab_id: state.active_resource_tab_id(),
            result: Err(anyhow::anyhow!("Pull request is not mergeable")),
        };

        let application = deliver_outcome(&mut state, outcome);

        assert!(application.changed);
        assert!(!application.refresh_started);
        assert!(state.pending_action.is_none());
        assert!(state
            .last_error
            .as_deref()
            .expect("error surfaced")
            .contains("commenting failed"));
        assert!(
            state.comment_composer.is_some(),
            "failed comment keeps the draft so nothing is lost"
        );
    }

    #[tokio::test]
    async fn successful_comment_closes_the_composer() {
        let mut state = actionable_state();
        let action = ResourceAction::Comment {
            body: "hello".into(),
        };
        state.begin_action_submission(&action);
        let mut composer = crate::app::CommentComposer::new();
        composer.insert_str("hello");
        state.comment_composer = Some(composer);
        let outcome = MutationOutcome {
            action,
            target: state.resource.id.clone(),
            origin_tab_id: state.active_resource_tab_id(),
            result: Ok(()),
        };

        deliver_outcome(&mut state, outcome);

        assert!(state.comment_composer.is_none());
    }

    #[tokio::test]
    async fn completed_comment_spares_a_replacement_draft_in_the_same_tab() {
        let mut state = actionable_state();
        let action = ResourceAction::Comment {
            body: "posted text".into(),
        };
        state.begin_action_submission(&action);
        // The user dismissed the frozen composer and opened a new draft
        // in the same tab while the first comment was still posting.
        state.comment_composer = Some(crate::app::CommentComposer::new());
        state
            .comment_composer
            .as_mut()
            .expect("replacement composer")
            .insert_str("replacement draft");
        let outcome = MutationOutcome {
            action,
            target: state.resource.id.clone(),
            origin_tab_id: state.active_resource_tab_id(),
            result: Ok(()),
        };

        deliver_outcome(&mut state, outcome);

        assert_eq!(
            state.comment_composer.as_ref().map(|c| c.body()),
            Some("replacement draft".into()),
            "a replacement draft must survive the earlier comment completing"
        );
    }

    #[tokio::test]
    async fn mutation_outcome_lands_in_its_origin_tab_not_the_active_one() {
        let mut state = actionable_state();
        let origin_tab_id = state.active_resource_tab_id();
        let target = state.resource.id.clone();
        state.begin_action_submission(&ResourceAction::Close);
        state.open_resource_in_tab(issue_resource(99, "Other tab"));
        let active_before = state.resource.clone();

        let application = deliver_outcome(
            &mut state,
            MutationOutcome {
                action: ResourceAction::Close,
                target: target.clone(),
                origin_tab_id,
                result: Ok(()),
            },
        );

        assert!(application.refresh_started);
        assert_eq!(
            state.resource, active_before,
            "the active tab's resource must stay untouched"
        );
        assert_eq!(
            state.loading.as_ref().map(|loading| loading.origin_tab_id),
            Some(origin_tab_id),
            "the refresh must be owned by the tab that submitted the action"
        );
        assert_eq!(
            state.loading.as_ref().map(|loading| loading.target.clone()),
            Some(target)
        );
    }

    #[tokio::test]
    async fn completed_comment_never_discards_another_tabs_draft() {
        let mut state = actionable_state();
        let origin_tab_id = state.active_resource_tab_id();
        let target = state.resource.id.clone();
        let action = ResourceAction::Comment {
            body: "sent from tab one".into(),
        };
        state.begin_action_submission(&action);
        state.comment_composer = None;
        state.open_resource_in_tab(issue_resource(99, "Other tab"));
        state.open_comment_composer();
        state
            .comment_composer
            .as_mut()
            .expect("second tab composer")
            .insert_str("unsent draft on tab two");

        deliver_outcome(
            &mut state,
            MutationOutcome {
                action,
                target,
                origin_tab_id,
                result: Ok(()),
            },
        );

        assert_eq!(
            state.comment_composer.as_ref().map(|c| c.body()),
            Some("unsent draft on tab two".into()),
            "a completed comment on one tab must not clear another tab's draft"
        );
    }

    #[tokio::test]
    async fn deferred_post_action_refresh_survives_an_in_flight_fetch() {
        let mut state = actionable_state();
        let origin_tab_id = state.active_resource_tab_id();
        let target = state.resource.id.clone();
        state.begin_action_submission(&ResourceAction::Close);
        state.begin_loading(target.clone(), "refreshing older data");

        let application = deliver_outcome(
            &mut state,
            MutationOutcome {
                action: ResourceAction::Close,
                target: target.clone(),
                origin_tab_id,
                result: Ok(()),
            },
        );

        assert!(!application.refresh_started);
        assert_eq!(
            state.pending_action_refreshes,
            vec![(origin_tab_id, target.clone())],
            "the refresh must be remembered while the older fetch runs"
        );

        // While the older fetch is still running, the queue must survive
        // untouched instead of being drained into a fetch that cannot start.
        let (fetch_tx, _fetch_rx) = tokio::sync::mpsc::unbounded_channel();
        let source = offline_source(&state);
        assert!(!start_pending_action_refreshes(
            &mut state, &source, &fetch_tx
        ));
        assert_eq!(
            state.pending_action_refreshes,
            vec![(origin_tab_id, target.clone())]
        );

        // The older fetch completes and wipes the deferred-refresh flag.
        state.finish_loading();
        state.refresh_requested = false;

        assert!(start_pending_action_refreshes(
            &mut state, &source, &fetch_tx
        ));
        assert!(state.pending_action_refreshes.is_empty());
        assert_eq!(
            state.loading.as_ref().map(|loading| loading.target.clone()),
            Some(target)
        );
    }

    #[tokio::test]
    async fn mutation_outcome_for_a_closed_tab_clears_pending_without_refresh() {
        let mut state = actionable_state();
        state.begin_action_submission(&ResourceAction::Close);
        let outcome = MutationOutcome {
            action: ResourceAction::Close,
            target: state.resource.id.clone(),
            origin_tab_id: 424_242,
            result: Ok(()),
        };

        let application = deliver_outcome(&mut state, outcome);

        assert!(application.changed);
        assert!(!application.refresh_started);
        assert!(state.pending_action.is_none());
    }

    #[tokio::test]
    async fn mutation_outcome_skips_refresh_when_the_tab_navigated_away() {
        let mut state = actionable_state();
        let origin_tab_id = state.active_resource_tab_id();
        let target = state.resource.id.clone();
        state.begin_action_submission(&ResourceAction::Close);
        state.resource = issue_resource(500, "Navigated elsewhere");
        state.status_message = None;

        let application = deliver_outcome(
            &mut state,
            MutationOutcome {
                action: ResourceAction::Close,
                target,
                origin_tab_id,
                result: Ok(()),
            },
        );

        assert!(application.changed);
        assert!(!application.refresh_started);
        assert!(state.pending_action.is_none());
        assert!(state.loading.is_none());
        assert_eq!(
            state.status_message, None,
            "completion messages must not leak onto the navigated-to resource"
        );
    }
}

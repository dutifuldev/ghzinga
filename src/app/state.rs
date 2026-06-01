use std::{collections::HashSet, path::PathBuf, str::FromStr};

use crate::domain::{PullRequest, Resource, ResourceId, ResourceKind};
use crate::input::HitArea;
use crate::render::{
    normalize_fixed_width, ContentWidthMode, ScrollbarMode, SpacingMode, SymbolMode, ThemeName,
    DEFAULT_FIXED_CONTENT_WIDTH, FIXED_CONTENT_WIDTH_STEP, MAX_FIXED_CONTENT_WIDTH,
    MIN_FIXED_CONTENT_WIDTH,
};

const SCROLLBAR_VISIBLE_FRAMES: u8 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tab {
    Overview,
    Activity,
    Commits,
    Checks,
    Files,
    Links,
}

impl Tab {
    pub fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Activity => "Activity",
            Self::Commits => "Commits",
            Self::Checks => "Checks",
            Self::Files => "Files",
            Self::Links => "Links",
        }
    }

    pub fn all_for(kind: ResourceKind) -> &'static [Tab] {
        match kind {
            ResourceKind::PullRequest => &[
                Self::Overview,
                Self::Activity,
                Self::Commits,
                Self::Checks,
                Self::Files,
                Self::Links,
            ],
            ResourceKind::Issue => &[Self::Overview, Self::Activity, Self::Links],
        }
    }
}

impl FromStr for Tab {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.trim().to_ascii_lowercase().as_str() {
            "overview" => Ok(Self::Overview),
            "activity" | "comments" => Ok(Self::Activity),
            "commits" => Ok(Self::Commits),
            "checks" | "ci" => Ok(Self::Checks),
            "files" => Ok(Self::Files),
            "links" => Ok(Self::Links),
            _ => {
                Err("expected one of overview, activity, commits, checks, files, links".to_string())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BlockId {
    Body,
    Activity(String),
    Commit(String),
    Check(String),
    File(String),
    Patch(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadingState {
    pub target: ResourceId,
    pub message: String,
    frame: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollbarDragState {
    pub top: u16,
    pub height: u16,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub resource: Resource,
    pub active_tab: Tab,
    pub scroll: u16,
    pub scroll_limit: u16,
    pub scrollbar_visible_frames: u8,
    pub scrollbar: ScrollbarMode,
    pub scrollbar_drag: Option<ScrollbarDragState>,
    pub expanded_blocks: HashSet<BlockId>,
    pub hit_areas: Vec<HitArea>,
    pub history: Vec<crate::domain::ResourceId>,
    pub should_quit: bool,
    pub refresh_requested: bool,
    pub last_refreshed_at: Option<String>,
    pub last_refresh_had_changes: Option<bool>,
    pub last_refresh_changed_sections: Vec<String>,
    pub last_error: Option<String>,
    pub status_message: Option<String>,
    pub loading: Option<LoadingState>,
    pub show_help: bool,
    pub show_settings: bool,
    pub reverse_chronological: bool,
    pub theme: ThemeName,
    pub symbols: SymbolMode,
    pub spacing: SpacingMode,
    pub width_mode: ContentWidthMode,
    pub fixed_width: u16,
    pub config_path: PathBuf,
}

impl AppState {
    pub fn new(resource: Resource) -> Self {
        Self {
            active_tab: Tab::Overview,
            resource,
            scroll: 0,
            scroll_limit: u16::MAX,
            scrollbar_visible_frames: 0,
            scrollbar: ScrollbarMode::OnScroll,
            scrollbar_drag: None,
            expanded_blocks: HashSet::new(),
            hit_areas: Vec::new(),
            history: Vec::new(),
            should_quit: false,
            refresh_requested: false,
            last_refreshed_at: None,
            last_refresh_had_changes: None,
            last_refresh_changed_sections: Vec::new(),
            last_error: None,
            status_message: None,
            loading: None,
            show_help: false,
            show_settings: false,
            reverse_chronological: false,
            theme: ThemeName::Default,
            symbols: SymbolMode::Ascii,
            spacing: SpacingMode::Comfortable,
            width_mode: ContentWidthMode::Fixed,
            fixed_width: DEFAULT_FIXED_CONTENT_WIDTH,
            config_path: crate::config::config_path(),
        }
    }

    pub fn tabs(&self) -> &'static [Tab] {
        Tab::all_for(self.resource.kind())
    }

    pub fn set_tab(&mut self, tab: Tab) {
        if self.tabs().contains(&tab) {
            self.active_tab = tab;
            self.scroll = 0;
            self.scroll_limit = u16::MAX;
        }
    }

    pub fn next_tab(&mut self) {
        let tabs = self.tabs();
        let current = tabs
            .iter()
            .position(|tab| *tab == self.active_tab)
            .unwrap_or(0);
        self.set_tab(tabs[(current + 1) % tabs.len()]);
    }

    pub fn previous_tab(&mut self) {
        let tabs = self.tabs();
        let current = tabs
            .iter()
            .position(|tab| *tab == self.active_tab)
            .unwrap_or(0);
        let next = if current == 0 {
            tabs.len() - 1
        } else {
            current - 1
        };
        self.set_tab(tabs[next]);
    }

    pub fn toggle_block(&mut self, id: BlockId) {
        if !self.expanded_blocks.insert(id.clone()) {
            self.expanded_blocks.remove(&id);
        }
    }

    pub fn expand_blocks(&mut self, blocks: impl IntoIterator<Item = BlockId>) {
        self.expanded_blocks.extend(blocks);
    }

    pub fn collapse_blocks(&mut self, blocks: impl IntoIterator<Item = BlockId>) {
        for block in blocks {
            self.expanded_blocks.remove(&block);
        }
    }

    pub fn active_tab_expandable_blocks(&self) -> Vec<BlockId> {
        if self.show_help || self.show_settings {
            return Vec::new();
        }
        expandable_blocks_for_tab(self.active_tab, &self.resource)
    }

    pub fn toggle_active_tab_expansion(&mut self) -> bool {
        let blocks = self.active_tab_expandable_blocks();
        if blocks.is_empty() {
            return false;
        }
        if blocks
            .iter()
            .all(|block| self.expanded_blocks.contains(block))
        {
            self.collapse_blocks(blocks);
        } else {
            self.expand_blocks(blocks);
        }
        true
    }

    pub fn block_expanded(&self, id: &BlockId) -> bool {
        self.expanded_blocks.contains(id)
    }

    pub fn replace_resource_reset_view(&mut self, resource: Resource) {
        self.resource = resource;
        self.active_tab = Tab::Overview;
        self.scroll = 0;
        self.scroll_limit = u16::MAX;
        self.clear_resource_view_state();
    }

    pub fn replace_resource_preserve_tab(&mut self, resource: Resource) {
        let active_tab = self.active_tab;
        self.resource = resource;
        self.active_tab = if self.tabs().contains(&active_tab) {
            active_tab
        } else {
            Tab::Overview
        };
        self.scroll = 0;
        self.scroll_limit = u16::MAX;
        self.clear_resource_view_state();
    }

    fn clear_resource_view_state(&mut self) {
        self.expanded_blocks.clear();
        self.hit_areas.clear();
        self.last_refreshed_at = None;
        self.last_refresh_had_changes = None;
        self.last_refresh_changed_sections.clear();
        self.loading = None;
    }

    pub fn push_current_to_history(&mut self) {
        self.history.push(self.resource.id.clone());
    }

    pub fn pop_history(&mut self) -> Option<crate::domain::ResourceId> {
        self.history.pop()
    }

    pub fn mark_refreshed(&mut self, at: impl Into<String>, changed: bool) {
        self.last_refreshed_at = Some(at.into());
        self.last_refresh_had_changes = Some(changed);
        if !changed {
            self.last_refresh_changed_sections.clear();
        }
    }

    pub fn apply_refreshed_resource(
        &mut self,
        resource: Resource,
        refreshed_at: impl Into<String>,
    ) -> bool {
        let changed_sections = self.resource.changed_sections(&resource);
        let changed =
            !changed_sections.is_empty() || self.resource.fingerprint() != resource.fingerprint();
        self.resource = resource;
        self.refresh_requested = false;
        self.last_error = None;
        self.loading = None;
        self.mark_refreshed(refreshed_at, changed);
        self.last_refresh_changed_sections = changed_sections;
        self.status_message = Some(if changed {
            if self.last_refresh_changed_sections.is_empty() {
                "refreshed from GitHub: changes detected".into()
            } else {
                format!(
                    "refreshed from GitHub: changed {}",
                    self.last_refresh_changed_sections.join(", ")
                )
            }
        } else {
            "refreshed from GitHub: no changes".into()
        });
        changed
    }

    pub fn begin_loading(&mut self, target: ResourceId, message: impl Into<String>) {
        self.loading = Some(LoadingState {
            target,
            message: message.into(),
            frame: 0,
        });
        self.last_error = None;
    }

    pub fn finish_loading(&mut self) {
        self.loading = None;
        self.refresh_requested = false;
    }

    pub fn loading_message(&self) -> Option<&str> {
        self.loading
            .as_ref()
            .map(|loading| loading.message.as_str())
    }

    pub fn loading_indicator(&self) -> Option<&'static str> {
        const FRAMES: [&str; 4] = ["|", "/", "-", "\\"];
        self.loading
            .as_ref()
            .map(|loading| FRAMES[loading.frame as usize % FRAMES.len()])
    }

    pub fn advance_loading_frame(&mut self) {
        if let Some(loading) = &mut self.loading {
            loading.frame = loading.frame.wrapping_add(1);
        }
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
        if self.show_help {
            self.show_settings = false;
        }
        self.scroll = 0;
        self.scroll_limit = u16::MAX;
    }

    pub fn toggle_settings(&mut self) {
        self.show_settings = !self.show_settings;
        if self.show_settings {
            self.show_help = false;
        }
        self.scroll = 0;
        self.scroll_limit = u16::MAX;
    }

    pub fn close_settings(&mut self) {
        self.show_settings = false;
        self.scroll = 0;
        self.scroll_limit = u16::MAX;
    }

    pub fn toggle_feed_order(&mut self) {
        self.reverse_chronological = !self.reverse_chronological;
        self.scroll = 0;
        self.scroll_limit = u16::MAX;
    }

    pub fn set_theme(&mut self, theme: ThemeName) -> bool {
        if self.theme == theme {
            return false;
        }
        self.theme = theme;
        true
    }

    pub fn set_symbols(&mut self, symbols: SymbolMode) -> bool {
        if self.symbols == symbols {
            return false;
        }
        self.symbols = symbols;
        true
    }

    pub fn set_spacing(&mut self, spacing: SpacingMode) -> bool {
        if self.spacing == spacing {
            return false;
        }
        self.spacing = spacing;
        self.scroll = 0;
        self.scroll_limit = u16::MAX;
        true
    }

    pub fn set_width_mode(&mut self, width_mode: ContentWidthMode) -> bool {
        if self.width_mode == width_mode {
            return false;
        }
        self.width_mode = width_mode;
        self.scroll = 0;
        self.scroll_limit = u16::MAX;
        true
    }

    pub fn set_fixed_width(&mut self, fixed_width: u16) -> bool {
        let fixed_width = normalize_fixed_width(fixed_width);
        if self.fixed_width == fixed_width {
            return false;
        }
        self.fixed_width = fixed_width;
        self.scroll = 0;
        self.scroll_limit = u16::MAX;
        true
    }

    pub fn set_scrollbar(&mut self, scrollbar: ScrollbarMode) -> bool {
        if self.scrollbar == scrollbar {
            return false;
        }
        self.scrollbar = scrollbar;
        self.scrollbar_drag = None;
        if scrollbar == ScrollbarMode::OnScroll {
            self.reveal_scrollbar();
        }
        true
    }

    pub fn cycle_theme(&mut self) -> bool {
        let themes = ThemeName::ALL;
        let index = themes
            .iter()
            .position(|theme| *theme == self.theme)
            .unwrap_or(0);
        let next = themes[(index + 1) % themes.len()];
        self.set_theme(next)
    }

    pub fn cycle_symbols(&mut self) -> bool {
        let next = match self.symbols {
            SymbolMode::Ascii => SymbolMode::Emoji,
            SymbolMode::Emoji => SymbolMode::Ascii,
        };
        self.set_symbols(next)
    }

    pub fn cycle_spacing(&mut self) -> bool {
        let next = match self.spacing {
            SpacingMode::Comfortable => SpacingMode::Compact,
            SpacingMode::Compact => SpacingMode::Comfortable,
        };
        self.set_spacing(next)
    }

    pub fn cycle_width_mode(&mut self) -> bool {
        let next = match self.width_mode {
            ContentWidthMode::Fixed => ContentWidthMode::Full,
            ContentWidthMode::Full => ContentWidthMode::Fixed,
        };
        self.set_width_mode(next)
    }

    pub fn cycle_scrollbar(&mut self) -> bool {
        let next = match self.scrollbar {
            ScrollbarMode::OnScroll => ScrollbarMode::Always,
            ScrollbarMode::Always => ScrollbarMode::Hidden,
            ScrollbarMode::Hidden => ScrollbarMode::OnScroll,
        };
        self.set_scrollbar(next)
    }

    pub fn increase_fixed_width(&mut self) -> bool {
        self.set_fixed_width(
            self.fixed_width
                .saturating_add(FIXED_CONTENT_WIDTH_STEP)
                .min(MAX_FIXED_CONTENT_WIDTH),
        )
    }

    pub fn decrease_fixed_width(&mut self) -> bool {
        self.set_fixed_width(
            self.fixed_width
                .saturating_sub(FIXED_CONTENT_WIDTH_STEP)
                .max(MIN_FIXED_CONTENT_WIDTH),
        )
    }

    pub fn set_scroll_limit(&mut self, limit: u16) {
        self.scroll_limit = limit;
        if self.scroll > self.scroll_limit {
            self.scroll = self.scroll_limit;
        }
    }

    pub fn scroll_down(&mut self, lines: u16) {
        self.scroll = self.scroll.saturating_add(lines).min(self.scroll_limit);
        self.reveal_scrollbar();
    }

    pub fn scroll_up(&mut self, lines: u16) {
        self.scroll = self.scroll.saturating_sub(lines);
        self.reveal_scrollbar();
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll = 0;
        self.reveal_scrollbar();
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll = self.scroll_limit;
        self.reveal_scrollbar();
    }

    pub fn scrollbar_visible(&self) -> bool {
        self.scroll_limit > 0
            && match self.scrollbar {
                ScrollbarMode::Always => true,
                ScrollbarMode::OnScroll => self.scrollbar_visible_frames > 0,
                ScrollbarMode::Hidden => false,
            }
    }

    pub fn advance_scrollbar_visibility(&mut self) {
        self.scrollbar_visible_frames = self.scrollbar_visible_frames.saturating_sub(1);
    }

    fn reveal_scrollbar(&mut self) {
        if self.scroll_limit > 0 && self.scrollbar == ScrollbarMode::OnScroll {
            self.scrollbar_visible_frames = SCROLLBAR_VISIBLE_FRAMES;
        }
    }

    pub fn begin_scrollbar_drag(&mut self, top: u16, height: u16, row: u16) {
        self.scrollbar_drag = Some(ScrollbarDragState { top, height });
        self.scroll_to_scrollbar_row(top, height, row);
    }

    pub fn drag_scrollbar(&mut self, row: u16) {
        if let Some(drag) = self.scrollbar_drag {
            self.scroll_to_scrollbar_row(drag.top, drag.height, row);
        }
    }

    pub fn end_scrollbar_drag(&mut self) {
        self.scrollbar_drag = None;
    }

    fn scroll_to_scrollbar_row(&mut self, top: u16, height: u16, row: u16) {
        if self.scroll_limit == 0 || height == 0 {
            self.scroll = 0;
            return;
        }
        let last = height.saturating_sub(1);
        let relative = row.saturating_sub(top).min(last);
        self.scroll = if last == 0 {
            0
        } else {
            ((u32::from(relative) * u32::from(self.scroll_limit)) / u32::from(last))
                .min(u32::from(self.scroll_limit)) as u16
        };
        self.reveal_scrollbar();
    }
}

fn expandable_blocks_for_tab(tab: Tab, resource: &Resource) -> Vec<BlockId> {
    match tab {
        Tab::Overview => overview_expandable_blocks(resource),
        Tab::Activity => activity_expandable_blocks(resource),
        Tab::Commits => resource
            .pull_request
            .as_ref()
            .map(commit_expandable_blocks)
            .unwrap_or_default(),
        Tab::Checks => resource
            .pull_request
            .as_ref()
            .map(check_expandable_blocks)
            .unwrap_or_default(),
        Tab::Files => resource
            .pull_request
            .as_ref()
            .map(file_expandable_blocks)
            .unwrap_or_default(),
        Tab::Links => Vec::new(),
    }
}

fn overview_expandable_blocks(resource: &Resource) -> Vec<BlockId> {
    let mut blocks = Vec::new();
    if !resource.body.trim().is_empty() {
        blocks.push(BlockId::Body);
    }
    if let Some(pr) = &resource.pull_request {
        blocks.extend(commit_expandable_blocks(pr));
    }
    blocks.extend(activity_expandable_blocks(resource));
    dedupe_blocks(blocks)
}

fn activity_expandable_blocks(resource: &Resource) -> Vec<BlockId> {
    dedupe_blocks(
        resource
            .activity
            .iter()
            .map(|entry| BlockId::Activity(entry.id.clone()))
            .collect(),
    )
}

fn commit_expandable_blocks(pr: &PullRequest) -> Vec<BlockId> {
    dedupe_blocks(
        pr.commits
            .iter()
            .map(|commit| BlockId::Commit(commit.oid.clone()))
            .collect(),
    )
}

fn check_expandable_blocks(pr: &PullRequest) -> Vec<BlockId> {
    dedupe_blocks(
        pr.checks
            .iter()
            .map(|check| BlockId::Check(format!("{}:{}", check.status.label(), check.name)))
            .collect(),
    )
}

fn file_expandable_blocks(pr: &PullRequest) -> Vec<BlockId> {
    let mut blocks = Vec::new();
    for file in &pr.files {
        blocks.push(BlockId::File(file.path.clone()));
        if file.patch.is_some() {
            blocks.push(BlockId::Patch(file.path.clone()));
        }
    }
    dedupe_blocks(blocks)
}

fn dedupe_blocks(blocks: Vec<BlockId>) -> Vec<BlockId> {
    let mut seen = std::collections::HashSet::new();
    blocks
        .into_iter()
        .filter(|block| seen.insert(block.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        ActivityEntry, ActivityKind, ChangedFile, CheckRun, CheckStatus, ReactionCounts, ResourceId,
    };

    fn issue_resource() -> Resource {
        Resource {
            id: ResourceId {
                owner: "owner".into(),
                repo: "repo".into(),
                number: 1,
                kind_hint: Some(ResourceKind::Issue),
            },
            title: "Issue".into(),
            url: "https://github.com/owner/repo/issues/1".into(),
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

    fn activity_entry(id: &str) -> ActivityEntry {
        ActivityEntry {
            id: id.into(),
            kind: ActivityKind::Comment,
            author: "alice".into(),
            body: "comment".into(),
            updated_at: "now".into(),
            path: None,
            line: None,
            url: None,
            author_association: None,
            reactions: ReactionCounts::default(),
            includes_created_edit: false,
            is_minimized: false,
            minimized_reason: None,
            thread_id: None,
            thread_resolved: None,
            thread_outdated: None,
        }
    }

    fn pr_resource() -> Resource {
        let mut resource = issue_resource();
        resource.id.kind_hint = Some(ResourceKind::PullRequest);
        resource.pull_request = Some(PullRequest {
            base_ref: "main".into(),
            head_ref: "topic".into(),
            requested_reviewers: vec![],
            review_decision: None,
            merge_state: None,
            additions: 3,
            deletions: 1,
            commits: vec![crate::domain::Commit {
                oid: "abc123".into(),
                message: "commit".into(),
                body: "commit body".into(),
                author: "alice".into(),
                authors: vec![],
                authored_at: None,
                committed_at: "now".into(),
                status: CheckStatus::Success,
                deployments: vec![],
            }],
            checks: vec![CheckRun {
                name: "ci".into(),
                status: CheckStatus::Failure,
                summary: Some("failed".into()),
                details_url: None,
                started_at: None,
                completed_at: None,
                raw_status: None,
                raw_conclusion: None,
            }],
            files: vec![ChangedFile {
                path: "src/lib.rs".into(),
                additions: 3,
                deletions: 1,
                change_type: "MODIFIED".into(),
                patch: Some("@@ -1 +1 @@".into()),
            }],
            metadata: vec![],
        });
        resource
    }

    #[test]
    fn issue_tabs_are_limited_to_issue_views() {
        let state = AppState::new(issue_resource());

        assert_eq!(state.tabs(), &[Tab::Overview, Tab::Activity, Tab::Links]);
    }

    #[test]
    fn next_tab_wraps() {
        let mut state = AppState::new(issue_resource());

        state.next_tab();
        state.next_tab();
        state.next_tab();

        assert_eq!(state.active_tab, Tab::Overview);
    }

    #[test]
    fn parses_tab_names() {
        assert_eq!("overview".parse::<Tab>().unwrap(), Tab::Overview);
        assert_eq!("ci".parse::<Tab>().unwrap(), Tab::Checks);
        assert!("unknown".parse::<Tab>().is_err());
    }

    #[test]
    fn toggles_expanded_blocks() {
        let mut state = AppState::new(issue_resource());
        let block = BlockId::Body;

        state.toggle_block(block.clone());
        assert!(state.block_expanded(&block));
        state.toggle_block(block.clone());
        assert!(!state.block_expanded(&block));
    }

    #[test]
    fn expands_and_collapses_block_sets() {
        let mut state = AppState::new(issue_resource());
        let blocks = vec![BlockId::Body, BlockId::Activity("comment-1".into())];

        state.expand_blocks(blocks.clone());

        assert!(state.block_expanded(&BlockId::Body));
        assert!(state.block_expanded(&BlockId::Activity("comment-1".into())));

        state.collapse_blocks(blocks);

        assert!(!state.block_expanded(&BlockId::Body));
        assert!(!state.block_expanded(&BlockId::Activity("comment-1".into())));
    }

    #[test]
    fn active_tab_expandable_blocks_follow_resource_and_tab() {
        let mut resource = pr_resource();
        resource.activity = vec![activity_entry("comment-1")];
        let mut state = AppState::new(resource);

        assert_eq!(
            state.active_tab_expandable_blocks(),
            vec![
                BlockId::Body,
                BlockId::Commit("abc123".into()),
                BlockId::Activity("comment-1".into())
            ]
        );

        state.set_tab(Tab::Files);

        assert_eq!(
            state.active_tab_expandable_blocks(),
            vec![
                BlockId::File("src/lib.rs".into()),
                BlockId::Patch("src/lib.rs".into())
            ]
        );
    }

    #[test]
    fn toggles_active_tab_expansion() {
        let mut resource = issue_resource();
        resource.activity = vec![activity_entry("comment-1")];
        let mut state = AppState::new(resource);

        assert!(state.toggle_active_tab_expansion());
        assert!(state.block_expanded(&BlockId::Body));
        assert!(state.block_expanded(&BlockId::Activity("comment-1".into())));

        assert!(state.toggle_active_tab_expansion());
        assert!(!state.block_expanded(&BlockId::Body));
        assert!(!state.block_expanded(&BlockId::Activity("comment-1".into())));
    }

    #[test]
    fn overlays_do_not_offer_active_tab_expansion() {
        let mut state = AppState::new(issue_resource());
        state.show_help = true;

        assert!(state.active_tab_expandable_blocks().is_empty());
        assert!(!state.toggle_active_tab_expansion());
        assert!(state.expanded_blocks.is_empty());
    }

    #[test]
    fn stores_last_refresh_status() {
        let mut state = AppState::new(issue_resource());

        state.mark_refreshed("12:34:56 UTC", true);

        assert_eq!(state.last_refreshed_at.as_deref(), Some("12:34:56 UTC"));
        assert_eq!(state.last_refresh_had_changes, Some(true));
    }

    #[test]
    fn replace_resource_reset_view_clears_navigation_state() {
        let mut state = AppState::new(pr_resource());
        state.set_tab(Tab::Files);
        state.scroll = 8;
        state.toggle_block(BlockId::Body);
        state.last_refreshed_at = Some("12:34:56 UTC".into());
        state.last_refresh_had_changes = Some(true);
        state.last_refresh_changed_sections = vec!["summary".into()];
        state.begin_loading(
            state.resource.id.clone(),
            "opening owner/repo#2 from GitHub",
        );

        state.replace_resource_reset_view(issue_resource());

        assert_eq!(state.active_tab, Tab::Overview);
        assert_eq!(state.scroll, 0);
        assert!(state.expanded_blocks.is_empty());
        assert!(state.last_refreshed_at.is_none());
        assert!(state.last_refresh_had_changes.is_none());
        assert!(state.last_refresh_changed_sections.is_empty());
        assert!(state.loading.is_none());
    }

    #[test]
    fn replace_resource_preserve_tab_keeps_valid_tab_and_falls_back_for_invalid_tab() {
        let mut state = AppState::new(pr_resource());
        state.set_tab(Tab::Activity);

        state.replace_resource_preserve_tab(issue_resource());

        assert_eq!(state.active_tab, Tab::Activity);

        state = AppState::new(pr_resource());
        state.set_tab(Tab::Files);

        state.replace_resource_preserve_tab(issue_resource());

        assert_eq!(state.active_tab, Tab::Overview);
    }

    #[test]
    fn loading_indicator_advances_through_ascii_frames() {
        let mut state = AppState::new(issue_resource());
        state.begin_loading(
            state.resource.id.clone(),
            "refreshing owner/repo#1 from GitHub",
        );

        assert_eq!(state.loading_indicator(), Some("|"));
        state.advance_loading_frame();
        assert_eq!(state.loading_indicator(), Some("/"));
        state.advance_loading_frame();
        assert_eq!(state.loading_indicator(), Some("-"));
        state.advance_loading_frame();
        assert_eq!(state.loading_indicator(), Some("\\"));
        state.advance_loading_frame();
        assert_eq!(state.loading_indicator(), Some("|"));
    }

    #[test]
    fn refreshed_resource_preserves_view_state_when_unchanged() {
        let mut state = AppState::new(issue_resource());
        state.set_tab(Tab::Activity);
        state.scroll = 7;
        state.toggle_block(BlockId::Activity("comment-1".into()));
        state.refresh_requested = true;
        state.last_error = Some("older error".into());

        let changed = state.apply_refreshed_resource(issue_resource(), "12:34:56 UTC");

        assert!(!changed);
        assert_eq!(state.active_tab, Tab::Activity);
        assert_eq!(state.scroll, 7);
        assert!(state.block_expanded(&BlockId::Activity("comment-1".into())));
        assert!(!state.refresh_requested);
        assert_eq!(state.last_error, None);
        assert_eq!(state.last_refreshed_at.as_deref(), Some("12:34:56 UTC"));
        assert_eq!(state.last_refresh_had_changes, Some(false));
        assert!(state.last_refresh_changed_sections.is_empty());
        assert_eq!(
            state.status_message.as_deref(),
            Some("refreshed from GitHub: no changes")
        );
    }

    #[test]
    fn refreshed_resource_records_changes_without_resetting_tab_or_scroll() {
        let mut state = AppState::new(issue_resource());
        state.set_tab(Tab::Activity);
        state.scroll = 4;
        let mut refreshed = issue_resource();
        refreshed.updated_at = "later".into();
        refreshed.body = "Changed body".into();
        refreshed.activity.push(crate::domain::ActivityEntry {
            id: "timeline-1".into(),
            kind: crate::domain::ActivityKind::Timeline,
            author: "alice".into(),
            body: "added label bug".into(),
            updated_at: "later".into(),
            path: None,
            line: None,
            url: None,
            author_association: None,
            reactions: ReactionCounts::default(),
            includes_created_edit: false,
            is_minimized: false,
            minimized_reason: None,
            thread_id: None,
            thread_resolved: None,
            thread_outdated: None,
        });

        let changed = state.apply_refreshed_resource(refreshed, "12:35:00 UTC");

        assert!(changed);
        assert_eq!(state.active_tab, Tab::Activity);
        assert_eq!(state.scroll, 4);
        assert_eq!(state.resource.body, "Changed body");
        assert_eq!(state.last_refresh_had_changes, Some(true));
        assert_eq!(
            state.last_refresh_changed_sections,
            vec!["summary".to_string(), "activity".to_string()]
        );
        assert_eq!(
            state.status_message.as_deref(),
            Some("refreshed from GitHub: changed summary, activity")
        );
    }

    #[test]
    fn help_toggle_resets_scroll() {
        let mut state = AppState::new(issue_resource());
        state.scroll = 12;
        state.scroll_limit = 20;

        state.toggle_help();

        assert!(state.show_help);
        assert_eq!(state.scroll, 0);
        assert_eq!(state.scroll_limit, u16::MAX);
    }

    #[test]
    fn scroll_down_clamps_to_rendered_scroll_limit() {
        let mut state = AppState::new(issue_resource());
        state.set_scroll_limit(7);

        for _ in 0..20 {
            state.scroll_down(3);
        }

        assert_eq!(state.scroll, 7);
    }

    #[test]
    fn scrolling_reveals_transient_scrollbar() {
        let mut state = AppState::new(issue_resource());
        state.set_scroll_limit(7);

        state.scroll_down(1);

        assert!(state.scrollbar_visible());
        state.advance_scrollbar_visibility();
        assert!(state.scrollbar_visible());
        for _ in 0..SCROLLBAR_VISIBLE_FRAMES {
            state.advance_scrollbar_visibility();
        }
        assert!(!state.scrollbar_visible());
    }

    #[test]
    fn scrollbar_visibility_mode_controls_visibility() {
        let mut state = AppState::new(issue_resource());
        state.set_scroll_limit(7);

        assert!(!state.scrollbar_visible());

        state.set_scrollbar(ScrollbarMode::Always);
        assert!(state.scrollbar_visible());

        state.set_scrollbar(ScrollbarMode::Hidden);
        state.scroll_down(1);
        assert!(!state.scrollbar_visible());
    }

    #[test]
    fn lowering_scroll_limit_clamps_existing_scroll() {
        let mut state = AppState::new(issue_resource());
        state.scroll = 40;

        state.set_scroll_limit(8);

        assert_eq!(state.scroll, 8);
    }

    #[test]
    fn tab_change_resets_unknown_scroll_limit_until_next_render() {
        let mut state = AppState::new(issue_resource());
        state.set_scroll_limit(7);
        state.scroll = 7;

        state.next_tab();

        assert_eq!(state.scroll, 0);
        assert_eq!(state.scroll_limit, u16::MAX);
    }
}

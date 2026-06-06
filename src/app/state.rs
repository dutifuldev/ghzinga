use std::{collections::HashSet, fmt, path::PathBuf, str::FromStr};

use crate::domain::{PullRequest, Resource, ResourceId, ResourceIdError, ResourceKind};
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

impl fmt::Display for Tab {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Overview => f.write_str("overview"),
            Self::Activity => f.write_str("activity"),
            Self::Commits => f.write_str("commits"),
            Self::Checks => f.write_str("checks"),
            Self::Files => f.write_str("files"),
            Self::Links => f.write_str("links"),
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
    pub request_id: u64,
    pub origin_tab_id: u64,
    frame: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollbarDragState {
    pub top: u16,
    pub height: u16,
}

#[derive(Debug, Clone)]
pub struct ResourceTabState {
    pub id: u64,
    pub resource: Resource,
    pub latest_fetch_request_id: u64,
    pub active_tab: Tab,
    pub scroll: u16,
    pub scroll_limit: u16,
    pub reverse_chronological: bool,
    pub expanded_blocks: HashSet<BlockId>,
    pub history: Vec<crate::domain::ResourceId>,
    pub last_refreshed_at: Option<String>,
    pub last_refresh_had_changes: Option<bool>,
    pub last_refresh_changed_sections: Vec<String>,
    pub last_error: Option<String>,
    pub status_message: Option<String>,
}

impl ResourceTabState {
    fn new(id: u64, resource: Resource) -> Self {
        Self {
            id,
            resource,
            latest_fetch_request_id: 0,
            active_tab: Tab::Overview,
            scroll: 0,
            scroll_limit: u16::MAX,
            reverse_chronological: false,
            expanded_blocks: HashSet::new(),
            history: Vec::new(),
            last_refreshed_at: None,
            last_refresh_had_changes: None,
            last_refresh_changed_sections: Vec::new(),
            last_error: None,
            status_message: None,
        }
    }

    pub(crate) fn from_session_parts(
        id: u64,
        resource: Resource,
        active_tab: Tab,
        scroll: u16,
        reverse_chronological: bool,
        expanded_blocks: HashSet<BlockId>,
    ) -> Self {
        Self {
            id,
            resource,
            latest_fetch_request_id: 0,
            active_tab,
            scroll,
            scroll_limit: u16::MAX,
            reverse_chronological,
            expanded_blocks,
            history: Vec::new(),
            last_refreshed_at: None,
            last_refresh_had_changes: None,
            last_refresh_changed_sections: Vec::new(),
            last_error: None,
            status_message: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AddResourcePrompt {
    pub input: String,
    pub error: Option<String>,
    pub fallback_repo: ResourceId,
    pub mode: AddResourceMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddResourceMode {
    NewTab,
    ReplaceCurrent,
}

#[derive(Debug, Clone)]
pub struct ResourceLinkPrompt {
    pub id: ResourceId,
    pub url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub resource: Resource,
    pub resource_tabs: Vec<ResourceTabState>,
    pub active_resource_tab: usize,
    pub resource_tab_scroll: usize,
    next_resource_tab_id: u64,
    next_loading_request_id: u64,
    latest_fetch_request_id: u64,
    pub add_resource_prompt: Option<AddResourcePrompt>,
    pub resource_link_prompt: Option<ResourceLinkPrompt>,
    pending_activity_focus: Option<String>,
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
    pub quit_confirmation: bool,
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
        let resource_tabs = vec![ResourceTabState::new(1, resource.clone())];
        Self {
            active_tab: Tab::Overview,
            resource,
            resource_tabs,
            active_resource_tab: 0,
            resource_tab_scroll: 0,
            next_resource_tab_id: 2,
            next_loading_request_id: 1,
            latest_fetch_request_id: 0,
            add_resource_prompt: None,
            resource_link_prompt: None,
            pending_activity_focus: None,
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
            quit_confirmation: false,
            reverse_chronological: false,
            theme: ThemeName::Default,
            symbols: SymbolMode::Ascii,
            spacing: SpacingMode::Comfortable,
            width_mode: ContentWidthMode::Fixed,
            fixed_width: DEFAULT_FIXED_CONTENT_WIDTH,
            config_path: crate::config::config_path(),
        }
    }

    pub(crate) fn from_session_tabs(
        mut resource_tabs: Vec<ResourceTabState>,
        active_index: usize,
    ) -> Self {
        if resource_tabs.is_empty() {
            panic!("restored session state requires at least one resource tab");
        }
        let active_index = active_index.min(resource_tabs.len() - 1);
        let resource = resource_tabs[active_index].resource.clone();
        let next_resource_tab_id = resource_tabs
            .iter()
            .map(|tab| tab.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let mut state = Self::new(resource);
        state.resource_tabs.clear();
        state.resource_tabs.append(&mut resource_tabs);
        state.next_resource_tab_id = next_resource_tab_id;
        state.restore_resource_tab(active_index);
        state
    }

    pub(crate) fn session_resource_tabs(&mut self) -> Vec<ResourceTabState> {
        self.snapshot_active_resource_tab();
        self.resource_tabs.clone()
    }

    pub fn tabs(&self) -> &'static [Tab] {
        Tab::all_for(self.resource.kind())
    }

    pub fn resource_tab_bar_visible(&self) -> bool {
        self.resource_tabs.len() > 1
    }

    pub fn open_add_resource_prompt(&mut self) {
        self.open_add_resource_prompt_with_mode(AddResourceMode::NewTab);
    }

    pub fn open_replace_resource_prompt(&mut self) {
        self.open_add_resource_prompt_with_mode(AddResourceMode::ReplaceCurrent);
    }

    fn open_add_resource_prompt_with_mode(&mut self, mode: AddResourceMode) {
        if self.show_help || self.show_settings {
            self.scroll = 0;
            self.scroll_limit = u16::MAX;
        }
        self.hit_areas.clear();
        self.scrollbar_drag = None;
        self.resource_link_prompt = None;
        self.quit_confirmation = false;
        self.add_resource_prompt = Some(AddResourcePrompt {
            input: String::new(),
            error: None,
            fallback_repo: self.resource.id.clone(),
            mode,
        });
        self.show_help = false;
        self.show_settings = false;
    }

    pub fn close_add_resource_prompt(&mut self) {
        self.add_resource_prompt = None;
    }

    pub fn clear_add_resource_input_or_close(&mut self) {
        let Some(prompt) = &mut self.add_resource_prompt else {
            return;
        };
        if prompt.input.is_empty() {
            self.add_resource_prompt = None;
        } else {
            prompt.input.clear();
            prompt.error = None;
        }
    }

    pub fn open_resource_link_prompt(&mut self, id: ResourceId, url: Option<String>) {
        self.hit_areas.clear();
        self.scrollbar_drag = None;
        self.add_resource_prompt = None;
        self.quit_confirmation = false;
        self.resource_link_prompt = Some(ResourceLinkPrompt { id, url });
        self.show_help = false;
        self.show_settings = false;
    }

    pub fn close_resource_link_prompt(&mut self) {
        self.resource_link_prompt = None;
    }

    pub fn request_quit_confirmation(&mut self) {
        self.hit_areas.clear();
        self.scrollbar_drag = None;
        self.add_resource_prompt = None;
        self.resource_link_prompt = None;
        self.show_help = false;
        self.show_settings = false;
        self.quit_confirmation = true;
    }

    pub fn close_quit_confirmation(&mut self) {
        self.quit_confirmation = false;
    }

    pub fn resource_link_prompt_target(&self) -> Option<ResourceId> {
        self.resource_link_prompt
            .as_ref()
            .map(|prompt| prompt.id.clone())
    }

    pub fn add_resource_input_mut(&mut self) -> Option<&mut String> {
        self.add_resource_prompt
            .as_mut()
            .map(|prompt| &mut prompt.input)
    }

    pub fn set_add_resource_error(&mut self, error: impl Into<String>) {
        if let Some(prompt) = &mut self.add_resource_prompt {
            prompt.error = Some(error.into());
        }
    }

    pub fn clear_add_resource_error(&mut self) {
        if let Some(prompt) = &mut self.add_resource_prompt {
            prompt.error = None;
        }
    }

    pub fn parse_add_resource_input(&self) -> Result<ResourceId, ResourceIdError> {
        let input = self
            .add_resource_prompt
            .as_ref()
            .map(|prompt| prompt.input.trim())
            .unwrap_or_default();
        let fallback_repo = self
            .add_resource_prompt
            .as_ref()
            .map(|prompt| &prompt.fallback_repo)
            .unwrap_or(&self.resource.id);
        parse_resource_reference(input, fallback_repo)
    }

    pub fn active_resource_tab_label(&self, index: usize) -> Option<String> {
        let tab = self.resource_tabs.get(index)?;
        Some(resource_tab_label(&tab.resource))
    }

    pub fn active_resource_tab_id(&self) -> u64 {
        self.resource_tabs
            .get(self.active_resource_tab)
            .map(|tab| tab.id)
            .unwrap_or_default()
    }

    pub fn switch_resource_tab(&mut self, index: usize) -> bool {
        if index >= self.resource_tabs.len() || index == self.active_resource_tab {
            return false;
        }
        self.snapshot_active_resource_tab();
        self.restore_resource_tab(index);
        true
    }

    pub fn scroll_resource_tabs_previous(&mut self) -> bool {
        let previous = self.resource_tab_scroll;
        self.resource_tab_scroll = self.resource_tab_scroll.saturating_sub(1);
        self.resource_tab_scroll != previous
    }

    pub fn scroll_resource_tabs_next(&mut self) -> bool {
        let previous = self.resource_tab_scroll;
        self.resource_tab_scroll = self
            .resource_tab_scroll
            .saturating_add(1)
            .min(self.resource_tabs.len().saturating_sub(1));
        self.resource_tab_scroll != previous
    }

    pub fn close_resource_tab(&mut self, index: usize) -> bool {
        if self.resource_tabs.len() <= 1 || index >= self.resource_tabs.len() {
            return false;
        }
        let closing_tab_id = self.resource_tabs[index].id;
        if self
            .loading
            .as_ref()
            .is_some_and(|loading| loading.origin_tab_id == closing_tab_id)
        {
            self.finish_loading();
            self.clear_transient_loading_status_messages();
        }
        self.snapshot_active_resource_tab();
        self.resource_tabs.remove(index);
        let next = if index == self.active_resource_tab {
            index.saturating_sub(1).min(self.resource_tabs.len() - 1)
        } else if index < self.active_resource_tab {
            self.active_resource_tab.saturating_sub(1)
        } else {
            self.active_resource_tab
        };
        self.resource_tab_scroll = self.resource_tab_scroll.min(self.resource_tabs.len() - 1);
        self.restore_resource_tab(next);
        true
    }

    pub fn open_resource_in_tab(&mut self, resource: Resource) {
        self.snapshot_active_resource_tab();
        let canonical = resource.id.canonical_name();
        if let Some(index) = self
            .resource_tabs
            .iter()
            .position(|tab| tab.resource.id.canonical_name() == canonical)
        {
            self.resource_tabs[index].resource = resource;
            self.restore_resource_tab(index);
        } else {
            let tab_id = self.allocate_resource_tab_id();
            self.resource_tabs
                .push(ResourceTabState::new(tab_id, resource));
            self.restore_resource_tab(self.resource_tabs.len() - 1);
        }
    }

    pub fn focus_resource_tab(&mut self, id: &crate::domain::ResourceId) -> bool {
        let canonical = id.canonical_name();
        let Some(index) = self
            .resource_tabs
            .iter()
            .position(|tab| tab.resource.id.canonical_name() == canonical)
        else {
            return false;
        };
        if index == self.active_resource_tab {
            return true;
        }
        self.switch_resource_tab(index);
        true
    }

    pub fn apply_to_resource_tab(&mut self, tab_id: u64, apply: impl FnOnce(&mut Self)) -> bool {
        let previous_tab_id = self.active_resource_tab_id();
        self.snapshot_active_resource_tab();
        let Some(target_index) = self.resource_tabs.iter().position(|tab| tab.id == tab_id) else {
            return false;
        };

        self.restore_resource_tab(target_index);
        apply(self);
        self.snapshot_active_resource_tab();

        if previous_tab_id != tab_id {
            if let Some(previous_index) = self
                .resource_tabs
                .iter()
                .position(|tab| tab.id == previous_tab_id)
            {
                self.restore_resource_tab(previous_index);
            }
        }
        true
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

    pub fn focus_activity_url(&mut self, url: &str) -> bool {
        let Some(fragment) = resource_url_fragment(url) else {
            return false;
        };
        let Ok(id) = ResourceId::parse(url) else {
            return false;
        };
        if id.canonical_name() != self.resource.id.canonical_name() {
            return false;
        }
        let Some(entry_id) = self.resource.activity.iter().find_map(|entry| {
            let entry_url = entry.url.as_deref()?;
            (resource_url_fragment(entry_url) == Some(fragment)).then(|| entry.id.clone())
        }) else {
            return false;
        };

        self.set_tab(Tab::Activity);
        self.expand_blocks([BlockId::Activity(entry_id.clone())]);
        self.pending_activity_focus = Some(entry_id);
        self.status_message = Some("focused linked activity".into());
        true
    }

    pub fn take_pending_activity_focus(&mut self) -> Option<String> {
        self.pending_activity_focus.take()
    }

    pub fn replace_resource_reset_view(&mut self, resource: Resource) {
        self.resource = resource;
        self.active_tab = Tab::Overview;
        self.scroll = 0;
        self.scroll_limit = u16::MAX;
        self.clear_resource_view_state();
        self.snapshot_active_resource_tab();
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
        self.snapshot_active_resource_tab();
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
        self.status_message = None;
        self.snapshot_active_resource_tab();
        changed
    }

    pub fn begin_loading(&mut self, target: ResourceId, message: impl Into<String>) -> u64 {
        let request_id = self.allocate_loading_request_id();
        let origin_tab_id = self.active_resource_tab_id();
        self.latest_fetch_request_id = request_id;
        self.loading = Some(LoadingState {
            target,
            message: message.into(),
            request_id,
            origin_tab_id,
            frame: 0,
        });
        self.last_error = None;
        request_id
    }

    pub fn latest_fetch_request_matches(&self, request_id: u64) -> bool {
        self.latest_fetch_request_id == request_id
    }

    pub fn finish_loading(&mut self) {
        self.loading = None;
        self.refresh_requested = false;
    }

    pub fn clear_transient_loading_status_messages(&mut self) {
        if let Some(prompt) = &mut self.add_resource_prompt {
            if prompt
                .error
                .as_deref()
                .is_some_and(is_transient_loading_status)
            {
                prompt.error = None;
            }
        }
        if self
            .status_message
            .as_deref()
            .is_some_and(is_transient_loading_status)
        {
            self.status_message = None;
        }
        for tab in &mut self.resource_tabs {
            if tab
                .status_message
                .as_deref()
                .is_some_and(is_transient_loading_status)
            {
                tab.status_message = None;
            }
        }
    }

    pub fn loading_request_matches(&self, request_id: u64) -> bool {
        self.loading
            .as_ref()
            .is_some_and(|loading| loading.request_id == request_id)
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

    pub fn reveal_scrollbar_for_focus(&mut self) {
        self.reveal_scrollbar();
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

    fn snapshot_active_resource_tab(&mut self) {
        if let Some(tab) = self.resource_tabs.get_mut(self.active_resource_tab) {
            tab.resource = self.resource.clone();
            tab.latest_fetch_request_id = self.latest_fetch_request_id;
            tab.active_tab = self.active_tab;
            tab.scroll = self.scroll;
            tab.scroll_limit = self.scroll_limit;
            tab.reverse_chronological = self.reverse_chronological;
            tab.expanded_blocks = self.expanded_blocks.clone();
            tab.history = self.history.clone();
            tab.last_refreshed_at = self.last_refreshed_at.clone();
            tab.last_refresh_had_changes = self.last_refresh_had_changes;
            tab.last_refresh_changed_sections = self.last_refresh_changed_sections.clone();
            tab.last_error = self.last_error.clone();
            tab.status_message = self.status_message.clone();
        }
    }

    fn restore_resource_tab(&mut self, index: usize) {
        let Some(tab) = self.resource_tabs.get(index).cloned() else {
            return;
        };
        self.active_resource_tab = index;
        self.resource_tab_scroll = index;
        self.resource = tab.resource;
        self.latest_fetch_request_id = tab.latest_fetch_request_id;
        self.active_tab = if self.tabs().contains(&tab.active_tab) {
            tab.active_tab
        } else {
            Tab::Overview
        };
        self.scroll = tab.scroll;
        self.scroll_limit = tab.scroll_limit;
        self.reverse_chronological = tab.reverse_chronological;
        self.expanded_blocks = tab.expanded_blocks;
        self.history = tab.history;
        self.last_refreshed_at = tab.last_refreshed_at;
        self.last_refresh_had_changes = tab.last_refresh_had_changes;
        self.last_refresh_changed_sections = tab.last_refresh_changed_sections;
        self.last_error = tab.last_error;
        self.status_message = tab.status_message;
        self.hit_areas.clear();
        self.scrollbar_drag = None;
        self.pending_activity_focus = None;
    }

    fn allocate_resource_tab_id(&mut self) -> u64 {
        let id = self.next_resource_tab_id;
        self.next_resource_tab_id = self.next_resource_tab_id.saturating_add(1);
        id
    }

    fn allocate_loading_request_id(&mut self) -> u64 {
        let id = self.next_loading_request_id;
        self.next_loading_request_id = self.next_loading_request_id.saturating_add(1);
        id
    }
}

pub fn parse_resource_reference(
    input: &str,
    fallback_repo: &ResourceId,
) -> Result<ResourceId, ResourceIdError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ResourceIdError::Invalid);
    }
    if let Ok(id) = ResourceId::parse(trimmed) {
        return Ok(id);
    }
    let mut parts = trimmed.split_whitespace();
    if let (Some(owner_repo), Some(number), None) = (parts.next(), parts.next(), parts.next()) {
        return ResourceId::from_owner_repo_number(owner_repo, number);
    }
    let number = trimmed.strip_prefix('#').unwrap_or(trimmed);
    if number.chars().all(|ch| ch.is_ascii_digit()) {
        return ResourceId::relative_to_repo(&fallback_repo.owner, &fallback_repo.repo, number);
    }
    Err(ResourceIdError::Invalid)
}

fn resource_tab_label(resource: &Resource) -> String {
    let kind = match resource.kind() {
        ResourceKind::PullRequest => "PR",
        ResourceKind::Issue => "Issue",
    };
    format!("{kind} #{} {}", resource.id.number, resource.title)
}

fn is_transient_loading_status(message: &str) -> bool {
    message.starts_with("still loading: ")
}

fn resource_url_fragment(url: &str) -> Option<&str> {
    let (_, fragment) = url.split_once('#')?;
    (!fragment.is_empty()).then_some(fragment)
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
        assert!(state.status_message.is_none());
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
        assert!(state.status_message.is_none());
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
    fn add_resource_prompt_resets_overlay_scroll_without_touching_resource_scroll() {
        let mut state = AppState::new(issue_resource());
        state.scroll = 12;
        state.scroll_limit = 20;
        state.show_help = true;

        state.open_add_resource_prompt();

        assert!(!state.show_help);
        assert_eq!(state.scroll, 0);
        assert_eq!(state.scroll_limit, u16::MAX);

        state.close_add_resource_prompt();
        state.scroll = 7;
        state.scroll_limit = 20;

        state.open_add_resource_prompt();

        assert_eq!(state.scroll, 7);
        assert_eq!(state.scroll_limit, 20);
    }

    #[test]
    fn transient_loading_cleanup_clears_prompt_errors() {
        let mut state = AppState::new(issue_resource());
        state.open_add_resource_prompt();
        state.set_add_resource_error("still loading: refreshing owner/repo#1 from GitHub");

        state.clear_transient_loading_status_messages();

        assert!(state.add_resource_prompt.as_ref().unwrap().error.is_none());

        state.set_add_resource_error("expected a GitHub PR/issue URL");

        state.clear_transient_loading_status_messages();

        assert_eq!(
            state.add_resource_prompt.as_ref().unwrap().error.as_deref(),
            Some("expected a GitHub PR/issue URL")
        );
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

    #[test]
    fn add_resource_parser_accepts_links_owner_repo_and_relative_numbers() {
        let fallback = ResourceId::from_owner_repo_number("owner/repo", "1").unwrap();

        assert_eq!(
            parse_resource_reference("https://github.com/other/project/pull/42", &fallback)
                .unwrap()
                .canonical_name(),
            "other/project#42"
        );
        assert_eq!(
            parse_resource_reference("other/project#43", &fallback)
                .unwrap()
                .canonical_name(),
            "other/project#43"
        );
        assert_eq!(
            parse_resource_reference("other/project 44", &fallback)
                .unwrap()
                .canonical_name(),
            "other/project#44"
        );
        assert_eq!(
            parse_resource_reference("#45", &fallback)
                .unwrap()
                .canonical_name(),
            "owner/repo#45"
        );
        assert_eq!(
            parse_resource_reference("46", &fallback)
                .unwrap()
                .canonical_name(),
            "owner/repo#46"
        );
    }

    #[test]
    fn add_resource_prompt_parses_relative_numbers_against_opened_repo() {
        let mut state = AppState::new(issue_resource());
        state.open_add_resource_prompt();
        state.add_resource_input_mut().unwrap().push_str("#77");

        let mut other = issue_resource();
        other.id.owner = "other".into();
        other.id.repo = "project".into();
        other.id.number = 2;
        other.title = "Other issue".into();
        state.open_resource_in_tab(other);

        let parsed = state.parse_add_resource_input().unwrap();

        assert_eq!(parsed.canonical_name(), "owner/repo#77");
    }

    #[test]
    fn resource_tabs_preserve_view_state_when_switching() {
        let mut state = AppState::new(issue_resource());
        let mut second = issue_resource();
        second.id.number = 2;
        second.title = "Second issue".into();

        state.set_tab(Tab::Activity);
        state.scroll = 5;
        state.toggle_block(BlockId::Body);
        state.history.push(state.resource.id.clone());
        state.mark_refreshed("12:34:56 UTC", true);
        state.last_refresh_changed_sections = vec!["summary".into()];
        state.last_error = Some("first tab error".into());
        state.status_message = Some("first tab status".into());
        state.open_resource_in_tab(second);

        assert_eq!(state.resource.id.number, 2);
        assert_eq!(state.active_tab, Tab::Overview);
        assert!(state.history.is_empty());
        assert!(state.last_refreshed_at.is_none());
        assert_eq!(state.resource_tabs.len(), 2);
        assert!(state.resource_tab_bar_visible());

        state.switch_resource_tab(0);

        assert_eq!(state.resource.id.number, 1);
        assert_eq!(state.active_tab, Tab::Activity);
        assert_eq!(state.scroll, 5);
        assert!(state.block_expanded(&BlockId::Body));
        assert_eq!(state.history.len(), 1);
        assert_eq!(state.last_refreshed_at.as_deref(), Some("12:34:56 UTC"));
        assert_eq!(state.last_refresh_had_changes, Some(true));
        assert_eq!(state.last_refresh_changed_sections, ["summary"]);
        assert_eq!(state.last_error.as_deref(), Some("first tab error"));
        assert_eq!(state.status_message.as_deref(), Some("first tab status"));
    }

    #[test]
    fn apply_to_inactive_resource_tab_preserves_error_and_status() {
        let mut state = AppState::new(issue_resource());
        let mut second = issue_resource();
        second.id.number = 2;
        second.title = "Second issue".into();
        state.open_resource_in_tab(second);
        let first_tab_id = state.resource_tabs[0].id;
        state.status_message = Some("active status".into());

        assert!(state.apply_to_resource_tab(first_tab_id, |state| {
            state.last_error = Some("inactive error".into());
            state.status_message = Some("inactive status".into());
        }));

        assert_eq!(state.resource.id.number, 2);
        assert_eq!(state.last_error, None);
        assert_eq!(state.status_message.as_deref(), Some("active status"));

        state.switch_resource_tab(0);

        assert_eq!(state.resource.id.number, 1);
        assert_eq!(state.last_error.as_deref(), Some("inactive error"));
        assert_eq!(state.status_message.as_deref(), Some("inactive status"));
    }

    #[test]
    fn opening_resource_tab_does_not_close_later_prompt_input() {
        let mut state = AppState::new(issue_resource());
        let mut fetched = issue_resource();
        fetched.id.number = 2;
        fetched.title = "Fetched issue".into();
        state.open_add_resource_prompt();
        state
            .add_resource_input_mut()
            .unwrap()
            .push_str("owner/repo#3");

        state.open_resource_in_tab(fetched);

        assert_eq!(state.resource.id.number, 2);
        let prompt = state.add_resource_prompt.as_ref().unwrap();
        assert_eq!(prompt.input, "owner/repo#3");
    }

    #[test]
    fn focusing_resource_tab_preserves_cached_resource() {
        let mut state = AppState::new(issue_resource());
        let mut cached = issue_resource();
        cached.id.number = 2;
        cached.title = "Cached issue".into();
        let cached_id = cached.id.clone();
        state.open_resource_in_tab(cached);
        state.switch_resource_tab(0);

        assert!(state.focus_resource_tab(&cached_id));

        assert_eq!(state.resource.id.number, 2);
        assert_eq!(state.resource.title, "Cached issue");
        assert_eq!(state.resource_tabs.len(), 2);
    }

    #[test]
    fn closing_active_resource_tab_focuses_neighbor_and_keeps_last_tab() {
        let mut state = AppState::new(issue_resource());
        let mut second = issue_resource();
        second.id.number = 2;
        second.title = "Second issue".into();
        state.open_resource_in_tab(second);

        assert!(state.close_resource_tab(1));
        assert_eq!(state.resource.id.number, 1);
        assert_eq!(state.resource_tabs.len(), 1);
        assert!(!state.resource_tab_bar_visible());

        assert!(!state.close_resource_tab(0));
        assert_eq!(state.resource_tabs.len(), 1);
    }

    #[test]
    fn closing_loading_origin_tab_clears_abandoned_request() {
        let mut state = AppState::new(issue_resource());
        let mut second = issue_resource();
        second.id.number = 2;
        second.title = "Second issue".into();
        state.open_resource_in_tab(second);
        state.begin_loading(
            state.resource.id.clone(),
            "refreshing owner/repo#2 from GitHub",
        );
        state.switch_resource_tab(0);
        state.status_message = Some("still loading: refreshing owner/repo#2 from GitHub".into());

        assert!(state.close_resource_tab(1));

        assert!(state.loading.is_none());
        assert!(state.status_message.is_none());
        assert_eq!(state.resource.id.number, 1);
        assert_eq!(state.resource_tabs.len(), 1);
    }
}

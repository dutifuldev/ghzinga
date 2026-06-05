use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::app::{AppState, BlockId};
use crate::input::{hit_test, HitTarget};
use crate::render::{ContentWidthMode, ScrollbarMode, SpacingMode, SymbolMode, ThemeName};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppIntent {
    None,
    Refresh,
    LoadFullDepth,
    OpenResource(crate::domain::ResourceId),
    Navigate(crate::domain::ResourceId),
    OpenUrl(String),
    CopyUrl(String),
    Back,
    SaveSettings,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Activate(HitTarget),
    Tick,
}

pub fn apply_event(state: &mut AppState, event: AppEvent) -> AppIntent {
    match event {
        AppEvent::Key(key) => apply_key(state, key),
        AppEvent::Mouse(mouse) => apply_mouse(state, mouse),
        AppEvent::Activate(target) => apply_target(state, target),
        AppEvent::Tick => AppIntent::None,
    }
}

fn apply_key(state: &mut AppState, key: KeyEvent) -> AppIntent {
    if state.add_resource_prompt.is_some() {
        return apply_add_resource_prompt_key(state, key);
    }
    if state.resource_link_prompt.is_some() {
        return apply_resource_link_prompt_key(state, key);
    }
    match key.code {
        KeyCode::Char('q') if is_plain_shortcut(key) => {
            state.should_quit = true;
            AppIntent::Quit
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.should_quit = true;
            AppIntent::Quit
        }
        KeyCode::Char('r') if is_plain_shortcut(key) => {
            state.refresh_requested = true;
            AppIntent::Refresh
        }
        KeyCode::Char('n') if is_plain_shortcut(key) => {
            state.open_add_resource_prompt();
            AppIntent::None
        }
        KeyCode::Char('f')
            if !state.show_help
                && !state.show_settings
                && state.resource.has_partial_depth_warning()
                && is_plain_shortcut(key) =>
        {
            AppIntent::LoadFullDepth
        }
        KeyCode::Char('o') if is_plain_shortcut(key) => {
            AppIntent::OpenUrl(visible_or_current_url(state))
        }
        KeyCode::Char('y') if !state.show_settings && is_plain_shortcut(key) => {
            AppIntent::CopyUrl(visible_or_current_url(state))
        }
        KeyCode::Char('?') if is_plain_shortcut(key) => {
            state.toggle_help();
            AppIntent::None
        }
        KeyCode::Char('s') if is_plain_shortcut(key) => {
            state.toggle_settings();
            AppIntent::None
        }
        KeyCode::Char('v') if is_plain_shortcut(key) => {
            state.toggle_feed_order();
            AppIntent::None
        }
        KeyCode::Char('a')
            if !state.show_help && !state.show_settings && is_plain_shortcut(key) =>
        {
            state.toggle_active_tab_expansion();
            AppIntent::None
        }
        KeyCode::Esc if state.show_settings => {
            state.close_settings();
            AppIntent::None
        }
        KeyCode::Char('t') if state.show_settings && is_plain_shortcut(key) => {
            if state.cycle_theme() {
                AppIntent::SaveSettings
            } else {
                AppIntent::None
            }
        }
        KeyCode::Char('y') if state.show_settings && is_plain_shortcut(key) => {
            if state.cycle_symbols() {
                AppIntent::SaveSettings
            } else {
                AppIntent::None
            }
        }
        KeyCode::Char('p') if state.show_settings && is_plain_shortcut(key) => {
            if state.cycle_spacing() {
                AppIntent::SaveSettings
            } else {
                AppIntent::None
            }
        }
        KeyCode::Char('w') if state.show_settings && is_plain_shortcut(key) => {
            if state.cycle_width_mode() {
                AppIntent::SaveSettings
            } else {
                AppIntent::None
            }
        }
        KeyCode::Char('b') if state.show_settings && is_plain_shortcut(key) => {
            if state.cycle_scrollbar() {
                AppIntent::SaveSettings
            } else {
                AppIntent::None
            }
        }
        KeyCode::Char('+') | KeyCode::Char('=')
            if state.show_settings && is_plain_shortcut(key) =>
        {
            if state.increase_fixed_width() {
                AppIntent::SaveSettings
            } else {
                AppIntent::None
            }
        }
        KeyCode::Char('-') if state.show_settings && is_plain_shortcut(key) => {
            if state.decrease_fixed_width() {
                AppIntent::SaveSettings
            } else {
                AppIntent::None
            }
        }
        KeyCode::Char(ch @ '1'..='6') if !state.show_settings && is_plain_shortcut(key) => {
            if let Some(tab) = numbered_tab(ch, state.tabs()) {
                state.set_tab(tab);
            }
            AppIntent::None
        }
        KeyCode::Tab | KeyCode::Char('\t') | KeyCode::Right => {
            state.next_tab();
            AppIntent::None
        }
        KeyCode::Char('i') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.next_tab();
            AppIntent::None
        }
        KeyCode::Backspace => AppIntent::Back,
        KeyCode::Enter => {
            let Some(target) = state
                .hit_areas
                .iter()
                .map(|area| area.target.clone())
                .find(HitTarget::is_content_action)
            else {
                return AppIntent::None;
            };
            apply_target(state, target)
        }
        KeyCode::BackTab | KeyCode::Left => {
            state.previous_tab();
            AppIntent::None
        }
        KeyCode::Down => {
            state.scroll_down(1);
            AppIntent::None
        }
        KeyCode::Up => {
            state.scroll_up(1);
            AppIntent::None
        }
        KeyCode::PageDown => {
            state.scroll_down(10);
            AppIntent::None
        }
        KeyCode::PageUp => {
            state.scroll_up(10);
            AppIntent::None
        }
        KeyCode::Home => {
            state.scroll_to_top();
            AppIntent::None
        }
        KeyCode::End => {
            state.scroll_to_bottom();
            AppIntent::None
        }
        KeyCode::Char('e') if is_plain_shortcut(key) => {
            state.toggle_block(BlockId::Body);
            AppIntent::None
        }
        _ => AppIntent::None,
    }
}

fn apply_add_resource_prompt_key(state: &mut AppState, key: KeyEvent) -> AppIntent {
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.clear_add_resource_input_or_close();
            AppIntent::None
        }
        KeyCode::Esc => {
            state.close_add_resource_prompt();
            AppIntent::None
        }
        KeyCode::Enter => confirm_add_resource_prompt(state),
        KeyCode::Backspace => {
            if let Some(input) = state.add_resource_input_mut() {
                input.pop();
            }
            state.clear_add_resource_error();
            AppIntent::None
        }
        KeyCode::Char(ch) if is_plain_shortcut(key) => {
            if let Some(input) = state.add_resource_input_mut() {
                input.push(ch);
            }
            state.clear_add_resource_error();
            AppIntent::None
        }
        _ => AppIntent::None,
    }
}

fn apply_resource_link_prompt_key(state: &mut AppState, key: KeyEvent) -> AppIntent {
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.close_resource_link_prompt();
            AppIntent::None
        }
        KeyCode::Esc => {
            state.close_resource_link_prompt();
            AppIntent::None
        }
        KeyCode::Enter => confirm_resource_link_here(state),
        KeyCode::Char('h') if is_plain_shortcut(key) => confirm_resource_link_here(state),
        KeyCode::Char('n') | KeyCode::Char('t') if is_plain_shortcut(key) => {
            confirm_resource_link_new_tab(state)
        }
        _ => AppIntent::None,
    }
}

fn confirm_add_resource_prompt(state: &mut AppState) -> AppIntent {
    match state.parse_add_resource_input() {
        Ok(id) => AppIntent::OpenResource(id),
        Err(error) => {
            state.set_add_resource_error(error.to_string());
            AppIntent::None
        }
    }
}

fn confirm_resource_link_here(state: &mut AppState) -> AppIntent {
    let Some(id) = state.resource_link_prompt_target() else {
        return AppIntent::None;
    };
    state.close_resource_link_prompt();
    AppIntent::Navigate(id)
}

fn confirm_resource_link_new_tab(state: &mut AppState) -> AppIntent {
    let Some(id) = state.resource_link_prompt_target() else {
        return AppIntent::None;
    };
    state.close_resource_link_prompt();
    AppIntent::OpenResource(id)
}

fn is_plain_shortcut(key: KeyEvent) -> bool {
    !key.modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
}

fn numbered_tab(ch: char, tabs: &[crate::app::Tab]) -> Option<crate::app::Tab> {
    let index = ch.to_digit(10)?.checked_sub(1)? as usize;
    tabs.get(index).copied()
}

fn apply_mouse(state: &mut AppState, mouse: MouseEvent) -> AppIntent {
    match mouse.kind {
        MouseEventKind::ScrollDown => {
            if state.add_resource_prompt.is_some() || state.resource_link_prompt.is_some() {
                return AppIntent::None;
            }
            state.scroll_down(3);
            AppIntent::None
        }
        MouseEventKind::ScrollUp => {
            if state.add_resource_prompt.is_some() || state.resource_link_prompt.is_some() {
                return AppIntent::None;
            }
            state.scroll_up(3);
            AppIntent::None
        }
        MouseEventKind::Down(MouseButton::Left) => {
            let Some(target) = hit_test(&state.hit_areas, mouse.column, mouse.row) else {
                return AppIntent::None;
            };
            if let HitTarget::Scrollbar { top, height } = target {
                state.begin_scrollbar_drag(top, height, mouse.row);
                return AppIntent::None;
            }
            apply_target(state, target)
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            state.drag_scrollbar(mouse.row);
            AppIntent::None
        }
        MouseEventKind::Up(MouseButton::Left) => {
            state.end_scrollbar_drag();
            AppIntent::None
        }
        _ => AppIntent::None,
    }
}

fn apply_target(state: &mut AppState, target: HitTarget) -> AppIntent {
    match target {
        HitTarget::Tab(tab) => {
            state.set_tab(tab);
            AppIntent::None
        }
        HitTarget::ResourceTab(index) => {
            state.switch_resource_tab(index);
            AppIntent::None
        }
        HitTarget::CloseResourceTab(index) => {
            state.close_resource_tab(index);
            AppIntent::None
        }
        HitTarget::PreviousResourceTab => {
            state.scroll_resource_tabs_previous();
            AppIntent::None
        }
        HitTarget::NextResourceTab => {
            state.scroll_resource_tabs_next();
            AppIntent::None
        }
        HitTarget::ToggleBlock(id) => {
            state.toggle_block(id);
            AppIntent::None
        }
        HitTarget::ExpandBlocks(blocks) => {
            state.expand_blocks(blocks);
            AppIntent::None
        }
        HitTarget::CollapseBlocks(blocks) => {
            state.collapse_blocks(blocks);
            AppIntent::None
        }
        HitTarget::ResourceLink { id, url } => apply_resource_link(state, id, url),
        HitTarget::Navigate(id) => AppIntent::Navigate(id),
        HitTarget::OpenHeaderUrl(url) => AppIntent::OpenUrl(url),
        HitTarget::OpenUrl(url) => AppIntent::OpenUrl(url),
        HitTarget::CopyVisibleUrl => AppIntent::CopyUrl(visible_or_current_url(state)),
        HitTarget::OpenVisibleUrl => AppIntent::OpenUrl(visible_or_current_url(state)),
        HitTarget::Refresh => {
            state.refresh_requested = true;
            AppIntent::Refresh
        }
        HitTarget::OpenResourcePrompt => {
            state.open_add_resource_prompt();
            AppIntent::None
        }
        HitTarget::ConfirmResourcePrompt => confirm_add_resource_prompt(state),
        HitTarget::CancelResourcePrompt => {
            state.close_add_resource_prompt();
            AppIntent::None
        }
        HitTarget::OpenLinkHere => confirm_resource_link_here(state),
        HitTarget::OpenLinkInNewTab => confirm_resource_link_new_tab(state),
        HitTarget::CancelResourceLinkPrompt => {
            state.close_resource_link_prompt();
            AppIntent::None
        }
        HitTarget::ModalOverlay => AppIntent::None,
        HitTarget::LoadFullDepth => AppIntent::LoadFullDepth,
        HitTarget::Quit => {
            state.should_quit = true;
            AppIntent::Quit
        }
        HitTarget::Help => {
            state.toggle_help();
            AppIntent::None
        }
        HitTarget::Settings => {
            state.toggle_settings();
            AppIntent::None
        }
        HitTarget::CloseSettings => {
            state.close_settings();
            AppIntent::None
        }
        HitTarget::SetTheme(theme) => match theme.parse::<ThemeName>() {
            Ok(theme) if state.set_theme(theme) => AppIntent::SaveSettings,
            _ => AppIntent::None,
        },
        HitTarget::SetSymbols(symbols) => match symbols.parse::<SymbolMode>() {
            Ok(symbols) if state.set_symbols(symbols) => AppIntent::SaveSettings,
            _ => AppIntent::None,
        },
        HitTarget::SetSpacing(spacing) => match spacing.parse::<SpacingMode>() {
            Ok(spacing) if state.set_spacing(spacing) => AppIntent::SaveSettings,
            _ => AppIntent::None,
        },
        HitTarget::SetWidthMode(width_mode) => match width_mode.parse::<ContentWidthMode>() {
            Ok(width_mode) if state.set_width_mode(width_mode) => AppIntent::SaveSettings,
            _ => AppIntent::None,
        },
        HitTarget::SetFixedWidth(width) => {
            if state.set_fixed_width(width) {
                AppIntent::SaveSettings
            } else {
                AppIntent::None
            }
        }
        HitTarget::SetScrollbar(scrollbar) => match scrollbar.parse::<ScrollbarMode>() {
            Ok(scrollbar) if state.set_scrollbar(scrollbar) => AppIntent::SaveSettings,
            _ => AppIntent::None,
        },
        HitTarget::Scrollbar { .. } => AppIntent::None,
    }
}

fn apply_resource_link(
    state: &mut AppState,
    id: crate::domain::ResourceId,
    url: Option<String>,
) -> AppIntent {
    if id.canonical_name() == state.resource.id.canonical_name() {
        if url
            .as_deref()
            .is_some_and(|url| state.focus_activity_url(url))
        {
            return AppIntent::None;
        }
        state.status_message = Some(format!("already viewing {}", id.canonical_name()));
        return AppIntent::None;
    }
    state.open_resource_link_prompt(id, url);
    AppIntent::None
}

fn current_resource_url(state: &AppState) -> String {
    state.resource.web_url()
}

fn visible_or_current_url(state: &AppState) -> String {
    state
        .hit_areas
        .iter()
        .find_map(|area| match &area.target {
            HitTarget::OpenUrl(url) => Some(url.clone()),
            HitTarget::ResourceLink { id, url } => {
                Some(url.clone().unwrap_or_else(|| id.web_url()))
            }
            HitTarget::Navigate(id) => Some(id.web_url()),
            _ => None,
        })
        .unwrap_or_else(|| current_resource_url(state))
}

#[cfg(test)]
mod tests {
    use crossterm::event::MouseEvent;

    use super::*;
    use crate::app::Tab;
    use crate::domain::{
        ActivityEntry, ActivityKind, PullRequest, ReactionCounts, Resource, ResourceId,
        ResourceKind, FULL_DEPTH_WARNING_HINT,
    };
    use crate::input::HitArea;
    use ratatui::layout::Rect;

    fn resource() -> Resource {
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

    fn pr_resource() -> Resource {
        let mut resource = resource();
        resource.id.kind_hint = Some(ResourceKind::PullRequest);
        resource.url = "https://github.com/owner/repo/pull/1".into();
        resource.pull_request = Some(PullRequest {
            base_ref: "main".into(),
            head_ref: "topic".into(),
            requested_reviewers: vec![],
            review_decision: None,
            merge_state: None,
            additions: 0,
            deletions: 0,
            commits: vec![],
            checks: vec![],
            files: vec![],
            metadata: vec![],
        });
        resource
    }

    fn activity_entry(id: &str, url: &str) -> ActivityEntry {
        ActivityEntry {
            id: id.into(),
            kind: ActivityKind::Comment,
            author: "alice".into(),
            body: "comment".into(),
            updated_at: "now".into(),
            path: None,
            line: None,
            url: Some(url.into()),
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

    #[test]
    fn keyboard_tab_changes_active_tab() {
        let mut state = AppState::new(resource());

        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty())),
        );

        assert_eq!(state.active_tab, Tab::Activity);
    }

    #[test]
    fn literal_tab_character_changes_active_tab_for_tmux() {
        let mut state = AppState::new(resource());

        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('\t'), KeyModifiers::empty())),
        );

        assert_eq!(state.active_tab, Tab::Activity);
    }

    #[test]
    fn ctrl_i_changes_active_tab_for_tmux_tab_encoding() {
        let mut state = AppState::new(resource());

        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::CONTROL)),
        );

        assert_eq!(state.active_tab, Tab::Activity);
    }

    #[test]
    fn number_keys_jump_to_visible_pr_tabs() {
        for (shortcut, expected) in [
            ('1', Tab::Overview),
            ('2', Tab::Activity),
            ('3', Tab::Commits),
            ('4', Tab::Checks),
            ('5', Tab::Files),
            ('6', Tab::Links),
        ] {
            let mut state = AppState::new(pr_resource());
            state.scroll = 5;

            let intent = apply_event(
                &mut state,
                AppEvent::Key(KeyEvent::new(
                    KeyCode::Char(shortcut),
                    KeyModifiers::empty(),
                )),
            );

            assert_eq!(intent, AppIntent::None, "{shortcut}");
            assert_eq!(state.active_tab, expected, "{shortcut}");
            assert_eq!(state.scroll, 0, "{shortcut}");
        }
    }

    #[test]
    fn number_keys_jump_to_visible_issue_tabs() {
        let mut state = AppState::new(resource());
        state.scroll = 5;

        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::empty())),
        );

        assert_eq!(state.active_tab, Tab::Links);
        assert_eq!(state.scroll, 0);
    }

    #[test]
    fn unsupported_number_key_keeps_current_tab() {
        let mut state = AppState::new(resource());
        state.set_tab(Tab::Activity);
        state.scroll = 5;

        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('6'), KeyModifiers::empty())),
        );

        assert_eq!(state.active_tab, Tab::Activity);
        assert_eq!(state.scroll, 5);
    }

    #[test]
    fn number_keys_are_inert_when_modified_or_settings_are_open() {
        let mut state = AppState::new(pr_resource());

        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('4'), KeyModifiers::CONTROL)),
        );
        assert_eq!(state.active_tab, Tab::Overview);

        state.show_settings = true;
        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('4'), KeyModifiers::empty())),
        );
        assert_eq!(state.active_tab, Tab::Overview);
    }

    #[test]
    fn control_letter_shortcuts_are_limited_to_tmux_safe_exceptions() {
        for shortcut in [
            'a', 'b', 'd', 'e', 'f', 'o', 'q', 'r', 's', 'u', 'v', 'y', '?',
        ] {
            let mut state = AppState::new(resource());
            state.scroll = 4;
            state.set_scroll_limit(9);

            let intent = apply_event(
                &mut state,
                AppEvent::Key(KeyEvent::new(
                    KeyCode::Char(shortcut),
                    KeyModifiers::CONTROL,
                )),
            );

            assert_eq!(intent, AppIntent::None, "Ctrl-{shortcut} should be inert");
            assert_eq!(state.active_tab, Tab::Overview, "Ctrl-{shortcut}");
            assert_eq!(state.scroll, 4, "Ctrl-{shortcut}");
            assert!(!state.refresh_requested, "Ctrl-{shortcut}");
            assert!(!state.should_quit, "Ctrl-{shortcut}");
            assert!(!state.show_help, "Ctrl-{shortcut}");
            assert!(!state.show_settings, "Ctrl-{shortcut}");
            assert!(!state.reverse_chronological, "Ctrl-{shortcut}");
            assert!(state.expanded_blocks.is_empty(), "Ctrl-{shortcut}");
        }
    }

    #[test]
    fn settings_control_shortcuts_do_not_change_preferences() {
        for shortcut in ['t', 'y', 'p', 'w', 'b', '+', '-'] {
            let mut state = AppState::new(resource());
            state.show_settings = true;
            let theme = state.theme;
            let symbols = state.symbols;
            let spacing = state.spacing;
            let width_mode = state.width_mode;
            let fixed_width = state.fixed_width;
            let scrollbar = state.scrollbar;

            let intent = apply_event(
                &mut state,
                AppEvent::Key(KeyEvent::new(
                    KeyCode::Char(shortcut),
                    KeyModifiers::CONTROL,
                )),
            );

            assert_eq!(intent, AppIntent::None, "Ctrl-{shortcut} should be inert");
            assert_eq!(state.theme, theme, "Ctrl-{shortcut}");
            assert_eq!(state.symbols, symbols, "Ctrl-{shortcut}");
            assert_eq!(state.spacing, spacing, "Ctrl-{shortcut}");
            assert_eq!(state.width_mode, width_mode, "Ctrl-{shortcut}");
            assert_eq!(state.fixed_width, fixed_width, "Ctrl-{shortcut}");
            assert_eq!(state.scrollbar, scrollbar, "Ctrl-{shortcut}");
            assert!(state.show_settings, "Ctrl-{shortcut}");
        }
    }

    #[test]
    fn ctrl_c_quits() {
        let mut state = AppState::new(resource());

        let intent = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        );

        assert_eq!(intent, AppIntent::Quit);
        assert!(state.should_quit);
    }

    #[test]
    fn keyboard_v_toggles_feed_order_and_resets_scroll() {
        let mut state = AppState::new(resource());
        state.scroll = 7;
        state.set_scroll_limit(10);

        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::empty())),
        );

        assert!(state.reverse_chronological);
        assert_eq!(state.scroll, 0);
        assert_eq!(state.scroll_limit, u16::MAX);
    }

    #[test]
    fn keyboard_a_toggles_current_tab_expansion() {
        let mut state = AppState::new(resource());

        let intent = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty())),
        );

        assert_eq!(intent, AppIntent::None);
        assert!(state.block_expanded(&BlockId::Body));

        let intent = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty())),
        );

        assert_eq!(intent, AppIntent::None);
        assert!(!state.block_expanded(&BlockId::Body));
    }

    #[test]
    fn keyboard_a_is_inert_for_overlays() {
        let mut state = AppState::new(resource());
        state.show_help = true;

        let intent = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty())),
        );

        assert_eq!(intent, AppIntent::None);
        assert!(state.expanded_blocks.is_empty());
    }

    #[test]
    fn keyboard_f_loads_full_depth_only_when_partial_warning_is_present() {
        let mut complete_state = AppState::new(resource());

        let intent = apply_event(
            &mut complete_state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::empty())),
        );

        assert_eq!(intent, AppIntent::None);

        let mut partial_state = AppState::new(resource());
        partial_state.resource.warnings.push(format!(
            "normal API depth shows the first 100 only for comments; {FULL_DEPTH_WARNING_HINT} for exhaustive pagination"
        ));

        let intent = apply_event(
            &mut partial_state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::empty())),
        );

        assert_eq!(intent, AppIntent::LoadFullDepth);
    }

    #[test]
    fn mouse_click_on_tab_changes_active_tab() {
        let mut state = AppState::new(resource());
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 0, 10, 1),
            HitTarget::Tab(Tab::Links),
        ));

        apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 4,
                row: 0,
                modifiers: KeyModifiers::empty(),
            }),
        );

        assert_eq!(state.active_tab, Tab::Links);
    }

    #[test]
    fn opening_resource_prompt_clears_stale_click_targets() {
        let mut state = AppState::new(resource());
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 0, 10, 1),
            HitTarget::Tab(Tab::Links),
        ));

        let open = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty())),
        );
        let click = apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 4,
                row: 0,
                modifiers: KeyModifiers::empty(),
            }),
        );

        assert_eq!(open, AppIntent::None);
        assert_eq!(click, AppIntent::None);
        assert!(state.add_resource_prompt.is_some());
        assert_eq!(state.active_tab, Tab::Overview);
    }

    #[test]
    fn mouse_wheel_scrolls_without_using_ctrl_shortcuts() {
        let mut state = AppState::new(resource());

        apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 4,
                row: 4,
                modifiers: KeyModifiers::empty(),
            }),
        );

        assert_eq!(state.scroll, 3);
    }

    #[test]
    fn repeated_scroll_down_at_bottom_is_idempotent() {
        let mut state = AppState::new(resource());
        state.set_scroll_limit(9);
        state.scroll_to_bottom();

        for _ in 0..20 {
            apply_event(
                &mut state,
                AppEvent::Mouse(MouseEvent {
                    kind: MouseEventKind::ScrollDown,
                    column: 4,
                    row: 4,
                    modifiers: KeyModifiers::empty(),
                }),
            );
        }

        assert_eq!(state.scroll, 9);
    }

    #[test]
    fn repeated_key_down_at_bottom_is_idempotent() {
        let mut state = AppState::new(resource());
        state.set_scroll_limit(4);
        state.scroll_to_bottom();

        for _ in 0..20 {
            apply_event(
                &mut state,
                AppEvent::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty())),
            );
        }

        assert_eq!(state.scroll, 4);
    }

    #[test]
    fn repeated_page_down_at_bottom_is_idempotent() {
        let mut state = AppState::new(resource());
        state.set_scroll_limit(12);
        state.scroll_to_bottom();

        for _ in 0..20 {
            apply_event(
                &mut state,
                AppEvent::Key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::empty())),
            );
        }

        assert_eq!(state.scroll, 12);
    }

    #[test]
    fn mouse_wheel_is_inert_while_resource_prompt_is_open() {
        let mut state = AppState::new(resource());
        state.set_scroll_limit(12);
        state.scroll = 6;
        state.open_add_resource_prompt();

        for kind in [MouseEventKind::ScrollDown, MouseEventKind::ScrollUp] {
            let intent = apply_event(
                &mut state,
                AppEvent::Mouse(MouseEvent {
                    kind,
                    column: 1,
                    row: 1,
                    modifiers: KeyModifiers::empty(),
                }),
            );

            assert_eq!(intent, AppIntent::None);
            assert_eq!(state.scroll, 6);
        }
    }

    #[test]
    fn mouse_wheel_is_inert_while_resource_link_prompt_is_open() {
        let mut state = AppState::new(resource());
        state.set_scroll_limit(12);
        state.scroll = 6;
        state.open_resource_link_prompt(
            ResourceId {
                owner: "owner".into(),
                repo: "repo".into(),
                number: 2,
                kind_hint: Some(ResourceKind::Issue),
            },
            Some("https://github.com/owner/repo/issues/2".into()),
        );

        for kind in [MouseEventKind::ScrollDown, MouseEventKind::ScrollUp] {
            let intent = apply_event(
                &mut state,
                AppEvent::Mouse(MouseEvent {
                    kind,
                    column: 1,
                    row: 1,
                    modifiers: KeyModifiers::empty(),
                }),
            );

            assert_eq!(intent, AppIntent::None);
            assert_eq!(state.scroll, 6);
        }
    }

    #[test]
    fn mouse_click_on_refresh_target_requests_refresh() {
        let mut state = AppState::new(resource());
        state
            .hit_areas
            .push(HitArea::new(Rect::new(0, 0, 9, 1), HitTarget::Refresh));

        let intent = apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 1,
                row: 0,
                modifiers: KeyModifiers::empty(),
            }),
        );

        assert_eq!(intent, AppIntent::Refresh);
        assert!(state.refresh_requested);
    }

    #[test]
    fn mouse_click_on_load_full_target_requests_full_depth_load() {
        let mut state = AppState::new(resource());
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 0, 11, 1),
            HitTarget::LoadFullDepth,
        ));

        let intent = apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 1,
                row: 0,
                modifiers: KeyModifiers::empty(),
            }),
        );

        assert_eq!(intent, AppIntent::LoadFullDepth);
    }

    #[test]
    fn keyboard_o_falls_back_to_current_resource_url() {
        let mut state = AppState::new(resource());

        let intent = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::empty())),
        );

        assert_eq!(
            intent,
            AppIntent::OpenUrl("https://github.com/owner/repo/issues/1".into())
        );
    }

    #[test]
    fn keyboard_o_ignores_non_github_current_resource_url() {
        let mut resource = pr_resource();
        resource.id.owner = "huggingface".into();
        resource.id.repo = "huggingface.js".into();
        resource.id.number = 2185;
        resource.url = "http://huggingface/huggingface.js#2185".into();
        let mut state = AppState::new(resource);

        let intent = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::empty())),
        );

        assert_eq!(
            intent,
            AppIntent::OpenUrl("https://github.com/huggingface/huggingface.js/pull/2185".into())
        );
    }

    #[test]
    fn keyboard_o_opens_first_visible_url() {
        let mut state = AppState::new(resource());
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 0, 10, 1),
            HitTarget::OpenUrl("https://github.com/owner/repo/actions/runs/1".into()),
        ));

        let intent = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::empty())),
        );

        assert_eq!(
            intent,
            AppIntent::OpenUrl("https://github.com/owner/repo/actions/runs/1".into())
        );
    }

    #[test]
    fn keyboard_y_falls_back_to_current_resource_url() {
        let mut state = AppState::new(resource());

        let intent = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::empty())),
        );

        assert_eq!(
            intent,
            AppIntent::CopyUrl("https://github.com/owner/repo/issues/1".into())
        );
    }

    #[test]
    fn keyboard_y_copies_first_visible_open_url() {
        let mut state = AppState::new(resource());
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 0, 10, 1),
            HitTarget::ToggleBlock(BlockId::Body),
        ));
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 1, 10, 1),
            HitTarget::OpenUrl("https://github.com/owner/repo/actions/runs/1".into()),
        ));

        let intent = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::empty())),
        );

        assert_eq!(
            intent,
            AppIntent::CopyUrl("https://github.com/owner/repo/actions/runs/1".into())
        );
    }

    #[test]
    fn keyboard_y_copies_first_visible_navigation_target() {
        let mut state = AppState::new(resource());
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 0, 10, 1),
            HitTarget::Navigate(ResourceId {
                owner: "owner".into(),
                repo: "repo".into(),
                number: 2,
                kind_hint: Some(ResourceKind::PullRequest),
            }),
        ));

        let intent = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::empty())),
        );

        assert_eq!(
            intent,
            AppIntent::CopyUrl("https://github.com/owner/repo/pull/2".into())
        );
    }

    #[test]
    fn keyboard_y_in_settings_cycles_symbols_instead_of_copying() {
        let mut state = AppState::new(resource());
        state.show_settings = true;

        let intent = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::empty())),
        );

        assert_eq!(intent, AppIntent::SaveSettings);
        assert_ne!(state.symbols, SymbolMode::Ascii);
    }

    #[test]
    fn keyboard_enter_activates_first_visible_content_action() {
        let mut state = AppState::new(resource());
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 0, 10, 1),
            HitTarget::Tab(Tab::Links),
        ));
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 1, 10, 1),
            HitTarget::ToggleBlock(BlockId::Body),
        ));

        let intent = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())),
        );

        assert_eq!(intent, AppIntent::None);
        assert!(state.block_expanded(&BlockId::Body));
        assert_eq!(state.active_tab, Tab::Overview);
    }

    #[test]
    fn keyboard_enter_activates_visible_expand_all_action() {
        let mut state = AppState::new(resource());
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 0, 12, 1),
            HitTarget::ExpandBlocks(vec![BlockId::Body, BlockId::Activity("comment-1".into())]),
        ));

        let intent = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())),
        );

        assert_eq!(intent, AppIntent::None);
        assert!(state.block_expanded(&BlockId::Body));
        assert!(state.block_expanded(&BlockId::Activity("comment-1".into())));
    }

    #[test]
    fn mouse_click_on_collapse_all_action_collapses_blocks() {
        let mut state = AppState::new(resource());
        state.expand_blocks(vec![BlockId::Body, BlockId::Activity("comment-1".into())]);
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 0, 14, 1),
            HitTarget::CollapseBlocks(vec![BlockId::Body, BlockId::Activity("comment-1".into())]),
        ));

        let intent = apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 1,
                row: 0,
                modifiers: KeyModifiers::empty(),
            }),
        );

        assert_eq!(intent, AppIntent::None);
        assert!(!state.block_expanded(&BlockId::Body));
        assert!(!state.block_expanded(&BlockId::Activity("comment-1".into())));
    }

    #[test]
    fn keyboard_enter_navigates_to_first_visible_content_link() {
        let mut state = AppState::new(resource());
        let id = ResourceId::from_owner_repo_number("owner/repo", "2").unwrap();
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 0, 10, 1),
            HitTarget::Navigate(id.clone()),
        ));

        let intent = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())),
        );

        assert_eq!(intent, AppIntent::Navigate(id));
    }

    #[test]
    fn keyboard_enter_opens_first_visible_url_action() {
        let mut state = AppState::new(resource());
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 0, 10, 1),
            HitTarget::OpenUrl("https://github.com/owner/repo/actions/runs/1".into()),
        ));

        let intent = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())),
        );

        assert_eq!(
            intent,
            AppIntent::OpenUrl("https://github.com/owner/repo/actions/runs/1".into())
        );
    }

    #[test]
    fn mouse_click_on_open_target_requests_visible_or_current_url() {
        let mut state = AppState::new(resource());
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 1, 10, 1),
            HitTarget::Navigate(ResourceId {
                owner: "owner".into(),
                repo: "repo".into(),
                number: 2,
                kind_hint: Some(ResourceKind::PullRequest),
            }),
        ));
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 0, 6, 1),
            HitTarget::OpenVisibleUrl,
        ));

        let intent = apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 1,
                row: 0,
                modifiers: KeyModifiers::empty(),
            }),
        );

        assert_eq!(
            intent,
            AppIntent::OpenUrl("https://github.com/owner/repo/pull/2".into())
        );
    }

    #[test]
    fn mouse_click_on_copy_target_requests_visible_or_current_url() {
        let mut state = AppState::new(resource());
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 1, 10, 1),
            HitTarget::OpenUrl("https://github.com/owner/repo/issues/1#issuecomment-1".into()),
        ));
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 0, 6, 1),
            HitTarget::CopyVisibleUrl,
        ));

        let intent = apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 1,
                row: 0,
                modifiers: KeyModifiers::empty(),
            }),
        );

        assert_eq!(
            intent,
            AppIntent::CopyUrl("https://github.com/owner/repo/issues/1#issuecomment-1".into())
        );
    }

    #[test]
    fn mouse_click_on_url_target_requests_open_url() {
        let mut state = AppState::new(resource());
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 0, 20, 1),
            HitTarget::OpenUrl("https://github.com/owner/repo/actions/runs/1".into()),
        ));

        let intent = apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 1,
                row: 0,
                modifiers: KeyModifiers::empty(),
            }),
        );

        assert_eq!(
            intent,
            AppIntent::OpenUrl("https://github.com/owner/repo/actions/runs/1".into())
        );
    }

    #[test]
    fn different_resource_link_opens_choice_prompt_and_confirms_here() {
        let mut state = AppState::new(resource());
        let id = ResourceId {
            owner: "owner".into(),
            repo: "repo".into(),
            number: 2,
            kind_hint: Some(ResourceKind::PullRequest),
        };

        let prompt = apply_event(
            &mut state,
            AppEvent::Activate(HitTarget::ResourceLink {
                id: id.clone(),
                url: Some("https://github.com/owner/repo/pull/2".into()),
            }),
        );
        let confirm = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())),
        );

        assert_eq!(prompt, AppIntent::None);
        assert_eq!(confirm, AppIntent::Navigate(id));
        assert!(state.resource_link_prompt.is_none());
    }

    #[test]
    fn different_resource_link_can_open_in_new_tab_from_prompt() {
        let mut state = AppState::new(resource());
        let id = ResourceId {
            owner: "owner".into(),
            repo: "repo".into(),
            number: 2,
            kind_hint: Some(ResourceKind::Issue),
        };

        apply_event(
            &mut state,
            AppEvent::Activate(HitTarget::ResourceLink {
                id: id.clone(),
                url: None,
            }),
        );
        let intent = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty())),
        );

        assert_eq!(intent, AppIntent::OpenResource(id));
        assert!(state.resource_link_prompt.is_none());
    }

    #[test]
    fn same_resource_comment_link_focuses_activity_without_prompt() {
        let mut resource = resource();
        resource.activity = vec![activity_entry(
            "comment-1",
            "https://github.com/owner/repo/issues/1#issuecomment-1",
        )];
        let mut state = AppState::new(resource);

        let intent = apply_event(
            &mut state,
            AppEvent::Activate(HitTarget::ResourceLink {
                id: ResourceId {
                    owner: "owner".into(),
                    repo: "repo".into(),
                    number: 1,
                    kind_hint: Some(ResourceKind::Issue),
                },
                url: Some("https://github.com/owner/repo/issues/1#issuecomment-1".into()),
            }),
        );

        assert_eq!(intent, AppIntent::None);
        assert!(state.resource_link_prompt.is_none());
        assert_eq!(state.active_tab, Tab::Activity);
        assert!(state.block_expanded(&BlockId::Activity("comment-1".into())));
        assert_eq!(
            state.take_pending_activity_focus().as_deref(),
            Some("comment-1")
        );
    }

    #[test]
    fn mouse_click_on_quit_target_requests_quit() {
        let mut state = AppState::new(resource());
        state
            .hit_areas
            .push(HitArea::new(Rect::new(0, 0, 6, 1), HitTarget::Quit));

        let intent = apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 1,
                row: 0,
                modifiers: KeyModifiers::empty(),
            }),
        );

        assert_eq!(intent, AppIntent::Quit);
        assert!(state.should_quit);
    }

    #[test]
    fn backspace_requests_resource_history_navigation() {
        let mut state = AppState::new(resource());

        let intent = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty())),
        );

        assert!(matches!(intent, AppIntent::Back));
    }

    #[test]
    fn keyboard_question_mark_toggles_help_overlay() {
        let mut state = AppState::new(resource());

        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::empty())),
        );

        assert!(state.show_help);
    }

    #[test]
    fn keyboard_s_toggles_settings() {
        let mut state = AppState::new(resource());

        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::empty())),
        );

        assert!(state.show_settings);

        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty())),
        );

        assert!(!state.show_settings);
    }

    #[test]
    fn settings_keyboard_changes_return_save_intent() {
        let mut state = AppState::new(resource());
        state.show_settings = true;

        let theme = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::empty())),
        );

        assert_eq!(theme, AppIntent::SaveSettings);
        assert_eq!(state.theme, ThemeName::Catppuccin);

        let symbols = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::empty())),
        );

        assert_eq!(symbols, AppIntent::SaveSettings);
        assert_eq!(state.symbols, SymbolMode::Emoji);

        let spacing = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::empty())),
        );

        assert_eq!(spacing, AppIntent::SaveSettings);
        assert_eq!(state.spacing, SpacingMode::Compact);

        let width_mode = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::empty())),
        );

        assert_eq!(width_mode, AppIntent::SaveSettings);
        assert_eq!(state.width_mode, ContentWidthMode::Full);

        let scrollbar = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::empty())),
        );

        assert_eq!(scrollbar, AppIntent::SaveSettings);
        assert_eq!(state.scrollbar, ScrollbarMode::Always);

        let width = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('+'), KeyModifiers::empty())),
        );

        assert_eq!(width, AppIntent::SaveSettings);
        assert_eq!(
            state.fixed_width,
            crate::render::DEFAULT_FIXED_CONTENT_WIDTH + crate::render::FIXED_CONTENT_WIDTH_STEP
        );
    }

    #[test]
    fn mouse_click_on_settings_targets_updates_preferences() {
        let mut state = AppState::new(resource());
        state
            .hit_areas
            .push(HitArea::new(Rect::new(0, 0, 10, 1), HitTarget::Settings));

        let open = apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 1,
                row: 0,
                modifiers: KeyModifiers::empty(),
            }),
        );

        assert_eq!(open, AppIntent::None);
        assert!(state.show_settings);

        state.hit_areas.clear();
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 1, 20, 1),
            HitTarget::SetTheme("solarized-dark".into()),
        ));
        let theme = apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 1,
                row: 1,
                modifiers: KeyModifiers::empty(),
            }),
        );

        assert_eq!(theme, AppIntent::SaveSettings);
        assert_eq!(state.theme, ThemeName::Solarized);

        state.hit_areas.clear();
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 2, 20, 1),
            HitTarget::SetSpacing("compact".into()),
        ));
        let spacing = apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 1,
                row: 2,
                modifiers: KeyModifiers::empty(),
            }),
        );

        assert_eq!(spacing, AppIntent::SaveSettings);
        assert_eq!(state.spacing, SpacingMode::Compact);

        state.hit_areas.clear();
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 3, 20, 1),
            HitTarget::SetWidthMode("full".into()),
        ));
        let width_mode = apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 1,
                row: 3,
                modifiers: KeyModifiers::empty(),
            }),
        );

        assert_eq!(width_mode, AppIntent::SaveSettings);
        assert_eq!(state.width_mode, ContentWidthMode::Full);

        state.hit_areas.clear();
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 4, 20, 1),
            HitTarget::SetFixedWidth(132),
        ));
        let fixed_width = apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 1,
                row: 4,
                modifiers: KeyModifiers::empty(),
            }),
        );

        assert_eq!(fixed_width, AppIntent::SaveSettings);
        assert_eq!(state.fixed_width, 132);

        state.hit_areas.clear();
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 5, 20, 1),
            HitTarget::SetScrollbar("hidden".into()),
        ));
        let scrollbar = apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 1,
                row: 5,
                modifiers: KeyModifiers::empty(),
            }),
        );

        assert_eq!(scrollbar, AppIntent::SaveSettings);
        assert_eq!(state.scrollbar, ScrollbarMode::Hidden);
    }

    #[test]
    fn mouse_scrollbar_click_and_drag_updates_scroll_position() {
        let mut state = AppState::new(resource());
        state.set_scroll_limit(100);
        state.scrollbar = ScrollbarMode::Always;
        state.hit_areas.push(HitArea::new(
            Rect::new(79, 10, 1, 21),
            HitTarget::Scrollbar {
                top: 10,
                height: 21,
            },
        ));

        let click = apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 79,
                row: 20,
                modifiers: KeyModifiers::empty(),
            }),
        );

        assert_eq!(click, AppIntent::None);
        assert_eq!(state.scroll, 50);
        assert!(state.scrollbar_drag.is_some());

        apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Drag(MouseButton::Left),
                column: 79,
                row: 30,
                modifiers: KeyModifiers::empty(),
            }),
        );

        assert_eq!(state.scroll, 100);

        apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Up(MouseButton::Left),
                column: 79,
                row: 30,
                modifiers: KeyModifiers::empty(),
            }),
        );

        assert!(state.scrollbar_drag.is_none());
    }

    #[test]
    fn keyboard_end_jumps_to_bottom_sentinel() {
        let mut state = AppState::new(resource());

        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::End, KeyModifiers::empty())),
        );

        assert_eq!(state.scroll, u16::MAX);
    }

    #[test]
    fn mouse_click_on_help_target_toggles_help() {
        let mut state = AppState::new(resource());
        state
            .hit_areas
            .push(HitArea::new(Rect::new(0, 0, 6, 1), HitTarget::Help));

        apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 1,
                row: 0,
                modifiers: KeyModifiers::empty(),
            }),
        );

        assert!(state.show_help);
    }

    #[test]
    fn keyboard_n_opens_resource_prompt_and_enter_confirms_relative_number() {
        let mut state = AppState::new(resource());

        let open = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty())),
        );
        assert_eq!(open, AppIntent::None);
        assert!(state.add_resource_prompt.is_some());

        for ch in ['4', '2'] {
            apply_event(
                &mut state,
                AppEvent::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::empty())),
            );
        }
        let confirm = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())),
        );

        assert_eq!(
            confirm,
            AppIntent::OpenResource(crate::domain::ResourceId {
                owner: "owner".into(),
                repo: "repo".into(),
                number: 42,
                kind_hint: None,
            })
        );
        assert!(state.add_resource_prompt.is_some());
    }

    #[test]
    fn invalid_resource_prompt_input_stays_open_with_error() {
        let mut state = AppState::new(resource());
        state.open_add_resource_prompt();
        state
            .add_resource_input_mut()
            .unwrap()
            .push_str("not a resource");

        let intent = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())),
        );

        assert_eq!(intent, AppIntent::None);
        let prompt = state.add_resource_prompt.as_ref().unwrap();
        assert!(prompt.error.is_some());
    }

    #[test]
    fn ctrl_c_in_resource_prompt_clears_input_then_closes_when_empty() {
        let mut state = AppState::new(resource());
        state.open_add_resource_prompt();
        state
            .add_resource_input_mut()
            .unwrap()
            .push_str("owner/repo#42");

        let first = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        );
        let second = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        );

        assert_eq!(first, AppIntent::None);
        assert_eq!(second, AppIntent::None);
        assert!(!state.should_quit);
        assert!(state.add_resource_prompt.is_none());
    }

    #[test]
    fn mouse_targets_switch_close_and_open_resource_tabs() {
        let mut state = AppState::new(resource());
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 0, 10, 1),
            HitTarget::OpenResourcePrompt,
        ));

        apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 1,
                row: 0,
                modifiers: KeyModifiers::empty(),
            }),
        );
        assert!(state.add_resource_prompt.is_some());

        state.close_add_resource_prompt();
        let mut second = resource();
        second.id.number = 2;
        second.title = "Second".into();
        state.open_resource_in_tab(second);
        state.hit_areas.clear();
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 1, 13, 1),
            HitTarget::ResourceTab(0),
        ));
        state.hit_areas.push(HitArea::new(
            Rect::new(10, 1, 3, 1),
            HitTarget::CloseResourceTab(1),
        ));

        apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 1,
                row: 1,
                modifiers: KeyModifiers::empty(),
            }),
        );
        assert_eq!(state.resource.id.number, 1);

        state.hit_areas.push(HitArea::new(
            Rect::new(0, 1, 13, 1),
            HitTarget::ResourceTab(1),
        ));
        state.hit_areas.push(HitArea::new(
            Rect::new(10, 1, 3, 1),
            HitTarget::CloseResourceTab(1),
        ));
        apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 11,
                row: 1,
                modifiers: KeyModifiers::empty(),
            }),
        );
        assert_eq!(state.resource_tabs.len(), 1);
    }

    #[test]
    fn modal_mouse_targets_win_over_underlying_content_hits() {
        let mut state = AppState::new(resource());
        state.open_add_resource_prompt();
        state
            .add_resource_input_mut()
            .unwrap()
            .push_str("owner/repo#42");
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 0, 8, 1),
            HitTarget::Navigate(crate::domain::ResourceId {
                owner: "owner".into(),
                repo: "repo".into(),
                number: 99,
                kind_hint: None,
            }),
        ));
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 0, 8, 1),
            HitTarget::ConfirmResourcePrompt,
        ));

        let intent = apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 1,
                row: 0,
                modifiers: KeyModifiers::empty(),
            }),
        );

        assert_eq!(
            intent,
            AppIntent::OpenResource(crate::domain::ResourceId {
                owner: "owner".into(),
                repo: "repo".into(),
                number: 42,
                kind_hint: None,
            })
        );
    }

    #[test]
    fn modal_overlay_mouse_target_blocks_underlying_content_hits() {
        let mut state = AppState::new(resource());
        state.open_add_resource_prompt();
        state.hit_areas.push(HitArea::new(
            Rect::new(0, 0, 8, 1),
            HitTarget::Navigate(crate::domain::ResourceId {
                owner: "owner".into(),
                repo: "repo".into(),
                number: 99,
                kind_hint: None,
            }),
        ));
        state
            .hit_areas
            .push(HitArea::new(Rect::new(0, 0, 8, 1), HitTarget::ModalOverlay));

        let intent = apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 1,
                row: 0,
                modifiers: KeyModifiers::empty(),
            }),
        );

        assert_eq!(intent, AppIntent::None);
        assert!(state.add_resource_prompt.is_some());
    }
}

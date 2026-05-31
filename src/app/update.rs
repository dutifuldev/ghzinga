use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::app::{AppState, BlockId};
use crate::input::{hit_test, HitTarget};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppIntent {
    None,
    Refresh,
    Navigate(crate::domain::ResourceId),
    OpenResource(crate::domain::ResourceId),
    OpenUrl(String),
    Back,
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
    match key.code {
        KeyCode::Char('q') => {
            state.should_quit = true;
            AppIntent::Quit
        }
        KeyCode::Char('r') => {
            state.refresh_requested = true;
            AppIntent::Refresh
        }
        KeyCode::Char('o') => AppIntent::OpenResource(state.resource.id.clone()),
        KeyCode::Char('?') => {
            state.toggle_help();
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
            state.scroll = state.scroll.saturating_add(1);
            AppIntent::None
        }
        KeyCode::Up => {
            state.scroll = state.scroll.saturating_sub(1);
            AppIntent::None
        }
        KeyCode::PageDown => {
            state.scroll = state.scroll.saturating_add(10);
            AppIntent::None
        }
        KeyCode::PageUp => {
            state.scroll = state.scroll.saturating_sub(10);
            AppIntent::None
        }
        KeyCode::Home => {
            state.scroll = 0;
            AppIntent::None
        }
        KeyCode::End => {
            state.scroll = u16::MAX;
            AppIntent::None
        }
        KeyCode::Char('e') => {
            state.toggle_block(BlockId::Body);
            AppIntent::None
        }
        _ => AppIntent::None,
    }
}

fn apply_mouse(state: &mut AppState, mouse: MouseEvent) -> AppIntent {
    match mouse.kind {
        MouseEventKind::ScrollDown => {
            state.scroll = state.scroll.saturating_add(3);
            AppIntent::None
        }
        MouseEventKind::ScrollUp => {
            state.scroll = state.scroll.saturating_sub(3);
            AppIntent::None
        }
        MouseEventKind::Down(MouseButton::Left) => {
            let Some(target) = hit_test(&state.hit_areas, mouse.column, mouse.row) else {
                return AppIntent::None;
            };
            apply_target(state, target)
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
        HitTarget::ToggleBlock(id) => {
            state.toggle_block(id);
            AppIntent::None
        }
        HitTarget::Navigate(id) => AppIntent::Navigate(id),
        HitTarget::OpenUrl(url) => AppIntent::OpenUrl(url),
        HitTarget::OpenCurrent => AppIntent::OpenResource(state.resource.id.clone()),
        HitTarget::Refresh => {
            state.refresh_requested = true;
            AppIntent::Refresh
        }
        HitTarget::Quit => {
            state.should_quit = true;
            AppIntent::Quit
        }
        HitTarget::Help => {
            state.toggle_help();
            AppIntent::None
        }
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::MouseEvent;

    use super::*;
    use crate::app::Tab;
    use crate::domain::{ReactionCounts, Resource, ResourceId, ResourceKind};
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
    fn keyboard_o_requests_open_current_resource() {
        let mut state = AppState::new(resource());

        let intent = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::empty())),
        );

        assert!(matches!(
            intent,
            AppIntent::OpenResource(id) if id.canonical_name() == "owner/repo#1"
        ));
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
    fn mouse_click_on_open_target_requests_open_current_resource() {
        let mut state = AppState::new(resource());
        state
            .hit_areas
            .push(HitArea::new(Rect::new(0, 0, 6, 1), HitTarget::OpenCurrent));

        let intent = apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 1,
                row: 0,
                modifiers: KeyModifiers::empty(),
            }),
        );

        assert!(matches!(
            intent,
            AppIntent::OpenResource(id) if id.canonical_name() == "owner/repo#1"
        ));
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
}

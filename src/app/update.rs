use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::app::{AddResourceMode, AppState, BlockId};
use crate::domain::{ActionKind, ResourceAction};
use crate::input::{hit_test, HitTarget};
use crate::render::{ContentWidthMode, ScrollbarMode, SpacingMode, SymbolMode, ThemeName};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppIntent {
    None,
    Refresh,
    LoadFullDepth,
    OpenResource(crate::domain::ResourceId),
    ReplaceResource(crate::domain::ResourceId),
    Navigate(crate::domain::ResourceId),
    OpenUrl(String),
    CopyUrl(String),
    SubmitAction(ResourceAction),
    Back,
    SaveSettings,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Paste(String),
    Activate(HitTarget),
    Tick,
}

pub fn apply_event(state: &mut AppState, event: AppEvent) -> AppIntent {
    match event {
        AppEvent::Key(key) => apply_key(state, key),
        AppEvent::Mouse(mouse) => apply_mouse(state, mouse),
        AppEvent::Paste(text) => apply_paste(state, &text),
        AppEvent::Activate(target) => apply_target(state, target),
        AppEvent::Tick => AppIntent::None,
    }
}

fn apply_paste(state: &mut AppState, text: &str) -> AppIntent {
    if state.comment_composer.is_some() {
        if !comment_is_posting(state) {
            if let Some(composer) = &mut state.comment_composer {
                composer.insert_str(text);
            }
        }
    } else if let Some(input) = state.add_resource_input_mut() {
        let single_line: String = text
            .chars()
            .map(|ch| if ch == '\n' || ch == '\r' { ' ' } else { ch })
            .collect();
        input.push_str(single_line.trim());
        state.clear_add_resource_error();
    }
    AppIntent::None
}

fn apply_key(state: &mut AppState, key: KeyEvent) -> AppIntent {
    if state.quit_confirmation {
        return apply_quit_confirmation_key(state, key);
    }
    if state.add_resource_prompt.is_some() {
        return apply_add_resource_prompt_key(state, key);
    }
    if state.resource_link_prompt.is_some() {
        return apply_resource_link_prompt_key(state, key);
    }
    if state.comment_composer.is_some() {
        return apply_composer_key(state, key);
    }
    if state.action_confirm.is_some() {
        return apply_action_confirm_key(state, key);
    }
    if state.edit_picker.is_some() {
        return apply_edit_picker_key(state, key);
    }
    if state.action_menu.is_some() {
        return apply_action_menu_key(state, key);
    }
    match key.code {
        KeyCode::Char('q') if is_plain_shortcut(key) => {
            if close_active_overlay(state) {
                AppIntent::None
            } else {
                state.request_quit_confirmation();
                AppIntent::None
            }
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.should_quit = true;
            AppIntent::Quit
        }
        KeyCode::Char('r') if is_plain_shortcut(key) => {
            state.refresh_requested = true;
            AppIntent::Refresh
        }
        KeyCode::Char('A') if is_plain_shortcut(key) => {
            state.open_action_menu();
            AppIntent::None
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
            state.open_replace_resource_prompt();
            AppIntent::None
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
        KeyCode::Char('x')
            if !state.show_help && !state.show_settings && is_plain_shortcut(key) =>
        {
            state.close_resource_tab(state.active_resource_tab);
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
        KeyCode::Tab | KeyCode::Char('\t') => {
            state.next_resource_tab();
            AppIntent::None
        }
        KeyCode::BackTab => {
            state.previous_resource_tab();
            AppIntent::None
        }
        KeyCode::Char('i') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.next_resource_tab();
            AppIntent::None
        }
        KeyCode::Right if key.modifiers.contains(KeyModifiers::SHIFT) => {
            state.next_resource_tab();
            AppIntent::None
        }
        KeyCode::Left if key.modifiers.contains(KeyModifiers::SHIFT) => {
            state.previous_resource_tab();
            AppIntent::None
        }
        KeyCode::Right => {
            state.next_tab();
            AppIntent::None
        }
        KeyCode::Char('l') if is_plain_shortcut(key) => {
            state.next_tab();
            AppIntent::None
        }
        KeyCode::Left => {
            state.previous_tab();
            AppIntent::None
        }
        KeyCode::Char('h') if is_plain_shortcut(key) => {
            state.previous_tab();
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
        KeyCode::Down => {
            state.scroll_down(1);
            AppIntent::None
        }
        KeyCode::Char('j') if is_plain_shortcut(key) => {
            state.scroll_down(1);
            AppIntent::None
        }
        KeyCode::Up => {
            state.scroll_up(1);
            AppIntent::None
        }
        KeyCode::Char('k') if is_plain_shortcut(key) => {
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

fn apply_quit_confirmation_key(state: &mut AppState, key: KeyEvent) -> AppIntent {
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.should_quit = true;
            AppIntent::Quit
        }
        KeyCode::Enter | KeyCode::Char('q') if is_plain_shortcut(key) => {
            state.should_quit = true;
            AppIntent::Quit
        }
        KeyCode::Esc | KeyCode::Char('n') if is_plain_shortcut(key) => {
            state.close_quit_confirmation();
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
        KeyCode::Esc | KeyCode::Char('q') if is_plain_shortcut(key) => {
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
        KeyCode::Esc | KeyCode::Char('q') if is_plain_shortcut(key) => {
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

fn apply_composer_key(state: &mut AppState, key: KeyEvent) -> AppIntent {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return apply_composer_control_key(state, key);
    }
    // While the comment is posting the draft is frozen: edits made now would
    // be silently lost when the posted snapshot closes the composer.
    if comment_is_posting(state) {
        if key.code == KeyCode::Esc {
            state.cancel_comment_composer();
        }
        return AppIntent::None;
    }
    if let Some(intent) = apply_composer_edit_key(state, key) {
        return intent;
    }
    AppIntent::None
}

fn comment_is_posting(state: &AppState) -> bool {
    state.composer_is_posting()
}

fn apply_composer_control_key(state: &mut AppState, key: KeyEvent) -> AppIntent {
    if key.modifiers.contains(KeyModifiers::ALT) {
        return AppIntent::None;
    }
    match key.code {
        KeyCode::Char('s') => submit_composer_comment(state),
        KeyCode::Char('c') => {
            state.cancel_comment_composer();
            AppIntent::None
        }
        _ => AppIntent::None,
    }
}

fn apply_composer_edit_key(state: &mut AppState, key: KeyEvent) -> Option<AppIntent> {
    let composer = state.comment_composer.as_mut()?;
    let width = composer.viewport_width();
    match key.code {
        KeyCode::Esc => state.cancel_comment_composer(),
        KeyCode::Enter => composer.newline(),
        KeyCode::Backspace => composer.backspace(),
        KeyCode::Delete => composer.delete(),
        KeyCode::Left => composer.move_left(),
        KeyCode::Right => composer.move_right(),
        KeyCode::Up => composer.move_vertical(width, -1),
        KeyCode::Down => composer.move_vertical(width, 1),
        KeyCode::Home => composer.move_home(),
        KeyCode::End => composer.move_end(),
        KeyCode::PageUp => composer.move_vertical(width, -page_step(composer.viewport_height())),
        KeyCode::PageDown => composer.move_vertical(width, page_step(composer.viewport_height())),
        KeyCode::Tab => composer.insert_str("    "),
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::ALT) => composer.insert_char(ch),
        _ => {}
    }
    Some(AppIntent::None)
}

fn page_step(height: usize) -> isize {
    isize::try_from(height.max(1)).unwrap_or(1)
}

fn submit_composer_comment(state: &mut AppState) -> AppIntent {
    let Some(composer) = &state.comment_composer else {
        return AppIntent::None;
    };
    // Descriptions may be cleared; comments and reviews need text (the
    // server rejects empty ones anyway, this is just the friendlier error).
    let empty_allowed = matches!(
        state.composer_target.as_ref().map(|target| target.kind),
        Some(crate::domain::EditKind::IssueBody) | Some(crate::domain::EditKind::PullRequestBody)
    );
    if composer.is_empty() && !empty_allowed {
        state.status_message = Some("comment is empty".into());
        return AppIntent::None;
    }
    if state.pending_action.is_some() {
        state.status_message = Some("an action is already in flight".into());
        return AppIntent::None;
    }
    let body = composer.body();
    let action = match &state.composer_target {
        Some(target) => ResourceAction::Edit {
            target: target.clone(),
            body,
        },
        None => ResourceAction::Comment { body },
    };
    AppIntent::SubmitAction(action)
}

fn apply_action_menu_key(state: &mut AppState, key: KeyEvent) -> AppIntent {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') if is_plain_shortcut(key) => {
            state.close_action_menu();
            AppIntent::None
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.close_action_menu();
            AppIntent::None
        }
        KeyCode::Up | KeyCode::Char('k') if is_plain_shortcut(key) => {
            state.move_action_menu_selection(-1);
            AppIntent::None
        }
        KeyCode::Down | KeyCode::Char('j') if is_plain_shortcut(key) => {
            state.move_action_menu_selection(1);
            AppIntent::None
        }
        KeyCode::Enter if is_plain_shortcut(key) => activate_selected_action(state),
        KeyCode::Char(ch) if is_plain_shortcut(key) => activate_action_by_shortcut(state, ch),
        _ => AppIntent::None,
    }
}

fn activate_selected_action(state: &mut AppState) -> AppIntent {
    let Some(kind) = state.selected_action_kind() else {
        return AppIntent::None;
    };
    activate_action_kind(state, kind)
}

fn activate_action_by_shortcut(state: &mut AppState, ch: char) -> AppIntent {
    let available = state
        .action_menu
        .as_ref()
        .map(|menu| menu.actions.clone())
        .unwrap_or_default();
    let kind = match ch {
        'c' => Some(ActionKind::Comment),
        'e' => Some(ActionKind::Edit),
        'm' => Some(ActionKind::Merge),
        'x' => Some(ActionKind::Close),
        'o' => Some(ActionKind::Reopen),
        _ => None,
    };
    match kind {
        Some(kind) if available.contains(&kind) => activate_action_kind(state, kind),
        _ => AppIntent::None,
    }
}

fn activate_action_kind(state: &mut AppState, kind: ActionKind) -> AppIntent {
    match kind {
        ActionKind::Comment => state.open_comment_composer(),
        ActionKind::Edit => state.open_edit_picker(),
        ActionKind::Close | ActionKind::Reopen | ActionKind::Merge => {
            state.open_action_confirm(kind)
        }
    }
    AppIntent::None
}

fn apply_edit_picker_key(state: &mut AppState, key: KeyEvent) -> AppIntent {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') if is_plain_shortcut(key) => {
            state.close_edit_picker();
            AppIntent::None
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.close_edit_picker();
            AppIntent::None
        }
        KeyCode::Up | KeyCode::Char('k') if is_plain_shortcut(key) => {
            state.move_edit_picker_selection(-1);
            AppIntent::None
        }
        KeyCode::Down | KeyCode::Char('j') if is_plain_shortcut(key) => {
            state.move_edit_picker_selection(1);
            AppIntent::None
        }
        KeyCode::Char(ch @ '1'..='9') if is_plain_shortcut(key) => {
            state.select_edit_choice(ch as usize - '1' as usize);
            AppIntent::None
        }
        KeyCode::Enter if is_plain_shortcut(key) => activate_selected_edit_choice(state),
        _ => AppIntent::None,
    }
}

fn activate_selected_edit_choice(state: &mut AppState) -> AppIntent {
    if let Some(target) = state.selected_edit_target() {
        state.open_edit_composer(target);
    }
    AppIntent::None
}

fn activate_edit_for_entry(state: &mut AppState, node_id: &str) -> AppIntent {
    let target = state
        .resource
        .activity
        .iter()
        .filter_map(|entry| entry.edit.clone())
        .find(|target| target.node_id == node_id);
    if let Some(target) = target {
        state.open_edit_composer(target);
    }
    AppIntent::None
}

fn apply_action_confirm_key(state: &mut AppState, key: KeyEvent) -> AppIntent {
    match key.code {
        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('q') if is_plain_shortcut(key) => {
            state.close_action_confirm();
            AppIntent::None
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.close_action_confirm();
            AppIntent::None
        }
        KeyCode::Up | KeyCode::Char('k') if is_plain_shortcut(key) => {
            state.move_merge_method_selection(-1);
            AppIntent::None
        }
        KeyCode::Down | KeyCode::Char('j') if is_plain_shortcut(key) => {
            state.move_merge_method_selection(1);
            AppIntent::None
        }
        KeyCode::Char(ch @ '1'..='3') if is_plain_shortcut(key) => {
            state.select_merge_method(ch as usize - '1' as usize);
            AppIntent::None
        }
        KeyCode::Enter | KeyCode::Char('y') if is_plain_shortcut(key) => {
            submit_confirmed_action(state)
        }
        _ => AppIntent::None,
    }
}

fn submit_confirmed_action(state: &mut AppState) -> AppIntent {
    let Some(action) = state
        .action_confirm
        .as_ref()
        .and_then(|confirm| confirm.chosen_action())
    else {
        return AppIntent::None;
    };
    if state.pending_action.is_some() {
        state.status_message = Some("an action is already in flight".into());
        return AppIntent::None;
    }
    state.close_action_confirm();
    AppIntent::SubmitAction(action)
}

fn confirm_add_resource_prompt(state: &mut AppState) -> AppIntent {
    match state.parse_add_resource_input() {
        Ok(id) => match state
            .add_resource_prompt
            .as_ref()
            .map(|prompt| prompt.mode)
            .unwrap_or(AddResourceMode::NewTab)
        {
            AddResourceMode::NewTab => AppIntent::OpenResource(id),
            AddResourceMode::ReplaceCurrent => AppIntent::ReplaceResource(id),
        },
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
            apply_mouse_scroll(state, ScrollDirection::Down);
            AppIntent::None
        }
        MouseEventKind::ScrollUp => {
            apply_mouse_scroll(state, ScrollDirection::Up);
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
            if let HitTarget::ComposerText { x, y } = target {
                place_composer_cursor(
                    state,
                    mouse.column.saturating_sub(x),
                    mouse.row.saturating_sub(y),
                );
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScrollDirection {
    Up,
    Down,
}

fn apply_mouse_scroll(state: &mut AppState, direction: ScrollDirection) {
    if let Some(composer) = &mut state.comment_composer {
        let width = composer.viewport_width();
        let height = composer.viewport_height();
        let delta = match direction {
            ScrollDirection::Down => 3,
            ScrollDirection::Up => -3,
        };
        composer.scroll_by(width, height, delta);
        return;
    }
    if state.add_resource_prompt.is_some()
        || state.resource_link_prompt.is_some()
        || state.action_menu.is_some()
        || state.action_confirm.is_some()
        || state.edit_picker.is_some()
    {
        return;
    }
    match direction {
        ScrollDirection::Down => state.scroll_down(3),
        ScrollDirection::Up => state.scroll_up(3),
    }
}

fn place_composer_cursor(state: &mut AppState, column: u16, row: u16) {
    if let Some(composer) = &mut state.comment_composer {
        let width = composer.viewport_width();
        let visual_row = composer.scroll + usize::from(row);
        composer.click(width, visual_row, usize::from(column));
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
        HitTarget::ConfirmQuit => {
            state.should_quit = true;
            AppIntent::Quit
        }
        HitTarget::CancelQuit => {
            state.close_quit_confirmation();
            AppIntent::None
        }
        HitTarget::OpenLinkHere => confirm_resource_link_here(state),
        HitTarget::OpenLinkInNewTab => confirm_resource_link_new_tab(state),
        HitTarget::CancelResourceLinkPrompt => {
            state.close_resource_link_prompt();
            AppIntent::None
        }
        HitTarget::ModalOverlay => {
            if state.comment_composer.is_some() {
                state.cancel_comment_composer();
            }
            state.close_add_resource_prompt();
            state.close_resource_link_prompt();
            state.close_quit_confirmation();
            state.close_action_menu();
            state.close_action_confirm();
            state.close_edit_picker();
            AppIntent::None
        }
        HitTarget::OpenActionMenu => {
            state.open_action_menu();
            AppIntent::None
        }
        HitTarget::ActionMenuItem(index) => {
            let kind = state
                .action_menu
                .as_ref()
                .and_then(|menu| menu.actions.get(index).copied());
            match kind {
                Some(kind) => activate_action_kind(state, kind),
                None => AppIntent::None,
            }
        }
        HitTarget::ConfirmAction => submit_confirmed_action(state),
        HitTarget::CancelAction => {
            state.close_action_confirm();
            AppIntent::None
        }
        HitTarget::EditPickerItem(index) => {
            state.select_edit_choice(index);
            activate_selected_edit_choice(state)
        }
        HitTarget::EditActivityEntry { node_id } => activate_edit_for_entry(state, &node_id),
        HitTarget::EditResourceBody => {
            if let Some(target) = crate::domain::body_edit_target(&state.resource) {
                state.open_edit_composer(target);
            }
            AppIntent::None
        }
        HitTarget::SelectMergeMethod(index) => {
            state.select_merge_method(index);
            AppIntent::None
        }
        HitTarget::ComposerSubmit => submit_composer_comment(state),
        HitTarget::ComposerCancel => {
            state.cancel_comment_composer();
            AppIntent::None
        }
        HitTarget::ComposerText { .. } => AppIntent::None,
        HitTarget::LoadFullDepth => AppIntent::LoadFullDepth,
        HitTarget::Quit => {
            state.request_quit_confirmation();
            AppIntent::None
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

fn close_active_overlay(state: &mut AppState) -> bool {
    if state.add_resource_prompt.is_some() {
        state.close_add_resource_prompt();
        return true;
    }
    if state.resource_link_prompt.is_some() {
        state.close_resource_link_prompt();
        return true;
    }
    if state.show_help {
        state.toggle_help();
        return true;
    }
    if state.show_settings {
        state.close_settings();
        return true;
    }
    false
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
            actions: crate::domain::ActionContext::default(),
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

    fn resource_with_number(number: u64) -> Resource {
        let mut resource = resource();
        resource.id.number = number;
        resource.title = format!("Issue {number}");
        resource.url = format!("https://github.com/owner/repo/issues/{number}");
        resource
    }

    fn state_with_resource_tabs() -> AppState {
        let mut state = AppState::new(resource_with_number(1));
        state.open_resource_in_tab(resource_with_number(2));
        state.open_resource_in_tab(resource_with_number(3));
        state
    }

    fn pr_resource() -> Resource {
        let mut resource = resource();
        resource.id.kind_hint = Some(ResourceKind::PullRequest);
        resource.url = "https://github.com/owner/repo/pull/1".into();
        resource.pull_request = Some(PullRequest {
            allowed_merge_methods: Vec::new(),
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
            edit: None,
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
    fn keyboard_tab_changes_active_resource_tab() {
        let mut state = state_with_resource_tabs();
        state.switch_resource_tab(0);

        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty())),
        );

        assert_eq!(state.active_resource_tab, 1);
        assert_eq!(state.resource.id.number, 2);
    }

    #[test]
    fn shift_tab_changes_active_resource_tab() {
        let mut state = state_with_resource_tabs();
        state.switch_resource_tab(0);

        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
        );

        assert_eq!(state.active_resource_tab, 2);
        assert_eq!(state.resource.id.number, 3);
    }

    #[test]
    fn literal_tab_character_changes_active_resource_tab_for_tmux() {
        let mut state = state_with_resource_tabs();
        state.switch_resource_tab(0);

        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('\t'), KeyModifiers::empty())),
        );

        assert_eq!(state.active_resource_tab, 1);
        assert_eq!(state.resource.id.number, 2);
    }

    #[test]
    fn ctrl_i_changes_active_resource_tab_for_tmux_tab_encoding() {
        let mut state = state_with_resource_tabs();
        state.switch_resource_tab(0);

        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::CONTROL)),
        );

        assert_eq!(state.active_resource_tab, 1);
        assert_eq!(state.resource.id.number, 2);
    }

    #[test]
    fn left_and_right_change_content_tabs() {
        let mut state = AppState::new(pr_resource());

        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Right, KeyModifiers::empty())),
        );

        assert_eq!(state.active_tab, Tab::Activity);

        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Left, KeyModifiers::empty())),
        );

        assert_eq!(state.active_tab, Tab::Overview);
    }

    #[test]
    fn shift_left_and_right_change_resource_tabs() {
        let mut state = state_with_resource_tabs();
        state.switch_resource_tab(1);
        state.set_tab(Tab::Activity);

        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT)),
        );

        assert_eq!(state.active_resource_tab, 2);
        assert_eq!(state.resource.id.number, 3);
        assert_eq!(state.active_tab, Tab::Overview);

        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Left, KeyModifiers::SHIFT)),
        );

        assert_eq!(state.active_resource_tab, 1);
        assert_eq!(state.resource.id.number, 2);
    }

    #[test]
    fn vim_hjkl_mirror_arrow_keys() {
        let mut state = AppState::new(pr_resource());
        state.set_scroll_limit(10);
        state.scroll = 5;

        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty())),
        );
        assert_eq!(state.scroll, 6);

        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::empty())),
        );
        assert_eq!(state.scroll, 5);

        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::empty())),
        );
        assert_eq!(state.active_tab, Tab::Activity);

        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::empty())),
        );
        assert_eq!(state.active_tab, Tab::Overview);
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
    fn keyboard_o_opens_replace_current_resource_prompt() {
        let mut state = AppState::new(resource());

        let intent = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::empty())),
        );

        assert_eq!(intent, AppIntent::None);
        assert!(matches!(
            state.add_resource_prompt.as_ref().map(|prompt| prompt.mode),
            Some(AddResourceMode::ReplaceCurrent)
        ));
    }

    #[test]
    fn replace_current_resource_prompt_confirms_replace_intent() {
        let mut state = AppState::new(resource());
        state.open_replace_resource_prompt();
        state.add_resource_input_mut().unwrap().push_str("#42");

        let intent = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())),
        );

        assert_eq!(
            intent,
            AppIntent::ReplaceResource(crate::domain::ResourceId {
                owner: "owner".into(),
                repo: "repo".into(),
                number: 42,
                kind_hint: None,
            })
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

        assert_eq!(intent, AppIntent::None);
        assert!(state.quit_confirmation);
        assert!(!state.should_quit);
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
    fn keyboard_q_closes_overlays_before_quit_confirmation() {
        let mut state = AppState::new(resource());
        state.show_help = true;

        let close_help = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::empty())),
        );
        let ask_quit = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::empty())),
        );

        assert_eq!(close_help, AppIntent::None);
        assert_eq!(ask_quit, AppIntent::None);
        assert!(!state.show_help);
        assert!(state.quit_confirmation);
        assert!(!state.should_quit);
    }

    #[test]
    fn keyboard_q_confirms_quit_only_after_confirmation_is_visible() {
        let mut state = AppState::new(resource());

        let ask_quit = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::empty())),
        );
        let confirm_quit = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::empty())),
        );

        assert_eq!(ask_quit, AppIntent::None);
        assert_eq!(confirm_quit, AppIntent::Quit);
        assert!(state.should_quit);
    }

    #[test]
    fn keyboard_x_closes_current_resource_tab_only_in_normal_view() {
        let mut state = AppState::new(resource());
        let mut second = resource();
        second.id.number = 2;
        second.title = "Second".into();
        state.open_resource_in_tab(second);
        assert_eq!(state.active_resource_tab, 1);

        let close = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty())),
        );

        assert_eq!(close, AppIntent::None);
        assert_eq!(state.resource_tabs.len(), 1);
        assert_eq!(state.resource.id.number, 1);

        state.show_help = true;
        let ignored = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty())),
        );

        assert_eq!(ignored, AppIntent::None);
        assert!(state.show_help);
        assert_eq!(state.resource_tabs.len(), 1);
    }

    #[test]
    fn keyboard_x_keeps_last_resource_tab_open() {
        let mut state = AppState::new(resource());

        let intent = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty())),
        );

        assert_eq!(intent, AppIntent::None);
        assert_eq!(state.resource_tabs.len(), 1);
        assert_eq!(state.resource.id.number, 1);
    }

    #[test]
    fn keyboard_q_closes_resource_prompt_without_quitting() {
        let mut state = AppState::new(resource());
        state.open_add_resource_prompt();
        state
            .add_resource_input_mut()
            .unwrap()
            .push_str("owner/repo#42");

        let intent = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::empty())),
        );

        assert_eq!(intent, AppIntent::None);
        assert!(state.add_resource_prompt.is_none());
        assert!(!state.should_quit);
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
    fn modal_overlay_mouse_target_closes_add_resource_prompt() {
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
        assert!(state.add_resource_prompt.is_none());
    }

    #[test]
    fn modal_overlay_mouse_target_closes_resource_link_prompt() {
        let mut state = AppState::new(resource());
        state.open_resource_link_prompt(
            crate::domain::ResourceId {
                owner: "owner".into(),
                repo: "repo".into(),
                number: 42,
                kind_hint: None,
            },
            Some("https://github.com/owner/repo/issues/42".into()),
        );
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
        assert!(state.resource_link_prompt.is_none());
    }

    fn actionable_issue_state() -> AppState {
        let mut resource = resource();
        resource.actions.node_id = "I_node".into();
        resource.actions.viewer_can_update = true;
        AppState::new(resource)
    }

    fn actionable_pr_state() -> AppState {
        let mut resource = resource();
        resource.actions.node_id = "PR_node".into();
        resource.actions.viewer_can_update = true;
        resource.id.kind_hint = Some(ResourceKind::PullRequest);
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
            allowed_merge_methods: vec![
                crate::domain::MergeMethod::Merge,
                crate::domain::MergeMethod::Squash,
            ],
        });
        AppState::new(resource)
    }

    fn press(state: &mut AppState, code: KeyCode) -> AppIntent {
        apply_event(
            state,
            AppEvent::Key(KeyEvent::new(code, KeyModifiers::empty())),
        )
    }

    fn press_ctrl(state: &mut AppState, ch: char) -> AppIntent {
        apply_event(
            state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::CONTROL)),
        )
    }

    #[test]
    fn shift_a_opens_action_menu_when_actions_are_available() {
        let mut state = actionable_issue_state();
        let intent = press(&mut state, KeyCode::Char('A'));
        assert_eq!(intent, AppIntent::None);
        let menu = state.action_menu.as_ref().expect("action menu open");
        assert_eq!(
            menu.actions,
            vec![ActionKind::Comment, ActionKind::Edit, ActionKind::Close]
        );
    }

    #[test]
    fn action_menu_reports_when_no_actions_are_available() {
        let mut state = AppState::new(resource());
        press(&mut state, KeyCode::Char('A'));
        assert!(state.action_menu.is_none());
        assert_eq!(
            state.status_message.as_deref(),
            Some("no actions available for this resource")
        );
    }

    #[test]
    fn action_menu_arrows_move_and_enter_opens_close_confirm() {
        let mut state = actionable_issue_state();
        press(&mut state, KeyCode::Char('A'));
        press(&mut state, KeyCode::Down);
        press(&mut state, KeyCode::Down);
        let intent = press(&mut state, KeyCode::Enter);
        assert_eq!(intent, AppIntent::None);
        assert!(state.action_menu.is_none());
        let confirm = state.action_confirm.as_ref().expect("confirm open");
        assert_eq!(confirm.kind, ActionKind::Close);
    }

    #[test]
    fn action_menu_esc_closes_without_side_effects() {
        let mut state = actionable_issue_state();
        press(&mut state, KeyCode::Char('A'));
        press(&mut state, KeyCode::Esc);
        assert!(state.action_menu.is_none());
        assert!(state.action_confirm.is_none());
    }

    #[test]
    fn action_menu_comment_shortcut_opens_composer() {
        let mut state = actionable_issue_state();
        press(&mut state, KeyCode::Char('A'));
        press(&mut state, KeyCode::Char('c'));
        assert!(state.action_menu.is_none());
        assert!(state.comment_composer.is_some());
    }

    #[test]
    fn close_confirm_enter_submits_close_action() {
        let mut state = actionable_issue_state();
        state.open_action_confirm(ActionKind::Close);
        let intent = press(&mut state, KeyCode::Enter);
        assert_eq!(intent, AppIntent::SubmitAction(ResourceAction::Close));
        assert!(state.action_confirm.is_none());
    }

    #[test]
    fn confirm_esc_cancels_without_submitting() {
        let mut state = actionable_issue_state();
        state.open_action_confirm(ActionKind::Reopen);
        let intent = press(&mut state, KeyCode::Esc);
        assert_eq!(intent, AppIntent::None);
        assert!(state.action_confirm.is_none());
    }

    #[test]
    fn merge_confirm_selects_method_by_number_and_submits() {
        let mut state = actionable_pr_state();
        press(&mut state, KeyCode::Char('A'));
        press(&mut state, KeyCode::Char('m'));
        let confirm = state.action_confirm.as_ref().expect("merge confirm");
        assert_eq!(
            confirm.merge_methods,
            vec![
                crate::domain::MergeMethod::Merge,
                crate::domain::MergeMethod::Squash
            ]
        );
        press(&mut state, KeyCode::Char('2'));
        let intent = press(&mut state, KeyCode::Enter);
        assert_eq!(
            intent,
            AppIntent::SubmitAction(ResourceAction::Merge {
                method: crate::domain::MergeMethod::Squash
            })
        );
    }

    #[test]
    fn merge_confirm_arrows_move_method_selection() {
        let mut state = actionable_pr_state();
        state.open_action_confirm(ActionKind::Merge);
        press(&mut state, KeyCode::Down);
        assert_eq!(
            state.action_confirm.as_ref().map(|c| c.selected_method),
            Some(1)
        );
        press(&mut state, KeyCode::Up);
        assert_eq!(
            state.action_confirm.as_ref().map(|c| c.selected_method),
            Some(0)
        );
    }

    #[test]
    fn composer_typing_newline_and_ctrl_s_submit_comment() {
        let mut state = actionable_issue_state();
        state.open_comment_composer();
        press(&mut state, KeyCode::Char('h'));
        press(&mut state, KeyCode::Char('i'));
        press(&mut state, KeyCode::Enter);
        press(&mut state, KeyCode::Char('!'));
        let intent = press_ctrl(&mut state, 's');
        assert_eq!(
            intent,
            AppIntent::SubmitAction(ResourceAction::Comment {
                body: "hi\n!".into()
            })
        );
        assert!(
            state.comment_composer.is_some(),
            "composer stays open until the comment is confirmed posted"
        );
    }

    #[test]
    fn composer_submit_with_empty_body_reports_status() {
        let mut state = actionable_issue_state();
        state.open_comment_composer();
        let intent = press_ctrl(&mut state, 's');
        assert_eq!(intent, AppIntent::None);
        assert_eq!(state.status_message.as_deref(), Some("comment is empty"));
    }

    #[test]
    fn composer_esc_needs_confirmation_when_dirty() {
        let mut state = actionable_issue_state();
        state.open_comment_composer();
        press(&mut state, KeyCode::Char('x'));
        press(&mut state, KeyCode::Esc);
        assert!(state.comment_composer.is_some());
        press(&mut state, KeyCode::Esc);
        assert!(state.comment_composer.is_none());
    }

    #[test]
    fn composer_esc_closes_immediately_when_empty() {
        let mut state = actionable_issue_state();
        state.open_comment_composer();
        press(&mut state, KeyCode::Esc);
        assert!(state.comment_composer.is_none());
    }

    #[test]
    fn paste_lands_in_the_composer_as_multiline_text() {
        let mut state = actionable_issue_state();
        state.open_comment_composer();
        apply_event(&mut state, AppEvent::Paste("one\r\ntwo".into()));
        assert_eq!(
            state.comment_composer.as_ref().map(|c| c.body()),
            Some("one\ntwo".into())
        );
    }

    #[test]
    fn paste_into_add_resource_prompt_flattens_newlines() {
        let mut state = AppState::new(resource());
        state.open_add_resource_prompt();
        apply_event(&mut state, AppEvent::Paste("owner/repo\n#12".into()));
        assert_eq!(
            state
                .add_resource_prompt
                .as_ref()
                .map(|prompt| prompt.input.clone()),
            Some("owner/repo #12".into())
        );
    }

    #[test]
    fn pending_action_blocks_a_second_submission() {
        let mut state = actionable_issue_state();
        state.pending_action = Some(ResourceAction::Close);
        state.open_comment_composer();
        press(&mut state, KeyCode::Char('x'));
        let intent = press_ctrl(&mut state, 's');
        assert_eq!(intent, AppIntent::None);
        assert_eq!(
            state.status_message.as_deref(),
            Some("an action is already in flight")
        );
    }

    #[test]
    fn activate_targets_mirror_keyboard_for_action_flows() {
        let mut state = actionable_issue_state();
        apply_event(&mut state, AppEvent::Activate(HitTarget::OpenActionMenu));
        assert!(state.action_menu.is_some());
        apply_event(&mut state, AppEvent::Activate(HitTarget::ActionMenuItem(2)));
        assert_eq!(
            state.action_confirm.as_ref().map(|confirm| confirm.kind),
            Some(ActionKind::Close)
        );
        let intent = apply_event(&mut state, AppEvent::Activate(HitTarget::ConfirmAction));
        assert_eq!(intent, AppIntent::SubmitAction(ResourceAction::Close));
    }

    #[test]
    fn composer_buttons_mirror_keyboard() {
        let mut state = actionable_issue_state();
        state.open_comment_composer();
        apply_event(&mut state, AppEvent::Paste("via button".into()));
        let intent = apply_event(&mut state, AppEvent::Activate(HitTarget::ComposerSubmit));
        assert_eq!(
            intent,
            AppIntent::SubmitAction(ResourceAction::Comment {
                body: "via button".into()
            })
        );
        apply_event(&mut state, AppEvent::Activate(HitTarget::ComposerCancel));
        assert!(
            state.comment_composer.is_some(),
            "dirty composer asks before discarding"
        );
        apply_event(&mut state, AppEvent::Activate(HitTarget::ComposerCancel));
        assert!(state.comment_composer.is_none());
    }

    #[test]
    fn modal_overlay_click_closes_action_modals() {
        let mut state = actionable_issue_state();
        press(&mut state, KeyCode::Char('A'));
        apply_event(&mut state, AppEvent::Activate(HitTarget::ModalOverlay));
        assert!(state.action_menu.is_none());
    }

    #[test]
    fn composer_mouse_click_places_cursor_via_hit_area() {
        let mut state = actionable_issue_state();
        state.open_comment_composer();
        apply_event(&mut state, AppEvent::Paste("abcdef".into()));
        if let Some(composer) = &mut state.comment_composer {
            composer.viewport = (40, 5);
        }
        state.hit_areas.push(HitArea::new(
            Rect::new(10, 5, 40, 5),
            HitTarget::ComposerText { x: 10, y: 5 },
        ));
        apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 12,
                row: 5,
                modifiers: KeyModifiers::empty(),
            }),
        );
        assert_eq!(
            state.comment_composer.as_ref().map(|c| c.cursor()),
            Some((0, 2))
        );
    }

    #[test]
    fn unrelated_drafts_stay_editable_while_a_comment_posts_elsewhere() {
        let mut state = actionable_issue_state();
        state.pending_action = Some(ResourceAction::Comment {
            body: "posted from another tab".into(),
        });
        state.open_comment_composer();
        apply_event(&mut state, AppEvent::Paste("fresh draft".into()));
        press(&mut state, KeyCode::Char('!'));
        assert_eq!(
            state.comment_composer.as_ref().map(|c| c.body()),
            Some("fresh draft!".into()),
            "a different draft must not be frozen by an unrelated posting comment"
        );
    }

    #[test]
    fn composer_draft_is_frozen_while_a_comment_is_posting() {
        let mut state = actionable_issue_state();
        state.open_comment_composer();
        apply_event(&mut state, AppEvent::Paste("submitted".into()));
        state.pending_action = Some(ResourceAction::Comment {
            body: "submitted".into(),
        });
        press(&mut state, KeyCode::Char('!'));
        apply_event(&mut state, AppEvent::Paste("more".into()));
        assert_eq!(
            state.comment_composer.as_ref().map(|c| c.body()),
            Some("submitted".into()),
            "edits during posting would be silently lost, so they are blocked"
        );
        press(&mut state, KeyCode::Esc);
        press(&mut state, KeyCode::Esc);
        assert!(
            state.comment_composer.is_none(),
            "escape still cancels while posting"
        );
    }

    #[test]
    fn modified_keys_do_not_submit_a_confirmation() {
        let mut state = actionable_issue_state();
        state.open_action_confirm(ActionKind::Close);
        let intent = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL)),
        );
        assert_eq!(intent, AppIntent::None);
        assert!(
            state.action_confirm.is_some(),
            "ctrl-modified keys must not confirm a destructive action"
        );
        let intent = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT)),
        );
        assert_eq!(intent, AppIntent::None);
        assert!(state.action_confirm.is_some());
    }

    #[test]
    fn modified_letters_do_not_activate_menu_items() {
        let mut state = actionable_issue_state();
        press(&mut state, KeyCode::Char('A'));
        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT)),
        );
        assert!(state.action_menu.is_some());
        assert!(state.action_confirm.is_none());
    }

    #[test]
    fn tab_switch_keeps_each_tabs_comment_draft() {
        let mut state = actionable_issue_state();
        state.open_comment_composer();
        apply_event(&mut state, AppEvent::Paste("draft for tab one".into()));
        state.open_resource_in_tab(resource_with_number(2));
        assert!(
            state.comment_composer.is_none(),
            "the new tab starts without a composer"
        );
        state.switch_resource_tab(0);
        assert_eq!(
            state.comment_composer.as_ref().map(|c| c.body()),
            Some("draft for tab one".into()),
            "returning to the first tab restores its draft"
        );
    }

    fn composer_body(state: &AppState) -> String {
        state
            .comment_composer
            .as_ref()
            .map(|composer| composer.body())
            .unwrap_or_default()
    }

    fn composer_cursor(state: &AppState) -> (usize, usize) {
        state
            .comment_composer
            .as_ref()
            .map(|composer| composer.cursor())
            .expect("composer open")
    }

    fn open_sized_composer(state: &mut AppState, text: &str, viewport: (u16, u16)) {
        state.open_comment_composer();
        apply_event(state, AppEvent::Paste(text.into()));
        if let Some(composer) = &mut state.comment_composer {
            composer.viewport = viewport;
        }
    }

    #[test]
    fn composer_edit_keys_each_do_their_job() {
        let mut state = actionable_issue_state();
        open_sized_composer(&mut state, "ab", (40, 5));
        press(&mut state, KeyCode::Backspace);
        assert_eq!(composer_body(&state), "a");
        press(&mut state, KeyCode::Home);
        press(&mut state, KeyCode::Delete);
        assert_eq!(composer_body(&state), "");
        apply_event(&mut state, AppEvent::Paste("xy".into()));
        press(&mut state, KeyCode::Left);
        assert_eq!(composer_cursor(&state), (0, 1));
        press(&mut state, KeyCode::Right);
        assert_eq!(composer_cursor(&state), (0, 2));
        press(&mut state, KeyCode::Home);
        assert_eq!(composer_cursor(&state), (0, 0));
        press(&mut state, KeyCode::End);
        assert_eq!(composer_cursor(&state), (0, 2));
        press(&mut state, KeyCode::Tab);
        assert_eq!(composer_body(&state), "xy    ");
    }

    #[test]
    fn composer_vertical_keys_move_through_rows() {
        let mut state = actionable_issue_state();
        open_sized_composer(&mut state, "a\nb\nc\nd\ne\nf\ng\nh", (10, 3));
        if let Some(composer) = &mut state.comment_composer {
            composer.click(10, 0, 1);
        }
        press(&mut state, KeyCode::Down);
        assert_eq!(composer_cursor(&state).0, 1);
        press(&mut state, KeyCode::Up);
        assert_eq!(composer_cursor(&state).0, 0);
        press(&mut state, KeyCode::PageDown);
        assert_eq!(
            composer_cursor(&state).0,
            3,
            "page down moves by exactly the viewport height"
        );
        press(&mut state, KeyCode::PageUp);
        assert_eq!(composer_cursor(&state).0, 0);
    }

    #[test]
    fn composer_ignores_alt_modified_characters() {
        let mut state = actionable_issue_state();
        open_sized_composer(&mut state, "seed", (40, 5));
        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::ALT)),
        );
        assert_eq!(composer_body(&state), "seed");
    }

    #[test]
    fn ctrl_alt_s_does_not_post_a_comment() {
        let mut state = actionable_issue_state();
        open_sized_composer(&mut state, "draft", (40, 5));
        let intent = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(
                KeyCode::Char('s'),
                KeyModifiers::CONTROL | KeyModifiers::ALT,
            )),
        );
        assert_eq!(intent, AppIntent::None);
    }

    #[test]
    fn composer_ctrl_c_cancels_like_escape() {
        let mut state = actionable_issue_state();
        open_sized_composer(&mut state, "draft", (40, 5));
        press_ctrl(&mut state, 'c');
        assert!(
            state
                .comment_composer
                .as_ref()
                .is_some_and(|composer| composer.confirm_discard),
            "first ctrl-c asks for discard confirmation"
        );
        press_ctrl(&mut state, 'c');
        assert!(state.comment_composer.is_none());
    }

    #[test]
    fn ctrl_a_does_not_open_the_action_menu() {
        let mut state = actionable_issue_state();
        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::CONTROL)),
        );
        assert!(state.action_menu.is_none());
    }

    #[test]
    fn menu_k_and_j_move_the_selection_both_ways() {
        let mut state = actionable_issue_state();
        press(&mut state, KeyCode::Char('A'));
        press(&mut state, KeyCode::Char('j'));
        assert_eq!(state.action_menu.as_ref().map(|m| m.selected), Some(1));
        press(&mut state, KeyCode::Char('k'));
        assert_eq!(state.action_menu.as_ref().map(|m| m.selected), Some(0));
    }

    #[test]
    fn menu_ctrl_c_closes_but_modified_q_does_not() {
        let mut state = actionable_issue_state();
        press(&mut state, KeyCode::Char('A'));
        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::ALT)),
        );
        assert!(state.action_menu.is_some(), "alt-q must not close the menu");
        press_ctrl(&mut state, 'c');
        assert!(state.action_menu.is_none(), "ctrl-c closes the menu");
    }

    #[test]
    fn menu_ignores_modified_navigation_and_activation_keys() {
        let mut state = actionable_issue_state();
        press(&mut state, KeyCode::Char('A'));
        let alt = |code| AppEvent::Key(KeyEvent::new(code, KeyModifiers::ALT));
        apply_event(&mut state, alt(KeyCode::Char('j')));
        assert_eq!(
            state.action_menu.as_ref().map(|m| m.selected),
            Some(0),
            "alt-j must not move the menu selection"
        );
        state.move_action_menu_selection(1);
        apply_event(&mut state, alt(KeyCode::Char('k')));
        assert_eq!(
            state.action_menu.as_ref().map(|m| m.selected),
            Some(1),
            "alt-k must not move the menu selection"
        );
        apply_event(&mut state, alt(KeyCode::Enter));
        assert!(
            state.action_menu.is_some() && state.action_confirm.is_none(),
            "alt-enter must not activate a menu item"
        );
    }

    #[test]
    fn menu_close_and_reopen_shortcuts_open_their_confirms() {
        let mut state = actionable_issue_state();
        press(&mut state, KeyCode::Char('A'));
        press(&mut state, KeyCode::Char('x'));
        assert_eq!(
            state.action_confirm.as_ref().map(|c| c.kind),
            Some(ActionKind::Close)
        );

        let mut state = actionable_issue_state();
        state.resource.state = "CLOSED".into();
        press(&mut state, KeyCode::Char('A'));
        press(&mut state, KeyCode::Char('o'));
        assert_eq!(
            state.action_confirm.as_ref().map(|c| c.kind),
            Some(ActionKind::Reopen)
        );
    }

    #[test]
    fn menu_ignores_shortcuts_for_unavailable_actions() {
        let mut state = actionable_issue_state();
        press(&mut state, KeyCode::Char('A'));
        press(&mut state, KeyCode::Char('m'));
        assert!(
            state.action_confirm.is_none(),
            "merge is not offered on an issue, so 'm' must do nothing"
        );
        assert!(state.action_menu.is_some());
    }

    #[test]
    fn confirm_ctrl_c_closes_but_plain_c_does_not() {
        let mut state = actionable_issue_state();
        state.open_action_confirm(ActionKind::Close);
        press(&mut state, KeyCode::Char('c'));
        assert!(
            state.action_confirm.is_some(),
            "plain c is not a cancel key in the confirm modal"
        );
        press_ctrl(&mut state, 'c');
        assert!(state.action_confirm.is_none());
    }

    #[test]
    fn confirm_ignores_modified_cancel_keys() {
        let mut state = actionable_issue_state();
        state.open_action_confirm(ActionKind::Close);
        apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::ALT)),
        );
        assert!(state.action_confirm.is_some());
    }

    #[test]
    fn merge_confirm_ignores_modified_selection_keys() {
        let mut state = actionable_pr_state();
        state.open_action_confirm(ActionKind::Merge);
        let alt = |code| AppEvent::Key(KeyEvent::new(code, KeyModifiers::ALT));
        apply_event(&mut state, alt(KeyCode::Char('j')));
        assert_eq!(
            state.action_confirm.as_ref().map(|c| c.selected_method),
            Some(0),
            "alt-j must not move the merge method selection"
        );
        state.select_merge_method(1);
        apply_event(&mut state, alt(KeyCode::Char('k')));
        assert_eq!(
            state.action_confirm.as_ref().map(|c| c.selected_method),
            Some(1),
            "alt-k must not move the merge method selection"
        );
        apply_event(&mut state, alt(KeyCode::Char('1')));
        assert_eq!(
            state.action_confirm.as_ref().map(|c| c.selected_method),
            Some(1),
            "alt-1 must not select a merge method"
        );
    }

    #[test]
    fn merge_number_one_selects_the_first_method() {
        let mut state = actionable_pr_state();
        state.open_action_confirm(ActionKind::Merge);
        state.select_merge_method(1);
        press(&mut state, KeyCode::Char('1'));
        assert_eq!(
            state.action_confirm.as_ref().map(|c| c.selected_method),
            Some(0)
        );
    }

    #[test]
    fn wheel_up_scrolls_the_composer_back() {
        let mut state = actionable_issue_state();
        open_sized_composer(&mut state, "1\n2\n3\n4\n5\n6\n7\n8", (10, 2));
        let wheel = |state: &mut AppState, kind: MouseEventKind| {
            apply_event(
                state,
                AppEvent::Mouse(MouseEvent {
                    kind,
                    column: 0,
                    row: 0,
                    modifiers: KeyModifiers::empty(),
                }),
            );
        };
        wheel(&mut state, MouseEventKind::ScrollDown);
        wheel(&mut state, MouseEventKind::ScrollDown);
        assert_eq!(state.comment_composer.as_ref().map(|c| c.scroll), Some(6));
        wheel(&mut state, MouseEventKind::ScrollUp);
        assert_eq!(state.comment_composer.as_ref().map(|c| c.scroll), Some(3));
    }

    #[test]
    fn wheel_does_not_scroll_content_behind_the_action_menu() {
        let mut state = actionable_issue_state();
        state.scroll_limit = 100;
        press(&mut state, KeyCode::Char('A'));
        apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 0,
                row: 0,
                modifiers: KeyModifiers::empty(),
            }),
        );
        assert_eq!(state.scroll, 0);
    }

    #[test]
    fn composer_click_accounts_for_scroll_offset() {
        let mut state = actionable_issue_state();
        open_sized_composer(&mut state, "a\nb\nc\nd\ne\nf", (10, 2));
        if let Some(composer) = &mut state.comment_composer {
            composer.scroll = 2;
        }
        state.hit_areas.push(HitArea::new(
            Rect::new(5, 8, 10, 2),
            HitTarget::ComposerText { x: 5, y: 8 },
        ));
        apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 5,
                row: 9,
                modifiers: KeyModifiers::empty(),
            }),
        );
        assert_eq!(
            composer_cursor(&state),
            (3, 0),
            "clicked visual row 1 plus scroll 2 lands on line 3"
        );
    }

    #[test]
    fn wheel_scrolls_composer_instead_of_content() {
        let mut state = actionable_issue_state();
        state.open_comment_composer();
        apply_event(&mut state, AppEvent::Paste("1\n2\n3\n4\n5\n6\n7\n8".into()));
        if let Some(composer) = &mut state.comment_composer {
            composer.viewport = (10, 2);
        }
        let content_scroll = state.scroll;
        apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 0,
                row: 0,
                modifiers: KeyModifiers::empty(),
            }),
        );
        assert_eq!(state.scroll, content_scroll);
        assert_eq!(state.comment_composer.as_ref().map(|c| c.scroll), Some(3));
    }

    fn state_with_editable_comment() -> AppState {
        let mut state = actionable_issue_state();
        state.resource.body = "original description".into();
        state.resource.activity.push(crate::domain::ActivityEntry {
            id: "IC_1".into(),
            edit: Some(crate::domain::EditTarget {
                node_id: "IC_1".into(),
                kind: crate::domain::EditKind::IssueComment,
                current_body: "my comment".into(),
            }),
            kind: crate::domain::ActivityKind::Comment,
            author: "me".into(),
            body: "my comment".into(),
            updated_at: "now".into(),
            path: None,
            line: None,
            url: None,
            author_association: None,
            reactions: Default::default(),
            includes_created_edit: false,
            is_minimized: false,
            minimized_reason: None,
            thread_id: None,
            thread_resolved: None,
            thread_outdated: None,
        });
        state
    }

    #[test]
    fn menu_edit_shortcut_opens_the_picker() {
        let mut state = state_with_editable_comment();
        press(&mut state, KeyCode::Char('A'));
        press(&mut state, KeyCode::Char('e'));
        let picker = state.edit_picker.as_ref().expect("picker open");
        assert_eq!(picker.choices.len(), 2);
        assert_eq!(picker.choices[0].label, "Description");
        assert!(picker.choices[1].label.starts_with("me: my comment"));
    }

    #[test]
    fn picker_enter_opens_a_prefilled_edit_composer() {
        let mut state = state_with_editable_comment();
        state.open_edit_picker();
        press(&mut state, KeyCode::Down);
        press(&mut state, KeyCode::Enter);
        assert!(state.edit_picker.is_none());
        assert_eq!(composer_body(&state), "my comment");
        assert_eq!(
            state.composer_target.as_ref().map(|t| t.node_id.clone()),
            Some("IC_1".into())
        );
    }

    #[test]
    fn picker_numbers_select_and_modified_keys_are_ignored() {
        let mut state = state_with_editable_comment();
        state.open_edit_picker();
        press(&mut state, KeyCode::Char('2'));
        assert_eq!(state.edit_picker.as_ref().map(|p| p.selected), Some(1));
        let alt = |code| AppEvent::Key(KeyEvent::new(code, KeyModifiers::ALT));
        apply_event(&mut state, alt(KeyCode::Char('1')));
        assert_eq!(
            state.edit_picker.as_ref().map(|p| p.selected),
            Some(1),
            "alt-1 must not move the picker selection"
        );
        apply_event(&mut state, alt(KeyCode::Char('k')));
        assert_eq!(state.edit_picker.as_ref().map(|p| p.selected), Some(1));
        apply_event(&mut state, alt(KeyCode::Enter));
        assert!(
            state.edit_picker.is_some(),
            "alt-enter must not activate a picker choice"
        );
        press(&mut state, KeyCode::Esc);
        assert!(state.edit_picker.is_none());
    }

    #[test]
    fn edit_composer_submits_an_edit_action() {
        let mut state = state_with_editable_comment();
        state.open_edit_picker();
        press(&mut state, KeyCode::Down);
        press(&mut state, KeyCode::Enter);
        press(&mut state, KeyCode::Char('!'));
        let intent = press_ctrl(&mut state, 's');
        assert_eq!(
            intent,
            AppIntent::SubmitAction(ResourceAction::Edit {
                target: crate::domain::EditTarget {
                    node_id: "IC_1".into(),
                    kind: crate::domain::EditKind::IssueComment,
                    current_body: "my comment".into(),
                },
                body: "my comment!".into(),
            })
        );
    }

    #[test]
    fn inline_edit_targets_open_the_right_composer() {
        let mut state = state_with_editable_comment();
        apply_event(
            &mut state,
            AppEvent::Activate(HitTarget::EditActivityEntry {
                node_id: "IC_1".into(),
            }),
        );
        assert_eq!(composer_body(&state), "my comment");
        state.close_comment_composer();

        apply_event(&mut state, AppEvent::Activate(HitTarget::EditResourceBody));
        assert_eq!(composer_body(&state), "original description");
        assert_eq!(
            state.composer_target.as_ref().map(|t| t.kind),
            Some(crate::domain::EditKind::IssueBody)
        );
    }

    #[test]
    fn cancelling_an_edit_clears_the_target() {
        let mut state = state_with_editable_comment();
        apply_event(&mut state, AppEvent::Activate(HitTarget::EditResourceBody));
        press(&mut state, KeyCode::Esc);
        press(&mut state, KeyCode::Esc);
        assert!(state.comment_composer.is_none());
        assert!(state.composer_target.is_none());
    }

    #[test]
    fn edit_draft_is_frozen_while_saving() {
        let mut state = state_with_editable_comment();
        apply_event(&mut state, AppEvent::Activate(HitTarget::EditResourceBody));
        state.pending_action = Some(ResourceAction::Edit {
            target: crate::domain::EditTarget {
                node_id: "I_node".into(),
                kind: crate::domain::EditKind::IssueBody,
                current_body: "original description".into(),
            },
            body: "original description".into(),
        });
        press(&mut state, KeyCode::Char('!'));
        assert_eq!(composer_body(&state), "original description");
    }

    #[test]
    fn description_edits_may_be_submitted_empty_but_comments_may_not() {
        let mut state = state_with_editable_comment();
        apply_event(&mut state, AppEvent::Activate(HitTarget::EditResourceBody));
        if let Some(composer) = &mut state.comment_composer {
            *composer = crate::app::CommentComposer::new();
        }
        let intent = press_ctrl(&mut state, 's');
        assert!(
            matches!(
                intent,
                AppIntent::SubmitAction(ResourceAction::Edit { ref body, .. }) if body.is_empty()
            ),
            "clearing a description is a valid edit: {intent:?}"
        );

        let mut state = state_with_editable_comment();
        state.open_comment_composer();
        let intent = press_ctrl(&mut state, 's');
        assert_eq!(intent, AppIntent::None);
        assert_eq!(state.status_message.as_deref(), Some("comment is empty"));
    }

    #[test]
    fn wheel_does_not_scroll_content_behind_the_edit_picker() {
        let mut state = state_with_editable_comment();
        state.scroll_limit = 100;
        state.open_edit_picker();
        apply_event(
            &mut state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 0,
                row: 0,
                modifiers: KeyModifiers::empty(),
            }),
        );
        assert_eq!(state.scroll, 0);
    }

    #[test]
    fn an_identical_edit_elsewhere_does_not_freeze_or_close_this_composer() {
        let mut state = state_with_editable_comment();
        state.pending_action = Some(ResourceAction::Edit {
            target: crate::domain::EditTarget {
                node_id: "OTHER".into(),
                kind: crate::domain::EditKind::IssueComment,
                current_body: "same text".into(),
            },
            body: "same text".into(),
        });
        apply_event(
            &mut state,
            AppEvent::Activate(HitTarget::EditActivityEntry {
                node_id: "IC_1".into(),
            }),
        );
        if let Some(composer) = &mut state.comment_composer {
            *composer = crate::app::CommentComposer::new();
            composer.insert_str("same text");
        }
        press(&mut state, KeyCode::Char('!'));
        assert_eq!(
            composer_body(&state),
            "same text!",
            "a different edit target with identical text must stay editable"
        );
    }
}

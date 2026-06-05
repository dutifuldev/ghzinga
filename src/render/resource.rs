use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
        StatefulWidget, Widget,
    },
    Frame,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{
    app::{AppState, BlockId, Tab},
    domain::{ActivityEntry, CheckRun, CheckStatus, Commit, MetadataItem, Resource, ResourceId},
    input::{HitArea, HitTarget},
    render::{
        markdown, time::compact_relative_time, time::relative_time_phrase, ContentWidthMode,
        Palette, ScrollbarMode, SpacingMode, SymbolMode, Symbols, ThemeName, ViewRects,
    },
};

struct ContentRow {
    line: Line<'static>,
    target: Option<HitTarget>,
    comfortable_gap_after: bool,
    activity_focus: Option<String>,
}

struct TimelineEntry<'a> {
    sort_key: Option<String>,
    sequence: usize,
    kind_order: u8,
    item: TimelineItem<'a>,
}

enum TimelineItem<'a> {
    Body,
    Commit(&'a Commit),
    Activity(&'a ActivityEntry),
}

#[derive(Clone)]
struct StyledPiece {
    segments: Vec<StyledSegment>,
}

#[derive(Clone)]
struct StyledSegment {
    text: String,
    style: Style,
}

impl StyledPiece {
    fn plain_text(&self) -> String {
        self.segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect()
    }

    fn display_width(&self) -> usize {
        self.segments
            .iter()
            .map(|segment| UnicodeWidthStr::width(segment.text.as_str()))
            .sum()
    }

    fn first_style(&self) -> Style {
        self.segments
            .first()
            .map(|segment| segment.style)
            .unwrap_or_default()
    }
}

#[derive(Clone)]
struct FooterItem {
    label: String,
    style: Style,
    target: Option<HitTarget>,
}

struct FooterLine {
    items: Vec<FooterItem>,
}

struct CheckGroupRenderContext<'a> {
    expanded_blocks: &'a std::collections::HashSet<BlockId>,
    width: usize,
    palette: &'a Palette,
    symbols: &'a Symbols,
}

const BODY_COLLAPSED_LINES: usize = 12;
const ACTIVITY_COLLAPSED_LINES: usize = 8;
const PATCH_COLLAPSED_ROWS: usize = 18;
const COMFORTABLE_GUTTER: u16 = 2;
const COMFORTABLE_MIN_CAP_WIDTH: u16 =
    crate::render::DEFAULT_FIXED_CONTENT_WIDTH + (COMFORTABLE_GUTTER * 2) + 12;

impl ContentRow {
    fn plain(text: impl Into<String>) -> Self {
        Self {
            line: Line::from(text.into()),
            target: None,
            comfortable_gap_after: false,
            activity_focus: None,
        }
    }

    fn styled(text: impl Into<String>, style: Style) -> Self {
        Self {
            line: Line::from(Span::styled(text.into(), style)),
            target: None,
            comfortable_gap_after: false,
            activity_focus: None,
        }
    }

    fn target(text: impl Into<String>, target: HitTarget) -> Self {
        Self {
            line: Line::from(text.into()),
            target: Some(target),
            comfortable_gap_after: false,
            activity_focus: None,
        }
    }

    fn target_styled(text: impl Into<String>, target: HitTarget, style: Style) -> Self {
        Self {
            line: Line::from(Span::styled(text.into(), style)),
            target: Some(target),
            comfortable_gap_after: false,
            activity_focus: None,
        }
    }

    fn target_line(line: Line<'static>, target: HitTarget) -> Self {
        Self {
            line,
            target: Some(target),
            comfortable_gap_after: false,
            activity_focus: None,
        }
    }

    fn with_comfortable_gap_after(mut self) -> Self {
        self.comfortable_gap_after = true;
        self
    }

    fn with_activity_focus(mut self, id: impl Into<String>) -> Self {
        self.activity_focus = Some(id.into());
        self
    }
}

pub fn render_app(frame: &mut Frame<'_>, state: &mut AppState) {
    let mut rects = rects_for_spacing(frame.area(), state.spacing);
    let palette = state.theme.palette();
    state.hit_areas.clear();
    frame.buffer_mut().set_style(
        rects.area,
        Style::default().fg(palette.text).bg(palette.panel_bg),
    );
    let resource_tabs_area = resource_tabs_area(&mut rects.header, state);
    let show_header_add_button =
        resource_tabs_area.is_none() && single_resource_add_button_visible(rects.header);
    let header_right_reserved = if show_header_add_button && rects.header.width >= 48 {
        add_resource_button_width()
            .min(rects.header.width)
            .saturating_add(1)
    } else {
        0
    };
    render_header(
        frame,
        chrome_area_for_spacing(rects.header, state.spacing),
        state,
        state.spacing,
        &palette,
        header_right_reserved,
    );
    if let Some(area) = resource_tabs_area {
        render_resource_tabs(
            frame,
            chrome_area_for_spacing(area, state.spacing),
            state,
            &palette,
        );
    } else if show_header_add_button {
        render_header_add_button(frame, rects.header, state, &palette);
    }
    render_status(
        frame,
        chrome_area_for_spacing(rects.status, state.spacing),
        state,
        &palette,
    );
    render_tabs(
        frame,
        chrome_area_for_spacing(rects.tabs, state.spacing),
        state,
        &palette,
        state.spacing,
    );
    let content_area = content_area_for_state(rects.content, state, active_content_tab(state));
    render_content(frame, rects.content, state, &palette);
    render_footer(
        frame,
        chrome_area_for_spacing(rects.footer, state.spacing),
        state,
        &palette,
        content_area.width as usize,
    );
    if state.add_resource_prompt.is_some() {
        render_add_resource_modal(frame, rects.area, state, &palette);
    } else if state.resource_link_prompt.is_some() {
        render_resource_link_modal(frame, rects.area, state, &palette);
    }
}

fn resource_tabs_area(header: &mut Rect, state: &AppState) -> Option<Rect> {
    if !state.resource_tab_bar_visible() || header.height < 2 {
        return None;
    }
    let area = Rect::new(header.x, header.y, header.width, 1);
    let consumed = if header.height >= 3 { 2 } else { 1 };
    header.y = header.y.saturating_add(consumed);
    header.height = header.height.saturating_sub(consumed);
    Some(area)
}

fn single_resource_add_button_visible(header: Rect) -> bool {
    header.width > 0 && header.height > 0
}

fn render_resource_tabs(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &mut AppState,
    palette: &Palette,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    Paragraph::new(" ".repeat(area.width as usize))
        .style(Style::default().fg(palette.text).bg(palette.surface0))
        .render(area, frame.buffer_mut());

    let add_label = add_resource_button_label();
    let add_width = (UnicodeWidthStr::width(add_label.as_str()) as u16)
        .min(area.width)
        .max(1);
    let add_x = area
        .x
        .saturating_add(area.width.saturating_sub(add_width.max(1)));
    let tab_right = add_x.saturating_sub(1);
    let start_index = visible_resource_tab_start(state, tab_right.saturating_sub(area.x));
    let mut x = area.x;
    let mut spans = Vec::<Span<'static>>::new();
    for index in start_index..state.resource_tabs.len() {
        if x >= tab_right {
            break;
        }
        let Some(label) = state.active_resource_tab_label(index) else {
            continue;
        };
        let active = index == state.active_resource_tab;
        let remaining = tab_right.saturating_sub(x);
        let desired_width = resource_tab_width(&label);
        let width = desired_width.min(remaining);
        if width < 4 {
            break;
        }
        let label_width = width.saturating_sub(4) as usize;
        let tab_text = format!(" {} × ", truncate_display(&label, label_width));
        let tab_text = fit_label_to_width(&tab_text, width);
        let style = if active {
            Style::default()
                .fg(palette.panel_bg)
                .bg(palette.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette.text).bg(palette.surface1)
        };
        state.hit_areas.push(HitArea::new(
            Rect::new(x, area.y, width, 1),
            HitTarget::ResourceTab(index),
        ));
        state.hit_areas.push(HitArea::new(
            Rect::new(x.saturating_add(width.saturating_sub(3)), area.y, 3, 1),
            HitTarget::CloseResourceTab(index),
        ));
        spans.push(Span::styled(tab_text, style));
        if x.saturating_add(width) < tab_right {
            spans.push(Span::styled(" ", Style::default().bg(palette.surface0)));
        }
        x = x.saturating_add(width.saturating_add(1));
    }
    Paragraph::new(Line::from(spans))
        .style(Style::default().fg(palette.text).bg(palette.surface0))
        .render(
            Rect::new(area.x, area.y, tab_right.saturating_sub(area.x), 1),
            frame.buffer_mut(),
        );
    render_add_resource_button(
        frame,
        Rect::new(add_x, area.y, add_width, 1),
        state,
        palette,
    );
}

fn visible_resource_tab_start(state: &AppState, available_width: u16) -> usize {
    let len = state.resource_tabs.len();
    if len == 0 || available_width < 4 {
        return 0;
    }
    let active = state.active_resource_tab.min(len.saturating_sub(1));
    let mut start = active;
    let mut used = 0_u16;
    for index in (0..=active).rev() {
        let Some(label) = state.active_resource_tab_label(index) else {
            continue;
        };
        let width = resource_tab_width(&label).min(available_width);
        if width < 4 {
            break;
        }
        let needed = width.saturating_add(u16::from(used > 0));
        if used.saturating_add(needed) > available_width {
            break;
        }
        start = index;
        used = used.saturating_add(needed);
    }
    start
}

fn resource_tab_width(label: &str) -> u16 {
    (UnicodeWidthStr::width(label) as u16)
        .saturating_add(6)
        .clamp(10, 32)
}

fn render_header_add_button(
    frame: &mut Frame<'_>,
    header_area: Rect,
    state: &mut AppState,
    palette: &Palette,
) {
    if header_area.width == 0 || header_area.height == 0 {
        return;
    }
    let width = add_resource_button_width().min(header_area.width).max(1);
    if width == 0 {
        return;
    }
    let row_offset = if header_area.width < 48 {
        header_area.height.saturating_sub(1)
    } else {
        header_top_padding_rows(header_area, state.spacing) as u16
    };
    let row = header_area.y.saturating_add(row_offset).min(
        header_area
            .y
            .saturating_add(header_area.height.saturating_sub(1)),
    );
    let rect = Rect::new(
        header_area
            .x
            .saturating_add(header_area.width.saturating_sub(width)),
        row,
        width,
        1,
    );
    render_add_resource_button(frame, rect, state, palette);
}

fn render_add_resource_button(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &mut AppState,
    palette: &Palette,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let label = add_resource_button_label_for_width(area.width);
    Paragraph::new(label)
        .style(
            Style::default()
                .fg(palette.panel_bg)
                .bg(palette.accent)
                .add_modifier(Modifier::BOLD),
        )
        .render(area, frame.buffer_mut());
    state
        .hit_areas
        .push(HitArea::new(area, HitTarget::OpenResourcePrompt));
}

fn add_resource_button_label() -> String {
    "[+]".to_string()
}

fn add_resource_button_label_for_width(width: u16) -> String {
    if width >= add_resource_button_width() {
        "[+]".to_string()
    } else {
        "+".to_string()
    }
}

fn add_resource_button_width() -> u16 {
    3
}

fn render_add_resource_modal(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &mut AppState,
    palette: &Palette,
) {
    let Some(prompt) = state.add_resource_prompt.as_ref() else {
        return;
    };
    if area.width == 0 || area.height == 0 {
        return;
    }
    let modal_width = area.width.min(area.width.saturating_sub(4).clamp(24, 68));
    let modal_height = area.height.min(area.height.saturating_sub(2).clamp(7, 9));
    let modal = Rect::new(
        area.x
            .saturating_add(area.width.saturating_sub(modal_width) / 2),
        area.y
            .saturating_add(area.height.saturating_sub(modal_height) / 2),
        modal_width,
        modal_height,
    );
    frame.render_widget(Clear, modal);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(palette.accent).bg(palette.surface0))
        .style(Style::default().fg(palette.text).bg(palette.surface0));
    frame.render_widget(block, modal);
    let inner = Rect::new(
        modal.x.saturating_add(1),
        modal.y.saturating_add(1),
        modal.width.saturating_sub(2),
        modal.height.saturating_sub(2),
    );
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    state
        .hit_areas
        .push(HitArea::new(area, HitTarget::ModalOverlay));
    let mut rows = Vec::<Line<'static>>::new();
    rows.push(Line::from(Span::styled(
        "Open PR or issue",
        Style::default()
            .fg(palette.text)
            .bg(palette.surface0)
            .add_modifier(Modifier::BOLD),
    )));
    rows.push(Line::from(Span::styled(
        "URL, owner/repo#123, owner/repo 123, or #123",
        dim_style(palette).bg(palette.surface0),
    )));
    rows.push(Line::from(""));
    let input_width = inner.width.saturating_sub(2) as usize;
    rows.push(Line::from(Span::styled(
        format!(
            " {}",
            truncate_display(&format!("{}█", prompt.input), input_width)
        ),
        Style::default().fg(palette.text).bg(palette.panel_bg),
    )));
    let detail = prompt
        .error
        .as_deref()
        .unwrap_or("Enter opens, Esc cancels");
    let detail_style = if prompt.error.is_some() {
        Style::default()
            .fg(palette.red)
            .bg(palette.surface0)
            .add_modifier(Modifier::BOLD)
    } else {
        dim_style(palette).bg(palette.surface0)
    };
    rows.push(Line::from(Span::styled(
        truncate_display(detail, inner.width as usize),
        detail_style,
    )));
    while rows.len() + 1 < inner.height as usize {
        rows.push(Line::from(""));
    }
    rows.push(Line::from(vec![
        Span::styled(
            "[open]",
            Style::default()
                .fg(palette.panel_bg)
                .bg(palette.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", Style::default().bg(palette.surface0)),
        Span::styled(
            "[cancel]",
            Style::default()
                .fg(palette.text)
                .bg(palette.surface1)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    Paragraph::new(rows)
        .style(Style::default().fg(palette.text).bg(palette.surface0))
        .render(inner, frame.buffer_mut());

    let button_y = inner.y.saturating_add(inner.height.saturating_sub(1));
    state.hit_areas.push(HitArea::new(
        Rect::new(inner.x, button_y, 6_u16.min(inner.width), 1),
        HitTarget::ConfirmResourcePrompt,
    ));
    if inner.width >= 10 {
        state.hit_areas.push(HitArea::new(
            Rect::new(inner.x.saturating_add(8), button_y, 8, 1),
            HitTarget::CancelResourcePrompt,
        ));
    }
}

fn render_resource_link_modal(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &mut AppState,
    palette: &Palette,
) {
    let Some(prompt) = state.resource_link_prompt.as_ref() else {
        return;
    };
    if area.width == 0 || area.height == 0 {
        return;
    }
    let modal_width = area.width.min(area.width.saturating_sub(4).clamp(28, 72));
    let modal_height = area.height.min(area.height.saturating_sub(2).clamp(7, 10));
    let modal = Rect::new(
        area.x
            .saturating_add(area.width.saturating_sub(modal_width) / 2),
        area.y
            .saturating_add(area.height.saturating_sub(modal_height) / 2),
        modal_width,
        modal_height,
    );
    frame.render_widget(Clear, modal);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(palette.accent).bg(palette.surface0))
        .style(Style::default().fg(palette.text).bg(palette.surface0));
    frame.render_widget(block, modal);
    let inner = Rect::new(
        modal.x.saturating_add(1),
        modal.y.saturating_add(1),
        modal.width.saturating_sub(2),
        modal.height.saturating_sub(2),
    );
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    state
        .hit_areas
        .push(HitArea::new(area, HitTarget::ModalOverlay));

    let mut rows = Vec::<Line<'static>>::new();
    rows.push(Line::from(Span::styled(
        "Open linked resource",
        Style::default()
            .fg(palette.text)
            .bg(palette.surface0)
            .add_modifier(Modifier::BOLD),
    )));
    rows.push(Line::from(Span::styled(
        truncate_display(&prompt.id.canonical_name(), inner.width as usize),
        link_style(palette).bg(palette.surface0),
    )));
    if let Some(url) = &prompt.url {
        rows.push(Line::from(Span::styled(
            truncate_display(url, inner.width as usize),
            dim_style(palette).bg(palette.surface0),
        )));
    }
    rows.push(Line::from(Span::styled(
        "Enter opens here, n opens in a new tab",
        dim_style(palette).bg(palette.surface0),
    )));
    while rows.len() + 1 < inner.height as usize {
        rows.push(Line::from(""));
    }
    rows.push(Line::from(vec![
        Span::styled(
            "[here]",
            Style::default()
                .fg(palette.panel_bg)
                .bg(palette.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", Style::default().bg(palette.surface0)),
        Span::styled(
            "[new tab]",
            Style::default()
                .fg(palette.panel_bg)
                .bg(palette.green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", Style::default().bg(palette.surface0)),
        Span::styled(
            "[cancel]",
            Style::default()
                .fg(palette.text)
                .bg(palette.surface1)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    Paragraph::new(rows)
        .style(Style::default().fg(palette.text).bg(palette.surface0))
        .render(inner, frame.buffer_mut());

    let button_y = inner.y.saturating_add(inner.height.saturating_sub(1));
    let mut x = inner.x;
    state.hit_areas.push(HitArea::new(
        Rect::new(x, button_y, 6_u16.min(inner.width), 1),
        HitTarget::OpenLinkHere,
    ));
    x = x.saturating_add(8);
    if inner.width >= 17 {
        state.hit_areas.push(HitArea::new(
            Rect::new(x, button_y, 9, 1),
            HitTarget::OpenLinkInNewTab,
        ));
    }
    x = x.saturating_add(11);
    if inner.width >= 27 {
        state.hit_areas.push(HitArea::new(
            Rect::new(x, button_y, 8, 1),
            HitTarget::CancelResourceLinkPrompt,
        ));
    }
}

fn rects_for_spacing(area: Rect, spacing: SpacingMode) -> ViewRects {
    let mut rects = ViewRects::compute(area);
    if spacing == SpacingMode::Comfortable && area.width >= 48 && rects.content.height > 2 {
        if rects.content.height > 3 {
            rects.header.height = rects.header.height.saturating_add(1);
            rects.status.y = rects.status.y.saturating_add(1);
            rects.tabs.y = rects.tabs.y.saturating_add(1);
            rects.content.y = rects.content.y.saturating_add(1);
            rects.content.height = rects.content.height.saturating_sub(1);
        }
        if area.height >= 32 && rects.content.height > 3 {
            rects.footer.height = rects.footer.height.saturating_add(1);
            rects.footer.y = rects.footer.y.saturating_sub(1);
            rects.content.height = rects.content.height.saturating_sub(1);
        }
        rects.tabs.height = rects.tabs.height.saturating_add(2);
        rects.content.y = rects.content.y.saturating_add(2);
        rects.content.height = rects.content.height.saturating_sub(2);
    }
    rects
}

fn render_header(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &mut AppState,
    spacing: SpacingMode,
    palette: &Palette,
    right_reserved: u16,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let resource = &state.resource;
    let kind = resource.kind();
    let mut header = Vec::new();
    let width = area.width.saturating_sub(right_reserved).max(1) as usize;
    let content_rows = area.height.saturating_sub(1) as usize;
    let updated = format!("updated {}", compact_relative_time(&resource.updated_at));
    let state_label = format!("[{} {}]", kind, resource.state);
    let top_padding = header_top_padding_rows(area, spacing);
    let usable_rows = content_rows.saturating_sub(top_padding);
    let header_style = header_block_style(palette);
    let updated_in_meta =
        (usable_rows > 0 && usable_rows < 3 && width >= 56).then_some(updated.as_str());
    for _ in 0..top_padding {
        header.push(Line::from("").style(header_style));
    }
    if usable_rows > 0 {
        header.push(header_meta_line(
            resource,
            &state_label,
            updated_in_meta,
            width,
            palette,
        ));
    }
    if usable_rows >= 3 {
        header.push(
            Line::from(Span::styled(
                truncate_display(&updated, width),
                dim_style(palette).bg(header_background(palette)),
            ))
            .style(header_style),
        );
    }
    let title_rows = content_rows.saturating_sub(header.len());
    if title_rows > 0 {
        let mut title_lines = markdown::wrap_plain_text(&resource.title, width);
        title_lines.truncate(title_rows);
        for title in title_lines {
            header.push(
                Line::from(Span::styled(title, header_title_style(palette))).style(header_style),
            );
        }
    }
    while header.len() + 1 < area.height as usize {
        header.push(Line::from("").style(header_style));
    }
    header.push(separator_line(area.width, palette));
    register_header_identity_hit_area(
        state,
        area,
        width,
        &state_label,
        updated_in_meta,
        top_padding,
    );
    Paragraph::new(header)
        .style(Style::default().fg(palette.text).bg(palette.panel_bg))
        .render(area, frame.buffer_mut());
}

fn header_background(palette: &Palette) -> Color {
    palette.surface0
}

fn header_block_style(palette: &Palette) -> Style {
    Style::default()
        .fg(palette.text)
        .bg(header_background(palette))
}

fn header_title_style(palette: &Palette) -> Style {
    header_block_style(palette).add_modifier(Modifier::BOLD)
}

fn header_top_padding_rows(area: Rect, spacing: SpacingMode) -> usize {
    usize::from(spacing == SpacingMode::Comfortable && area.width >= 48 && area.height >= 4)
}

fn register_header_identity_hit_area(
    state: &mut AppState,
    area: Rect,
    width: usize,
    state_label: &str,
    updated: Option<&str>,
    top_padding: usize,
) {
    if area.width == 0 || area.height <= 1 {
        return;
    }
    let label = header_identity_label(&state.resource, state_label, updated, width);
    let label_width = header_identity_width(&label, state_label, updated, width);
    if label_width == 0 {
        return;
    }
    state.hit_areas.push(HitArea::new(
        Rect::new(
            area.x,
            area.y.saturating_add(top_padding as u16),
            label_width.min(area.width),
            1,
        ),
        HitTarget::OpenHeaderUrl(state.resource.web_url()),
    ));
}

fn header_identity_width(
    label: &str,
    state_label: &str,
    updated: Option<&str>,
    width: usize,
) -> u16 {
    let state_width = UnicodeWidthStr::width(state_label);
    let updated_width = updated.map(UnicodeWidthStr::width).unwrap_or_default();
    let separators = 1 + usize::from(updated.is_some());
    let reserved = state_width
        .saturating_add(updated_width)
        .saturating_add(separators);
    let id_width = width.saturating_sub(reserved).max(1);
    UnicodeWidthStr::width(truncate_display(label, id_width).as_str()) as u16
}

fn header_identity_available_width(
    state_label: &str,
    updated: Option<&str>,
    width: usize,
) -> usize {
    let state_width = UnicodeWidthStr::width(state_label);
    let updated_width = updated.map(UnicodeWidthStr::width).unwrap_or_default();
    let separators = 1 + usize::from(updated.is_some());
    width
        .saturating_sub(
            state_width
                .saturating_add(updated_width)
                .saturating_add(separators),
        )
        .max(1)
}

fn header_identity_label(
    resource: &Resource,
    state_label: &str,
    updated: Option<&str>,
    width: usize,
) -> String {
    let url = resource.web_url();
    let available = header_identity_available_width(state_label, updated, width);
    if UnicodeWidthStr::width(url.as_str()) <= available {
        url
    } else {
        format!(
            "{} / {} #{}",
            resource.id.owner, resource.id.repo, resource.id.number
        )
    }
}

fn header_meta_line(
    resource: &Resource,
    state_label: &str,
    updated: Option<&str>,
    width: usize,
    palette: &Palette,
) -> Line<'static> {
    let label = header_identity_label(resource, state_label, updated, width);
    let id_width = usize::from(header_identity_width(&label, state_label, updated, width)).max(1);
    let mut spans = vec![
        Span::styled(
            truncate_display(&label, id_width),
            Style::default()
                .fg(palette.accent)
                .bg(header_background(palette))
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ),
        Span::styled(" ", header_block_style(palette)),
        Span::styled(
            state_label.to_string(),
            resource_state_style(resource, palette)
                .bg(header_background(palette))
                .add_modifier(Modifier::BOLD),
        ),
    ];
    if let Some(updated) = updated {
        spans.push(Span::styled(" ", header_block_style(palette)));
        spans.push(Span::styled(
            truncate_display(updated, width),
            dim_style(palette).bg(header_background(palette)),
        ));
    }
    Line::from(spans).style(header_block_style(palette))
}

fn render_tabs(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &mut AppState,
    palette: &Palette,
    spacing: SpacingMode,
) {
    let mut lines = Vec::<Line<'static>>::new();
    let mut spans = Vec::<Span<'static>>::new();
    let mut x = area.x;
    let mut y = area.y;
    let framed_nav = comfortable_nav_frame_enabled(area, spacing);
    let tab_rows = if framed_nav {
        area.height.saturating_sub(2)
    } else {
        area.height
    };
    let tab_bottom = area.y.saturating_add(tab_rows);
    let symbols = state.symbols.symbols();
    for tab in state.tabs() {
        let raw_label = if *tab == state.active_tab {
            format!("[{}]", tab_label(*tab, &symbols))
        } else {
            format!(" {} ", tab_label(*tab, &symbols))
        };
        let raw_width = UnicodeWidthStr::width(raw_label.as_str()) as u16;
        if x > area.x && x.saturating_add(raw_width) > area.x.saturating_add(area.width) {
            lines.push(Line::from(spans));
            spans = Vec::new();
            x = area.x;
            y = y.saturating_add(1);
        }
        if y >= tab_bottom {
            break;
        }
        let label = fit_label_to_width(&raw_label, area.width);
        let width = UnicodeWidthStr::width(label.as_str()) as u16;
        if width == 0 {
            continue;
        }
        state.hit_areas.push(HitArea::new(
            Rect::new(x, y, width, 1),
            HitTarget::Tab(*tab),
        ));
        x = x.saturating_add(width);
        let style = if *tab == state.active_tab {
            Style::default()
                .fg(palette.panel_bg)
                .bg(palette.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette.subtext0)
        };
        spans.push(Span::styled(label, style));
    }
    lines.push(Line::from(spans));
    if framed_nav {
        while lines.len() < tab_rows as usize {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(""));
        lines.push(separator_line(area.width, palette));
    }
    Paragraph::new(lines)
        .style(Style::default().fg(palette.text).bg(palette.panel_bg))
        .render(area, frame.buffer_mut());
}

fn tab_label(tab: Tab, symbols: &Symbols) -> String {
    let icon = match tab {
        Tab::Overview => symbols.tab_overview,
        Tab::Activity => symbols.tab_activity,
        Tab::Commits => symbols.tab_commits,
        Tab::Checks => symbols.tab_checks,
        Tab::Files => symbols.tab_files,
        Tab::Links => symbols.tab_links,
    };
    if icon.is_empty() {
        tab.label().to_string()
    } else {
        format!("{icon} {}", tab.label())
    }
}

fn comfortable_nav_frame_enabled(area: Rect, spacing: SpacingMode) -> bool {
    spacing == SpacingMode::Comfortable && area.width >= 48 && area.height >= 3
}

fn render_status(frame: &mut Frame<'_>, area: Rect, state: &AppState, palette: &Palette) {
    let resource = &state.resource;
    let symbols = state.symbols.symbols();
    let mut pieces = Vec::new();
    pieces.push(StyledPiece {
        segments: vec![StyledSegment {
            text: format!(
                "{} {}",
                resource_state_symbol(resource, &symbols),
                resource.state
            ),
            style: resource_state_badge_style(resource, palette),
        }],
    });
    push_status_piece(
        &mut pieces,
        format!("{} @{}", symbols.author, resource.author),
        Style::default().fg(palette.subtext0),
    );
    if resource.is_pull_request() {
        if let Some(pr) = &resource.pull_request {
            push_status_piece(
                &mut pieces,
                branch_summary(pr),
                Style::default().fg(palette.accent),
            );
        }
        push_status_piece(
            &mut pieces,
            format!(
                "{} checks {}",
                checks_symbol(resource, &symbols),
                checks_summary(resource)
            ),
            checks_badge_style(resource, palette),
        );
        if let Some(pr) = &resource.pull_request {
            pieces.push(changed_files_status_piece(
                symbols.files,
                pr.files.len(),
                pr.additions,
                pr.deletions,
                palette,
            ));
        }
    }

    let detail = status_detail_line(state, &symbols);
    let detail_rows = usize::from(area.height > 1);
    let summary_rows = (area.height as usize).saturating_sub(detail_rows).max(1);
    let mut lines = fit_lines_to_height(
        wrap_styled_pieces(&pieces, area.width as usize),
        summary_rows,
        area.width as usize,
        palette,
    );
    if area.height > 1 {
        lines.extend(status_detail_lines(
            detail.as_deref(),
            state.last_error.is_some(),
            area.width as usize,
            detail_rows,
            palette,
        ));
    }
    if lines.is_empty() {
        lines.push(separator_line(area.width, palette));
    }
    Paragraph::new(lines)
        .style(Style::default().fg(palette.text).bg(palette.panel_bg))
        .render(area, frame.buffer_mut());
}

fn push_status_piece(pieces: &mut Vec<StyledPiece>, text: String, style: Style) {
    pieces.push(StyledPiece {
        segments: vec![StyledSegment { text, style }],
    });
}

fn changed_files_status_piece(
    files_symbol: &str,
    file_count: usize,
    additions: u64,
    deletions: u64,
    palette: &Palette,
) -> StyledPiece {
    let label_style = Style::default()
        .fg(palette.teal)
        .add_modifier(Modifier::BOLD);
    StyledPiece {
        segments: vec![
            StyledSegment {
                text: format!("{} changed ", file_count_summary(files_symbol, file_count)),
                style: label_style,
            },
            StyledSegment {
                text: format!("+{}", compact_count(additions)),
                style: Style::default()
                    .fg(palette.green)
                    .add_modifier(Modifier::BOLD),
            },
            StyledSegment {
                text: " ".into(),
                style: label_style,
            },
            StyledSegment {
                text: format!("-{}", compact_count(deletions)),
                style: Style::default()
                    .fg(palette.red)
                    .add_modifier(Modifier::BOLD),
            },
        ],
    }
}

fn compact_count(value: u64) -> String {
    if value < 1_000 {
        return value.to_string();
    }
    let tenths = (value + 50) / 100;
    let whole = tenths / 10;
    let fraction = tenths % 10;
    if fraction == 0 {
        format!("{whole}k")
    } else {
        format!("{whole}.{fraction}k")
    }
}

fn status_detail_line(state: &AppState, symbols: &Symbols) -> Option<String> {
    if let Some(error) = &state.last_error {
        return Some(format!("{} {error}", symbols.error));
    }
    if let Some(message) = loading_status_text(state) {
        return Some(message);
    }
    if let Some(message) = &state.status_message {
        return Some(format!("{} {message}", symbols.info));
    }
    let mut details = Vec::new();
    if let Some(refresh) = refresh_summary(state) {
        details.push(format!("{} {refresh}", symbols.refresh));
    }
    if let Some(changes) = refresh_changes_summary(state) {
        details.push(format!("{} {changes}", symbols.changed));
    }
    if !details.is_empty() {
        return Some(details.join("  "));
    }
    None
}

fn loading_status_text(state: &AppState) -> Option<String> {
    let message = state.loading_message()?;
    let indicator = state.loading_indicator().unwrap_or("|");
    Some(format!("Loading {indicator}: {message}"))
}

fn branch_summary(pr: &crate::domain::PullRequest) -> String {
    let head = if pr.head_ref.trim().is_empty() {
        "head"
    } else {
        pr.head_ref.as_str()
    };
    let base = if pr.base_ref.trim().is_empty() {
        "base"
    } else {
        pr.base_ref.as_str()
    };
    format!("{head} -> {base}")
}

fn file_count_summary(symbol: &str, count: usize) -> String {
    let noun = if count == 1 { "file" } else { "files" };
    if symbol == "files" {
        format!("{count} {noun}")
    } else {
        format!("{symbol} {count} {noun}")
    }
}

fn status_detail_lines(
    detail: Option<&str>,
    is_error: bool,
    width: usize,
    max_rows: usize,
    palette: &Palette,
) -> Vec<Line<'static>> {
    if max_rows == 0 {
        return Vec::new();
    }
    let Some(detail) = detail else {
        return vec![Line::from("")];
    };
    let style = if is_error {
        Style::default()
            .fg(palette.red)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(palette.subtext0)
    };
    fit_lines_to_height(
        markdown::wrap_plain_text(detail, width)
            .into_iter()
            .map(|line| Line::from(Span::styled(line, style)))
            .collect(),
        max_rows,
        width,
        palette,
    )
}

fn fit_lines_to_height(
    mut lines: Vec<Line<'static>>,
    max_rows: usize,
    width: usize,
    palette: &Palette,
) -> Vec<Line<'static>> {
    if lines.len() <= max_rows {
        return lines;
    }
    lines.truncate(max_rows);
    if let Some(last) = lines.last_mut() {
        if UnicodeWidthStr::width(line_text(last).as_str()).saturating_add(4) <= width {
            last.spans.push(Span::styled(
                " ...",
                Style::default()
                    .fg(palette.overlay1)
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }
    lines
}

fn wrap_styled_pieces(pieces: &[StyledPiece], width: usize) -> Vec<Line<'static>> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut spans = Vec::new();
    let mut current_width = 0;
    for piece in pieces {
        let piece_width = piece.display_width();
        if piece_width > width {
            if current_width > 0 {
                lines.push(Line::from(spans));
                spans = Vec::new();
                current_width = 0;
            }
            let style = piece.first_style();
            for segment in markdown::wrap_display_text(&piece.plain_text(), width) {
                lines.push(Line::from(Span::styled(
                    truncate_display(&segment, width),
                    style,
                )));
            }
            continue;
        }
        let separator_width = usize::from(current_width > 0) * 2;
        if current_width > 0 && current_width + separator_width + piece_width > width {
            lines.push(Line::from(spans));
            spans = Vec::new();
            current_width = 0;
        }
        if current_width > 0 {
            spans.push(Span::raw("  "));
            current_width += 2;
        }
        for segment in &piece.segments {
            current_width += UnicodeWidthStr::width(segment.text.as_str());
            spans.push(Span::styled(segment.text.clone(), segment.style));
        }
    }
    if !spans.is_empty() {
        lines.push(Line::from(spans));
    }
    lines
}

fn resource_state_symbol(resource: &Resource, symbols: &Symbols) -> &'static str {
    match resource.state.to_ascii_uppercase().as_str() {
        "OPEN" => symbols.state_open,
        "MERGED" => symbols.state_merged,
        "CLOSED" => symbols.state_closed,
        _ => symbols.state_unknown,
    }
}

fn resource_state_style(resource: &Resource, palette: &Palette) -> Style {
    let color = match resource.state.to_ascii_uppercase().as_str() {
        "OPEN" => palette.green,
        "MERGED" => palette.accent,
        "CLOSED" => palette.red,
        "LOADING" => palette.yellow,
        _ => palette.subtext0,
    };
    Style::default().fg(color)
}

fn resource_state_badge_style(resource: &Resource, palette: &Palette) -> Style {
    let color = match resource.state.to_ascii_uppercase().as_str() {
        "OPEN" => palette.green,
        "MERGED" => palette.accent,
        "CLOSED" => palette.red,
        "LOADING" => palette.yellow,
        _ => palette.surface1,
    };
    Style::default()
        .fg(palette.panel_bg)
        .bg(color)
        .add_modifier(Modifier::BOLD)
}

fn checks_symbol(resource: &Resource, symbols: &Symbols) -> &'static str {
    match summarize_checks(resource).as_str() {
        "PASS" => symbols.checks_pass,
        "FAIL" => symbols.checks_fail,
        "PENDING" => symbols.checks_pending,
        _ => symbols.checks_unknown,
    }
}

fn checks_badge_style(resource: &Resource, palette: &Palette) -> Style {
    let color = match summarize_checks(resource).as_str() {
        "PASS" => palette.green,
        "FAIL" => palette.red,
        "PENDING" => palette.yellow,
        _ => palette.surface1,
    };
    Style::default()
        .fg(palette.panel_bg)
        .bg(color)
        .add_modifier(Modifier::BOLD)
}

fn check_status_symbol(status: CheckStatus, symbols: &Symbols) -> &'static str {
    match status {
        CheckStatus::Success => symbols.check_success,
        CheckStatus::Failure => symbols.check_failure,
        CheckStatus::Pending => symbols.check_pending,
        CheckStatus::Skipped => symbols.check_skipped,
        CheckStatus::Neutral => symbols.check_neutral,
        CheckStatus::Unknown => symbols.check_unknown,
    }
}

fn check_status_style(status: CheckStatus, palette: &Palette) -> Style {
    let color = match status {
        CheckStatus::Success => palette.green,
        CheckStatus::Failure => palette.red,
        CheckStatus::Pending => palette.yellow,
        CheckStatus::Skipped => palette.subtext0,
        CheckStatus::Neutral => palette.blue,
        CheckStatus::Unknown => palette.peach,
    };
    Style::default().fg(color)
}

fn heading_style(palette: &Palette) -> Style {
    Style::default()
        .fg(palette.accent)
        .add_modifier(Modifier::BOLD)
}

fn button_style(palette: &Palette) -> Style {
    Style::default()
        .fg(palette.panel_bg)
        .bg(palette.accent)
        .add_modifier(Modifier::BOLD)
}

fn link_style(palette: &Palette) -> Style {
    Style::default()
        .fg(palette.accent)
        .add_modifier(Modifier::BOLD)
}

fn dim_style(palette: &Palette) -> Style {
    Style::default().fg(palette.overlay0)
}

fn heading_row(text: impl Into<String>, palette: &Palette) -> ContentRow {
    ContentRow::styled(text, heading_style(palette))
}

fn separator_line(width: u16, palette: &Palette) -> Line<'static> {
    Line::from(Span::styled(
        horizontal_rule(width),
        Style::default().fg(palette.surface1).bg(palette.panel_bg),
    ))
}

fn separator_row(width: usize, palette: &Palette) -> ContentRow {
    ContentRow::styled(horizontal_rule(width as u16), dim_style(palette))
}

fn expand_label(expanded: bool, symbols: &Symbols) -> &'static str {
    if expanded {
        symbols.less
    } else {
        symbols.more
    }
}

fn text_is_truncated(input: &str, width: usize, max_lines: usize) -> bool {
    markdown::wrap_plain_text(input, width).len() > max_lines
}

fn expand_all_control(
    blocks: Vec<BlockId>,
    expanded_blocks: &std::collections::HashSet<BlockId>,
    symbols: &Symbols,
) -> Option<(String, HitTarget)> {
    if blocks.is_empty() {
        return None;
    }
    let all_expanded = blocks.iter().all(|block| expanded_blocks.contains(block));
    let label_width = UnicodeWidthStr::width(symbols.expand_all)
        .max(UnicodeWidthStr::width(symbols.collapse_all));
    if all_expanded {
        Some((
            pad_label(symbols.collapse_all, label_width),
            HitTarget::CollapseBlocks(blocks),
        ))
    } else {
        Some((
            pad_label(symbols.expand_all, label_width),
            HitTarget::ExpandBlocks(blocks),
        ))
    }
}

fn pad_label(label: &str, width: usize) -> String {
    let mut padded = label.to_string();
    let label_width = UnicodeWidthStr::width(label);
    for _ in label_width..width {
        padded.push(' ');
    }
    padded
}

fn footer_expandable_blocks(state: &AppState, width: usize, palette: &Palette) -> Vec<BlockId> {
    state
        .active_tab_expandable_blocks()
        .into_iter()
        .filter(|block| footer_block_has_hidden_content(block, &state.resource, width, palette))
        .collect()
}

fn footer_block_has_hidden_content(
    block: &BlockId,
    resource: &Resource,
    width: usize,
    palette: &Palette,
) -> bool {
    match block {
        BlockId::Body => text_is_truncated(&resource.body, width, BODY_COLLAPSED_LINES),
        BlockId::Activity(id) => resource
            .activity
            .iter()
            .find(|entry| &entry.id == id)
            .is_some_and(|entry| text_is_truncated(&entry.body, width, ACTIVITY_COLLAPSED_LINES)),
        BlockId::Patch(path) => resource
            .pull_request
            .as_ref()
            .and_then(|pr| pr.files.iter().find(|file| &file.path == path))
            .and_then(|file| file.patch.as_deref())
            .is_some_and(|patch| {
                diff_patch_rows(patch, width, palette).len() > PATCH_COLLAPSED_ROWS
            }),
        BlockId::Commit(_) | BlockId::Check(_) | BlockId::File(_) => true,
    }
}

fn refresh_summary(state: &AppState) -> Option<String> {
    let refreshed_at = state.last_refreshed_at.as_deref()?;
    let refreshed_at = refreshed_at.trim_end_matches(" UTC");
    let change = match state.last_refresh_had_changes {
        Some(true) => "changed",
        Some(false) => "no-change",
        None => "unknown",
    };
    Some(format!("Refresh: {refreshed_at} {change}"))
}

fn refresh_changes_summary(state: &AppState) -> Option<String> {
    if state.last_refresh_changed_sections.is_empty() {
        None
    } else {
        Some(format!(
            "Changed: {}",
            state.last_refresh_changed_sections.join(", ")
        ))
    }
}

fn render_content(frame: &mut Frame<'_>, area: Rect, state: &mut AppState, palette: &Palette) {
    let content_area = content_area_for_state(area, state, active_content_tab(state));
    let rows = content_rows(state, content_area.width as usize, palette);
    let rows = apply_spacing(rows, state.spacing);
    let rows =
        apply_content_frame_spacing(rows, content_area.width as usize, state.spacing, palette);
    let rows = wrap_content_rows(rows, content_area.width, state.spacing);
    let row_count = rows.len();
    let max_scroll = row_count.saturating_sub(content_area.height as usize) as u16;
    state.set_scroll_limit(max_scroll);
    if let Some(focus_id) = state.take_pending_activity_focus() {
        if let Some(index) = rows
            .iter()
            .position(|row| row.activity_focus.as_deref() == Some(focus_id.as_str()))
        {
            state.scroll = (index as u16).min(state.scroll_limit);
            state.reveal_scrollbar_for_focus();
        }
    }
    let visible_rows = rows
        .into_iter()
        .enumerate()
        .skip(state.scroll as usize)
        .take(content_area.height as usize)
        .collect::<Vec<_>>();
    let mut visible = Vec::new();
    for (visible_index, (_row_index, row)) in visible_rows.into_iter().enumerate() {
        if let Some(target) = row.target {
            state.hit_areas.push(HitArea::new(
                Rect::new(
                    content_area.x,
                    content_area.y.saturating_add(visible_index as u16),
                    content_area.width,
                    1,
                ),
                target,
            ));
        }
        visible.push(row.line);
    }
    Paragraph::new(visible)
        .style(Style::default().fg(palette.text).bg(palette.panel_bg))
        .render(content_area, frame.buffer_mut());
    render_scrollbar(frame, content_area, state, palette, row_count);
    state.advance_scrollbar_visibility();
}

fn render_scrollbar(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &mut AppState,
    palette: &Palette,
    row_count: usize,
) {
    if !state.scrollbar_visible() || area.width < 8 || area.height < 3 {
        return;
    }

    let content_length = scrollbar_content_length(state, area.height, row_count);
    let scrollbar_position = scrollbar_position(state, content_length);
    let mut scrollbar_state = ScrollbarState::new(content_length)
        .position(scrollbar_position)
        .viewport_content_length(area.height as usize);
    Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .thumb_symbol("█")
        .thumb_style(Style::default().fg(palette.accent).bg(palette.panel_bg))
        .track_symbol(Some("│"))
        .track_style(dim_style(palette).bg(palette.panel_bg))
        .begin_symbol(None)
        .end_symbol(None)
        .render(area, frame.buffer_mut(), &mut scrollbar_state);
    state.hit_areas.push(HitArea::new(
        Rect::new(
            area.x.saturating_add(area.width.saturating_sub(1)),
            area.y,
            1,
            area.height,
        ),
        HitTarget::Scrollbar {
            top: area.y,
            height: area.height,
        },
    ));
    snap_scrollbar_endpoint(frame, area, state, palette, content_length);
}

fn snap_scrollbar_endpoint(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &AppState,
    palette: &Palette,
    content_length: usize,
) {
    if state.scroll_limit == 0 {
        return;
    }
    let current = state.scroll.min(state.scroll_limit);
    if current == 0 {
        paint_scrollbar_endpoint_thumb(frame, area, palette, content_length, ScrollEndpoint::Top);
    } else if current == state.scroll_limit {
        paint_scrollbar_endpoint_thumb(
            frame,
            area,
            palette,
            content_length,
            ScrollEndpoint::Bottom,
        );
    }
}

enum ScrollEndpoint {
    Top,
    Bottom,
}

fn paint_scrollbar_endpoint_thumb(
    frame: &mut Frame<'_>,
    area: Rect,
    palette: &Palette,
    content_length: usize,
    endpoint: ScrollEndpoint,
) {
    let Some(x) = area.x.checked_add(area.width.saturating_sub(1)) else {
        return;
    };
    let thumb_len = scrollbar_endpoint_thumb_length(area.height, content_length);
    let top = area.y;
    let bottom = area.y.saturating_add(area.height.saturating_sub(1));
    for y in top..=bottom {
        frame.buffer_mut()[(x, y)]
            .set_symbol("│")
            .set_style(dim_style(palette).bg(palette.panel_bg));
    }
    let thumb_start = match endpoint {
        ScrollEndpoint::Top => top,
        ScrollEndpoint::Bottom => bottom.saturating_sub(thumb_len.saturating_sub(1)),
    };
    let thumb_end = thumb_start
        .saturating_add(thumb_len.saturating_sub(1))
        .min(bottom);
    for y in thumb_start..=thumb_end {
        frame.buffer_mut()[(x, y)]
            .set_symbol("█")
            .set_style(Style::default().fg(palette.accent).bg(palette.panel_bg));
    }
}

fn scrollbar_endpoint_thumb_length(viewport_height: u16, content_length: usize) -> u16 {
    if viewport_height == 0 {
        return 0;
    }
    let track_length = f64::from(viewport_height);
    let viewport_length = f64::from(viewport_height);
    let max_viewport_position = content_length.saturating_sub(1) as f64 + viewport_length;
    if max_viewport_position <= 0.0 {
        return 1;
    }
    ((viewport_length * track_length / max_viewport_position).round() as u16)
        .clamp(1, viewport_height)
}

fn scrollbar_content_length(state: &AppState, viewport_height: u16, row_count: usize) -> usize {
    let minimum = usize::from(state.scroll_limit).saturating_add(usize::from(viewport_height));
    row_count.max(minimum)
}

fn scrollbar_position(state: &AppState, content_length: usize) -> usize {
    let scroll_limit = usize::from(state.scroll_limit);
    if scroll_limit == 0 || content_length == 0 {
        return 0;
    }
    let current = usize::from(state.scroll.min(state.scroll_limit));
    current.saturating_mul(content_length.saturating_sub(1)) / scroll_limit
}

fn active_content_tab(state: &AppState) -> Tab {
    if state.show_help || state.show_settings {
        Tab::Overview
    } else {
        state.active_tab
    }
}

fn content_area_for_state(area: Rect, state: &AppState, tab: Tab) -> Rect {
    content_area_for_preferences(
        area,
        state.spacing,
        state.width_mode,
        state.fixed_width,
        tab,
    )
}

#[cfg(test)]
fn content_area_for_spacing(area: Rect, spacing: SpacingMode, tab: Tab) -> Rect {
    content_area_for_preferences(
        area,
        spacing,
        ContentWidthMode::Fixed,
        crate::render::DEFAULT_FIXED_CONTENT_WIDTH,
        tab,
    )
}

fn content_area_for_preferences(
    area: Rect,
    spacing: SpacingMode,
    width_mode: ContentWidthMode,
    fixed_width: u16,
    tab: Tab,
) -> Rect {
    if spacing == SpacingMode::Compact || area.width < 48 {
        return area;
    }

    let width_after_gutter = area.width.saturating_sub(COMFORTABLE_GUTTER * 2);
    let width = if tab == Tab::Files
        || width_mode == ContentWidthMode::Full
        || area.width < COMFORTABLE_MIN_CAP_WIDTH
    {
        width_after_gutter
    } else {
        width_after_gutter.min(crate::render::normalize_fixed_width(fixed_width))
    };

    Rect::new(
        area.x.saturating_add(COMFORTABLE_GUTTER),
        area.y,
        width,
        area.height,
    )
}

fn chrome_area_for_spacing(area: Rect, spacing: SpacingMode) -> Rect {
    if spacing == SpacingMode::Compact || area.width < 48 {
        return area;
    }

    Rect::new(
        area.x.saturating_add(COMFORTABLE_GUTTER),
        area.y,
        area.width.saturating_sub(COMFORTABLE_GUTTER * 2),
        area.height,
    )
}

fn apply_spacing(rows: Vec<ContentRow>, spacing: SpacingMode) -> Vec<ContentRow> {
    if spacing == SpacingMode::Compact {
        return rows;
    }

    let mut spaced = Vec::with_capacity(rows.len() + 8);
    let mut iter = rows.into_iter().peekable();
    while let Some(row) = iter.next() {
        let insert_gap = (is_section_rule(&row) || row.comfortable_gap_after)
            && iter
                .peek()
                .is_some_and(|next| !line_text(&next.line).trim().is_empty());
        spaced.push(row);
        if insert_gap {
            spaced.push(ContentRow::plain(""));
        }
    }
    spaced
}

fn apply_content_frame_spacing(
    rows: Vec<ContentRow>,
    _width: usize,
    spacing: SpacingMode,
    _palette: &Palette,
) -> Vec<ContentRow> {
    if spacing == SpacingMode::Compact || rows.is_empty() {
        return rows;
    }

    let mut framed = Vec::with_capacity(rows.len() + 2);
    framed.push(ContentRow::plain(""));
    framed.extend(trim_content_frame_blanks(rows));
    framed.push(ContentRow::plain(""));
    framed
}

fn trim_content_frame_blanks(mut rows: Vec<ContentRow>) -> Vec<ContentRow> {
    let first_content = rows
        .iter()
        .position(|row| !is_blank_row(row))
        .unwrap_or(rows.len());
    if first_content > 0 {
        rows.drain(..first_content);
    }
    while rows.last().is_some_and(is_blank_row) {
        rows.pop();
    }
    rows
}

fn is_blank_row(row: &ContentRow) -> bool {
    line_text(&row.line).trim().is_empty()
}

fn is_section_rule(row: &ContentRow) -> bool {
    let text = line_text(&row.line);
    !text.is_empty() && text.chars().all(|ch| ch == '─' || ch == '-')
}

fn wrap_content_rows(rows: Vec<ContentRow>, width: u16, spacing: SpacingMode) -> Vec<ContentRow> {
    let width = usize::from(width).max(1);
    rows.into_iter()
        .flat_map(|row| wrap_content_row(row, width, spacing))
        .collect()
}

fn wrap_content_row(row: ContentRow, width: usize, spacing: SpacingMode) -> Vec<ContentRow> {
    let text = line_text(&row.line);
    if UnicodeWidthStr::width(text.as_str()) <= width {
        return vec![row];
    }
    let style = row_primary_style(&row.line);
    let focus = row.activity_focus.clone();
    let indent = continuation_indent(width, spacing);
    let wrap_width = width.saturating_sub(indent).max(1);
    markdown::wrap_display_text(&text, wrap_width)
        .into_iter()
        .enumerate()
        .map(|(index, line)| continuation_line(line, index, indent))
        .map(|line| {
            let mut wrapped = match &row.target {
                Some(target) if style != Style::default() => {
                    ContentRow::target_styled(line, target.clone(), style)
                }
                Some(target) => ContentRow::target(line, target.clone()),
                None if style != Style::default() => ContentRow::styled(line, style),
                None => ContentRow::plain(line),
            };
            wrapped.activity_focus = focus.clone();
            wrapped
        })
        .collect()
}

fn continuation_indent(width: usize, spacing: SpacingMode) -> usize {
    if spacing == SpacingMode::Comfortable && width >= 48 {
        2
    } else {
        0
    }
}

fn continuation_line(line: String, index: usize, indent: usize) -> String {
    if index == 0 || indent == 0 || line.is_empty() {
        line
    } else {
        format!("{}{}", " ".repeat(indent), line)
    }
}

fn line_text(line: &Line<'static>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<Vec<_>>()
        .join("")
}

fn row_primary_style(line: &Line<'static>) -> Style {
    line.spans
        .iter()
        .find_map(|span| (span.style != Style::default()).then_some(span.style))
        .unwrap_or(line.style)
}

fn content_rows(state: &mut AppState, width: usize, palette: &Palette) -> Vec<ContentRow> {
    let symbols = state.symbols.symbols();
    if state.show_help {
        return help_rows(width, palette, &symbols);
    }
    if state.show_settings {
        return settings_rows(state, width, palette);
    }
    match state.active_tab {
        Tab::Overview => overview_rows(state, width, palette),
        Tab::Activity => activity_rows(state, width, palette),
        Tab::Commits => commits_rows(state, width, palette),
        Tab::Checks => checks_rows(state, width, palette),
        Tab::Files => files_rows(state, width, palette),
        Tab::Links => links_rows(&state.resource, width, palette),
    }
}

fn help_rows(width: usize, palette: &Palette, symbols: &Symbols) -> Vec<ContentRow> {
    let mut rows = Vec::new();
    rows.push(heading_row("Help", palette));
    rows.push(ContentRow::plain(""));
    rows.push(heading_row("Mouse", palette));
    let expand_help = format!(
        "- Click {} / {} or {} / {} to expand or collapse long text, checks, and files.",
        symbols.more, symbols.less, symbols.expand_all, symbols.collapse_all
    );
    rows.extend(
        [
            "- Plus opens resources; resource tabs switch or close them.",
            "- Click tabs to switch sections.",
            expand_help.as_str(),
            "- Click visible GitHub issue or PR references to navigate.",
            "- Use the mouse wheel to scroll the current view.",
        ]
        .into_iter()
        .flat_map(|line| markdown::wrap_plain_text(line, width))
        .map(ContentRow::plain),
    );
    rows.push(heading_row("Keyboard", palette));
    rows.extend(
        [
            "- q: quit",
            "- ?: toggle this help",
            "- s: open or close settings",
            "- t / y / p / w / b in settings: cycle theme / symbols / spacing / width mode / scrollbar",
            "- - / + in settings: decrease or increase fixed content width",
            "- r: refresh now",
            "- n: open another PR or issue in a resource tab",
            "- f: load full GitHub pages when a partial-depth warning is shown",
            "- y: copy first visible URL, or current resource URL",
            "- o: open first visible URL, or current resource",
            "- Tab / Shift-Tab / Left / Right: switch tabs",
            "- 1-6: jump to the visible tab in that position",
            "- v: reverse chronological feed order",
            "- a: expand or collapse all rows in the current tab",
            "- Up / Down / PageUp / PageDown / Home / End: scroll",
            "- e: expand or collapse the main body",
            "- Backspace: return after following a link",
            "",
        ]
        .into_iter()
        .flat_map(|line| markdown::wrap_plain_text(line, width))
        .map(ContentRow::plain),
    );
    rows.push(heading_row("Refresh", palette));
    rows.extend(
        [
            "- Live mode refreshes automatically on the configured interval.",
            "- The status band shows the last refresh time and whether content changed.",
        ]
        .into_iter()
        .flat_map(|line| markdown::wrap_plain_text(line, width))
        .map(ContentRow::plain),
    );
    rows
}

fn settings_rows(state: &AppState, width: usize, palette: &Palette) -> Vec<ContentRow> {
    let mut rows = Vec::new();
    rows.push(heading_row("Settings", palette));
    rows.push(separator_row(width, palette));
    rows.extend(
        markdown::wrap_plain_text(&format!("Config: {}", state.config_path.display()), width)
            .into_iter()
            .map(|line| ContentRow::styled(line, dim_style(palette))),
    );
    rows.push(ContentRow::target_styled(
        "[close settings]",
        HitTarget::CloseSettings,
        button_style(palette),
    ));
    rows.push(ContentRow::plain(""));
    rows.push(heading_row("Width", palette));
    rows.push(settings_option_row(
        "fixed",
        "Cap reading tabs to the configured width",
        state.width_mode == ContentWidthMode::Fixed,
        HitTarget::SetWidthMode("fixed".into()),
        palette,
    ));
    rows.push(settings_option_row(
        "full",
        "Use the full content width",
        state.width_mode == ContentWidthMode::Full,
        HitTarget::SetWidthMode("full".into()),
        palette,
    ));
    rows.push(ContentRow::styled(
        format!("Fixed width: {} columns", state.fixed_width),
        dim_style(palette),
    ));
    for width in [88, 100, 118, 132, 148] {
        rows.push(settings_option_row(
            width.to_string(),
            "Fixed-width preset",
            state.fixed_width == width,
            HitTarget::SetFixedWidth(width),
            palette,
        ));
    }
    rows.push(ContentRow::plain(""));
    rows.push(heading_row("Spacing", palette));
    rows.push(settings_option_row(
        "comfortable",
        "Gmail-style density with gh-dash-like spacing",
        state.spacing == SpacingMode::Comfortable,
        HitTarget::SetSpacing("comfortable".into()),
        palette,
    ));
    rows.push(settings_option_row(
        "compact",
        "Dense rows for smaller terminals",
        state.spacing == SpacingMode::Compact,
        HitTarget::SetSpacing("compact".into()),
        palette,
    ));
    rows.push(ContentRow::plain(""));
    rows.push(heading_row("Scrollbar", palette));
    for (name, description, mode) in [
        (
            "on-scroll",
            "Show while scrolling, then fade",
            ScrollbarMode::OnScroll,
        ),
        (
            "always",
            "Show whenever content can scroll",
            ScrollbarMode::Always,
        ),
        ("hidden", "Never show the scrollbar", ScrollbarMode::Hidden),
    ] {
        rows.push(settings_option_row(
            name,
            description,
            state.scrollbar == mode,
            HitTarget::SetScrollbar(name.into()),
            palette,
        ));
    }
    rows.push(ContentRow::plain(""));
    rows.push(heading_row("Symbols", palette));
    rows.push(settings_option_row(
        "ascii",
        "Plain terminal-safe labels",
        state.symbols == SymbolMode::Ascii,
        HitTarget::SetSymbols("ascii".into()),
        palette,
    ));
    rows.push(settings_option_row(
        "emoji",
        "Richer semantic markers",
        state.symbols == SymbolMode::Emoji,
        HitTarget::SetSymbols("emoji".into()),
        palette,
    ));
    rows.push(ContentRow::plain(""));
    rows.push(heading_row("Theme", palette));
    for theme in ThemeName::ALL {
        rows.push(settings_option_row(
            theme.to_string(),
            theme_description(*theme),
            state.theme == *theme,
            HitTarget::SetTheme(theme.to_string()),
            palette,
        ));
    }
    rows.extend(
        [
            "",
            "Keyboard: t theme | y symbols | p spacing | w width mode | b scrollbar | -/+ width | s or Esc close",
            "Changes are applied live and saved immediately.",
        ]
        .into_iter()
        .flat_map(|line| markdown::wrap_plain_text(line, width))
        .map(|line| ContentRow::styled(line, dim_style(palette))),
    );
    rows
}

fn settings_option_row(
    name: impl Into<String>,
    description: impl Into<String>,
    selected: bool,
    target: HitTarget,
    palette: &Palette,
) -> ContentRow {
    let name = name.into();
    let description = description.into();
    let marker = if selected { "[x]" } else { "[ ]" };
    let marker_style = if selected {
        Style::default()
            .fg(palette.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(palette.overlay1)
    };
    let name_style = if selected {
        Style::default()
            .fg(palette.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(palette.text)
    };
    ContentRow::target_line(
        Line::from(vec![
            Span::styled(marker, marker_style),
            Span::raw(" "),
            Span::styled(name, name_style),
            Span::raw("  "),
            Span::styled(description, dim_style(palette)),
        ]),
        target,
    )
}

fn theme_description(theme: ThemeName) -> &'static str {
    match theme {
        ThemeName::Default => "Current default Tokyo Night colors",
        ThemeName::Catppuccin => "Herdr Catppuccin Mocha",
        ThemeName::CatppuccinLatte => "Herdr Catppuccin Latte",
        ThemeName::Terminal => "Terminal ANSI colors",
        ThemeName::TokyoNight => "Herdr Tokyo Night",
        ThemeName::TokyoNightDay => "Herdr Tokyo Night Day",
        ThemeName::Dracula => "Herdr Dracula",
        ThemeName::Nord => "Herdr Nord",
        ThemeName::Gruvbox => "Herdr Gruvbox dark",
        ThemeName::GruvboxLight => "Herdr Gruvbox light",
        ThemeName::OneDark => "Herdr One Dark",
        ThemeName::OneLight => "Herdr One Light",
        ThemeName::Solarized => "Herdr Solarized dark",
        ThemeName::SolarizedLight => "Herdr Solarized light",
        ThemeName::Kanagawa => "Herdr Kanagawa",
        ThemeName::KanagawaLotus => "Herdr Kanagawa Lotus",
        ThemeName::RosePine => "Herdr Rose Pine",
        ThemeName::RosePineDawn => "Herdr Rose Pine Dawn",
        ThemeName::Vesper => "Herdr Vesper",
    }
}

fn overview_rows(state: &mut AppState, width: usize, palette: &Palette) -> Vec<ContentRow> {
    let mut rows = Vec::new();

    push_conversation_rows(&mut rows, state, width, palette);

    rows.push(ContentRow::plain(""));
    rows.push(heading_row("Details", palette));
    rows.push(separator_row(width, palette));
    rows.push(ContentRow::plain(format!(
        "Labels: {}",
        labels_summary(&state.resource.labels)
    )));
    rows.push(ContentRow::plain(format!(
        "Assignees: {}",
        people_summary(&state.resource.assignees)
    )));
    rows.push(ContentRow::plain(format!(
        "Reactions: {}",
        reaction_summary(&state.resource.reactions)
    )));
    if !state.resource.warnings.is_empty() {
        rows.push(ContentRow::plain(""));
        rows.push(ContentRow::styled(
            format!("Warnings: {}", state.resource.warnings.len()),
            Style::default()
                .fg(palette.yellow)
                .add_modifier(Modifier::BOLD),
        ));
        rows.push(heading_row("Warnings", palette));
        for warning in &state.resource.warnings {
            rows.extend(
                markdown::wrap_plain_text(&format!("- {warning}"), width)
                    .into_iter()
                    .map(ContentRow::plain),
            );
        }
    }
    if let Some(pr) = &state.resource.pull_request {
        rows.push(ContentRow::plain(""));
        rows.push(heading_row("Change summary", palette));
        rows.push(ContentRow::plain(format!("Review: {}", review_summary(pr))));
        if let Some(threads) = review_threads_summary(&state.resource) {
            rows.push(ContentRow::plain(format!("Threads: {threads}")));
        }
        rows.push(ContentRow::plain(format!(
            "Reviewers: {}",
            people_summary(&pr.requested_reviewers)
        )));
        rows.push(ContentRow::plain(format!("Merge: {}", merge_summary(pr))));
        rows.push(ContentRow::plain(format!(
            "{} files, {} commits, +{} -{}",
            pr.files.len(),
            pr.commits.len(),
            pr.additions,
            pr.deletions
        )));
        rows.push(ContentRow::plain(format!(
            "Checks: {}",
            checks_summary(&state.resource)
        )));
        push_metadata_rows(&mut rows, "PR metadata", &pr.metadata, width, palette);
    }
    push_metadata_rows(
        &mut rows,
        "Metadata",
        &state.resource.metadata,
        width,
        palette,
    );
    rows
}

fn push_conversation_rows(
    rows: &mut Vec<ContentRow>,
    state: &AppState,
    width: usize,
    palette: &Palette,
) {
    let entries = chronological_timeline_entries(&state.resource, state.reverse_chronological);
    if entries.is_empty() {
        rows.push(ContentRow::plain("No conversation items."));
        return;
    }

    for (index, entry) in entries.iter().enumerate() {
        match entry.item {
            TimelineItem::Body => push_body_timeline_rows(rows, state, width, palette),
            TimelineItem::Commit(commit) => {
                push_commit_timeline_rows(rows, state, commit, width, palette)
            }
            TimelineItem::Activity(activity) => {
                push_activity_timeline_rows(rows, state, activity, width, palette)
            }
        }
        if index + 1 < entries.len() {
            if let Some(last) = rows.last_mut() {
                last.comfortable_gap_after = true;
            }
            rows.push(separator_row(width, palette));
        }
    }
}

fn chronological_timeline_entries(
    resource: &Resource,
    reverse_chronological: bool,
) -> Vec<TimelineEntry<'_>> {
    let body = TimelineEntry {
        sort_key: sortable_timestamp(&resource.created_at),
        sequence: 0,
        kind_order: 0,
        item: TimelineItem::Body,
    };

    let mut entries = Vec::new();
    let mut sequence = 1;
    if let Some(pr) = &resource.pull_request {
        for commit in &pr.commits {
            let timestamp = if commit.committed_at.trim().is_empty() {
                commit.authored_at.as_deref().unwrap_or_default()
            } else {
                commit.committed_at.as_str()
            };
            entries.push(TimelineEntry {
                sort_key: sortable_timestamp(timestamp),
                sequence,
                kind_order: 1,
                item: TimelineItem::Commit(commit),
            });
            sequence += 1;
        }
    }

    for activity in &resource.activity {
        entries.push(TimelineEntry {
            sort_key: sortable_timestamp(&activity.updated_at),
            sequence,
            kind_order: 2,
            item: TimelineItem::Activity(activity),
        });
        sequence += 1;
    }

    if entries.iter().all(|entry| entry.sort_key.is_some()) {
        entries.sort_by(|left, right| {
            left.sort_key
                .cmp(&right.sort_key)
                .then(left.kind_order.cmp(&right.kind_order))
                .then(left.sequence.cmp(&right.sequence))
        });
    }
    if reverse_chronological {
        entries.reverse();
        entries.push(body);
    } else {
        entries.insert(0, body);
    }
    entries
}

fn sortable_timestamp(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.len() >= 10
        && trimmed.as_bytes().get(4) == Some(&b'-')
        && trimmed.as_bytes().get(7) == Some(&b'-')
    {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn push_body_timeline_rows(
    rows: &mut Vec<ContentRow>,
    state: &AppState,
    width: usize,
    palette: &Palette,
) {
    let resource = &state.resource;
    let symbols = state.symbols.symbols();
    rows.push(ContentRow::styled(
        format!(
            "{} @{} opened {}",
            symbols.body,
            resource.author,
            relative_time_phrase(&resource.created_at)
        ),
        heading_style(palette),
    ));
    if resource.body.trim().is_empty() {
        rows.push(ContentRow::styled(
            "No description provided.",
            dim_style(palette),
        ));
        return;
    }
    let can_expand = text_is_truncated(&resource.body, width, BODY_COLLAPSED_LINES);
    let expanded = can_expand && state.block_expanded(&BlockId::Body);
    let wrapped = markdown::wrap_plain_text(&resource.body, width);
    let (visible, _truncated) = markdown::visible_prefix(&wrapped, BODY_COLLAPSED_LINES, expanded);
    rows.extend(
        visible
            .into_iter()
            .map(|line| linkable_text_row(line, resource)),
    );
    if can_expand {
        rows.push(ContentRow::target_styled(
            expand_label(expanded, &symbols),
            HitTarget::ToggleBlock(BlockId::Body),
            button_style(palette),
        ));
    }
}

fn push_commit_timeline_rows(
    rows: &mut Vec<ContentRow>,
    state: &AppState,
    commit: &Commit,
    width: usize,
    palette: &Palette,
) {
    let symbols = state.symbols.symbols();
    let block = BlockId::Commit(commit.oid.clone());
    let expanded = state.block_expanded(&block);
    let marker = expand_label(expanded, &symbols);
    rows.push(ContentRow::target_styled(
        format!(
            "* commit {} by @{} {} [{}] {}",
            truncate_ascii(&commit.oid, 8),
            commit.author,
            relative_time_phrase(&commit.committed_at),
            commit.status.label(),
            marker
        ),
        HitTarget::ToggleBlock(block.clone()),
        link_style(palette),
    ));
    rows.push(ContentRow::plain(truncate_ascii(&commit.message, width)));
    if expanded {
        push_expanded_commit_details(rows, commit, width, &state.resource);
        rows.push(ContentRow::target_styled(
            expand_label(true, &symbols),
            HitTarget::ToggleBlock(block),
            button_style(palette),
        ));
    }
}

fn push_activity_timeline_rows(
    rows: &mut Vec<ContentRow>,
    state: &AppState,
    entry: &ActivityEntry,
    width: usize,
    palette: &Palette,
) {
    let resource = &state.resource;
    let symbols = state.symbols.symbols();
    rows.push(ContentRow::styled(
        format!(
            "{} {} by @{} {}",
            activity_icon(entry, &symbols),
            entry.kind.label(),
            entry.author,
            relative_time_phrase(&entry.updated_at)
        ),
        activity_heading_style(entry, palette),
    ));
    if let Some(path) = &entry.path {
        rows.push(ContentRow::plain(format!(
            "{}:{}",
            path,
            entry.line.unwrap_or_default()
        )));
    }
    if let Some(summary) = review_thread_summary(entry) {
        rows.push(ContentRow::plain(summary));
    }
    if let Some(summary) = activity_metadata_summary(entry) {
        rows.push(ContentRow::plain(truncate_ascii(&summary, width)));
    }
    let block = BlockId::Activity(entry.id.clone());
    let can_expand = text_is_truncated(&entry.body, width, ACTIVITY_COLLAPSED_LINES);
    let expanded = can_expand && state.block_expanded(&block);
    if let Some(url) = &entry.url {
        rows.push(activity_detail_row(url, resource, palette));
    }
    if expanded {
        if let Some(url) = &entry.url {
            rows.push(linkable_text_row(format!("url: {url}"), resource));
        }
    }
    let wrapped = markdown::wrap_plain_text(&entry.body, width);
    let (visible, _truncated) =
        markdown::visible_prefix(&wrapped, ACTIVITY_COLLAPSED_LINES, expanded);
    rows.extend(
        visible
            .into_iter()
            .map(|line| linkable_text_row(line, resource)),
    );
    if can_expand {
        rows.push(ContentRow::target_styled(
            expand_label(expanded, &symbols),
            HitTarget::ToggleBlock(block),
            button_style(palette),
        ));
    }
}

fn activity_icon(entry: &ActivityEntry, symbols: &Symbols) -> &'static str {
    match entry.kind {
        crate::domain::ActivityKind::Comment => symbols.activity_comment,
        crate::domain::ActivityKind::Review => symbols.activity_review,
        crate::domain::ActivityKind::ReviewComment => symbols.activity_review_comment,
        crate::domain::ActivityKind::CommitComment => symbols.activity_commit_comment,
        crate::domain::ActivityKind::Timeline => symbols.activity_timeline,
    }
}

fn activity_detail_row(url: &str, resource: &Resource, palette: &Palette) -> ContentRow {
    let target = parse_link_token(url, resource)
        .map(|(_display, target)| target)
        .unwrap_or_else(|| HitTarget::OpenUrl(url.to_string()));
    ContentRow::target_styled("[details]", target, link_style(palette))
}

fn activity_heading_style(entry: &ActivityEntry, palette: &Palette) -> Style {
    let color = match entry.kind {
        crate::domain::ActivityKind::Comment => palette.teal,
        crate::domain::ActivityKind::Review => palette.green,
        crate::domain::ActivityKind::ReviewComment => palette.peach,
        crate::domain::ActivityKind::CommitComment => palette.yellow,
        crate::domain::ActivityKind::Timeline => palette.subtext0,
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

fn labels_summary(labels: &[String]) -> String {
    if labels.is_empty() {
        "none".to_string()
    } else {
        labels.join(", ")
    }
}

fn push_metadata_rows(
    rows: &mut Vec<ContentRow>,
    heading: &str,
    metadata: &[MetadataItem],
    width: usize,
    palette: &Palette,
) {
    if metadata.is_empty() {
        return;
    }
    rows.push(ContentRow::plain(""));
    rows.push(heading_row(heading, palette));
    for item in metadata {
        rows.extend(
            markdown::wrap_display_text(&format!("{}: {}", item.label, item.value), width)
                .into_iter()
                .map(ContentRow::plain),
        );
    }
}

fn activity_rows(state: &mut AppState, width: usize, palette: &Palette) -> Vec<ContentRow> {
    let symbols = state.symbols.symbols();
    let mut rows = Vec::new();
    if state.resource.activity.is_empty() {
        rows.push(ContentRow::plain("No comments."));
        return rows;
    }
    let mut entries = state.resource.activity.iter().collect::<Vec<_>>();
    if state.reverse_chronological {
        entries.reverse();
    }
    for entry in entries {
        rows.push(
            ContentRow::plain(format!(
                "{} by @{} {}",
                entry.kind.label(),
                entry.author,
                relative_time_phrase(&entry.updated_at)
            ))
            .with_activity_focus(entry.id.clone()),
        );
        if let Some(path) = &entry.path {
            rows.push(ContentRow::plain(format!(
                "{}:{}",
                path,
                entry.line.unwrap_or_default()
            )));
        }
        if let Some(summary) = review_thread_summary(entry) {
            rows.push(ContentRow::plain(summary));
        }
        if let Some(summary) = activity_metadata_summary(entry) {
            rows.push(ContentRow::plain(truncate_ascii(&summary, width)));
        }
        let block = BlockId::Activity(entry.id.clone());
        let can_expand = text_is_truncated(&entry.body, width, ACTIVITY_COLLAPSED_LINES);
        let expanded = can_expand && state.block_expanded(&block);
        if let Some(url) = &entry.url {
            rows.push(activity_detail_row(url, &state.resource, palette));
        }
        if expanded {
            if let Some(url) = &entry.url {
                rows.push(linkable_text_row(format!("url: {url}"), &state.resource));
            }
        }
        let wrapped = markdown::wrap_plain_text(&entry.body, width);
        let (visible, _truncated) =
            markdown::visible_prefix(&wrapped, ACTIVITY_COLLAPSED_LINES, expanded);
        rows.extend(
            visible
                .into_iter()
                .map(|line| linkable_text_row(line, &state.resource)),
        );
        if can_expand {
            rows.push(ContentRow::target_styled(
                expand_label(expanded, &symbols),
                HitTarget::ToggleBlock(block),
                button_style(palette),
            ));
        }
        if let Some(last) = rows.last_mut() {
            last.comfortable_gap_after = true;
        }
    }
    rows
}

fn commits_rows(state: &AppState, width: usize, palette: &Palette) -> Vec<ContentRow> {
    let resource = &state.resource;
    let symbols = state.symbols.symbols();
    let Some(pr) = &resource.pull_request else {
        return vec![ContentRow::plain(
            "Commits are only available for pull requests.",
        )];
    };
    let mut rows = Vec::new();
    let mut commits = pr.commits.iter().collect::<Vec<_>>();
    if state.reverse_chronological {
        commits.reverse();
    }
    for commit in commits {
        let block = BlockId::Commit(commit.oid.clone());
        let expanded = state.block_expanded(&block);
        let marker = expand_label(expanded, &symbols);
        rows.push(ContentRow::target_styled(
            format!(
                "{} {} [{}] {}",
                truncate_ascii(&commit.oid, 8),
                truncate_ascii(&commit.message, width.saturating_sub(25)),
                commit.status.label(),
                marker
            ),
            HitTarget::ToggleBlock(block.clone()),
            link_style(palette),
        ));
        rows.push(
            ContentRow::plain(format!(
                "@{} {}",
                commit.author,
                relative_time_phrase(&commit.committed_at)
            ))
            .with_comfortable_gap_after(),
        );
        if expanded {
            push_expanded_commit_details(&mut rows, commit, width, resource);
            rows.push(ContentRow::target_styled(
                expand_label(true, &symbols),
                HitTarget::ToggleBlock(block),
                button_style(palette),
            ));
            rows.push(ContentRow::plain(""));
        }
    }
    rows
}

fn push_expanded_commit_details(
    rows: &mut Vec<ContentRow>,
    commit: &Commit,
    width: usize,
    resource: &Resource,
) {
    if !commit.authors.is_empty() {
        rows.push(ContentRow::plain(format!(
            "authors: {}",
            commit.authors.join(", ")
        )));
    }
    if let Some(authored_at) = commit.authored_at.as_deref() {
        rows.push(ContentRow::plain(format!(
            "authored: {}",
            relative_time_phrase(authored_at)
        )));
    }
    rows.push(ContentRow::plain(format!(
        "committed: {}",
        relative_time_phrase(&commit.committed_at)
    )));
    if !commit.deployments.is_empty() {
        rows.push(ContentRow::plain("deployments"));
        for deployment in &commit.deployments {
            rows.push(ContentRow::plain(truncate_ascii(
                &format!(
                    "- {} [{}] {}",
                    deployment.environment,
                    deployment.state,
                    relative_time_phrase(&deployment.updated_at)
                ),
                width,
            )));
            if let Some(description) = deployment.description.as_deref() {
                rows.extend(
                    markdown::wrap_plain_text(&format!("  {description}"), width)
                        .into_iter()
                        .map(ContentRow::plain),
                );
            }
            if let Some(url) = deployment.environment_url.as_deref() {
                rows.push(linkable_text_row(format!("  environment: {url}"), resource));
            }
            if let Some(url) = deployment.log_url.as_deref() {
                rows.push(linkable_text_row(format!("  logs: {url}"), resource));
            }
        }
    }
    if !commit.body.trim().is_empty() {
        rows.push(ContentRow::plain("body"));
        let wrapped = markdown::wrap_plain_text(&commit.body, width);
        rows.extend(wrapped.into_iter().map(ContentRow::plain));
    }
}

fn checks_rows(state: &AppState, width: usize, palette: &Palette) -> Vec<ContentRow> {
    let resource = &state.resource;
    let symbols = state.symbols.symbols();
    let Some(pr) = &resource.pull_request else {
        return vec![ContentRow::plain(
            "Checks are only available for pull requests.",
        )];
    };
    let counts = pr.check_counts();
    let mut rows = vec![
        ContentRow::plain(format!("Summary: {}", checks_summary(resource))),
        ContentRow::plain(format!(
            "{} total: {} pass, {} pending, {} fail, {} skipped, {} neutral, {} unknown",
            counts.total(),
            counts.success,
            counts.pending,
            counts.failure,
            counts.skipped,
            counts.neutral,
            counts.unknown
        )),
        ContentRow::plain(""),
    ];
    let context = CheckGroupRenderContext {
        expanded_blocks: &state.expanded_blocks,
        width,
        palette,
        symbols: &symbols,
    };
    push_check_group(
        &mut rows,
        "Failing",
        &pr.checks,
        CheckStatus::Failure,
        &context,
    );
    push_check_group(
        &mut rows,
        "Pending",
        &pr.checks,
        CheckStatus::Pending,
        &context,
    );
    push_check_group(
        &mut rows,
        "Passing",
        &pr.checks,
        CheckStatus::Success,
        &context,
    );
    push_check_group(
        &mut rows,
        "Neutral",
        &pr.checks,
        CheckStatus::Neutral,
        &context,
    );
    push_check_group(
        &mut rows,
        "Skipped",
        &pr.checks,
        CheckStatus::Skipped,
        &context,
    );
    push_check_group(
        &mut rows,
        "Unknown",
        &pr.checks,
        CheckStatus::Unknown,
        &context,
    );
    if pr.checks.is_empty() {
        rows.push(ContentRow::plain("No checks reported yet."));
    }
    rows
}

fn push_check_group(
    rows: &mut Vec<ContentRow>,
    label: &str,
    checks: &[CheckRun],
    status: CheckStatus,
    context: &CheckGroupRenderContext<'_>,
) {
    let matching = checks
        .iter()
        .filter(|check| check.status == status)
        .collect::<Vec<_>>();
    if matching.is_empty() {
        return;
    }
    rows.push(ContentRow::styled(
        format!("{label} ({})", matching.len()),
        heading_style(context.palette),
    ));
    for check in matching {
        let summary = check.summary.clone().unwrap_or_default();
        let block = BlockId::Check(format!("{}:{}", check.status.label(), check.name));
        let expanded = context.expanded_blocks.contains(&block);
        let marker = expand_label(expanded, context.symbols);
        let prefix = format!(
            "[{} {}] ",
            check_status_symbol(check.status, context.symbols),
            check.status.label()
        );
        let reserved = UnicodeWidthStr::width(prefix.as_str()) + UnicodeWidthStr::width(marker) + 1;
        let check_row = ContentRow::target_styled(
            format!(
                "{}{} {}",
                prefix,
                truncate_ascii(&check.name, context.width.saturating_sub(reserved)),
                marker
            ),
            HitTarget::ToggleBlock(block.clone()),
            check_status_style(check.status, context.palette).add_modifier(Modifier::BOLD),
        );
        rows.push(if expanded {
            check_row
        } else {
            check_row.with_comfortable_gap_after()
        });
        if expanded {
            rows.push(ContentRow::plain(format!("name: {}", check.name)));
            rows.push(ContentRow::plain(format!(
                "status: {}",
                check.status.label()
            )));
            if let Some(raw) = check.raw_status.as_deref() {
                rows.push(ContentRow::plain(format!("github status: {raw}")));
            }
            if let Some(raw) = check.raw_conclusion.as_deref() {
                rows.push(ContentRow::plain(format!("github conclusion: {raw}")));
            }
            if let Some(started) = check.started_at.as_deref() {
                rows.push(ContentRow::plain(format!(
                    "started: {}",
                    relative_time_phrase(started)
                )));
            }
            if let Some(completed) = check.completed_at.as_deref() {
                rows.push(ContentRow::plain(format!(
                    "completed: {}",
                    relative_time_phrase(completed)
                )));
            }
            if let Some(url) = check.details_url.as_deref() {
                rows.push(linkable_check_url_row(url));
            }
            if !summary.is_empty() {
                rows.push(ContentRow::plain(format!("summary: {summary}")));
            }
            rows.push(ContentRow::target_styled(
                expand_label(true, context.symbols),
                HitTarget::ToggleBlock(block),
                button_style(context.palette),
            ));
            rows.push(ContentRow::plain(""));
        }
    }
    rows.push(ContentRow::plain(""));
}

fn linkable_check_url_row(url: &str) -> ContentRow {
    if let Ok(id) = ResourceId::parse(url) {
        if should_open_exact_url(url, &id) {
            ContentRow::target(
                format!("details: {url}"),
                HitTarget::OpenUrl(url.to_string()),
            )
        } else {
            ContentRow::target(
                format!("details: {url}"),
                HitTarget::ResourceLink {
                    id,
                    url: Some(url.to_string()),
                },
            )
        }
    } else {
        ContentRow::target(
            format!("details: {url}"),
            HitTarget::OpenUrl(url.to_string()),
        )
    }
}

fn files_rows(state: &AppState, width: usize, palette: &Palette) -> Vec<ContentRow> {
    let Some(pr) = &state.resource.pull_request else {
        return vec![ContentRow::plain(
            "Files are only available for pull requests.",
        )];
    };
    let symbols = state.symbols.symbols();
    files_rows_for_pr(pr, width, &state.expanded_blocks, palette, &symbols)
}

fn files_rows_for_pr(
    pr: &crate::domain::PullRequest,
    width: usize,
    expanded_blocks: &std::collections::HashSet<BlockId>,
    palette: &Palette,
    symbols: &Symbols,
) -> Vec<ContentRow> {
    let mut rows = Vec::new();
    for file in &pr.files {
        let block = BlockId::File(file.path.clone());
        let expanded = expanded_blocks.contains(&block);
        let marker = expand_label(expanded, symbols);
        let file_row = ContentRow::target_line(
            file_summary_line(
                file.additions,
                file.deletions,
                &file.change_type,
                &file.path,
                marker,
                width,
                palette,
            ),
            HitTarget::ToggleBlock(block.clone()),
        );
        rows.push(if expanded {
            file_row
        } else {
            file_row.with_comfortable_gap_after()
        });
        if expanded {
            rows.push(ContentRow::plain(format!("path: {}", file.path)));
            rows.push(ContentRow::plain(format!(
                "change: {}, additions: {}, deletions: {}",
                file.change_type, file.additions, file.deletions
            )));
            if let Some(patch) = &file.patch {
                rows.push(ContentRow::plain("patch"));
                let patch_block = BlockId::Patch(file.path.clone());
                let patch_expanded = expanded_blocks.contains(&patch_block);
                let patch_rows = diff_patch_rows(patch, width, palette);
                let can_expand_patch = patch_rows.len() > PATCH_COLLAPSED_ROWS;
                let patch_expanded = patch_expanded && can_expand_patch;
                let visible_count = if patch_expanded {
                    patch_rows.len()
                } else {
                    PATCH_COLLAPSED_ROWS
                };
                rows.extend(patch_rows.into_iter().take(visible_count));
                if can_expand_patch {
                    rows.push(ContentRow::target_styled(
                        if patch_expanded {
                            symbols.less_patch
                        } else {
                            symbols.more_patch
                        },
                        HitTarget::ToggleBlock(patch_block),
                        button_style(palette),
                    ));
                }
            } else {
                rows.push(ContentRow::plain("patch: not loaded"));
            }
            rows.push(ContentRow::target_styled(
                expand_label(true, symbols),
                HitTarget::ToggleBlock(block),
                button_style(palette),
            ));
            rows.push(ContentRow::plain(""));
        }
    }
    rows
}

fn file_summary_line(
    additions: u64,
    deletions: u64,
    change_type: &str,
    path: &str,
    marker: &str,
    width: usize,
    palette: &Palette,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("+{additions:<4}"),
            Style::default()
                .fg(palette.green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            format!("-{deletions:<4}"),
            Style::default()
                .fg(palette.red)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            format!("{change_type:<8}"),
            Style::default().fg(palette.subtext0),
        ),
        Span::raw(" "),
        Span::styled(
            truncate_ascii(path, width.saturating_sub(27)),
            Style::default()
                .fg(palette.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(marker.to_string(), button_style(palette)),
    ])
}

fn diff_patch_rows(patch: &str, width: usize, palette: &Palette) -> Vec<ContentRow> {
    patch
        .lines()
        .flat_map(|line| {
            let kind = diff_line_kind(line);
            let style = diff_line_style(kind, palette);
            let text = diff_line_text(line, kind);
            wrap_diff_text(text, width)
                .into_iter()
                .map(move |line| ContentRow::styled(line, style))
        })
        .collect()
}

#[derive(Clone, Copy)]
enum DiffLineKind {
    Addition,
    Deletion,
    Context,
    Hunk,
    Metadata,
}

fn diff_line_kind(line: &str) -> DiffLineKind {
    if line.starts_with("@@") {
        DiffLineKind::Hunk
    } else if is_diff_metadata_line(line) {
        DiffLineKind::Metadata
    } else if line.starts_with('+') {
        DiffLineKind::Addition
    } else if line.starts_with('-') {
        DiffLineKind::Deletion
    } else if line.starts_with(' ') {
        DiffLineKind::Context
    } else {
        DiffLineKind::Metadata
    }
}

fn is_diff_metadata_line(line: &str) -> bool {
    line.starts_with("diff --git")
        || line.starts_with("index ")
        || line.starts_with("new file mode ")
        || line.starts_with("deleted file mode ")
        || line.starts_with("old mode ")
        || line.starts_with("new mode ")
        || line.starts_with("similarity index ")
        || line.starts_with("dissimilarity index ")
        || line.starts_with("rename from ")
        || line.starts_with("rename to ")
        || line.starts_with("copy from ")
        || line.starts_with("copy to ")
        || line.starts_with("--- ")
        || line.starts_with("+++ ")
        || line.starts_with("\\ No newline")
}

fn diff_line_text(line: &str, kind: DiffLineKind) -> &str {
    match kind {
        DiffLineKind::Addition | DiffLineKind::Deletion | DiffLineKind::Context => {
            line.get(1..).unwrap_or("")
        }
        DiffLineKind::Hunk | DiffLineKind::Metadata => line,
    }
}

fn wrap_diff_text(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    if text.is_empty() {
        return vec![" ".to_string()];
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0;

    for ch in text.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if current_width > 0 && current_width + ch_width > width {
            lines.push(current);
            current = String::new();
            current_width = 0;
        }
        current.push(ch);
        current_width += ch_width;
        if current_width >= width {
            lines.push(current);
            current = String::new();
            current_width = 0;
        }
    }

    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }
    lines
}

fn diff_line_style(kind: DiffLineKind, palette: &Palette) -> Style {
    match kind {
        DiffLineKind::Addition => Style::default().fg(palette.panel_bg).bg(palette.green),
        DiffLineKind::Deletion => Style::default().fg(palette.panel_bg).bg(palette.red),
        DiffLineKind::Hunk => Style::default()
            .fg(palette.teal)
            .add_modifier(Modifier::BOLD),
        DiffLineKind::Metadata => Style::default().fg(palette.overlay1),
        DiffLineKind::Context => Style::default().fg(palette.text),
    }
}

fn links_rows(resource: &Resource, width: usize, palette: &Palette) -> Vec<ContentRow> {
    let mut rows = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for id in &resource.related_resources {
        if seen.insert(id.canonical_name()) {
            rows.push(
                ContentRow::target_styled(
                    truncate_ascii(&id.canonical_name(), width),
                    HitTarget::ResourceLink {
                        id: id.clone(),
                        url: None,
                    },
                    link_style(palette),
                )
                .with_comfortable_gap_after(),
            );
        }
    }
    for token in linked_resource_tokens(resource) {
        if let Some((display, target)) = parse_link_token(token, resource) {
            let HitTarget::ResourceLink { id, .. } = &target else {
                continue;
            };
            let key = id.canonical_name();
            if seen.insert(key) {
                rows.push(
                    ContentRow::target_styled(
                        truncate_ascii(&display, width),
                        target,
                        link_style(palette),
                    )
                    .with_comfortable_gap_after(),
                );
            }
        }
    }
    if rows.is_empty() {
        rows.push(ContentRow::plain(
            "No GitHub issue or PR links detected yet.",
        ));
    }
    rows
}

fn linked_resource_tokens(resource: &Resource) -> Vec<&str> {
    let mut tokens = resource
        .body
        .split_whitespace()
        .filter(|token| is_link_candidate_token(token))
        .collect::<Vec<_>>();
    for entry in &resource.activity {
        tokens.extend(
            entry
                .body
                .split_whitespace()
                .filter(|token| is_link_candidate_token(token)),
        );
    }
    tokens
}

fn is_link_candidate_token(token: &str) -> bool {
    token.contains("github.com") || token.contains('#') || token.contains("](")
}

fn parse_link_token(token: &str, resource: &Resource) -> Option<(String, HitTarget)> {
    let clean = link_target_from_token(token)?;
    let is_url = clean.starts_with("https://") || clean.starts_with("http://");
    if let Ok(id) = ResourceId::parse(&clean) {
        if is_url && should_open_exact_url(&clean, &id) && !is_resource_comment_url(&clean) {
            return Some((clean.clone(), HitTarget::OpenUrl(clean)));
        }
        return Some((
            clean.clone(),
            HitTarget::ResourceLink {
                id,
                url: is_url.then_some(clean),
            },
        ));
    }
    if is_url {
        return Some((clean.clone(), HitTarget::OpenUrl(clean)));
    }
    if let Some(number) = clean.strip_prefix('#') {
        if number.chars().all(|ch| ch.is_ascii_digit()) {
            let id =
                ResourceId::relative_to_repo(&resource.id.owner, &resource.id.repo, number).ok()?;
            return Some((
                format!("{}#{}", resource.id.repo_name_with_owner(), number),
                HitTarget::ResourceLink { id, url: None },
            ));
        }
    }
    None
}

fn link_target_from_token(token: &str) -> Option<String> {
    let trimmed = token.trim_matches(|c: char| {
        matches!(
            c,
            ')' | '(' | ',' | '.' | ';' | ':' | '"' | '\'' | '[' | ']' | '<' | '>'
        )
    });
    if trimmed.is_empty() {
        return None;
    }
    if let Some(target) = markdown_link_target(trimmed) {
        return Some(target);
    }
    Some(trimmed.to_string())
}

fn markdown_link_target(token: &str) -> Option<String> {
    let open = token.rfind("](")?;
    let target = token[open + 2..]
        .trim_matches(|c: char| matches!(c, ')' | ',' | '.' | ';' | ':' | '"' | '\'' | '<' | '>'));
    if target.is_empty() {
        return None;
    }
    Some(target.to_string())
}

fn should_open_exact_url(url: &str, id: &ResourceId) -> bool {
    let Some(kind) = id.kind_hint else {
        return false;
    };
    let segment = match kind {
        crate::domain::ResourceKind::PullRequest => "pull",
        crate::domain::ResourceKind::Issue => "issues",
    };
    let bare = format!(
        "https://github.com/{}/{}/{}/{}",
        id.owner, id.repo, segment, id.number
    );
    url != bare
}

fn is_resource_comment_url(url: &str) -> bool {
    let Some((_base, fragment)) = url.split_once('#') else {
        return false;
    };
    fragment.starts_with("issuecomment-")
        || fragment.starts_with("discussion_r")
        || fragment.starts_with("pullrequestreview-")
}

fn linkable_text_row(text: String, resource: &Resource) -> ContentRow {
    if let Some((_display, target)) = text
        .split_whitespace()
        .find_map(|token| parse_link_token(token, resource))
    {
        ContentRow::target(text, target)
    } else {
        ContentRow::plain(text)
    }
}

fn render_footer(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &mut AppState,
    palette: &Palette,
    content_width: usize,
) {
    let symbols = state.symbols.symbols();
    let mut controls = vec![footer_control(
        symbols.footer_refresh,
        HitTarget::Refresh,
        palette,
    )];
    if !state.show_help && !state.show_settings {
        if let Some(control) = expand_all_control(
            footer_expandable_blocks(state, content_width, palette),
            &state.expanded_blocks,
            &symbols,
        ) {
            controls.push(footer_control(control.0, control.1, palette));
        }
        if state.resource.has_partial_depth_warning() {
            controls.push(footer_control(
                symbols.footer_load_full,
                HitTarget::LoadFullDepth,
                palette,
            ));
        }
    }
    controls.push(footer_control(
        symbols.footer_settings,
        HitTarget::Settings,
        palette,
    ));
    controls.push(footer_control(
        symbols.footer_help,
        HitTarget::Help,
        palette,
    ));
    controls.push(footer_control(
        symbols.footer_quit,
        HitTarget::Quit,
        palette,
    ));
    let control_lines = footer_control_lines(controls, area.width);
    let bottom_padding = footer_bottom_padding_rows(area, state.spacing);
    let control_capacity = (area.height as usize).saturating_sub(bottom_padding);
    let control_lines = footer_visible_control_lines(control_lines, control_capacity);

    let message = if let Some(error) = &state.last_error {
        Some(format!("ERROR: {error}"))
    } else if let Some(message) = loading_status_text(state) {
        Some(message)
    } else {
        state.status_message.clone()
    };
    let message_style = if state.last_error.is_some() {
        Style::default()
            .fg(palette.red)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(palette.subtext0)
    };
    let control_height = control_lines.len().min(control_capacity);
    let remaining = area.height.saturating_sub(control_height as u16) as usize;
    let mut message_lines = Vec::<Line<'static>>::new();
    if remaining > 0 {
        for line in message
            .as_deref()
            .map(|message| markdown::wrap_plain_text(message, area.width as usize))
            .unwrap_or_default()
            .into_iter()
            .take(remaining)
        {
            message_lines.push(Line::from(Span::styled(line, message_style)));
        }
    }
    Paragraph::new(message_lines)
        .style(Style::default().fg(palette.text).bg(palette.panel_bg))
        .render(area, frame.buffer_mut());
    render_footer_controls(frame, area, state, control_lines);
}

fn footer_bottom_padding_rows(area: Rect, spacing: SpacingMode) -> usize {
    usize::from(spacing == SpacingMode::Comfortable && area.width >= 48 && area.height >= 3)
}

fn footer_control(label: impl Into<String>, target: HitTarget, palette: &Palette) -> FooterItem {
    FooterItem {
        label: label.into(),
        style: button_style(palette),
        target: Some(target),
    }
}

fn footer_control_lines(controls: Vec<FooterItem>, width: u16) -> Vec<FooterLine> {
    if width == 0 {
        return Vec::new();
    }
    wrap_footer_items(&controls, width)
}

fn wrap_footer_items(items: &[FooterItem], width: u16) -> Vec<FooterLine> {
    let mut lines = Vec::<FooterLine>::new();
    let mut current = FooterLine { items: Vec::new() };
    let mut current_width = 0u16;
    for item in items {
        let label = fit_label_to_width(&item.label, width);
        let item_width = UnicodeWidthStr::width(label.as_str()) as u16;
        if item_width == 0 {
            continue;
        }
        let required_width = if current.items.is_empty() {
            item_width
        } else {
            item_width.saturating_add(1)
        };
        if !current.items.is_empty() && current_width.saturating_add(required_width) > width {
            lines.push(current);
            current = FooterLine { items: Vec::new() };
            current_width = 0;
        }
        current.items.push(FooterItem {
            label,
            style: item.style,
            target: item.target.clone(),
        });
        current_width = current_width.saturating_add(if current_width == 0 {
            item_width
        } else {
            item_width.saturating_add(1)
        });
    }
    if !current.items.is_empty() {
        lines.push(current);
    }
    lines
}

fn footer_visible_control_lines(lines: Vec<FooterLine>, max_lines: usize) -> Vec<FooterLine> {
    if max_lines == 0 || lines.len() <= max_lines {
        return lines;
    }
    if let Some(expand_index) = lines.iter().position(footer_line_has_expand_all) {
        if expand_index >= max_lines {
            let mut visible = lines
                .into_iter()
                .enumerate()
                .filter_map(|(index, line)| {
                    (index < max_lines.saturating_sub(1) || index == expand_index).then_some(line)
                })
                .collect::<Vec<_>>();
            visible.truncate(max_lines);
            return visible;
        }
    }
    lines.into_iter().take(max_lines).collect()
}

fn footer_line_has_expand_all(line: &FooterLine) -> bool {
    line.items.iter().any(footer_item_is_expand_all)
}

fn footer_item_is_expand_all(item: &FooterItem) -> bool {
    matches!(
        &item.target,
        Some(HitTarget::ExpandBlocks(_) | HitTarget::CollapseBlocks(_))
    )
}

fn render_footer_controls(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &mut AppState,
    control_lines: Vec<FooterLine>,
) {
    let bottom_padding = footer_bottom_padding_rows(area, state.spacing);
    let line_count = control_lines
        .len()
        .min((area.height as usize).saturating_sub(bottom_padding));
    let start_y = area
        .y
        .saturating_add(area.height.saturating_sub(bottom_padding as u16))
        .saturating_sub(line_count as u16);
    for (line_index, line) in control_lines.into_iter().take(line_count).enumerate() {
        let y = start_y.saturating_add(line_index as u16);
        let mut x = area.x;
        let mut spans = Vec::<Span<'static>>::new();
        for (item_index, item) in line.items.into_iter().enumerate() {
            if item_index > 0 {
                spans.push(Span::raw(" "));
                x = x.saturating_add(1);
            }
            let width = UnicodeWidthStr::width(item.label.as_str()) as u16;
            if let Some(target) = item.target {
                state
                    .hit_areas
                    .push(HitArea::new(Rect::new(x, y, width, 1), target));
            }
            spans.push(Span::styled(item.label, item.style));
            x = x.saturating_add(width);
        }
        Paragraph::new(Line::from(spans))
            .style(Style::default())
            .render(Rect::new(area.x, y, area.width, 1), frame.buffer_mut());
    }
}

fn checks_summary(resource: &Resource) -> String {
    let Some(pr) = &resource.pull_request else {
        return "n/a".to_string();
    };
    let counts = pr.check_counts();
    let rollup = summarize_checks(resource);
    if counts.total() == 0 {
        return rollup;
    }
    format!(
        "{rollup} ({} pass, {} pending, {} fail)",
        counts.success, counts.pending, counts.failure
    )
}

fn reaction_summary(reactions: &crate::domain::ReactionCounts) -> String {
    let parts = [
        ("+1", reactions.thumbs_up),
        ("-1", reactions.thumbs_down),
        ("laugh", reactions.laugh),
        ("hooray", reactions.hooray),
        ("confused", reactions.confused),
        ("heart", reactions.heart),
        ("rocket", reactions.rocket),
        ("eyes", reactions.eyes),
    ]
    .into_iter()
    .filter(|&(_label, count)| count > 0)
    .map(|(label, count)| format!("{label}:{count}"))
    .collect::<Vec<_>>();

    if parts.is_empty() {
        "0".to_string()
    } else {
        parts.join(" ")
    }
}

fn people_summary(people: &[String]) -> String {
    if people.is_empty() {
        "none".to_string()
    } else {
        people
            .iter()
            .map(|person| format!("@{person}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn review_summary(pr: &crate::domain::PullRequest) -> String {
    pr.review_decision
        .as_deref()
        .map(format_github_state)
        .unwrap_or_else(|| "None requested".to_string())
}

fn review_threads_summary(resource: &Resource) -> Option<String> {
    #[derive(Default)]
    struct ThreadState {
        resolved: Option<bool>,
        outdated: bool,
    }

    let mut threads = std::collections::HashMap::<String, ThreadState>::new();
    for entry in &resource.activity {
        if entry.kind != crate::domain::ActivityKind::ReviewComment {
            continue;
        }
        let key = entry.thread_id.clone().unwrap_or_else(|| entry.id.clone());
        let state = threads.entry(key).or_default();
        if entry.thread_resolved == Some(false) {
            state.resolved = Some(false);
        } else if state.resolved.is_none() {
            state.resolved = entry.thread_resolved;
        }
        state.outdated |= entry.thread_outdated.unwrap_or(false);
    }

    if threads.is_empty() {
        return None;
    }
    let total = threads.len();
    let unresolved = threads
        .values()
        .filter(|state| state.resolved == Some(false))
        .count();
    let outdated = threads.values().filter(|state| state.outdated).count();
    let noun = if total == 1 { "thread" } else { "threads" };
    let mut summary = format!("{unresolved} unresolved / {total} {noun}");
    if outdated > 0 {
        summary.push_str(&format!(", {outdated} outdated"));
    }
    Some(summary)
}

fn review_thread_summary(entry: &crate::domain::ActivityEntry) -> Option<String> {
    if entry.kind != crate::domain::ActivityKind::ReviewComment {
        return None;
    }
    let mut parts = Vec::new();
    if let Some(resolved) = entry.thread_resolved {
        parts.push(if resolved { "resolved" } else { "unresolved" });
    }
    if let Some(outdated) = entry.thread_outdated {
        parts.push(if outdated { "outdated" } else { "current" });
    }
    if parts.is_empty() {
        None
    } else {
        Some(format!("thread: {}", parts.join(", ")))
    }
}

fn activity_metadata_summary(entry: &crate::domain::ActivityEntry) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(association) = entry
        .author_association
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("association {association}"));
    }
    if entry.includes_created_edit {
        parts.push("edited".to_string());
    }
    if entry.is_minimized {
        let reason = entry
            .minimized_reason
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("unknown");
        parts.push(format!("minimized {reason}"));
    }
    if entry.reactions.total() > 0 {
        parts.push(format!("reactions {}", reaction_summary(&entry.reactions)));
    }
    if parts.is_empty() {
        None
    } else {
        Some(format!("meta: {}", parts.join(", ")))
    }
}

fn merge_summary(pr: &crate::domain::PullRequest) -> String {
    pr.merge_state
        .as_deref()
        .map(format_github_state)
        .unwrap_or_else(|| "Unknown".to_string())
}

fn format_github_state(value: &str) -> String {
    value
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let lower = part.to_ascii_lowercase();
            let mut chars = lower.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn summarize_checks(resource: &Resource) -> String {
    let Some(pr) = &resource.pull_request else {
        return "n/a".to_string();
    };
    if pr
        .checks
        .iter()
        .any(|check| check.status == CheckStatus::Failure)
    {
        return "FAIL".to_string();
    }
    if pr
        .checks
        .iter()
        .any(|check| check.status == CheckStatus::Pending)
    {
        return "PENDING".to_string();
    }
    if pr
        .checks
        .iter()
        .any(|check| check.status == CheckStatus::Success)
    {
        return "PASS".to_string();
    }
    "UNKNOWN".to_string()
}

fn horizontal_rule(width: u16) -> String {
    "─".repeat(width as usize)
}

fn truncate_ascii(input: &str, max_width: usize) -> String {
    truncate_display(input, max_width)
}

fn fit_label_to_width(input: &str, max_width: u16) -> String {
    let max_width = max_width as usize;
    if max_width == 0 {
        String::new()
    } else {
        truncate_display(input, max_width)
    }
}

fn truncate_display(input: &str, max_width: usize) -> String {
    if UnicodeWidthStr::width(input) <= max_width {
        return input.to_string();
    }
    if max_width <= 3 {
        return ".".repeat(max_width);
    }
    let mut prefix = String::new();
    let mut width = 0;
    for ch in input.chars() {
        let ch_width = UnicodeWidthStr::width(ch.to_string().as_str()).max(1);
        if width + ch_width > max_width.saturating_sub(3) {
            break;
        }
        prefix.push(ch);
        width += ch_width;
    }
    format!("{prefix}...")
}

#[cfg(test)]
mod tests {
    use crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use ratatui::{backend::TestBackend, style::Color, Terminal};

    use super::*;
    use crate::app::{apply_event, AppEvent, AppIntent};
    use crate::domain::{
        ActivityEntry, ActivityKind, ChangedFile, CheckRun, CheckStatus, Commit, Deployment,
        MetadataItem, PullRequest, ReactionCounts, ResourceId, ResourceKind,
        FULL_DEPTH_WARNING_HINT,
    };

    fn pr_resource() -> Resource {
        Resource {
            id: ResourceId {
                owner: "openclaw".into(),
                repo: "openclaw".into(),
                number: 81834,
                kind_hint: Some(ResourceKind::PullRequest),
            },
            title: "feat(senseaudio): add SenseAudio TTS provider".into(),
            url: "https://github.com/openclaw/openclaw/pull/81834".into(),
            state: "OPEN".into(),
            author: "KLilyZ".into(),
            created_at: "1mo".into(),
            updated_at: "1d".into(),
            labels: vec!["docs".into(), "size: L".into()],
            assignees: vec!["osolmaz".into()],
            reactions: ReactionCounts {
                thumbs_up: 2,
                ..ReactionCounts::default()
            },
            body: "## Summary\nProblem: senseaudio has ASR but no TTS.\nWhat changed: registers a speechProvider."
                .into(),
            activity: vec![ActivityEntry {
                id: "c1".into(),
                kind: ActivityKind::Comment,
                author: "github-actions".into(),
                body: "Dependency changes detected.".into(),
                updated_at: "1mo".into(),
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
            }],
            related_resources: vec![ResourceId {
                owner: "openclaw".into(),
                repo: "openclaw".into(),
                number: 66943,
                kind_hint: Some(ResourceKind::Issue),
            }],
            metadata: vec![
                MetadataItem {
                    label: "Draft".into(),
                    value: "no".into(),
                },
                MetadataItem {
                    label: "Cross repository".into(),
                    value: "yes".into(),
                },
            ],
            warnings: vec![],
            pull_request: Some(PullRequest {
                base_ref: "main".into(),
                head_ref: "feat/senseaudio-tts".into(),
                requested_reviewers: vec!["maintainer".into()],
                review_decision: None,
                merge_state: Some("CLEAN".into()),
                additions: 1100,
                deletions: 22,
                commits: vec![Commit {
                    oid: "fb948c9".into(),
                    message: "feat(senseaudio): add SenseAudio TTS provider".into(),
                    body: "Registers a SenseAudio speech provider.\n\nCo-Authored-By: Claude <noreply@anthropic.com>".into(),
                    author: "KLilyZ".into(),
                    authors: vec!["KLilyZ".into(), "claude".into()],
                    authored_at: Some("2w".into()),
                    committed_at: "1mo".into(),
                    status: CheckStatus::Success,
                    deployments: Vec::new(),
                }],
                checks: vec![CheckRun {
                    name: "ci/test".into(),
                    status: CheckStatus::Success,
                    summary: Some("86 successful".into()),
                    details_url: None,
                    started_at: None,
                    completed_at: None,
                    raw_status: None,
                    raw_conclusion: None,
                }],
                files: vec![ChangedFile {
                    path: "extensions/senseaudio/index.ts".into(),
                    additions: 3,
                    deletions: 1,
                    change_type: "MODIFIED".into(),
                    patch: Some(
                        "diff --git a/extensions/senseaudio/index.ts b/extensions/senseaudio/index.ts\n@@ -1,2 +1,3 @@\n import existing\n+import speech"
                            .into(),
                    ),
                }],
                metadata: vec![MetadataItem {
                    label: "Head ref OID".into(),
                    value: "fb4165fe62f1d126ba8c4bde3abe10fd7e985778".into(),
                }],
            }),
        }
    }

    fn long_body() -> String {
        (0..20)
            .map(|index| format!("line {index}: enough text to require expansion"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn draw(state: &mut AppState, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render_app(frame, state)).unwrap();
        format!("{:?}", terminal.backend().buffer())
    }

    fn draw_row_text(state: &mut AppState, width: u16, height: u16, y: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render_app(frame, state)).unwrap();
        (0..width)
            .map(|x| terminal.backend().buffer()[(x, y)].symbol())
            .collect::<Vec<_>>()
            .join("")
            .trim_end()
            .to_string()
    }

    fn draw_cell_symbol(state: &mut AppState, width: u16, height: u16, x: u16, y: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render_app(frame, state)).unwrap();
        terminal.backend().buffer()[(x, y)].symbol().to_string()
    }

    fn draw_cell_fg_for_text(
        state: &mut AppState,
        width: u16,
        height: u16,
        needle: &str,
        needle_offset: u16,
    ) -> Option<Color> {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render_app(frame, state)).unwrap();
        for y in 0..height {
            let row = (0..width)
                .map(|x| terminal.backend().buffer()[(x, y)].symbol())
                .collect::<Vec<_>>()
                .join("");
            if let Some(index) = row.find(needle) {
                let x = row[..index].chars().count() as u16 + needle_offset;
                return Some(terminal.backend().buffer()[(x, y)].fg);
            }
        }
        None
    }

    fn draw_cell_bg_for_text(
        state: &mut AppState,
        width: u16,
        height: u16,
        needle: &str,
        needle_offset: u16,
    ) -> Option<Color> {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render_app(frame, state)).unwrap();
        for y in 0..height {
            let row = (0..width)
                .map(|x| terminal.backend().buffer()[(x, y)].symbol())
                .collect::<Vec<_>>()
                .join("");
            if let Some(index) = row.find(needle) {
                let x = row[..index].chars().count() as u16 + needle_offset;
                return Some(terminal.backend().buffer()[(x, y)].bg);
            }
        }
        None
    }

    fn draw_column_symbols(
        state: &mut AppState,
        width: u16,
        height: u16,
        x: u16,
        y: u16,
        column_height: u16,
    ) -> Vec<String> {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render_app(frame, state)).unwrap();
        (0..column_height)
            .map(|offset| {
                terminal.backend().buffer()[(x, y.saturating_add(offset))]
                    .symbol()
                    .to_string()
            })
            .collect()
    }

    fn rendered_target_rect(state: &AppState, target: impl Fn(&HitTarget) -> bool) -> Option<Rect> {
        state
            .hit_areas
            .iter()
            .find(|area| target(&area.target))
            .map(|area| area.rect)
    }

    fn click_rendered_target(
        state: &mut AppState,
        target: impl Fn(&HitTarget) -> bool,
    ) -> AppIntent {
        let area = state
            .hit_areas
            .iter()
            .find(|area| target(&area.target))
            .cloned()
            .expect("expected rendered hit target");
        apply_event(
            state,
            AppEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: area.rect.x + area.rect.width.saturating_sub(1) / 2,
                row: area.rect.y + area.rect.height.saturating_sub(1) / 2,
                modifiers: KeyModifiers::empty(),
            }),
        )
    }

    #[test]
    fn renders_pr_overview_in_ascii_chrome() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new(pr_resource());

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();
        let content = format!("{:?}", terminal.backend().buffer());

        assert!(content.contains("https://github.com/openclaw/openclaw/pull/81834"));
        assert!(content.contains("[Overview]"));
        assert!(content.contains("checks PASS"));
        assert!(content.contains("* @KLilyZ opened"));
        assert!(content.contains("* commit fb948c9"));
        assert!(content.contains("[+ more]") || content.contains("Problem: senseaudio"));
        assert!(!content.contains("┌"));
        assert!(!content.contains("│"));
        assert!(content.contains("─"));
    }

    #[test]
    fn renders_resource_and_pr_metadata() {
        let backend = TestBackend::new(120, 80);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new(pr_resource());

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();
        let content = format!("{:?}", terminal.backend().buffer());

        assert!(content.contains("Draft: no"));
        assert!(content.contains("Cross repository: yes"));
        assert!(content.contains("Head ref OID: fb4165fe62f1d126ba8c4bde3abe10fd7e985778"));
    }

    #[test]
    fn overview_omits_redundant_tab_title_and_keeps_bulk_action_at_bottom() {
        let mut state = AppState::new(pr_resource());

        let content = draw(&mut state, 120, 80);
        let body = content.find("Problem: senseaudio").unwrap();
        let details = content.find("Details").unwrap();
        let expand = content.find("[expand").unwrap();

        assert!(!content.contains("Conversation"));
        assert!(body < details);
        assert!(details < expand);
    }

    #[test]
    fn overview_conversation_interleaves_body_commits_and_activity_by_time() {
        let mut resource = pr_resource();
        resource.created_at = "2026-01-01T00:00:00Z".into();
        resource.body = "Opening body".into();
        resource.pull_request.as_mut().unwrap().commits[0].committed_at =
            "2026-01-03T00:00:00Z".into();
        resource.pull_request.as_mut().unwrap().commits[0].message = "Middle commit".into();
        resource.activity[0].updated_at = "2026-01-02T00:00:00Z".into();
        resource.activity[0].body = "Earlier comment".into();
        let mut later_review = resource.activity[0].clone();
        later_review.id = "review-later".into();
        later_review.kind = ActivityKind::Review;
        later_review.updated_at = "2026-01-04T00:00:00Z".into();
        later_review.body = "Later review".into();
        resource.activity.push(later_review);
        let mut state = AppState::new(resource);

        let content = draw(&mut state, 120, 80);
        let body = content.find("Opening body").unwrap();
        let comment = content.find("Earlier comment").unwrap();
        let commit = content.find("Middle commit").unwrap();
        let review = content.find("Later review").unwrap();
        let details = content.find("Details").unwrap();

        assert!(body < comment);
        assert!(comment < commit);
        assert!(commit < review);
        assert!(review < details);
    }

    #[test]
    fn overview_conversation_starts_with_description_even_when_commit_is_older() {
        let mut resource = pr_resource();
        resource.created_at = "2026-01-02T00:00:00Z".into();
        resource.body = "Opening body".into();
        resource.pull_request.as_mut().unwrap().commits[0].committed_at =
            "2026-01-01T00:00:00Z".into();
        resource.pull_request.as_mut().unwrap().commits[0].message = "Older commit".into();
        resource.activity.clear();
        let mut state = AppState::new(resource);

        let content = draw(&mut state, 120, 80);
        let body = content.find("Opening body").unwrap();
        let commit = content.find("Older commit").unwrap();

        assert!(body < commit);
    }

    #[test]
    fn overview_reverse_order_keeps_description_after_chronological_items() {
        let mut resource = pr_resource();
        resource.body = "Opening body".into();
        resource.pull_request.as_mut().unwrap().commits[0].message =
            "Commit before activity".into();
        resource.activity[0].body = "Activity after commit".into();
        let mut state = AppState::new(resource);
        state.reverse_chronological = true;

        let content = draw(&mut state, 120, 80);
        let body = content.find("Opening body").unwrap();
        let commit = content.find("Commit before activity").unwrap();
        let activity = content.find("Activity after commit").unwrap();

        assert!(activity < commit);
        assert!(commit < body);
    }

    #[test]
    fn overview_conversation_keeps_relative_timestamp_order_stable() {
        let mut resource = pr_resource();
        resource.body = "Opening body".into();
        resource.pull_request.as_mut().unwrap().commits[0].message =
            "Commit before activity".into();
        resource.activity[0].body = "Activity after commit".into();
        let mut state = AppState::new(resource);

        let content = draw(&mut state, 120, 80);
        let body = content.find("Opening body").unwrap();
        let commit = content.find("Commit before activity").unwrap();
        let activity = content.find("Activity after commit").unwrap();

        assert!(body < commit);
        assert!(commit < activity);
    }

    #[test]
    fn renders_enrichment_warnings() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut resource = pr_resource();
        resource.warnings = vec!["timeline unavailable: permission denied".into()];
        let mut state = AppState::new(resource);

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();
        let content = format!("{:?}", terminal.backend().buffer());

        assert!(content.contains("Warnings: 1"));
        assert!(content.contains("Warnings"));
        assert!(content.contains("timeline unavailable"));
    }

    #[test]
    fn renders_review_and_merge_state_in_overview() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut resource = pr_resource();
        let pr = resource.pull_request.as_mut().unwrap();
        pr.review_decision = Some("REVIEW_REQUIRED".into());
        pr.merge_state = Some("BLOCKED".into());
        let mut state = AppState::new(resource);

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();
        let content = format!("{:?}", terminal.backend().buffer());

        assert!(content.contains("Review: Review Required"));
        assert!(content.contains("Merge: Blocked"));
    }

    #[test]
    fn renders_reaction_breakdown() {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut resource = pr_resource();
        resource.reactions.eyes = 1;
        resource.reactions.heart = 2;
        let mut state = AppState::new(resource);

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();
        let content = format!("{:?}", terminal.backend().buffer());

        assert!(content.contains("Reactions: +1:2 heart:2 eyes:1"));
    }

    #[test]
    fn status_band_shows_high_signal_pr_summary_without_activity_counts() {
        let backend = TestBackend::new(140, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut resource = pr_resource();
        let mut review = resource.activity[0].clone();
        review.id = "review-1".into();
        review.kind = ActivityKind::Review;
        review.body = "Approved".into();
        let mut review_comment = resource.activity[0].clone();
        review_comment.id = "thread-1".into();
        review_comment.kind = ActivityKind::ReviewComment;
        review_comment.thread_id = Some("thread-1".into());
        review_comment.thread_resolved = Some(false);
        let mut commit_comment = resource.activity[0].clone();
        commit_comment.id = "commit-comment-1".into();
        commit_comment.kind = ActivityKind::CommitComment;
        let mut timeline = resource.activity[0].clone();
        timeline.id = "timeline-1".into();
        timeline.kind = ActivityKind::Timeline;
        resource
            .activity
            .extend([review, review_comment, commit_comment, timeline]);
        if let Some(pr) = &mut resource.pull_request {
            let file = pr.files[0].clone();
            pr.files = vec![file; 22];
            pr.additions = 1250;
            pr.deletions = 195;
        }
        let mut state = AppState::new(resource);

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();
        let content = format!("{:?}", terminal.backend().buffer());

        assert!(content.contains("OK OPEN"));
        assert!(content.contains("feat/senseaudio-tts -> main"));
        assert!(content.contains("OK checks PASS"));
        assert!(content.contains("22 files changed +1.3k -195"));
        assert!(!content.contains("diff 1445"));
        let plus_offset = "22 files changed ".len() as u16;
        let minus_offset = plus_offset + "+1.3k ".len() as u16;
        let palette = Palette::default_dark();
        assert_eq!(
            draw_cell_fg_for_text(
                &mut state,
                140,
                36,
                "22 files changed +1.3k -195",
                plus_offset,
            ),
            Some(palette.green)
        );
        assert_eq!(
            draw_cell_fg_for_text(
                &mut state,
                140,
                36,
                "22 files changed +1.3k -195",
                minus_offset,
            ),
            Some(palette.red)
        );
        assert!(!content.contains("comments 2"));
        assert!(!content.contains("reviews 1"));
        assert!(!content.contains("timeline 1"));
        assert!(!content.contains("unresolved /"));
        assert_eq!(
            resource_state_badge_style(&state.resource, &Palette::default_dark()).bg,
            Some(Palette::default_dark().green)
        );
    }

    #[test]
    fn renders_last_refresh_status() {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new(pr_resource());
        state.mark_refreshed("12:34:56 UTC", false);

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();
        let content = format!("{:?}", terminal.backend().buffer());

        assert!(content.contains("Refresh: 12:34:56 no-change"));
    }

    #[test]
    fn renders_last_refresh_changed_sections() {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new(pr_resource());
        state.mark_refreshed("12:34:56 UTC", true);
        state.last_refresh_changed_sections = vec!["activity".into(), "checks".into()];

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();
        let content = format!("{:?}", terminal.backend().buffer());

        assert!(content.contains("Refresh: 12:34:56 changed"));
        assert!(content.contains("Changed: activity, checks"));
    }

    #[test]
    fn renders_loading_state_in_status_and_footer() {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new(pr_resource());
        state.begin_loading(
            state.resource.id.clone(),
            "refreshing openclaw/openclaw#81834 from GitHub",
        );

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();
        let content = format!("{:?}", terminal.backend().buffer());

        assert!(content.contains("feat/senseaudio-tts -> main"));
        assert!(content.contains("OK checks PASS"));
        assert!(content.contains("1 file changed +1.1k -22"));
        assert!(content.contains("Loading |: refreshing openclaw/openclaw#81834 from GitHub"));

        state.advance_loading_frame();
        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();
        let content = format!("{:?}", terminal.backend().buffer());

        assert!(content.contains("Loading /: refreshing openclaw/openclaw#81834 from GitHub"));
    }

    #[test]
    fn render_registers_tab_hit_areas() {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new(pr_resource());

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();

        assert!(state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::Tab(Tab::Checks)));
    }

    #[test]
    fn resource_tab_overflow_keeps_active_tab_visible() {
        let backend = TestBackend::new(64, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new(pr_resource());
        for number in 81835..81845 {
            let mut resource = pr_resource();
            resource.id.number = number;
            resource.title = format!("follow-up resource {number}");
            resource.url = format!("https://github.com/openclaw/openclaw/pull/{number}");
            state.open_resource_in_tab(resource);
        }
        let active = state.active_resource_tab;

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();

        assert!(state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::ResourceTab(active)));
    }

    #[test]
    fn add_resource_modal_registers_overlay_above_underlying_hits() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new(pr_resource());
        state.open_add_resource_prompt();

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();

        let overlay = state
            .hit_areas
            .iter()
            .find(|area| area.target == HitTarget::ModalOverlay)
            .expect("modal overlay hit area");
        let target = crate::input::hit_test(&state.hit_areas, overlay.rect.x, overlay.rect.y);

        assert_eq!(target, Some(HitTarget::ModalOverlay));
        assert!(state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::ConfirmResourcePrompt));
        assert!(state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::CancelResourcePrompt));
    }

    #[test]
    fn add_resource_modal_renders_inside_tiny_terminal() {
        let backend = TestBackend::new(12, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new(pr_resource());
        state.open_add_resource_prompt();

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();

        assert!(state.hit_areas.iter().any(|area| {
            area.target == HitTarget::ModalOverlay && area.rect.width <= 12 && area.rect.height <= 5
        }));
    }

    #[test]
    fn render_registers_more_button_hit_area() {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut resource = pr_resource();
        resource.body = (0..30)
            .map(|index| format!("line {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut state = AppState::new(resource);

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();

        assert!(state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::ToggleBlock(BlockId::Body)));
    }

    #[test]
    fn render_registers_github_link_hit_area() {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut resource = pr_resource();
        resource.body = "Related: https://github.com/openclaw/openclaw/issues/66943".into();
        let mut state = AppState::new(resource);
        state.set_tab(Tab::Links);

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();

        assert!(state.hit_areas.iter().any(|area| matches!(
            &area.target,
            HitTarget::ResourceLink { id, .. } if id.number == 66943 && id.kind_hint == Some(ResourceKind::Issue)
        )));
    }

    #[test]
    fn render_registers_relative_issue_link_hit_area() {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut resource = pr_resource();
        resource.body = "Pairs with #66943".into();
        resource.related_resources.clear();
        let mut state = AppState::new(resource);
        state.set_tab(Tab::Links);

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();

        assert!(state.hit_areas.iter().any(|area| matches!(
            &area.target,
            HitTarget::ResourceLink { id, .. } if id.canonical_name() == "openclaw/openclaw#66943"
        )));
    }

    #[test]
    fn render_registers_visible_overview_link_hit_area() {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut resource = pr_resource();
        resource.body = "Pairs with #66943".into();
        let mut state = AppState::new(resource);

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();

        assert!(state.hit_areas.iter().any(|area| matches!(
            &area.target,
            HitTarget::ResourceLink { id, .. } if id.canonical_name() == "openclaw/openclaw#66943"
        )));
    }

    #[test]
    fn render_registers_owner_repo_hash_link_hit_area() {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut resource = pr_resource();
        resource.body = "Cross repo follow-up dutifuldev/ghzinga#12.".into();
        resource.related_resources.clear();
        let mut state = AppState::new(resource);

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();

        assert!(state.hit_areas.iter().any(|area| matches!(
            &area.target,
            HitTarget::ResourceLink { id, .. } if id.canonical_name() == "dutifuldev/ghzinga#12"
        )));
    }

    #[test]
    fn render_registers_markdown_relative_issue_link_hit_area() {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut resource = pr_resource();
        resource.body = "Pairs with [tracking issue](#66943).".into();
        let mut state = AppState::new(resource);

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();

        assert!(state.hit_areas.iter().any(|area| matches!(
            &area.target,
            HitTarget::ResourceLink { id, .. } if id.canonical_name() == "openclaw/openclaw#66943"
        )));
    }

    #[test]
    fn links_tab_registers_markdown_absolute_pr_link_hit_area() {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut resource = pr_resource();
        resource.body =
            "Pairs with [follow-up PR](https://github.com/openclaw/openclaw/pull/81835).".into();
        resource.related_resources.clear();
        let mut state = AppState::new(resource);
        state.set_tab(Tab::Links);

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();
        let content = format!("{:?}", terminal.backend().buffer());

        assert!(content.contains("https://github.com/openclaw/openclaw/pull/81835"));
        assert!(state.hit_areas.iter().any(|area| matches!(
            &area.target,
            HitTarget::ResourceLink { id, .. }
                if id.number == 81835 && id.kind_hint == Some(ResourceKind::PullRequest)
        )));
    }

    #[test]
    fn links_tab_detects_owner_repo_hash_references() {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut resource = pr_resource();
        resource.body = "Cross repo follow-up dutifuldev/ghzinga#12.".into();
        resource.related_resources.clear();
        let mut state = AppState::new(resource);
        state.set_tab(Tab::Links);

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();
        let content = format!("{:?}", terminal.backend().buffer());

        assert!(content.contains("dutifuldev/ghzinga#12"));
        assert!(state.hit_areas.iter().any(|area| matches!(
            &area.target,
            HitTarget::ResourceLink { id, .. } if id.canonical_name() == "dutifuldev/ghzinga#12"
        )));
    }

    #[test]
    fn links_tab_omits_non_resource_markdown_urls() {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut resource = pr_resource();
        resource.body = "Docs [file](https://github.com/openclaw/openclaw/blob/main/README.md) and [issue](#66943).".into();
        resource.related_resources.clear();
        let mut state = AppState::new(resource);
        state.set_tab(Tab::Links);

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();
        let content = format!("{:?}", terminal.backend().buffer());

        assert!(content.contains("openclaw/openclaw#66943"));
        assert!(!content.contains("blob/main/README.md"));
        assert!(state.hit_areas.iter().all(|area| {
            matches!(
                area.target,
                HitTarget::ResourceLink { .. }
                    | HitTarget::Navigate(_)
                    | HitTarget::Tab(_)
                    | HitTarget::OpenHeaderUrl(_)
                    | HitTarget::Refresh
                    | HitTarget::OpenResourcePrompt
                    | HitTarget::CopyVisibleUrl
                    | HitTarget::OpenVisibleUrl
                    | HitTarget::Settings
                    | HitTarget::Help
                    | HitTarget::Quit
            )
        }));
    }

    #[test]
    fn render_registers_markdown_permalink_as_resource_link() {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut resource = pr_resource();
        resource.body =
            "See [thread](https://github.com/openclaw/openclaw/pull/81834#discussion_r1).".into();
        let mut state = AppState::new(resource);

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();

        assert!(state.hit_areas.iter().any(|area| matches!(
            &area.target,
            HitTarget::ResourceLink { id, url: Some(url) }
                if id.canonical_name() == "openclaw/openclaw#81834"
                    && url == "https://github.com/openclaw/openclaw/pull/81834#discussion_r1"
        )));
    }

    #[test]
    fn links_tab_renders_explicit_related_resources_once() {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut resource = pr_resource();
        resource.body = "Also see #66943".into();
        let mut state = AppState::new(resource);
        state.set_tab(Tab::Links);

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();
        let content = format!("{:?}", terminal.backend().buffer());

        assert_eq!(content.matches("openclaw/openclaw#66943").count(), 1);
        assert!(state.hit_areas.iter().any(|area| matches!(
            &area.target,
            HitTarget::ResourceLink { id, .. } if id.canonical_name() == "openclaw/openclaw#66943"
        )));
    }

    #[test]
    fn render_registers_visible_activity_link_hit_area() {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut resource = pr_resource();
        resource.activity[0].body =
            "Follow up at https://github.com/openclaw/openclaw/pull/81835".into();
        let mut state = AppState::new(resource);
        state.set_tab(Tab::Activity);

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();

        assert!(state.hit_areas.iter().any(|area| matches!(
            &area.target,
            HitTarget::ResourceLink { id, .. }
                if id.number == 81835 && id.kind_hint == Some(ResourceKind::PullRequest)
        )));
    }

    #[test]
    fn render_registers_exact_comment_url_as_resource_link() {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut resource = pr_resource();
        resource.activity[0].body =
            "Permalink https://github.com/openclaw/openclaw/pull/81834#discussion_r1".into();
        let mut state = AppState::new(resource);
        state.set_tab(Tab::Activity);

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();

        assert!(state.hit_areas.iter().any(|area| matches!(
            &area.target,
            HitTarget::ResourceLink { id, url: Some(url) }
                if id.canonical_name() == "openclaw/openclaw#81834"
                    && url == "https://github.com/openclaw/openclaw/pull/81834#discussion_r1"
        )));
    }

    #[test]
    fn render_registers_footer_action_hit_areas() {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new(pr_resource());

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();
        let content = format!("{:?}", terminal.backend().buffer());

        assert!(content.contains("[refresh] [expand"));
        assert!(!content.contains("[copy]"));
        assert!(!content.contains("[open]"));
        assert!(!content.contains("scroll 0/"));
        assert!(content.contains("[expand"));
        assert!(state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::Refresh));
        assert!(!state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::CopyVisibleUrl));
        assert!(!state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::OpenVisibleUrl));
        assert!(state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::Settings));
        assert!(state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::Quit));
        assert!(state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::Help));
        let quit_rect =
            rendered_target_rect(&state, |target| *target == HitTarget::Quit).expect("quit target");
        let expand_rect = rendered_target_rect(
            &state,
            |target| matches!(target, HitTarget::ExpandBlocks(blocks) if !blocks.is_empty()),
        )
        .expect("expand all target");
        let footer = chrome_area_for_spacing(
            rects_for_spacing(Rect::new(0, 0, 120, 36), state.spacing).footer,
            state.spacing,
        );

        let refresh_rect =
            rendered_target_rect(&state, |target| *target == HitTarget::Refresh).unwrap();
        let settings_rect =
            rendered_target_rect(&state, |target| *target == HitTarget::Settings).unwrap();
        assert_eq!(expand_rect.y, quit_rect.y);
        assert_eq!(expand_rect.y, footer.y + footer.height.saturating_sub(2));
        assert!(expand_rect.x > refresh_rect.x);
        assert!(expand_rect.x < settings_rect.x);
    }

    #[test]
    fn footer_shows_load_full_only_for_partial_depth_resources() {
        let mut complete_state = AppState::new(pr_resource());
        let complete_content = draw(&mut complete_state, 120, 36);

        assert!(!complete_content.contains("[load full]"));
        assert!(!complete_state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::LoadFullDepth));

        let mut partial_resource = pr_resource();
        partial_resource.warnings.push(format!(
            "normal API depth shows the first 100 only for comments; {FULL_DEPTH_WARNING_HINT} for exhaustive pagination"
        ));
        let mut partial_state = AppState::new(partial_resource);
        let partial_content = draw(&mut partial_state, 120, 36);

        assert!(partial_content.contains("[refresh] [expand"));
        let load_full = partial_content.find("[load full]").unwrap();
        let expand = partial_content.find("[expand").unwrap();
        let settings = partial_content.find("[settings]").unwrap();
        assert!(expand < load_full);
        assert!(load_full < settings);
        assert!(partial_state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::LoadFullDepth));
    }

    #[test]
    fn footer_omits_verbose_shortcut_and_scroll_hint() {
        let mut state = AppState::new(pr_resource());

        let content = draw(&mut state, 120, 36);

        assert!(!content.contains("scroll 0/"));
        assert!(!content.contains("arrows/page scroll"));
        assert!(!content.contains("1-6/tab switch"));
    }

    #[test]
    fn footer_expand_and_collapse_labels_keep_stable_width() {
        let symbols = SymbolMode::Emoji.symbols();
        let blocks = vec![BlockId::Body];
        let expanded = std::collections::HashSet::from([BlockId::Body]);
        let collapsed = std::collections::HashSet::new();

        let (expand, _) = expand_all_control(blocks.clone(), &collapsed, &symbols).unwrap();
        let (collapse, _) = expand_all_control(blocks, &expanded, &symbols).unwrap();

        assert_eq!(
            UnicodeWidthStr::width(expand.as_str()),
            UnicodeWidthStr::width(collapse.as_str())
        );
    }

    #[test]
    fn content_scrollbar_is_transient_after_scroll_input() {
        let mut state = AppState::new(pr_resource());

        let initial = draw(&mut state, 120, 20);
        assert!(!initial.contains('█'));

        state.scroll_down(5);
        let scrolling = draw(&mut state, 120, 20);

        assert!(scrolling.contains('█'));
        for _ in 0..12 {
            draw(&mut state, 120, 20);
        }
        let settled = draw(&mut state, 120, 20);
        assert!(!settled.contains('█'));
    }

    #[test]
    fn content_scrollbar_modes_control_rendering_and_hit_area() {
        let mut always = AppState::new(pr_resource());
        always.scrollbar = ScrollbarMode::Always;
        let content = draw(&mut always, 120, 20);
        assert!(content.contains('█'));
        assert!(always
            .hit_areas
            .iter()
            .any(|area| matches!(area.target, HitTarget::Scrollbar { .. })));

        let mut hidden = AppState::new(pr_resource());
        hidden.scrollbar = ScrollbarMode::Hidden;
        hidden.scroll_down(5);
        let content = draw(&mut hidden, 120, 20);
        assert!(!content.contains('█'));
        assert!(hidden
            .hit_areas
            .iter()
            .all(|area| !matches!(area.target, HitTarget::Scrollbar { .. })));
    }

    #[test]
    fn content_scrollbar_reaches_bottom_at_scroll_limit() {
        let mut resource = pr_resource();
        let activity = resource.activity[0].clone();
        for _ in 0..20 {
            resource.activity.push(activity.clone());
        }
        let mut state = AppState::new(resource);
        state.set_tab(Tab::Activity);
        draw(&mut state, 120, 20);
        state.scroll_to_bottom();

        let rects = rects_for_spacing(Rect::new(0, 0, 120, 20), state.spacing);
        let content_area =
            content_area_for_spacing(rects.content, state.spacing, active_content_tab(&state));
        let symbol = draw_cell_symbol(
            &mut state,
            120,
            20,
            content_area.x + content_area.width - 1,
            content_area.y + content_area.height - 1,
        );

        assert_eq!(symbol, "█");
    }

    #[test]
    fn content_scrollbar_snaps_to_bottom_across_common_sizes() {
        for (width, height) in [(80, 24), (120, 36), (160, 50)] {
            let mut resource = pr_resource();
            let activity = resource.activity[0].clone();
            for _ in 0..40 {
                resource.activity.push(activity.clone());
            }
            let mut state = AppState::new(resource);
            state.set_tab(Tab::Activity);
            draw(&mut state, width, height);
            state.scroll_to_bottom();

            let rects = rects_for_spacing(Rect::new(0, 0, width, height), state.spacing);
            let content_area =
                content_area_for_spacing(rects.content, state.spacing, active_content_tab(&state));
            let symbol = draw_cell_symbol(
                &mut state,
                width,
                height,
                content_area.x + content_area.width - 1,
                content_area.y + content_area.height - 1,
            );

            assert_eq!(symbol, "█", "size {width}x{height}");
        }
    }

    #[test]
    fn content_scrollbar_bottom_endpoint_is_contiguous_thumb() {
        let mut resource = pr_resource();
        let activity = resource.activity[0].clone();
        for _ in 0..40 {
            resource.activity.push(activity.clone());
        }
        let mut state = AppState::new(resource);
        state.set_tab(Tab::Activity);
        draw(&mut state, 120, 36);
        state.scroll_to_bottom();

        let rects = rects_for_spacing(Rect::new(0, 0, 120, 36), state.spacing);
        let content_area =
            content_area_for_spacing(rects.content, state.spacing, active_content_tab(&state));
        let symbols = draw_column_symbols(
            &mut state,
            120,
            36,
            content_area.x + content_area.width - 1,
            content_area.y,
            content_area.height,
        );
        let trailing_thumb = symbols
            .iter()
            .rev()
            .take_while(|symbol| symbol.as_str() == "█")
            .count();
        let thumb_above_endpoint = symbols[..symbols.len().saturating_sub(trailing_thumb)]
            .iter()
            .any(|symbol| symbol == "█");

        assert!(trailing_thumb > 0);
        assert!(!thumb_above_endpoint);
    }

    #[test]
    fn scrollbar_content_length_tracks_rows_not_only_scroll_limit() {
        let mut state = AppState::new(pr_resource());
        state.scroll_limit = 80;

        assert_eq!(scrollbar_content_length(&state, 20, 100), 100);
        assert_eq!(scrollbar_content_length(&state, 20, 0), 100);

        state.scroll = 80;
        assert_eq!(scrollbar_position(&state, 100), 99);
    }

    #[test]
    fn scrollbar_endpoint_thumb_length_keeps_visible_proportion() {
        assert_eq!(scrollbar_endpoint_thumb_length(20, 100), 3);
        assert_eq!(scrollbar_endpoint_thumb_length(20, 20), 10);
        assert_eq!(scrollbar_endpoint_thumb_length(20, 10), 14);
        assert_eq!(scrollbar_endpoint_thumb_length(0, 10), 0);
    }

    #[test]
    fn oversized_status_pieces_wrap_without_early_ellipsis() {
        let lines = wrap_styled_pieces(
            &[
                StyledPiece {
                    segments: vec![StyledSegment {
                        text: "OK OPEN".into(),
                        style: Style::default(),
                    }],
                },
                StyledPiece {
                    segments: vec![StyledSegment {
                        text: "assignees @extraordinarily-long-user-name @second-reviewer".into(),
                        style: Style::default().add_modifier(Modifier::BOLD),
                    }],
                },
            ],
            14,
        );
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");

        assert!(lines.len() > 2);
        assert!(lines
            .iter()
            .all(|line| UnicodeWidthStr::width(line_text(line).as_str()) <= 14));
        assert!(!text.contains("..."));
        assert!(text.contains("@second"));
    }

    #[test]
    fn extremely_narrow_tabs_fit_visible_width() {
        let mut state = AppState::new(pr_resource());

        let content = draw(&mut state, 6, 20);

        assert!(content.contains("[Ov..."));
        assert!(state
            .hit_areas
            .iter()
            .filter(|area| matches!(area.target, HitTarget::Tab(_)))
            .all(|area| area.rect.width <= 6));
    }

    #[test]
    fn extremely_narrow_footer_controls_fit_visible_width() {
        let mut state = AppState::new(pr_resource());

        let content = draw(&mut state, 8, 20);

        assert!(content.contains("[refr..."));
        assert!(state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::Refresh));
        assert!(state
            .hit_areas
            .iter()
            .filter(|area| {
                matches!(
                    area.target,
                    HitTarget::Refresh
                        | HitTarget::CopyVisibleUrl
                        | HitTarget::OpenVisibleUrl
                        | HitTarget::Help
                        | HitTarget::Quit
                )
            })
            .all(|area| area.rect.width <= 8));
    }

    #[test]
    fn narrow_render_wraps_chrome_without_losing_click_targets() {
        let mut resource = pr_resource();
        resource.title =
            "feat(senseaudio): add a provider with long metadata that needs wrapping".into();
        let mut state = AppState::new(resource);
        state.status_message =
            Some("background refresh finished after collecting checks and comments".into());

        let content = draw(&mut state, 24, 24);
        let mut tab_rows = state
            .hit_areas
            .iter()
            .filter_map(|area| matches!(area.target, HitTarget::Tab(_)).then_some(area.rect.y))
            .collect::<Vec<_>>();
        tab_rows.sort_unstable();
        tab_rows.dedup();
        let mut footer_rows = state
            .hit_areas
            .iter()
            .filter_map(|area| {
                matches!(
                    area.target,
                    HitTarget::Refresh | HitTarget::Help | HitTarget::Settings | HitTarget::Quit
                )
                .then_some(area.rect.y)
            })
            .collect::<Vec<_>>();
        footer_rows.sort_unstable();
        footer_rows.dedup();

        assert!(content.contains("[Overview]"));
        assert!(content.contains("[refresh]"));
        assert!(content.contains("background"));
        assert!(tab_rows.len() > 1);
        assert!(footer_rows.len() > 1);
        assert!(state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::Refresh));
        assert!(!state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::CopyVisibleUrl));
        assert!(!state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::OpenVisibleUrl));
        assert!(state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::Help));
        assert!(state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::Quit));
    }

    #[test]
    fn content_rows_wrap_long_text_and_preserve_click_targets() {
        let rows = wrap_content_rows(
            vec![ContentRow::target_styled(
                "details: https://github.com/openclaw/openclaw/pull/81834#discussion_r1234567890🙂tail",
                HitTarget::OpenUrl(
                    "https://github.com/openclaw/openclaw/pull/81834#discussion_r1234567890🙂tail"
                        .into(),
                ),
                link_style(&Palette::default_dark()),
            )],
            24,
            SpacingMode::Compact,
        );

        assert!(rows.len() > 1);
        assert!(rows
            .iter()
            .all(|row| UnicodeWidthStr::width(line_text(&row.line).as_str()) <= 24));
        assert!(rows.iter().all(|row| matches!(
            &row.target,
            Some(HitTarget::OpenUrl(url))
                if url == "https://github.com/openclaw/openclaw/pull/81834#discussion_r1234567890🙂tail"
        )));
    }

    #[test]
    fn comfortable_wrapped_rows_use_hanging_indent() {
        let rows = wrap_content_rows(
            vec![ContentRow::plain(
                "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda",
            )],
            50,
            SpacingMode::Comfortable,
        );

        assert!(rows.len() > 1);
        assert!(!line_text(&rows[0].line).starts_with("  "));
        assert!(line_text(&rows[1].line).starts_with("  "));
        assert!(rows
            .iter()
            .all(|row| UnicodeWidthStr::width(line_text(&row.line).as_str()) <= 50));
    }

    #[test]
    fn compact_wrapped_rows_stay_flush_left() {
        let rows = wrap_content_rows(
            vec![ContentRow::plain(
                "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda",
            )],
            50,
            SpacingMode::Compact,
        );

        assert!(rows.len() > 1);
        assert!(rows
            .iter()
            .all(|row| !line_text(&row.line).starts_with("  ")));
    }

    #[test]
    fn comfortable_spacing_adds_content_gutter_when_width_allows() {
        let area = Rect::new(0, 4, 120, 20);

        let comfortable = content_area_for_spacing(area, SpacingMode::Comfortable, Tab::Overview);
        let compact = content_area_for_spacing(area, SpacingMode::Compact, Tab::Overview);

        assert_eq!(comfortable.x, 2);
        assert_eq!(comfortable.width, 116);
        assert_eq!(compact, area);
    }

    #[test]
    fn comfortable_spacing_caps_reading_tabs_on_wide_terminals() {
        let area = Rect::new(0, 4, 160, 20);

        let overview = content_area_for_spacing(area, SpacingMode::Comfortable, Tab::Overview);
        let activity = content_area_for_spacing(area, SpacingMode::Comfortable, Tab::Activity);

        assert_eq!(overview.x, 2);
        assert_eq!(overview.width, 118);
        assert_eq!(activity.x, 2);
        assert_eq!(activity.width, 118);
    }

    #[test]
    fn width_mode_full_uses_available_content_width() {
        let area = Rect::new(0, 4, 160, 20);

        let full = content_area_for_preferences(
            area,
            SpacingMode::Comfortable,
            ContentWidthMode::Full,
            118,
            Tab::Overview,
        );

        assert_eq!(full.x, 2);
        assert_eq!(full.width, 156);
    }

    #[test]
    fn fixed_width_setting_controls_reading_width() {
        let area = Rect::new(0, 4, 180, 20);

        let fixed = content_area_for_preferences(
            area,
            SpacingMode::Comfortable,
            ContentWidthMode::Fixed,
            132,
            Tab::Overview,
        );

        assert_eq!(fixed.x, 2);
        assert_eq!(fixed.width, 132);
    }

    #[test]
    fn comfortable_spacing_keeps_files_full_width_for_diffs() {
        let area = Rect::new(0, 4, 160, 20);

        let files = content_area_for_spacing(area, SpacingMode::Comfortable, Tab::Files);

        assert_eq!(files.x, 2);
        assert_eq!(files.width, 156);
    }

    #[test]
    fn comfortable_spacing_keeps_extremely_narrow_content_full_width() {
        let area = Rect::new(0, 4, 40, 20);

        assert_eq!(
            content_area_for_spacing(area, SpacingMode::Comfortable, Tab::Overview),
            area
        );
    }

    #[test]
    fn comfortable_content_gutter_moves_click_targets_with_visible_rows() {
        let mut resource = pr_resource();
        resource.body = (0..30)
            .map(|index| format!("line {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut comfortable_state = AppState::new(resource.clone());
        let mut compact_state = AppState::new(resource);
        compact_state.spacing = SpacingMode::Compact;

        draw(&mut comfortable_state, 120, 80);
        draw(&mut compact_state, 120, 80);

        let comfortable_rect = rendered_target_rect(&comfortable_state, |target| {
            *target == HitTarget::ToggleBlock(BlockId::Body)
        })
        .expect("comfortable body expansion target");
        let compact_rect = rendered_target_rect(&compact_state, |target| {
            *target == HitTarget::ToggleBlock(BlockId::Body)
        })
        .expect("compact body expansion target");

        assert_eq!(comfortable_rect.x, 2);
        assert_eq!(comfortable_rect.width, 116);
        assert_eq!(compact_rect.x, 0);
        assert_eq!(compact_rect.width, 120);
    }

    #[test]
    fn comfortable_spacing_pads_chrome_click_targets_equally() {
        let mut comfortable_state = AppState::new(pr_resource());
        let mut compact_state = AppState::new(pr_resource());
        compact_state.spacing = SpacingMode::Compact;

        draw(&mut comfortable_state, 120, 36);
        draw(&mut compact_state, 120, 36);

        let comfortable_tab = rendered_target_rect(&comfortable_state, |target| {
            *target == HitTarget::Tab(Tab::Activity)
        })
        .expect("comfortable tab target");
        let compact_tab = rendered_target_rect(&compact_state, |target| {
            *target == HitTarget::Tab(Tab::Activity)
        })
        .expect("compact tab target");
        let comfortable_footer =
            rendered_target_rect(&comfortable_state, |target| *target == HitTarget::Refresh)
                .expect("comfortable footer target");
        let compact_footer =
            rendered_target_rect(&compact_state, |target| *target == HitTarget::Refresh)
                .expect("compact footer target");

        assert_eq!(comfortable_tab.x, compact_tab.x + COMFORTABLE_GUTTER);
        assert_eq!(comfortable_footer.x, compact_footer.x + COMFORTABLE_GUTTER);
    }

    #[test]
    fn comfortable_spacing_reserves_nav_padding_and_separator() {
        let mut state = AppState::new(pr_resource());
        let base = ViewRects::compute(Rect::new(0, 0, 120, 36));
        let rects = rects_for_spacing(Rect::new(0, 0, 120, 36), state.spacing);
        let tabs = chrome_area_for_spacing(rects.tabs, state.spacing);

        assert_eq!(rects.tabs.height, base.tabs.height + 2);
        assert_eq!(rects.content.y, base.content.y + 3);

        let separator = draw_cell_symbol(
            &mut state,
            120,
            36,
            tabs.x,
            tabs.y + tabs.height.saturating_sub(1),
        );
        assert_eq!(separator, "─");
    }

    #[test]
    fn comfortable_footer_keeps_padding_below_buttons() {
        let mut state = AppState::new(pr_resource());
        let rects = rects_for_spacing(Rect::new(0, 0, 120, 36), state.spacing);
        let footer = chrome_area_for_spacing(rects.footer, state.spacing);

        let bottom_row = draw_row_text(&mut state, 120, 36, 35);
        let refresh_rect =
            rendered_target_rect(&state, |target| *target == HitTarget::Refresh).unwrap();

        assert_eq!(bottom_row.trim(), "");
        assert_eq!(refresh_rect.y, footer.y + footer.height.saturating_sub(2));
    }

    #[test]
    fn comfortable_spacing_adds_breathing_room_after_section_rules() {
        fn rows() -> Vec<ContentRow> {
            vec![
                heading_row("Activity", &Palette::default_dark()),
                separator_row(12, &Palette::default_dark()),
                ContentRow::plain("Comment"),
            ]
        }

        let comfortable = apply_spacing(rows(), SpacingMode::Comfortable);
        let compact = apply_spacing(rows(), SpacingMode::Compact);

        assert_eq!(compact.len(), 3);
        assert_eq!(comfortable.len(), 4);
        assert_eq!(line_text(&comfortable[2].line), "");
        assert_eq!(line_text(&comfortable[3].line), "Comment");
    }

    #[test]
    fn comfortable_overview_timeline_pads_items_above_and_below_separators() {
        let mut resource = pr_resource();
        let activity = resource.activity[0].clone();
        resource.activity.push(ActivityEntry {
            id: "c2".into(),
            updated_at: "2mo".into(),
            ..activity
        });
        let mut state = AppState::new(resource);

        let rows = apply_spacing(
            overview_rows(&mut state, 80, &Palette::default_dark()),
            SpacingMode::Comfortable,
        );
        let separator_index = rows
            .iter()
            .position(is_section_rule)
            .expect("timeline separator");

        assert!(separator_index > 0);
        assert_eq!(line_text(&rows[separator_index - 1].line), "");
        assert_eq!(line_text(&rows[separator_index + 1].line), "");
    }

    #[test]
    fn comfortable_content_frame_adds_vertical_padding() {
        let palette = Palette::default_dark();
        let rows = apply_content_frame_spacing(
            vec![ContentRow::plain("First item")],
            12,
            SpacingMode::Comfortable,
            &palette,
        );

        assert_eq!(line_text(&rows[0].line), "");
        assert_eq!(line_text(&rows[1].line), "First item");
        assert_eq!(line_text(&rows[2].line), "");
    }

    #[test]
    fn comfortable_content_frame_normalizes_edge_blanks_before_padding() {
        let palette = Palette::default_dark();
        let rows = apply_content_frame_spacing(
            vec![
                ContentRow::plain(""),
                ContentRow::plain("First item"),
                ContentRow::plain(""),
                ContentRow::plain(""),
            ],
            12,
            SpacingMode::Comfortable,
            &palette,
        );

        assert_eq!(rows.len(), 3);
        assert_eq!(line_text(&rows[0].line), "");
        assert_eq!(line_text(&rows[1].line), "First item");
        assert_eq!(line_text(&rows[2].line), "");
    }

    #[test]
    fn compact_content_frame_does_not_add_nav_separator_or_padding() {
        let palette = Palette::default_dark();
        let rows = apply_content_frame_spacing(
            vec![ContentRow::plain("First item")],
            12,
            SpacingMode::Compact,
            &palette,
        );

        assert_eq!(rows.len(), 1);
        assert_eq!(line_text(&rows[0].line), "First item");
    }

    #[test]
    fn comfortable_spacing_skips_existing_blank_after_rule() {
        let rows = vec![
            heading_row("Activity", &Palette::default_dark()),
            separator_row(12, &Palette::default_dark()),
            ContentRow::plain(""),
        ];

        let comfortable = apply_spacing(rows, SpacingMode::Comfortable);

        assert_eq!(comfortable.len(), 3);
        assert_eq!(line_text(&comfortable[2].line), "");
    }

    #[test]
    fn comfortable_spacing_adds_single_gap_after_marked_rows() {
        let rows = vec![
            ContentRow::plain("first").with_comfortable_gap_after(),
            ContentRow::plain("second").with_comfortable_gap_after(),
            ContentRow::plain(""),
            ContentRow::plain("third"),
        ];

        let comfortable = apply_spacing(rows, SpacingMode::Comfortable);

        assert_eq!(comfortable.len(), 5);
        assert_eq!(line_text(&comfortable[0].line), "first");
        assert_eq!(line_text(&comfortable[1].line), "");
        assert_eq!(line_text(&comfortable[2].line), "second");
        assert_eq!(line_text(&comfortable[3].line), "");
        assert_eq!(line_text(&comfortable[4].line), "third");
    }

    #[test]
    fn compact_spacing_ignores_comfortable_gap_markers() {
        let rows = vec![
            ContentRow::plain("first").with_comfortable_gap_after(),
            ContentRow::plain("second"),
        ];

        let compact = apply_spacing(rows, SpacingMode::Compact);

        assert_eq!(compact.len(), 2);
        assert_eq!(line_text(&compact[0].line), "first");
        assert_eq!(line_text(&compact[1].line), "second");
    }

    #[test]
    fn comfortable_spacing_separates_repeated_file_rows() {
        let resource = pr_resource();
        let mut pr = resource.pull_request.unwrap();
        let mut second = pr.files[0].clone();
        second.path = "docs/second-file.md".into();
        pr.files.push(second);

        let rows = files_rows_for_pr(
            &pr,
            120,
            &std::collections::HashSet::new(),
            &Palette::default_dark(),
            &SymbolMode::Ascii.symbols(),
        );
        let compact = apply_spacing(rows, SpacingMode::Compact);
        let comfortable = apply_spacing(
            files_rows_for_pr(
                &pr,
                120,
                &std::collections::HashSet::new(),
                &Palette::default_dark(),
                &SymbolMode::Ascii.symbols(),
            ),
            SpacingMode::Comfortable,
        );

        let first_compact = compact
            .iter()
            .position(|row| line_text(&row.line).contains("extensions/senseaudio/index.ts"))
            .unwrap();
        let first_comfortable = comfortable
            .iter()
            .position(|row| line_text(&row.line).contains("extensions/senseaudio/index.ts"))
            .unwrap();

        assert!(line_text(&compact[first_compact + 1].line).contains("docs/second-file.md"));
        assert_eq!(line_text(&comfortable[first_comfortable + 1].line), "");
        assert!(line_text(&comfortable[first_comfortable + 2].line).contains("docs/second-file.md"));
    }

    #[test]
    fn narrow_content_wraps_metadata_without_clipping() {
        let mut resource = pr_resource();
        resource.pull_request = None;
        resource.activity.clear();
        resource.body.clear();
        resource.related_resources.clear();
        resource.labels.clear();
        resource.assignees.clear();
        resource.metadata = vec![MetadataItem {
            label: "Very long metadata label".into(),
            value: "https://github.com/openclaw/openclaw/pull/81834#issuecomment-very-long-responsive-url🙂tail".into(),
        }];
        let mut state = AppState::new(resource);
        state.spacing = SpacingMode::Compact;

        let content = draw(&mut state, 32, 28);

        assert!(content.contains("Very long metadata label"));
        assert!(content.contains("https://github.com/openclaw"));
        assert!(state.hit_areas.iter().all(|area| area.rect.width <= 32));
    }

    #[test]
    fn header_wrap_keeps_identity_state_updated_and_title_visible() {
        let mut resource = pr_resource();
        resource.title = "Very long pull request title that needs a reliable wrapped header".into();
        resource.updated_at = "2026-06-01T12:00:00Z".into();
        let mut state = AppState::new(resource);
        state.spacing = SpacingMode::Compact;

        let content = draw(&mut state, 36, 24);

        assert!(content.contains("openclaw / openclaw #81834"));
        assert!(!content.contains("openclaw/openclaw#81834"));
        assert!(content.contains("[PR OPEN]"));
        assert!(content.contains("updated"));
        assert!(content.contains("Very long pull request"));
    }

    #[test]
    fn comfortable_header_adds_top_padding_without_losing_title() {
        let mut resource = pr_resource();
        resource.title = "Readable title after padded identity".into();
        let mut state = AppState::new(resource);
        state.spacing = SpacingMode::Comfortable;

        let top_row = draw_row_text(&mut state, 120, 36, 0);
        let identity_row = draw_row_text(&mut state, 120, 36, 1);
        let content = draw(&mut state, 120, 36);

        assert_eq!(top_row.trim(), "");
        assert!(identity_row.contains("https://github.com/openclaw/openclaw/pull/81834"));
        assert!(content.contains("Readable title after padded identity"));
    }

    #[test]
    fn header_identity_and_title_use_highlight_background() {
        let mut resource = pr_resource();
        resource.title = "Highlighted title block".into();
        let mut state = AppState::new(resource);
        state.spacing = SpacingMode::Compact;
        let palette = state.theme.palette();

        let identity_bg = draw_cell_bg_for_text(
            &mut state,
            120,
            36,
            "https://github.com/openclaw/openclaw/pull/81834",
            0,
        )
        .expect("identity cell background");
        let title_bg = draw_cell_bg_for_text(&mut state, 120, 36, "Highlighted title block", 0)
            .expect("title cell background");

        assert_eq!(identity_bg, palette.surface0);
        assert_eq!(title_bg, palette.surface0);
        assert_ne!(identity_bg, palette.panel_bg);
    }

    #[test]
    fn header_identity_is_clickable_github_link() {
        let mut state = AppState::new(pr_resource());
        let content = draw(&mut state, 120, 36);

        assert!(content.contains("https://github.com/openclaw/openclaw/pull/81834"));

        let rect = rendered_target_rect(&state, |target| {
            matches!(
                target,
                HitTarget::OpenHeaderUrl(url)
                    if url == "https://github.com/openclaw/openclaw/pull/81834"
            )
        })
        .expect("header identity link");

        assert_eq!(rect.y, 1);
        assert_eq!(rect.x, COMFORTABLE_GUTTER);

        let intent = click_rendered_target(&mut state, |target| {
            matches!(
                target,
                HitTarget::OpenHeaderUrl(url)
                    if url == "https://github.com/openclaw/openclaw/pull/81834"
            )
        });

        assert_eq!(
            intent,
            AppIntent::OpenUrl("https://github.com/openclaw/openclaw/pull/81834".into())
        );
    }

    #[test]
    fn compact_header_identity_stays_on_first_row() {
        let mut state = AppState::new(pr_resource());
        state.spacing = SpacingMode::Compact;

        let identity_row = draw_row_text(&mut state, 120, 36, 0);
        draw(&mut state, 120, 36);
        let rect = rendered_target_rect(&state, |target| {
            matches!(
                target,
                HitTarget::OpenHeaderUrl(url)
                    if url == "https://github.com/openclaw/openclaw/pull/81834"
            )
        })
        .expect("compact header identity link");

        assert!(identity_row.contains("https://github.com/openclaw/openclaw/pull/81834"));
        assert_eq!(rect.y, 0);
        assert_eq!(rect.x, 0);
    }

    #[test]
    fn narrow_header_identity_avoids_terminal_autolink_shape() {
        let mut resource = pr_resource();
        resource.id.owner = "huggingface".into();
        resource.id.repo = "huggingface.js".into();
        resource.id.number = 2185;
        resource.url = "http://huggingface/huggingface.js#2185".into();
        let mut state = AppState::new(resource);
        state.spacing = SpacingMode::Compact;

        let content = draw(&mut state, 48, 24);

        assert!(content.contains("huggingface / huggingface.js #2185"));
        assert!(!content.contains("huggingface/huggingface.js#2185"));
        assert!(state.hit_areas.iter().any(|area| matches!(
            &area.target,
            HitTarget::OpenHeaderUrl(url)
                if url == "https://github.com/huggingface/huggingface.js/pull/2185"
        )));
    }

    #[test]
    fn rendered_tab_hit_area_can_be_clicked() {
        let mut state = AppState::new(pr_resource());
        draw(&mut state, 120, 36);

        let intent =
            click_rendered_target(&mut state, |target| *target == HitTarget::Tab(Tab::Checks));

        assert_eq!(intent, AppIntent::None);
        assert_eq!(state.active_tab, Tab::Checks);
    }

    #[test]
    fn rendered_body_more_hit_area_can_be_clicked() {
        let mut resource = pr_resource();
        resource.body = (0..30)
            .map(|index| format!("line {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut state = AppState::new(resource);
        draw(&mut state, 120, 36);

        let intent = click_rendered_target(&mut state, |target| {
            *target == HitTarget::ToggleBlock(BlockId::Body)
        });

        assert_eq!(intent, AppIntent::None);
        assert!(state.block_expanded(&BlockId::Body));
    }

    #[test]
    fn short_body_does_not_show_noop_expand_or_collapse_control() {
        let mut state = AppState::new(pr_resource());

        draw(&mut state, 120, 36);
        assert!(!state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::ToggleBlock(BlockId::Body)));

        state.toggle_block(BlockId::Body);
        let content = draw(&mut state, 120, 36);

        assert!(!content.contains("[- less]"));
        assert!(!state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::ToggleBlock(BlockId::Body)));
    }

    #[test]
    fn rendered_expand_all_control_expands_current_tab_blocks() {
        let mut resource = pr_resource();
        resource.body = long_body();
        resource.activity[0].body = long_body();
        let mut state = AppState::new(resource);
        let content = draw(&mut state, 120, 36);
        let footer = chrome_area_for_spacing(
            rects_for_spacing(Rect::new(0, 0, 120, 36), state.spacing).footer,
            state.spacing,
        );

        assert!(content.contains("[expand"));
        let expand_rect = rendered_target_rect(&state, |target| {
            matches!(target, HitTarget::ExpandBlocks(blocks) if blocks.contains(&BlockId::Body))
        })
        .expect("footer expand all target");
        assert_eq!(expand_rect.y, footer.y + footer.height.saturating_sub(2));

        let intent = click_rendered_target(
            &mut state,
            |target| matches!(target, HitTarget::ExpandBlocks(blocks) if blocks.contains(&BlockId::Body)),
        );

        assert_eq!(intent, AppIntent::None);
        assert!(state.block_expanded(&BlockId::Body));
        assert!(state
            .expanded_blocks
            .iter()
            .any(|block| matches!(block, BlockId::Activity(_))));

        let content = draw(&mut state, 120, 36);

        assert!(content.contains("[collapse"));
    }

    #[test]
    fn footer_expand_all_is_final_bottom_bar_command_not_scroll_status() {
        let mut state = AppState::new(pr_resource());
        let footer = chrome_area_for_spacing(
            rects_for_spacing(Rect::new(0, 0, 120, 36), state.spacing).footer,
            state.spacing,
        );

        let bottom = draw_row_text(
            &mut state,
            120,
            36,
            footer.y.saturating_add(footer.height.saturating_sub(2)),
        );

        assert!(bottom.contains("[refresh] [expand"));
        assert!(!bottom.contains("[copy]"));
        assert!(!bottom.contains("[open]"));
        assert!(!bottom.contains("[expand all]"));
        assert!(!bottom.contains("scroll "));
    }

    #[test]
    fn wrapped_footer_keeps_expand_all_pinned_to_bottom_command_bar() {
        let mut resource = pr_resource();
        resource.body = long_body();
        let mut state = AppState::new(resource);
        let content = draw(&mut state, 24, 24);
        let footer = chrome_area_for_spacing(
            rects_for_spacing(Rect::new(0, 0, 24, 24), state.spacing).footer,
            state.spacing,
        );

        assert!(content.contains("[refresh]"));
        assert!(!content.contains("[copy]"));
        assert!(!content.contains("[open]"));
        assert!(content.contains("[expand"));

        let refresh_rect =
            rendered_target_rect(&state, |target| *target == HitTarget::Refresh).unwrap();
        let expand_rect = rendered_target_rect(&state, |target| {
            matches!(target, HitTarget::ExpandBlocks(blocks) if blocks.contains(&BlockId::Body))
        })
        .expect("footer expand all target");

        assert!(refresh_rect.y >= footer.y);
        assert!(expand_rect.y >= footer.y);
        assert!(expand_rect.y < footer.y + footer.height);
    }

    #[test]
    fn rendered_collapse_all_control_collapses_current_tab_blocks() {
        let mut resource = pr_resource();
        resource.body = long_body();
        let mut state = AppState::new(resource);
        draw(&mut state, 120, 80);
        let _ = click_rendered_target(
            &mut state,
            |target| matches!(target, HitTarget::ExpandBlocks(blocks) if blocks.contains(&BlockId::Body)),
        );
        draw(&mut state, 120, 80);

        let intent = click_rendered_target(
            &mut state,
            |target| matches!(target, HitTarget::CollapseBlocks(blocks) if blocks.contains(&BlockId::Body)),
        );

        assert_eq!(intent, AppIntent::None);
        assert!(!state.block_expanded(&BlockId::Body));
        assert!(state.expanded_blocks.is_empty());
    }

    #[test]
    fn rendered_body_more_hit_area_can_be_activated_with_enter() {
        let mut resource = pr_resource();
        resource.body = (0..30)
            .map(|index| format!("line {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut state = AppState::new(resource);
        draw(&mut state, 120, 36);

        let intent = apply_event(
            &mut state,
            AppEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())),
        );

        assert_eq!(intent, AppIntent::None);
        assert!(state.block_expanded(&BlockId::Body));
    }

    #[test]
    fn rendered_file_more_hit_area_can_be_clicked() {
        let mut state = AppState::new(pr_resource());
        state.set_tab(Tab::Files);
        draw(&mut state, 120, 36);

        let file_block = BlockId::File("extensions/senseaudio/index.ts".into());
        let intent = click_rendered_target(&mut state, |target| {
            *target == HitTarget::ToggleBlock(file_block.clone())
        });

        assert_eq!(intent, AppIntent::None);
        assert!(state.block_expanded(&file_block));
    }

    #[test]
    fn commit_rows_are_click_expandable() {
        let mut state = AppState::new(pr_resource());
        state.set_tab(Tab::Commits);

        let content = draw(&mut state, 120, 80);

        assert!(content.contains("fb948c9"));
        assert!(content.contains("[+ more]"));
        assert!(state.hit_areas.iter().any(|area| matches!(
            &area.target,
            HitTarget::ToggleBlock(BlockId::Commit(oid)) if oid == "fb948c9"
        )));

        state.toggle_block(BlockId::Commit("fb948c9".into()));
        let content = draw(&mut state, 120, 80);

        assert!(content.contains("authors: KLilyZ, claude"));
        assert!(content.contains("authored: 2w ago"));
        assert!(content.contains("committed: 1mo ago"));
        assert!(!content.contains("authored: 2026-"));
        assert!(content.contains("Registers a SenseAudio speech provider."));
        assert!(content.contains("[- less]"));
    }

    #[test]
    fn commits_tab_can_render_newest_first() {
        let mut resource = pr_resource();
        let pr = resource.pull_request.as_mut().unwrap();
        pr.commits[0].message = "Older commit".into();
        let mut newer = pr.commits[0].clone();
        newer.oid = "newer123".into();
        newer.message = "Newer commit".into();
        pr.commits.push(newer);
        let mut state = AppState::new(resource);
        state.set_tab(Tab::Commits);
        state.reverse_chronological = true;

        let content = draw(&mut state, 120, 80);
        let newer = content.find("Newer commit").unwrap();
        let older = content.find("Older commit").unwrap();

        assert!(newer < older);
    }

    #[test]
    fn expanded_commit_rows_show_full_commit_body() {
        let mut resource = pr_resource();
        resource
            .pull_request
            .as_mut()
            .unwrap()
            .commits
            .first_mut()
            .unwrap()
            .body = (0..30)
            .map(|index| format!("commit body line {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut state = AppState::new(resource);
        state.set_tab(Tab::Commits);

        let collapsed = draw(&mut state, 120, 80);

        assert!(collapsed.contains("[+ more]"));
        assert!(!collapsed.contains("commit body line 29"));

        state.toggle_block(BlockId::Commit("fb948c9".into()));
        let expanded = draw(&mut state, 120, 80);

        assert!(expanded.contains("commit body line 0"));
        assert!(expanded.contains("commit body line 29"));
        assert!(!expanded.contains("[body truncated]"));
    }

    #[test]
    fn expanded_commit_rows_show_deployments() {
        let mut resource = pr_resource();
        resource
            .pull_request
            .as_mut()
            .unwrap()
            .commits
            .first_mut()
            .unwrap()
            .deployments = vec![Deployment {
            environment: "preview".into(),
            state: "SUCCESS".into(),
            description: Some("Preview deployed".into()),
            environment_url: Some("https://github.com/openclaw/openclaw/deployments/1".into()),
            log_url: Some("https://github.com/openclaw/openclaw/actions/runs/1".into()),
            created_at: Some("2026-05-30T03:20:00Z".into()),
            updated_at: "2026-05-30T03:21:00Z".into(),
        }];
        let mut state = AppState::new(resource);
        state.set_tab(Tab::Commits);
        state.toggle_block(BlockId::Commit("fb948c9".into()));

        let content = draw(&mut state, 120, 80);

        assert!(content.contains("deployments"));
        assert!(content.contains("preview [SUCCESS]"));
        assert!(content.contains("Preview deployed"));
        assert!(state.hit_areas.iter().any(|area| matches!(
            &area.target,
            HitTarget::OpenUrl(url)
                if url == "https://github.com/openclaw/openclaw/deployments/1"
        )));
        assert!(state.hit_areas.iter().any(|area| matches!(
            &area.target,
            HitTarget::OpenUrl(url) if url == "https://github.com/openclaw/openclaw/actions/runs/1"
        )));
    }

    #[test]
    fn expanded_file_rows_show_patch_context() {
        let mut state = AppState::new(pr_resource());
        state.set_tab(Tab::Files);
        state.toggle_block(BlockId::File("extensions/senseaudio/index.ts".into()));

        let content = draw(&mut state, 120, 80);

        assert!(content.contains("patch"));
        assert!(content.contains("diff --git"));
        assert!(content.contains("import speech"));
        assert!(!content.contains("+import speech"));
    }

    #[test]
    fn file_rows_style_diff_lines_by_change_kind() {
        let mut resource = pr_resource();
        let file = resource
            .pull_request
            .as_mut()
            .unwrap()
            .files
            .first_mut()
            .unwrap();
        file.patch = Some(
            "diff --git a/extensions/senseaudio/index.ts b/extensions/senseaudio/index.ts\n\
@@ -1,2 +1,2 @@\n\
-import old\n\
+import speech\n\
 context line"
                .into(),
        );
        let path = file.path.clone();
        let mut expanded_blocks = std::collections::HashSet::new();
        expanded_blocks.insert(BlockId::File(path.clone()));
        let palette = Palette::default_dark();
        let symbols = crate::render::symbols::SymbolMode::Ascii.symbols();
        let rows = files_rows_for_pr(
            resource.pull_request.as_ref().unwrap(),
            120,
            &expanded_blocks,
            &palette,
            &symbols,
        );
        let summary = rows
            .iter()
            .find(|row| line_text(&row.line).contains(&path))
            .expect("file summary row");

        assert_eq!(summary.line.spans[0].style.fg, Some(palette.green));
        assert_eq!(summary.line.spans[2].style.fg, Some(palette.red));
        assert_eq!(
            row_primary_style(
                &rows
                    .iter()
                    .find(|row| line_text(&row.line) == "import old")
                    .expect("deletion row")
                    .line
            )
            .bg,
            Some(palette.red)
        );
        assert_eq!(
            row_primary_style(
                &rows
                    .iter()
                    .find(|row| line_text(&row.line) == "import speech")
                    .expect("addition row")
                    .line
            )
            .bg,
            Some(palette.green)
        );
        assert_eq!(
            row_primary_style(
                &rows
                    .iter()
                    .find(|row| line_text(&row.line).starts_with("@@"))
                    .expect("hunk row")
                    .line
            )
            .fg,
            Some(palette.teal)
        );
    }

    #[test]
    fn diff_rows_hide_change_markers_and_preserve_indentation() {
        let palette = Palette::default_dark();
        let patch = [
            "diff --git a/src/main.rs b/src/main.rs",
            "--- a/src/main.rs",
            "+++ b/src/main.rs",
            "@@ -1,4 +1,4 @@",
            "-    let old_value = 1;",
            "+    let new_value = 1;",
            "     println!(\"still indented\");",
            "+",
        ]
        .join("\n");
        let rows = diff_patch_rows(&patch, 120, &palette);

        let rendered = rows
            .iter()
            .map(|row| line_text(&row.line))
            .collect::<Vec<_>>();

        assert!(rendered.contains(&"--- a/src/main.rs".to_string()));
        assert!(rendered.contains(&"+++ b/src/main.rs".to_string()));
        assert!(rendered.contains(&"    let old_value = 1;".to_string()));
        assert!(rendered.contains(&"    let new_value = 1;".to_string()));
        let context_line = rendered
            .iter()
            .find(|line| line.ends_with("println!(\"still indented\");"))
            .expect("context line");
        assert!(context_line.starts_with("    "));
        assert!(rendered.contains(&" ".to_string()));
        assert!(!rendered.contains(&"-    let old_value = 1;".to_string()));
        assert!(!rendered.contains(&"+    let new_value = 1;".to_string()));
        assert_eq!(
            row_primary_style(
                &rows
                    .iter()
                    .find(|row| line_text(&row.line) == "    let old_value = 1;")
                    .expect("deletion row")
                    .line
            )
            .bg,
            Some(palette.red)
        );
        assert_eq!(
            row_primary_style(
                &rows
                    .iter()
                    .find(|row| line_text(&row.line) == "    let new_value = 1;")
                    .expect("addition row")
                    .line
            )
            .bg,
            Some(palette.green)
        );
    }

    #[test]
    fn long_patch_rows_are_click_expandable() {
        let mut resource = pr_resource();
        let file = resource
            .pull_request
            .as_mut()
            .unwrap()
            .files
            .first_mut()
            .unwrap();
        file.patch = Some(
            (0..30)
                .map(|index| format!("+patch line {index}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        let path = file.path.clone();
        let file_block = BlockId::File(path.clone());
        let patch_block = BlockId::Patch(path);
        let mut state = AppState::new(resource);
        state.set_tab(Tab::Files);
        state.toggle_block(file_block);

        let content = draw(&mut state, 120, 80);

        assert!(content.contains("patch line 0"));
        assert!(content.contains("patch line 17"));
        assert!(!content.contains("patch line 29"));
        assert!(content.contains("[+ more patch]"));
        assert!(state
            .hit_areas
            .iter()
            .any(|area| { area.target == HitTarget::ToggleBlock(patch_block.clone()) }));

        let intent = click_rendered_target(&mut state, |target| {
            *target == HitTarget::ToggleBlock(patch_block.clone())
        });

        assert_eq!(intent, AppIntent::None);
        assert!(state.block_expanded(&patch_block));

        let content = draw(&mut state, 120, 80);

        assert!(content.contains("patch line 29"));
        assert!(content.contains("[- less patch]"));
    }

    #[test]
    fn rendered_visible_link_hit_area_opens_choice_prompt() {
        let mut resource = pr_resource();
        resource.body = "Pairs with #66943".into();
        let mut state = AppState::new(resource);
        draw(&mut state, 120, 36);

        let intent = click_rendered_target(&mut state, |target| {
            matches!(
                target,
                HitTarget::ResourceLink { id, .. } if id.canonical_name() == "openclaw/openclaw#66943"
            )
        });

        assert_eq!(intent, AppIntent::None);
        assert!(state
            .resource_link_prompt
            .as_ref()
            .is_some_and(|prompt| { prompt.id.canonical_name() == "openclaw/openclaw#66943" }));
    }

    #[test]
    fn rendered_absolute_pr_link_hit_area_opens_choice_prompt() {
        let mut resource = pr_resource();
        resource.body = "Pairs with https://github.com/openclaw/openclaw/pull/81835".into();
        resource.related_resources.clear();
        let mut state = AppState::new(resource);
        state.set_tab(Tab::Links);
        draw(&mut state, 120, 36);

        let intent = click_rendered_target(&mut state, |target| {
            matches!(
                target,
                HitTarget::ResourceLink { id, .. }
                    if id.number == 81835 && id.kind_hint == Some(ResourceKind::PullRequest)
            )
        });

        assert_eq!(intent, AppIntent::None);
        assert!(state.resource_link_prompt.as_ref().is_some_and(|prompt| {
            prompt.id.number == 81835 && prompt.id.kind_hint == Some(ResourceKind::PullRequest)
        }));
    }

    #[test]
    fn rendered_footer_controls_can_be_clicked() {
        let mut refresh_state = AppState::new(pr_resource());
        draw(&mut refresh_state, 120, 36);
        let refresh =
            click_rendered_target(&mut refresh_state, |target| *target == HitTarget::Refresh);
        assert_eq!(refresh, AppIntent::Refresh);
        assert!(refresh_state.refresh_requested);

        let mut help_state = AppState::new(pr_resource());
        draw(&mut help_state, 120, 36);
        let help = click_rendered_target(&mut help_state, |target| *target == HitTarget::Help);
        assert_eq!(help, AppIntent::None);
        assert!(help_state.show_help);

        let mut settings_state = AppState::new(pr_resource());
        draw(&mut settings_state, 120, 36);
        let settings =
            click_rendered_target(&mut settings_state, |target| *target == HitTarget::Settings);
        assert_eq!(settings, AppIntent::None);
        assert!(settings_state.show_settings);

        let mut quit_state = AppState::new(pr_resource());
        draw(&mut quit_state, 120, 36);
        let quit = click_rendered_target(&mut quit_state, |target| *target == HitTarget::Quit);
        assert_eq!(quit, AppIntent::Quit);
        assert!(quit_state.should_quit);
    }

    #[test]
    fn renders_help_overlay() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new(pr_resource());
        state.show_help = true;

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();
        let content = format!("{:?}", terminal.backend().buffer());

        assert!(content.contains("Help"));
        assert!(content.contains("Mouse"));
        assert!(content.contains("Keyboard"));
    }

    #[test]
    fn renders_settings_view_with_clickable_options() {
        let backend = TestBackend::new(120, 80);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new(pr_resource());
        state.show_settings = true;

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();
        let content = format!("{:?}", terminal.backend().buffer());

        assert!(content.contains("Settings"));
        assert!(content.contains("Config:"));
        assert!(content.contains("solarized"));
        assert!(content.contains("rose-pine"));
        assert!(content.contains("emoji"));
        assert!(content.contains("comfortable"));
        assert!(content.contains("compact"));
        assert!(content.contains("Width"));
        assert!(content.contains("Scrollbar"));
        assert!(content.contains("on-scroll"));
        assert!(content.contains("always"));
        assert!(content.contains("hidden"));
        assert!(content.contains("fixed"));
        assert!(content.contains("full"));
        assert!(content.contains("118"));
        assert!(state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::SetTheme("solarized".into())));
        assert!(state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::SetSymbols("emoji".into())));
        assert!(state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::SetSpacing("compact".into())));
        assert!(state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::SetWidthMode("full".into())));
        assert!(state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::SetFixedWidth(132)));
        assert!(state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::SetScrollbar("always".into())));
        assert!(state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::CloseSettings));
    }

    #[test]
    fn render_clamps_end_scroll_to_available_content() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new(pr_resource());
        state.show_help = true;
        state.scroll = u16::MAX;

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();

        assert!(state.scroll < u16::MAX);
    }

    #[test]
    fn file_rows_are_click_expandable() {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new(pr_resource());
        state.set_tab(Tab::Files);

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();
        let content = format!("{:?}", terminal.backend().buffer());

        assert!(content.contains("[+ more]"));
        assert!(state.hit_areas.iter().any(|area| matches!(
            &area.target,
            HitTarget::ToggleBlock(BlockId::File(path))
                if path == "extensions/senseaudio/index.ts"
        )));

        state.toggle_block(BlockId::File("extensions/senseaudio/index.ts".into()));
        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();
        let content = format!("{:?}", terminal.backend().buffer());

        assert!(content.contains("path: extensions/senseaudio/index.ts"));
        assert!(content.contains("change: MODIFIED, additions: 3, deletions: 1"));
        assert!(content.contains("[- less]"));
    }

    #[test]
    fn files_expand_all_opens_files_and_patch_blocks() {
        let mut resource = pr_resource();
        resource
            .pull_request
            .as_mut()
            .unwrap()
            .files
            .first_mut()
            .unwrap()
            .patch = Some(
            (0..30)
                .map(|index| format!("+patch line {index}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        let mut state = AppState::new(resource);
        state.set_tab(Tab::Files);
        let content = draw(&mut state, 120, 36);
        let footer = chrome_area_for_spacing(
            rects_for_spacing(Rect::new(0, 0, 120, 36), state.spacing).footer,
            state.spacing,
        );

        assert!(content.contains("[expand"));
        let expand_rect = rendered_target_rect(&state, |target| {
            matches!(target, HitTarget::ExpandBlocks(blocks) if blocks.iter().any(|block| matches!(block, BlockId::Patch(_))))
        })
        .expect("footer files expand all target");
        assert_eq!(expand_rect.y, footer.y + footer.height.saturating_sub(2));

        let intent = click_rendered_target(
            &mut state,
            |target| matches!(target, HitTarget::ExpandBlocks(blocks) if blocks.iter().any(|block| matches!(block, BlockId::Patch(_)))),
        );

        assert_eq!(intent, AppIntent::None);
        assert!(state
            .expanded_blocks
            .iter()
            .any(|block| matches!(block, BlockId::File(_))));
        assert!(state
            .expanded_blocks
            .iter()
            .any(|block| matches!(block, BlockId::Patch(_))));
    }

    #[test]
    fn activity_entries_are_truncated_and_click_expandable() {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut resource = pr_resource();
        resource.activity[0].body = (0..20)
            .map(|index| format!("activity line {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut state = AppState::new(resource);
        state.set_tab(Tab::Activity);

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();
        let content = format!("{:?}", terminal.backend().buffer());

        assert!(content.contains("[+ more]"));
        assert!(state.hit_areas.iter().any(|area| matches!(
            &area.target,
            HitTarget::ToggleBlock(BlockId::Activity(id)) if id == "c1"
        )));
    }

    #[test]
    fn activity_permalink_details_are_clickable_without_expansion() {
        let mut resource = pr_resource();
        resource.activity[0].body = "short comment".into();
        resource.activity[0].url =
            Some("https://github.com/openclaw/openclaw/pull/81834#issuecomment-1".into());
        let mut state = AppState::new(resource);
        state.set_tab(Tab::Activity);

        let content = draw(&mut state, 120, 36);

        assert!(content.contains("[details]"));
        assert!(!content.contains("[+ more]"));
        assert!(state.hit_areas.iter().any(|area| matches!(
            &area.target,
            HitTarget::ResourceLink { id, url: Some(url) }
                if id.canonical_name() == "openclaw/openclaw#81834"
                    && url == "https://github.com/openclaw/openclaw/pull/81834#issuecomment-1"
        )));
        let intent = click_rendered_target(&mut state, |target| {
            matches!(
                target,
                HitTarget::ResourceLink { id, url: Some(url) }
                    if id.canonical_name() == "openclaw/openclaw#81834"
                        && url == "https://github.com/openclaw/openclaw/pull/81834#issuecomment-1"
            )
        });

        assert_eq!(intent, AppIntent::None);
        assert_eq!(state.active_tab, Tab::Activity);
        assert_eq!(
            state.status_message.as_deref(),
            Some("focused linked activity")
        );

        state.toggle_block(BlockId::Activity("c1".into()));
        let content = draw(&mut state, 120, 36);

        assert!(!content.contains("[- less]"));
        assert!(!state.hit_areas.iter().any(|area| matches!(
            &area.target,
            HitTarget::ToggleBlock(BlockId::Activity(id)) if id == "c1"
        )));
    }

    #[test]
    fn review_thread_activity_shows_thread_state() {
        let mut resource = pr_resource();
        resource.activity[0].kind = ActivityKind::ReviewComment;
        resource.activity[0].path = Some("src/reviewed.rs".into());
        resource.activity[0].line = Some(12);
        resource.activity[0].url =
            Some("https://github.com/openclaw/openclaw/pull/81834#discussion_r1".into());
        resource.activity[0].author_association = Some("MEMBER".into());
        resource.activity[0].reactions.eyes = 1;
        resource.activity[0].includes_created_edit = true;
        resource.activity[0].thread_resolved = Some(false);
        resource.activity[0].thread_outdated = Some(true);
        let mut state = AppState::new(resource);
        state.set_tab(Tab::Activity);

        let content = draw(&mut state, 120, 36);

        assert!(content.contains("Review comment by @github-actions"));
        assert!(content.contains("src/reviewed.rs:12"));
        assert!(content.contains("thread: unresolved, outdated"));
        assert!(content.contains("meta: association MEMBER, edited, reactions eyes:1"));
        assert!(content.contains("[details]"));
        assert!(state.hit_areas.iter().any(|area| matches!(
            &area.target,
            HitTarget::ResourceLink { id, url: Some(url) }
                if id.canonical_name() == "openclaw/openclaw#81834"
                    && url == "https://github.com/openclaw/openclaw/pull/81834#discussion_r1"
        )));

        state.toggle_block(BlockId::Activity("c1".into()));
        let content = draw(&mut state, 120, 36);

        assert!(
            !content.contains("url: https://github.com/openclaw/openclaw/pull/81834#discussion_r1")
        );
        assert!(!content.contains("[- less]"));
    }

    #[test]
    fn review_threads_summary_counts_unique_unresolved_and_outdated_threads() {
        let mut resource = pr_resource();
        resource.activity = vec![
            ActivityEntry {
                id: "r1-c1".into(),
                kind: ActivityKind::ReviewComment,
                author: "alice".into(),
                body: "please fix".into(),
                updated_at: "now".into(),
                path: Some("src/lib.rs".into()),
                line: Some(1),
                url: None,
                author_association: None,
                reactions: ReactionCounts::default(),
                includes_created_edit: false,
                is_minimized: false,
                minimized_reason: None,
                thread_id: Some("thread-1".into()),
                thread_resolved: Some(false),
                thread_outdated: Some(true),
            },
            ActivityEntry {
                id: "r1-c2".into(),
                kind: ActivityKind::ReviewComment,
                author: "bob".into(),
                body: "reply".into(),
                updated_at: "now".into(),
                path: Some("src/lib.rs".into()),
                line: Some(1),
                url: None,
                author_association: None,
                reactions: ReactionCounts::default(),
                includes_created_edit: false,
                is_minimized: false,
                minimized_reason: None,
                thread_id: Some("thread-1".into()),
                thread_resolved: Some(false),
                thread_outdated: Some(false),
            },
            ActivityEntry {
                id: "r2-c1".into(),
                kind: ActivityKind::ReviewComment,
                author: "alice".into(),
                body: "resolved".into(),
                updated_at: "now".into(),
                path: Some("src/main.rs".into()),
                line: Some(2),
                url: None,
                author_association: None,
                reactions: ReactionCounts::default(),
                includes_created_edit: false,
                is_minimized: false,
                minimized_reason: None,
                thread_id: Some("thread-2".into()),
                thread_resolved: Some(true),
                thread_outdated: Some(false),
            },
        ];

        assert_eq!(
            review_threads_summary(&resource).as_deref(),
            Some("1 unresolved / 2 threads, 1 outdated")
        );
    }

    #[test]
    fn renders_review_thread_summary_in_pr_status() {
        let mut resource = pr_resource();
        resource.activity[0].kind = ActivityKind::ReviewComment;
        resource.activity[0].thread_id = Some("thread-1".into());
        resource.activity[0].thread_resolved = Some(false);
        resource.activity[0].thread_outdated = Some(true);
        let mut state = AppState::new(resource);

        let content = draw(&mut state, 120, 40);

        assert!(content.contains("Threads: 1 unresolved / 1 thread, 1 outdated"));
    }

    #[test]
    fn checks_are_grouped_by_status() {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut resource = pr_resource();
        let pr = resource.pull_request.as_mut().unwrap();
        pr.checks = vec![
            CheckRun {
                name: "ci/pass".into(),
                status: CheckStatus::Success,
                summary: None,
                details_url: None,
                started_at: None,
                completed_at: None,
                raw_status: None,
                raw_conclusion: None,
            },
            CheckRun {
                name: "ci/fail".into(),
                status: CheckStatus::Failure,
                summary: Some("exit 1".into()),
                details_url: None,
                started_at: None,
                completed_at: None,
                raw_status: None,
                raw_conclusion: None,
            },
            CheckRun {
                name: "ci/pending".into(),
                status: CheckStatus::Pending,
                summary: None,
                details_url: None,
                started_at: None,
                completed_at: None,
                raw_status: None,
                raw_conclusion: None,
            },
            CheckRun {
                name: "ci/skipped".into(),
                status: CheckStatus::Skipped,
                summary: None,
                details_url: None,
                started_at: None,
                completed_at: None,
                raw_status: None,
                raw_conclusion: None,
            },
            CheckRun {
                name: "ci/neutral".into(),
                status: CheckStatus::Neutral,
                summary: None,
                details_url: None,
                started_at: None,
                completed_at: None,
                raw_status: None,
                raw_conclusion: None,
            },
        ];
        let mut state = AppState::new(resource);
        state.set_tab(Tab::Checks);

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();
        let content = format!("{:?}", terminal.backend().buffer());

        assert!(content.contains("5 total: 1 pass, 1 pending, 1 fail"));
        assert!(content.contains("Failing (1)"));
        assert!(content.contains("Pending (1)"));
        assert!(content.contains("Passing (1)"));
        assert!(content.contains("Neutral (1)"));
        assert!(content.contains("Skipped (1)"));
    }

    #[test]
    fn check_rows_are_click_expandable() {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut resource = pr_resource();
        let pr = resource.pull_request.as_mut().unwrap();
        pr.checks = vec![CheckRun {
            name: "Very Long Workflow Name/very-long-check-name-that-needs-room".into(),
            status: CheckStatus::Failure,
            summary: Some("failed because the test command exited with status 1".into()),
            details_url: Some("https://github.com/openclaw/openclaw/actions/runs/1/job/2".into()),
            started_at: Some("2d".into()),
            completed_at: Some("2d".into()),
            raw_status: Some("COMPLETED".into()),
            raw_conclusion: Some("FAILURE".into()),
        }];
        let mut state = AppState::new(resource);
        state.set_tab(Tab::Checks);

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();
        let content = format!("{:?}", terminal.backend().buffer());

        assert!(content.contains("[+ more]"));
        assert!(state.hit_areas.iter().any(|area| matches!(
            &area.target,
            HitTarget::ToggleBlock(BlockId::Check(id))
                if id == "FAIL:Very Long Workflow Name/very-long-check-name-that-needs-room"
        )));

        state.toggle_block(BlockId::Check(
            "FAIL:Very Long Workflow Name/very-long-check-name-that-needs-room".into(),
        ));
        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();
        let content = format!("{:?}", terminal.backend().buffer());

        assert!(
            content.contains("name: Very Long Workflow Name/very-long-check-name-that-needs-room")
        );
        assert!(content.contains("status: FAIL"));
        assert!(content.contains("github status: COMPLETED"));
        assert!(content.contains("github conclusion: FAILURE"));
        assert!(content.contains("started: 2d ago"));
        assert!(content.contains("completed: 2d ago"));
        assert!(!content.contains("started: 2026-"));
        assert!(
            content.contains("details: https://github.com/openclaw/openclaw/actions/runs/1/job/2")
        );
        assert!(content.contains("summary: failed because the test command exited with status 1"));
        assert!(content.contains("[- less]"));
        assert!(state.hit_areas.iter().any(|area| matches!(
            &area.target,
            HitTarget::OpenUrl(url)
                if url == "https://github.com/openclaw/openclaw/actions/runs/1/job/2"
        )));
        let intent = click_rendered_target(&mut state, |target| {
            matches!(
                target,
                HitTarget::OpenUrl(url)
                    if url == "https://github.com/openclaw/openclaw/actions/runs/1/job/2"
            )
        });
        assert_eq!(
            intent,
            AppIntent::OpenUrl("https://github.com/openclaw/openclaw/actions/runs/1/job/2".into())
        );
    }

    #[test]
    fn truncation_does_not_split_emoji_bytes() {
        assert_eq!(truncate_ascii("rating: shrimp", 10), "rating:...");
        assert_eq!(truncate_ascii("rating: 🦐 gold shrimp", 12), "rating: ...");
        assert_eq!(
            truncate_ascii("rating: 🦐 gold shrimp", 13),
            "rating: 🦐..."
        );
    }
}

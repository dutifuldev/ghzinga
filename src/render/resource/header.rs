use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::{
    app::AppState,
    domain::{Resource, ResourceKind},
    input::{HitArea, HitTarget},
    render::{markdown, time::compact_relative_time, Palette, SpacingMode},
};

use super::{
    chrome_area_for_spacing, dim_style, fit_label_to_width, resource_state_style, separator_line,
    truncate_display,
};

pub(super) fn resource_tabs_area(header: &mut Rect, state: &AppState) -> Option<Rect> {
    if !state.resource_tab_bar_visible() || header.height < 2 {
        return None;
    }
    let area = Rect::new(header.x, header.y, header.width, 1);
    let consumed = if header.height >= 3 { 2 } else { 1 };
    header.y = header.y.saturating_add(consumed);
    header.height = header.height.saturating_sub(consumed);
    Some(area)
}

pub(super) fn single_resource_add_button_visible(header: Rect) -> bool {
    header.width > 0 && header.height > 0
}

pub(super) fn render_resource_tabs(
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
    let available_width = tab_right.saturating_sub(area.x);
    let specs = resource_tab_specs(state);
    let plan = plan_resource_tabs(&specs, state.resource_tab_scroll, available_width);
    let mut x = area.x;
    let mut spans = Vec::<Span<'static>>::new();

    if plan.show_previous {
        render_resource_tab_gap(&mut spans, &mut x, tab_right, palette);
        render_resource_tab_arrow(
            &mut spans,
            state,
            Rect::new(
                x,
                area.y,
                RESOURCE_TAB_ARROW_WIDTH.min(tab_right.saturating_sub(x)),
                1,
            ),
            "‹",
            HitTarget::PreviousResourceTab,
            palette,
        );
        x = x.saturating_add(RESOURCE_TAB_ARROW_WIDTH.min(tab_right.saturating_sub(x)));
    }

    for planned in &plan.tabs {
        if let Some(spec) = specs.get(planned.index) {
            render_resource_tab_gap(&mut spans, &mut x, tab_right, palette);
            let width = planned.width.min(tab_right.saturating_sub(x));
            if width == 0 {
                break;
            }
            let active = planned.index == state.active_resource_tab;
            let label_width = width.saturating_sub(4) as usize;
            let tab_text = format!(" {} × ", resource_tab_content(spec, label_width));
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
                HitTarget::ResourceTab(planned.index),
            ));
            state.hit_areas.push(HitArea::new(
                Rect::new(x.saturating_add(width.saturating_sub(3)), area.y, 3, 1),
                HitTarget::CloseResourceTab(planned.index),
            ));
            spans.push(Span::styled(tab_text, style));
            x = x.saturating_add(width);
        }
    }

    if plan.show_next {
        render_resource_tab_gap(&mut spans, &mut x, tab_right, palette);
        render_resource_tab_arrow(
            &mut spans,
            state,
            Rect::new(
                x,
                area.y,
                RESOURCE_TAB_ARROW_WIDTH.min(tab_right.saturating_sub(x)),
                1,
            ),
            "›",
            HitTarget::NextResourceTab,
            palette,
        );
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

const RESOURCE_TAB_MAX_WIDTH: u16 = 32;
const RESOURCE_TAB_ARROW_WIDTH: u16 = 3;

#[derive(Debug, Clone)]
struct ResourceTabSpec {
    index: usize,
    identity: String,
    title: String,
    full_label: String,
    min_width: u16,
    preferred_width: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlannedResourceTab {
    index: usize,
    width: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResourceTabPlan {
    tabs: Vec<PlannedResourceTab>,
    show_previous: bool,
    show_next: bool,
}

fn resource_tab_specs(state: &AppState) -> Vec<ResourceTabSpec> {
    state
        .resource_tabs
        .iter()
        .enumerate()
        .map(|(index, tab)| {
            let identity = resource_tab_identity(&tab.resource);
            let title = tab.resource.title.clone();
            let full_label = if title.is_empty() {
                identity.clone()
            } else {
                format!("{identity} {title}")
            };
            let identity_width = UnicodeWidthStr::width(identity.as_str()) as u16;
            let full_width = UnicodeWidthStr::width(full_label.as_str()) as u16;
            let min_width = identity_width.saturating_add(4).max(4);
            let preferred_width = full_width
                .saturating_add(4)
                .clamp(min_width, RESOURCE_TAB_MAX_WIDTH);
            ResourceTabSpec {
                index,
                identity,
                title,
                full_label,
                min_width,
                preferred_width,
            }
        })
        .collect()
}

fn resource_tab_identity(resource: &Resource) -> String {
    let kind = match resource.kind() {
        ResourceKind::PullRequest => "PR",
        ResourceKind::Issue => "Issue",
    };
    format!("{kind} #{}", resource.id.number)
}

fn plan_resource_tabs(
    specs: &[ResourceTabSpec],
    resource_tab_scroll: usize,
    available_width: u16,
) -> ResourceTabPlan {
    if specs.is_empty() || available_width == 0 {
        return ResourceTabPlan {
            tabs: Vec::new(),
            show_previous: false,
            show_next: false,
        };
    }
    let preferred = specs
        .iter()
        .map(|spec| spec.preferred_width)
        .collect::<Vec<_>>();
    if measured_tab_width(&preferred) <= available_width {
        return all_resource_tabs(specs, preferred);
    }
    let minimum = specs.iter().map(|spec| spec.min_width).collect::<Vec<_>>();
    if measured_tab_width(&minimum) <= available_width {
        return all_resource_tabs(specs, shrink_resource_tab_widths(specs, available_width));
    }
    overflow_resource_tab_plan(specs, resource_tab_scroll, available_width)
}

fn all_resource_tabs(specs: &[ResourceTabSpec], widths: Vec<u16>) -> ResourceTabPlan {
    ResourceTabPlan {
        tabs: specs
            .iter()
            .zip(widths)
            .map(|(spec, width)| PlannedResourceTab {
                index: spec.index,
                width,
            })
            .collect(),
        show_previous: false,
        show_next: false,
    }
}

fn shrink_resource_tab_widths(specs: &[ResourceTabSpec], available_width: u16) -> Vec<u16> {
    let mut widths = specs
        .iter()
        .map(|spec| spec.preferred_width)
        .collect::<Vec<_>>();
    while measured_tab_width(&widths) > available_width {
        let Some(index) = widths
            .iter()
            .enumerate()
            .filter(|(index, width)| **width > specs[*index].min_width)
            .max_by_key(|(index, width)| (**width - specs[*index].min_width, **width))
            .map(|(index, _)| index)
        else {
            break;
        };
        widths[index] = widths[index].saturating_sub(1);
    }
    widths
}

fn overflow_resource_tab_plan(
    specs: &[ResourceTabSpec],
    resource_tab_scroll: usize,
    available_width: u16,
) -> ResourceTabPlan {
    let mut start = resource_tab_scroll.min(specs.len().saturating_sub(1));
    let mut end = start.saturating_add(1);
    let mut widths = specs.iter().map(|spec| spec.min_width).collect::<Vec<_>>();
    if overflow_tab_width(&widths, start, end) > available_width {
        let controls = overflow_controls_width(start > 0, start + 1 < specs.len());
        let width = available_width.saturating_sub(controls).max(1);
        widths[start] = widths[start].min(width);
        return ResourceTabPlan {
            tabs: vec![PlannedResourceTab {
                index: specs[start].index,
                width: widths[start],
            }],
            show_previous: start > 0 && available_width >= RESOURCE_TAB_ARROW_WIDTH,
            show_next: start + 1 < specs.len()
                && available_width >= RESOURCE_TAB_ARROW_WIDTH.saturating_mul(2),
        };
    }

    while end < specs.len() && overflow_tab_width(&widths, start, end + 1) <= available_width {
        end += 1;
    }
    while start > 0
        && end.saturating_sub(start) <= 1
        && overflow_tab_width(&widths, start - 1, end) <= available_width
    {
        start -= 1;
    }

    ResourceTabPlan {
        tabs: (start..end)
            .map(|index| PlannedResourceTab {
                index: specs[index].index,
                width: widths[index],
            })
            .collect(),
        show_previous: start > 0,
        show_next: end < specs.len(),
    }
}

fn measured_tab_width(widths: &[u16]) -> u16 {
    widths
        .iter()
        .copied()
        .fold(0_u16, |sum, width| sum.saturating_add(width))
        .saturating_add(widths.len().saturating_sub(1) as u16)
}

fn overflow_tab_width(widths: &[u16], start: usize, end: usize) -> u16 {
    let show_previous = start > 0;
    let show_next = end < widths.len();
    let mut item_widths = widths[start..end].to_vec();
    if show_previous {
        item_widths.insert(0, RESOURCE_TAB_ARROW_WIDTH);
    }
    if show_next {
        item_widths.push(RESOURCE_TAB_ARROW_WIDTH);
    }
    measured_tab_width(&item_widths)
}

fn overflow_controls_width(show_previous: bool, show_next: bool) -> u16 {
    let count = u16::from(show_previous) + u16::from(show_next);
    RESOURCE_TAB_ARROW_WIDTH
        .saturating_mul(count)
        .saturating_add(count)
}

fn resource_tab_content(spec: &ResourceTabSpec, max_width: usize) -> String {
    if UnicodeWidthStr::width(spec.full_label.as_str()) <= max_width {
        return spec.full_label.clone();
    }
    let identity_width = UnicodeWidthStr::width(spec.identity.as_str());
    if max_width <= identity_width {
        return truncate_display(&spec.identity, max_width);
    }
    let title_width = max_width.saturating_sub(identity_width).saturating_sub(1);
    if spec.title.is_empty() || title_width < 4 {
        return spec.identity.clone();
    }
    format!(
        "{} {}",
        spec.identity,
        truncate_display(&spec.title, title_width)
    )
}

fn render_resource_tab_gap(
    spans: &mut Vec<Span<'static>>,
    x: &mut u16,
    tab_right: u16,
    palette: &Palette,
) {
    if !spans.is_empty() && *x < tab_right {
        spans.push(Span::styled(" ", Style::default().bg(palette.surface0)));
        *x = x.saturating_add(1);
    }
}

fn render_resource_tab_arrow(
    spans: &mut Vec<Span<'static>>,
    state: &mut AppState,
    area: Rect,
    arrow: &'static str,
    target: HitTarget,
    palette: &Palette,
) {
    if area.width == 0 {
        return;
    }
    let text = fit_label_to_width(&format!("[{arrow}]"), area.width);
    spans.push(Span::styled(
        text,
        Style::default()
            .fg(palette.panel_bg)
            .bg(palette.accent)
            .add_modifier(Modifier::BOLD),
    ));
    state.hit_areas.push(HitArea::new(area, target));
}

pub(super) fn render_header_add_button(
    frame: &mut Frame<'_>,
    header_area: Rect,
    state: &mut AppState,
    palette: &Palette,
) {
    if header_area.width == 0 || header_area.height == 0 {
        return;
    }
    let horizontal_area = chrome_area_for_spacing(header_area, state.spacing);
    if horizontal_area.width == 0 {
        return;
    }
    let width = add_resource_button_width()
        .min(horizontal_area.width)
        .max(1);
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
        horizontal_area
            .x
            .saturating_add(horizontal_area.width.saturating_sub(width)),
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

pub(super) fn add_resource_button_width() -> u16 {
    3
}

pub(super) fn render_header(
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

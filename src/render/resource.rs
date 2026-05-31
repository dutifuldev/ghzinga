use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::{
    app::{AppState, BlockId, Tab},
    domain::{ActivityEntry, CheckRun, CheckStatus, Commit, MetadataItem, Resource, ResourceId},
    input::{HitArea, HitTarget},
    render::{markdown, Palette, Symbols, ViewRects},
};

struct ContentRow {
    line: Line<'static>,
    target: Option<HitTarget>,
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
    text: String,
    style: Style,
}

struct CheckGroupRenderContext<'a> {
    expanded_blocks: &'a std::collections::HashSet<BlockId>,
    width: usize,
    palette: &'a Palette,
    symbols: &'a Symbols,
}

impl ContentRow {
    fn plain(text: impl Into<String>) -> Self {
        Self {
            line: Line::from(text.into()),
            target: None,
        }
    }

    fn styled(text: impl Into<String>, style: Style) -> Self {
        Self {
            line: Line::from(Span::styled(text.into(), style)),
            target: None,
        }
    }

    fn target(text: impl Into<String>, target: HitTarget) -> Self {
        Self {
            line: Line::from(text.into()),
            target: Some(target),
        }
    }

    fn target_styled(text: impl Into<String>, target: HitTarget, style: Style) -> Self {
        Self {
            line: Line::from(Span::styled(text.into(), style)),
            target: Some(target),
        }
    }
}

pub fn render_app(frame: &mut Frame<'_>, state: &mut AppState) {
    let rects = ViewRects::compute(frame.area());
    let palette = state.theme.palette();
    state.hit_areas.clear();
    frame.buffer_mut().set_style(
        rects.area,
        Style::default().fg(palette.text).bg(palette.panel_bg),
    );
    render_header(frame, rects.header, &state.resource, &palette);
    render_tabs(frame, rects.tabs, state, &palette);
    render_status(frame, rects.status, state, &palette);
    render_content(frame, rects.content, state, &palette);
    render_footer(frame, rects.footer, state, &palette);
}

fn render_header(frame: &mut Frame<'_>, area: Rect, resource: &Resource, palette: &Palette) {
    let kind = resource.kind();
    let mut header = Vec::new();
    let meta_width = area.width as usize;
    let updated = format!("updated {}", resource.updated_at);
    let state = format!("[{} {}]", kind, resource.state);
    let compact = area.width < 56;
    let used = UnicodeWidthStr::width(state.as_str())
        + if compact {
            1
        } else {
            UnicodeWidthStr::width(updated.as_str()) + 2
        };
    let id_width = meta_width.saturating_sub(used).max(1);
    let mut meta_spans = vec![
        Span::styled(
            truncate_display(&resource.id.canonical_name(), id_width),
            Style::default()
                .fg(palette.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            state,
            resource_state_style(resource, palette).add_modifier(Modifier::BOLD),
        ),
    ];
    if !compact {
        meta_spans.push(Span::raw(" "));
        meta_spans.push(Span::styled(
            truncate_display(&updated, meta_width),
            dim_style(palette),
        ));
    }
    header.push(Line::from(meta_spans));

    let title_rows = area.height.saturating_sub(if compact { 3 } else { 2 }) as usize;
    let mut title_lines = markdown::wrap_plain_text(&resource.title, area.width as usize);
    title_lines.truncate(title_rows.max(1));
    for title in title_lines {
        header.push(Line::from(Span::styled(
            title,
            Style::default().fg(palette.text),
        )));
    }
    if compact && header.len() + 1 < area.height as usize {
        header.push(Line::from(Span::styled(
            truncate_display(&updated, area.width as usize),
            dim_style(palette),
        )));
    }
    while header.len() + 1 < area.height as usize {
        header.push(Line::from(""));
    }
    if area.height > 0 {
        header.push(separator_line(area.width, palette));
    }
    Paragraph::new(header)
        .style(Style::default().fg(palette.text).bg(palette.panel_bg))
        .render(area, frame.buffer_mut());
}

fn render_tabs(frame: &mut Frame<'_>, area: Rect, state: &mut AppState, palette: &Palette) {
    let mut lines = Vec::<Line<'static>>::new();
    let mut spans = Vec::<Span<'static>>::new();
    let mut x = area.x;
    let mut y = area.y;
    for tab in state.tabs() {
        let label = if *tab == state.active_tab {
            format!("[{}]", tab.label())
        } else {
            format!(" {} ", tab.label())
        };
        let width = UnicodeWidthStr::width(label.as_str()) as u16;
        if x > area.x && x.saturating_add(width) > area.x.saturating_add(area.width) {
            lines.push(Line::from(spans));
            spans = Vec::new();
            x = area.x;
            y = y.saturating_add(1);
        }
        if y >= area.y.saturating_add(area.height) {
            break;
        }
        state.hit_areas.push(HitArea::new(
            Rect::new(x, y, width.min(area.width), 1),
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
    Paragraph::new(lines)
        .style(Style::default().fg(palette.text).bg(palette.panel_bg))
        .render(area, frame.buffer_mut());
}

fn render_status(frame: &mut Frame<'_>, area: Rect, state: &AppState, palette: &Palette) {
    let resource = &state.resource;
    let symbols = state.symbols.symbols();
    let mut pieces = Vec::new();
    pieces.push(StyledPiece {
        text: format!(
            "{} {}",
            resource_state_symbol(resource, &symbols),
            resource.state
        ),
        style: resource_state_style(resource, palette).add_modifier(Modifier::BOLD),
    });
    push_status_piece(
        &mut pieces,
        format!("{} @{}", symbols.author, resource.author),
        Style::default().fg(palette.subtext0),
    );
    push_status_piece(
        &mut pieces,
        format!("{} {}", symbols.comments, resource.activity.len()),
        Style::default().fg(palette.teal),
    );
    push_status_piece(
        &mut pieces,
        format!("{} {}", symbols.reactions, resource.reactions.total()),
        Style::default().fg(palette.yellow),
    );
    if !resource.assignees.is_empty() {
        push_status_piece(
            &mut pieces,
            format!(
                "{} {}",
                symbols.assignees,
                people_summary(&resource.assignees)
            ),
            Style::default().fg(palette.blue),
        );
    }
    if resource.is_pull_request() {
        push_status_piece(
            &mut pieces,
            format!(
                "{} checks {}",
                checks_symbol(resource, &symbols),
                checks_summary(resource)
            ),
            checks_style(resource, palette).add_modifier(Modifier::BOLD),
        );
        if let Some(threads) = review_threads_summary(resource) {
            push_status_piece(
                &mut pieces,
                format!("{} {threads}", symbols.threads),
                Style::default().fg(palette.peach),
            );
        }
        if let Some(pr) = &resource.pull_request {
            push_status_piece(
                &mut pieces,
                format!(
                    "{} {} +{} -{}",
                    symbols.files,
                    pr.files.len(),
                    pr.additions,
                    pr.deletions
                ),
                Style::default().fg(palette.subtext0),
            );
        }
    }
    if !resource.warnings.is_empty() {
        push_status_piece(
            &mut pieces,
            format!("{} Warnings: {}", symbols.warning, resource.warnings.len()),
            Style::default()
                .fg(palette.yellow)
                .add_modifier(Modifier::BOLD),
        );
    }
    if let Some(refresh) = refresh_summary(state) {
        push_status_piece(
            &mut pieces,
            format!("{} {refresh}", symbols.refresh),
            Style::default().fg(palette.accent),
        );
    }
    if let Some(changes) = refresh_changes_summary(state) {
        push_status_piece(
            &mut pieces,
            format!("{} {changes}", symbols.changed),
            Style::default()
                .fg(palette.green)
                .add_modifier(Modifier::BOLD),
        );
    }

    let mut lines = wrap_styled_pieces(&pieces, area.width as usize);
    if let Some(message) = status_detail_line(state, &symbols) {
        let style = if state.last_error.is_some() {
            Style::default()
                .fg(palette.red)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette.subtext0)
        };
        lines.push(Line::from(Span::styled(
            truncate_display(&message, area.width as usize),
            style,
        )));
    }
    if lines.is_empty() {
        lines.push(separator_line(area.width, palette));
    }
    if lines.len() > area.height as usize {
        lines.truncate(area.height as usize);
        if let Some(last) = lines.last_mut() {
            *last = Line::from(Span::styled(
                truncate_display("...", area.width as usize),
                Style::default()
                    .fg(palette.overlay1)
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }
    Paragraph::new(lines)
        .style(Style::default().fg(palette.text).bg(palette.panel_bg))
        .render(area, frame.buffer_mut());
}

fn push_status_piece(pieces: &mut Vec<StyledPiece>, text: String, style: Style) {
    pieces.push(StyledPiece { text, style });
}

fn status_detail_line(state: &AppState, symbols: &Symbols) -> Option<String> {
    if let Some(error) = &state.last_error {
        return Some(format!("{} {error}", symbols.error));
    }
    if let Some(message) = &state.status_message {
        return Some(format!("{} {message}", symbols.info));
    }
    None
}

fn wrap_styled_pieces(pieces: &[StyledPiece], width: usize) -> Vec<Line<'static>> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut spans = Vec::new();
    let mut current_width = 0;
    for piece in pieces {
        let separator_width = usize::from(current_width > 0) * 2;
        let piece_width = UnicodeWidthStr::width(piece.text.as_str());
        if current_width > 0 && current_width + separator_width + piece_width > width {
            lines.push(Line::from(spans));
            spans = Vec::new();
            current_width = 0;
        }
        if current_width > 0 {
            spans.push(Span::raw("  "));
            current_width += 2;
        }
        let remaining = width.saturating_sub(current_width).max(1);
        let text = truncate_display(&piece.text, remaining);
        current_width += UnicodeWidthStr::width(text.as_str());
        spans.push(Span::styled(text, piece.style));
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
        _ => palette.subtext0,
    };
    Style::default().fg(color)
}

fn checks_symbol(resource: &Resource, symbols: &Symbols) -> &'static str {
    match summarize_checks(resource).as_str() {
        "PASS" => symbols.checks_pass,
        "FAIL" => symbols.checks_fail,
        "PENDING" => symbols.checks_pending,
        _ => symbols.checks_unknown,
    }
}

fn checks_style(resource: &Resource, palette: &Palette) -> Style {
    match summarize_checks(resource).as_str() {
        "PASS" => Style::default().fg(palette.green),
        "FAIL" => Style::default().fg(palette.red),
        "PENDING" => Style::default().fg(palette.yellow),
        _ => Style::default().fg(palette.subtext0),
    }
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
    let rows = content_rows(state, area.width as usize, palette);
    let max_scroll = rows.len().saturating_sub(area.height as usize) as u16;
    if state.scroll > max_scroll {
        state.scroll = max_scroll;
    }
    let visible_rows = rows
        .into_iter()
        .enumerate()
        .skip(state.scroll as usize)
        .take(area.height as usize)
        .collect::<Vec<_>>();
    let mut visible = Vec::new();
    for (visible_index, (_row_index, row)) in visible_rows.into_iter().enumerate() {
        if let Some(target) = row.target {
            state.hit_areas.push(HitArea::new(
                Rect::new(
                    area.x,
                    area.y.saturating_add(visible_index as u16),
                    area.width,
                    1,
                ),
                target,
            ));
        }
        visible.push(row.line);
    }
    Paragraph::new(visible)
        .style(Style::default().fg(palette.text).bg(palette.panel_bg))
        .render(area, frame.buffer_mut());
}

fn content_rows(state: &mut AppState, width: usize, palette: &Palette) -> Vec<ContentRow> {
    let symbols = state.symbols.symbols();
    if state.show_help {
        return help_rows(width, palette, &symbols);
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
        "- Click {} or {} to expand or collapse long text, checks, and files.",
        symbols.more, symbols.less
    );
    rows.extend(
        [
            "- Click tabs to switch sections.",
            expand_help.as_str(),
            "- Click visible GitHub issue or PR references to navigate.",
            "- Use the mouse wheel to scroll the current view.",
            "",
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
            "- r: refresh now",
            "- o: open current resource in browser through gh",
            "- Tab / Shift-Tab / Left / Right: switch tabs",
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

fn overview_rows(state: &mut AppState, width: usize, palette: &Palette) -> Vec<ContentRow> {
    let mut rows = vec![
        heading_row("Conversation", palette),
        separator_row(width, palette),
    ];

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
    let entries = chronological_timeline_entries(&state.resource);
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
            rows.push(separator_row(width, palette));
        }
    }
}

fn chronological_timeline_entries(resource: &Resource) -> Vec<TimelineEntry<'_>> {
    let mut entries = Vec::new();
    entries.push(TimelineEntry {
        sort_key: sortable_timestamp(&resource.created_at),
        sequence: 0,
        kind_order: 0,
        item: TimelineItem::Body,
    });

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
            symbols.body, resource.author, resource.created_at
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
    let expanded = state.block_expanded(&BlockId::Body);
    let wrapped = markdown::wrap_plain_text(&resource.body, width);
    let (visible, truncated) = markdown::visible_prefix(&wrapped, 12, expanded);
    rows.extend(
        visible
            .into_iter()
            .map(|line| linkable_text_row(line, resource)),
    );
    if truncated || expanded {
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
            commit.committed_at,
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
            entry.updated_at
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
    let expanded = state.block_expanded(&block);
    if expanded {
        if let Some(url) = &entry.url {
            rows.push(linkable_text_row(format!("url: {url}"), resource));
        }
    }
    let wrapped = markdown::wrap_plain_text(&entry.body, width);
    let (visible, truncated) = markdown::visible_prefix(&wrapped, 8, expanded);
    rows.extend(
        visible
            .into_iter()
            .map(|line| linkable_text_row(line, resource)),
    );
    if truncated || expanded {
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
        crate::domain::ActivityKind::Timeline => symbols.activity_timeline,
    }
}

fn activity_heading_style(entry: &ActivityEntry, palette: &Palette) -> Style {
    let color = match entry.kind {
        crate::domain::ActivityKind::Comment => palette.teal,
        crate::domain::ActivityKind::Review => palette.green,
        crate::domain::ActivityKind::ReviewComment => palette.peach,
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
        rows.push(ContentRow::plain(truncate_ascii(
            &format!("{}: {}", item.label, item.value),
            width,
        )));
    }
}

fn activity_rows(state: &mut AppState, width: usize, palette: &Palette) -> Vec<ContentRow> {
    let symbols = state.symbols.symbols();
    let mut rows = vec![
        ContentRow::styled(
            format!("Activity ({} entries)", state.resource.activity.len()),
            heading_style(palette),
        ),
        separator_row(width, palette),
    ];
    if state.resource.activity.is_empty() {
        rows.push(ContentRow::plain("No comments."));
        return rows;
    }
    for entry in &state.resource.activity {
        rows.push(ContentRow::plain(format!(
            "{} by @{} {}",
            entry.kind.label(),
            entry.author,
            entry.updated_at
        )));
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
        let expanded = state.block_expanded(&block);
        if expanded {
            if let Some(url) = &entry.url {
                rows.push(linkable_text_row(format!("url: {url}"), &state.resource));
            }
        }
        let wrapped = markdown::wrap_plain_text(&entry.body, width);
        let (visible, truncated) = markdown::visible_prefix(&wrapped, 8, expanded);
        rows.extend(
            visible
                .into_iter()
                .map(|line| linkable_text_row(line, &state.resource)),
        );
        if truncated || expanded {
            rows.push(ContentRow::target_styled(
                expand_label(expanded, &symbols),
                HitTarget::ToggleBlock(block),
                button_style(palette),
            ));
        }
        rows.push(ContentRow::plain(""));
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
    let mut rows = vec![
        ContentRow::styled(
            format!("Commits ({})", pr.commits.len()),
            heading_style(palette),
        ),
        separator_row(width, palette),
    ];
    for commit in &pr.commits {
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
        rows.push(ContentRow::plain(format!(
            "@{} {}",
            commit.author, commit.committed_at
        )));
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
        rows.push(ContentRow::plain(format!("authored: {authored_at}")));
    }
    rows.push(ContentRow::plain(format!(
        "committed: {}",
        commit.committed_at
    )));
    if !commit.deployments.is_empty() {
        rows.push(ContentRow::plain("deployments"));
        for deployment in &commit.deployments {
            rows.push(ContentRow::plain(truncate_ascii(
                &format!(
                    "- {} [{}] {}",
                    deployment.environment, deployment.state, deployment.updated_at
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
        ContentRow::styled(
            format!("Checks: {}", checks_summary(resource)),
            heading_style(palette),
        ),
        separator_row(width, palette),
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
        rows.push(ContentRow::target_styled(
            format!(
                "{}{} {}",
                prefix,
                truncate_ascii(&check.name, context.width.saturating_sub(reserved)),
                marker
            ),
            HitTarget::ToggleBlock(block.clone()),
            check_status_style(check.status, context.palette).add_modifier(Modifier::BOLD),
        ));
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
                rows.push(ContentRow::plain(format!("started: {started}")));
            }
            if let Some(completed) = check.completed_at.as_deref() {
                rows.push(ContentRow::plain(format!("completed: {completed}")));
            }
            if let Some(url) = check.details_url.as_deref() {
                rows.push(linkable_check_url_row(url));
            }
            if !summary.is_empty() {
                rows.push(ContentRow::plain(format!("summary: {}", summary)));
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
            ContentRow::target(format!("details: {url}"), HitTarget::Navigate(id))
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
    let mut rows = vec![
        ContentRow::styled(
            format!("Files changed ({})", pr.files.len()),
            heading_style(palette),
        ),
        separator_row(width, palette),
    ];
    for file in &pr.files {
        let block = BlockId::File(file.path.clone());
        let expanded = expanded_blocks.contains(&block);
        let marker = expand_label(expanded, symbols);
        rows.push(ContentRow::target_styled(
            format!(
                "+{:<4} -{:<4} {:<8} {} {}",
                file.additions,
                file.deletions,
                file.change_type,
                truncate_ascii(&file.path, width.saturating_sub(27)),
                marker
            ),
            HitTarget::ToggleBlock(block.clone()),
            link_style(palette),
        ));
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
                let wrapped = patch
                    .lines()
                    .flat_map(|line| markdown::wrap_plain_text(line, width))
                    .collect::<Vec<_>>();
                let (visible, truncated) = markdown::visible_prefix(&wrapped, 18, patch_expanded);
                rows.extend(visible.into_iter().map(ContentRow::plain));
                if truncated || patch_expanded {
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

fn links_rows(resource: &Resource, width: usize, palette: &Palette) -> Vec<ContentRow> {
    let mut rows = vec![heading_row("Links", palette), separator_row(width, palette)];
    let mut seen = std::collections::HashSet::new();
    for id in &resource.related_resources {
        if seen.insert(id.canonical_name()) {
            rows.push(ContentRow::target_styled(
                truncate_ascii(&id.canonical_name(), width),
                HitTarget::Navigate(id.clone()),
                link_style(palette),
            ));
        }
    }
    for token in linked_resource_tokens(resource) {
        if let Some((display, target)) = parse_link_token(token, resource) {
            let key = match &target {
                HitTarget::Navigate(id) => id.canonical_name(),
                HitTarget::OpenUrl(url) => url.clone(),
                _ => display.clone(),
            };
            if seen.insert(key) {
                rows.push(ContentRow::target_styled(
                    truncate_ascii(&display, width),
                    target,
                    link_style(palette),
                ));
            }
        }
    }
    if rows.len() == 2 {
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
        .filter(|token| token.contains("github.com") || token.starts_with('#'))
        .collect::<Vec<_>>();
    for entry in &resource.activity {
        tokens.extend(
            entry
                .body
                .split_whitespace()
                .filter(|token| token.contains("github.com") || token.starts_with('#')),
        );
    }
    tokens
}

fn parse_link_token(token: &str, resource: &Resource) -> Option<(String, HitTarget)> {
    let clean = token.trim_matches(|c: char| {
        matches!(
            c,
            ')' | '(' | ',' | '.' | ';' | ':' | '"' | '\'' | '[' | ']'
        )
    });
    let is_url = clean.starts_with("https://") || clean.starts_with("http://");
    if let Ok(id) = ResourceId::parse(clean) {
        if is_url && should_open_exact_url(clean, &id) {
            return Some((clean.to_string(), HitTarget::OpenUrl(clean.to_string())));
        }
        return Some((clean.to_string(), HitTarget::Navigate(id)));
    }
    if is_url {
        return Some((clean.to_string(), HitTarget::OpenUrl(clean.to_string())));
    }
    if let Some(number) = clean.strip_prefix('#') {
        if number.chars().all(|ch| ch.is_ascii_digit()) {
            let id =
                ResourceId::relative_to_repo(&resource.id.owner, &resource.id.repo, number).ok()?;
            return Some((
                format!("{}#{}", resource.id.repo_name_with_owner(), number),
                HitTarget::Navigate(id),
            ));
        }
    }
    None
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

fn render_footer(frame: &mut Frame<'_>, area: Rect, state: &mut AppState, palette: &Palette) {
    let symbols = state.symbols.symbols();
    let controls = [
        (symbols.footer_refresh, HitTarget::Refresh),
        (symbols.footer_open, HitTarget::OpenCurrent),
        (symbols.footer_help, HitTarget::Help),
        (symbols.footer_quit, HitTarget::Quit),
    ];
    let mut x = area.x;
    let mut y = area.y;
    let mut control_spans = Vec::new();
    let mut control_lines = Vec::<Line<'static>>::new();
    for (label, target) in controls.iter() {
        let width = UnicodeWidthStr::width(*label) as u16;
        if x > area.x && x.saturating_add(width) > area.x.saturating_add(area.width) {
            control_lines.push(Line::from(control_spans));
            control_spans = Vec::new();
            x = area.x;
            y = y.saturating_add(1);
        }
        if y >= area.y.saturating_add(area.height) {
            break;
        }
        state.hit_areas.push(HitArea::new(
            Rect::new(x, y, width.min(area.width), 1),
            target.clone(),
        ));
        if x > area.x {
            control_spans.push(Span::raw(" "));
        }
        control_spans.push(Span::styled(*label, button_style(palette)));
        x = x.saturating_add(width + 1);
    }
    if !control_spans.is_empty() {
        control_lines.push(Line::from(control_spans));
    }

    let default_message = format!(
        "r refresh | o open | q quit | ? help | tab/shift-tab switch | arrows/page scroll | e more/less | tab {} | scroll {}",
        state.active_tab.label(),
        state.scroll
    );
    let message = if let Some(error) = &state.last_error {
        format!("ERROR: {error}")
    } else if let Some(message) = &state.status_message {
        message.clone()
    } else {
        default_message
    };
    let message_style = if state.last_error.is_some() {
        Style::default()
            .fg(palette.red)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(palette.subtext0)
    };
    let remaining = area.height.saturating_sub(control_lines.len() as u16) as usize;
    let mut lines = control_lines;
    if remaining > 0 {
        for line in markdown::wrap_plain_text(&message, area.width as usize)
            .into_iter()
            .take(remaining)
        {
            lines.push(Line::from(Span::styled(line, message_style)));
        }
    } else if let Some(last) = lines.last_mut() {
        last.spans.push(Span::styled(" ...", dim_style(palette)));
    }
    Paragraph::new(lines)
        .style(Style::default().fg(palette.text).bg(palette.panel_bg))
        .render(area, frame.buffer_mut());
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
    "-".repeat(width as usize)
}

fn truncate_ascii(input: &str, max_width: usize) -> String {
    truncate_display(input, max_width)
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
    use ratatui::{backend::TestBackend, Terminal};

    use super::*;
    use crate::app::{apply_event, AppEvent, AppIntent};
    use crate::domain::{
        ActivityEntry, ActivityKind, ChangedFile, CheckRun, CheckStatus, Commit, Deployment,
        MetadataItem, PullRequest, ReactionCounts, ResourceId, ResourceKind,
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
                    authored_at: Some("2026-05-14T13:10:00Z".into()),
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

    fn draw(state: &mut AppState, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render_app(frame, state)).unwrap();
        format!("{:?}", terminal.backend().buffer())
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

        assert!(content.contains("openclaw/openclaw#81834"));
        assert!(content.contains("[Overview]"));
        assert!(content.contains("Conversation"));
        assert!(content.contains("checks PASS"));
        assert!(content.contains("* @KLilyZ opened"));
        assert!(content.contains("* commit fb948c9"));
        assert!(content.contains("[+ more]") || content.contains("Problem: senseaudio"));
        assert!(!content.contains("┌"));
        assert!(!content.contains("│"));
        assert!(!content.contains("─"));
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
        let backend = TestBackend::new(120, 36);
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
        let backend = TestBackend::new(120, 36);
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
            HitTarget::Navigate(id) if id.number == 66943 && id.kind_hint == Some(ResourceKind::Issue)
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
            HitTarget::Navigate(id) if id.canonical_name() == "openclaw/openclaw#66943"
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
            HitTarget::Navigate(id) if id.canonical_name() == "openclaw/openclaw#66943"
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
            HitTarget::Navigate(id) if id.canonical_name() == "openclaw/openclaw#66943"
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
            HitTarget::Navigate(id)
                if id.number == 81835 && id.kind_hint == Some(ResourceKind::PullRequest)
        )));
    }

    #[test]
    fn render_registers_exact_comment_url_as_open_url() {
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
            HitTarget::OpenUrl(url)
                if url == "https://github.com/openclaw/openclaw/pull/81834#discussion_r1"
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

        assert!(content.contains("[refresh] [open] [help] [quit]"));
        assert!(state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::Refresh));
        assert!(state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::OpenCurrent));
        assert!(state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::Quit));
        assert!(state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::Help));
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
                    HitTarget::Refresh | HitTarget::OpenCurrent | HitTarget::Help | HitTarget::Quit
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
        assert!(state
            .hit_areas
            .iter()
            .any(|area| area.target == HitTarget::OpenCurrent));
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
        assert!(content.contains("authored: 2026-05-14T13:10:00Z"));
        assert!(content.contains("committed: 1mo"));
        assert!(content.contains("Registers a SenseAudio speech provider."));
        assert!(content.contains("[- less]"));
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
        assert!(content.contains("+import speech"));
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

        assert!(content.contains("+patch line 0"));
        assert!(content.contains("+patch line 17"));
        assert!(!content.contains("+patch line 29"));
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

        assert!(content.contains("+patch line 29"));
        assert!(content.contains("[- less patch]"));
    }

    #[test]
    fn rendered_visible_link_hit_area_can_be_clicked_to_navigate() {
        let mut resource = pr_resource();
        resource.body = "Pairs with #66943".into();
        let mut state = AppState::new(resource);
        draw(&mut state, 120, 36);

        let intent = click_rendered_target(&mut state, |target| {
            matches!(
                target,
                HitTarget::Navigate(id) if id.canonical_name() == "openclaw/openclaw#66943"
            )
        });

        assert!(matches!(
            intent,
            AppIntent::Navigate(id) if id.canonical_name() == "openclaw/openclaw#66943"
        ));
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

        let mut open_state = AppState::new(pr_resource());
        draw(&mut open_state, 120, 36);
        let open =
            click_rendered_target(&mut open_state, |target| *target == HitTarget::OpenCurrent);
        assert!(matches!(
            open,
            AppIntent::OpenResource(id)
                if id.canonical_name() == "openclaw/openclaw#81834"
                    && id.kind_hint == Some(ResourceKind::PullRequest)
        ));

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

        state.toggle_block(BlockId::Activity("c1".into()));
        let content = draw(&mut state, 120, 36);

        assert!(
            content.contains("url: https://github.com/openclaw/openclaw/pull/81834#discussion_r1")
        );
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

        let content = draw(&mut state, 120, 36);

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
            started_at: Some("2026-05-30T03:28:54Z".into()),
            completed_at: Some("2026-05-30T03:28:56Z".into()),
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
        assert!(content.contains("started: 2026-05-30T03:28:54Z"));
        assert!(content.contains("completed: 2026-05-30T03:28:56Z"));
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

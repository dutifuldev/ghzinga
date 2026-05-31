use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
    Frame,
};

use crate::{
    app::{AppState, BlockId, Tab},
    domain::{CheckRun, CheckStatus, MetadataItem, Resource, ResourceId},
    input::{HitArea, HitTarget},
    render::{markdown, ViewRects},
};

struct ContentRow {
    line: Line<'static>,
    target: Option<HitTarget>,
}

impl ContentRow {
    fn plain(text: impl Into<String>) -> Self {
        Self {
            line: Line::from(text.into()),
            target: None,
        }
    }

    fn target(text: impl Into<String>, target: HitTarget) -> Self {
        Self {
            line: Line::from(text.into()),
            target: Some(target),
        }
    }
}

pub fn render_app(frame: &mut Frame<'_>, state: &mut AppState) {
    let rects = ViewRects::compute(frame.area());
    state.hit_areas.clear();
    render_header(frame, rects.header, &state.resource);
    render_tabs(frame, rects.tabs, state);
    render_status(frame, rects.status, state, rects.wide);
    render_content(frame, rects.content, state);
    render_footer(frame, rects.footer, state);
}

fn render_header(frame: &mut Frame<'_>, area: Rect, resource: &Resource) {
    let kind = resource.kind();
    let header = vec![
        Line::from(vec![
            Span::styled(
                resource.id.canonical_name(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(" [{} {}] ", kind, resource.state)),
            Span::raw(format!("updated {}", resource.updated_at)),
        ]),
        Line::from(resource.title.clone()),
        Line::from(horizontal_rule(area.width)),
    ];
    Paragraph::new(header).render(area, frame.buffer_mut());
}

fn render_tabs(frame: &mut Frame<'_>, area: Rect, state: &mut AppState) {
    let mut spans = Vec::new();
    let mut x = area.x;
    for tab in state.tabs() {
        let label = if *tab == state.active_tab {
            format!("[{}]", tab.label())
        } else {
            format!(" {} ", tab.label())
        };
        let width = label.len() as u16;
        state.hit_areas.push(HitArea::new(
            Rect::new(x, area.y, width, 1),
            HitTarget::Tab(*tab),
        ));
        x = x.saturating_add(width);
        spans.push(Span::raw(label));
    }
    Paragraph::new(Line::from(spans)).render(area, frame.buffer_mut());
}

fn render_status(frame: &mut Frame<'_>, area: Rect, state: &AppState, wide: bool) {
    let resource = &state.resource;
    let mut lines = Vec::new();
    if wide {
        let width = area.width as usize;
        lines.push(Line::from("Status"));
        lines.push(Line::from(horizontal_rule(area.width)));
        lines.push(Line::from(truncate_ascii(
            &format!("Type: {}", resource.kind()),
            width,
        )));
        lines.push(Line::from(truncate_ascii(
            &format!("State: {}", resource.state),
            width,
        )));
        lines.push(Line::from(truncate_ascii(
            &format!("Author: @{}", resource.author),
            width,
        )));
        lines.push(Line::from(truncate_ascii(
            &format!("Assignees: {}", people_summary(&resource.assignees)),
            width,
        )));
        lines.push(Line::from(truncate_ascii(
            &format!("Reactions: {}", reaction_summary(&resource.reactions)),
            width,
        )));
        lines.push(Line::from(truncate_ascii(
            &format!("Activity: {}", resource.activity.len()),
            width,
        )));
        if !resource.warnings.is_empty() {
            lines.push(Line::from(truncate_ascii(
                &format!("Warnings: {}", resource.warnings.len()),
                width,
            )));
        }
        for item in resource.metadata.iter().take(4) {
            lines.push(Line::from(truncate_ascii(
                &format!("{}: {}", item.label, item.value),
                width,
            )));
        }
        if let Some(refresh) = refresh_summary(state) {
            lines.push(Line::from(truncate_ascii(&refresh, width)));
        }
        if let Some(changes) = refresh_changes_summary(state) {
            lines.push(Line::from(truncate_ascii(&changes, width)));
        }
        if let Some(pr) = &resource.pull_request {
            lines.push(Line::from(truncate_ascii(
                &format!("Branches: {} <- {}", pr.base_ref, pr.head_ref),
                width,
            )));
            lines.push(Line::from(truncate_ascii(
                &format!("Review: {}", review_summary(pr)),
                width,
            )));
            lines.push(Line::from(truncate_ascii(
                &format!("Reviewers: {}", people_summary(&pr.requested_reviewers)),
                width,
            )));
            lines.push(Line::from(truncate_ascii(
                &format!("Merge: {}", merge_summary(pr)),
                width,
            )));
            lines.push(Line::from(truncate_ascii(
                &format!("Checks: {}", checks_summary(resource)),
                width,
            )));
            lines.push(Line::from(truncate_ascii(
                &format!("Files: {}", pr.files.len()),
                width,
            )));
            lines.push(Line::from(truncate_ascii(
                &format!("Lines: +{} -{}", pr.additions, pr.deletions),
                width,
            )));
            for item in pr.metadata.iter().take(3) {
                lines.push(Line::from(truncate_ascii(
                    &format!("{}: {}", item.label, item.value),
                    width,
                )));
            }
        }
    } else {
        let mut line = format!(
            "{} {} | comments {} | reactions {}",
            resource.kind(),
            resource.state,
            resource.activity.len(),
            reaction_summary(&resource.reactions)
        );
        if !resource.assignees.is_empty() {
            line.push_str(&format!(
                " | assigned {}",
                people_summary(&resource.assignees)
            ));
        }
        if resource.is_pull_request() {
            line.push_str(&format!(" | checks {}", checks_summary(resource)));
        }
        if !resource.warnings.is_empty() {
            line.push_str(&format!(" | warnings {}", resource.warnings.len()));
        }
        if let Some(refresh) = refresh_summary(state) {
            line.push_str(&format!(" | {refresh}"));
        }
        if let Some(changes) = refresh_changes_summary(state) {
            line.push_str(&format!(" | {changes}"));
        }
        lines.push(Line::from(truncate_ascii(&line, area.width as usize)));
        lines.push(Line::from(horizontal_rule(area.width)));
    }
    Paragraph::new(lines).render(area, frame.buffer_mut());
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

fn render_content(frame: &mut Frame<'_>, area: Rect, state: &mut AppState) {
    let rows = content_rows(state, area.width as usize);
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
    Paragraph::new(visible).render(area, frame.buffer_mut());
}

fn content_rows(state: &mut AppState, width: usize) -> Vec<ContentRow> {
    if state.show_help {
        return help_rows(width);
    }
    match state.active_tab {
        Tab::Overview => overview_rows(state, width),
        Tab::Activity => activity_rows(state, width),
        Tab::Commits => commits_rows(state, width),
        Tab::Checks => checks_rows(state, width),
        Tab::Files => files_rows(state, width),
        Tab::Links => links_rows(&state.resource, width),
    }
}

fn help_rows(width: usize) -> Vec<ContentRow> {
    [
        "Help",
        "",
        "Mouse",
        "- Click tabs to switch sections.",
        "- Click [more] or [less] to expand or collapse long text, checks, and files.",
        "- Click visible GitHub issue or PR references to navigate.",
        "- Use the mouse wheel to scroll the current view.",
        "",
        "Keyboard",
        "- q: quit",
        "- ?: toggle this help",
        "- r: refresh now",
        "- o: open current resource in browser through gh",
        "- Tab / Shift-Tab / Left / Right: switch tabs",
        "- Up / Down / PageUp / PageDown / Home / End: scroll",
        "- e: expand or collapse the main body",
        "- Backspace: return after following a link",
        "",
        "Refresh",
        "- Live mode refreshes automatically on the configured interval.",
        "- The status panel shows the last refresh time and whether content changed.",
    ]
    .into_iter()
    .flat_map(|line| markdown::wrap_plain_text(line, width))
    .map(ContentRow::plain)
    .collect()
}

fn overview_rows(state: &mut AppState, width: usize) -> Vec<ContentRow> {
    let expanded = state.block_expanded(&BlockId::Body);
    let wrapped = markdown::wrap_plain_text(&state.resource.body, width);
    let (visible, truncated) = markdown::visible_prefix(&wrapped, 12, expanded);
    let mut rows = vec![
        ContentRow::plain("Overview"),
        ContentRow::plain(horizontal_rule(width as u16)),
        ContentRow::plain(format!("Labels: {}", state.resource.labels.join(", "))),
        ContentRow::plain(format!(
            "Assignees: {}",
            people_summary(&state.resource.assignees)
        )),
        ContentRow::plain(format!(
            "Reactions: {}",
            reaction_summary(&state.resource.reactions)
        )),
    ];
    if !state.resource.warnings.is_empty() {
        rows.push(ContentRow::plain(""));
        rows.push(ContentRow::plain("Warnings"));
        for warning in &state.resource.warnings {
            rows.extend(
                markdown::wrap_plain_text(&format!("- {warning}"), width)
                    .into_iter()
                    .map(ContentRow::plain),
            );
        }
    }
    rows.push(ContentRow::plain(""));
    rows.push(ContentRow::plain("Body"));
    rows.extend(
        visible
            .into_iter()
            .map(|line| linkable_text_row(line, &state.resource)),
    );
    if truncated || expanded {
        rows.push(ContentRow::target(
            if expanded { "[less]" } else { "[more]" },
            HitTarget::ToggleBlock(BlockId::Body),
        ));
    }
    if let Some(pr) = &state.resource.pull_request {
        rows.push(ContentRow::plain(""));
        rows.push(ContentRow::plain("Change summary"));
        rows.push(ContentRow::plain(format!("Review: {}", review_summary(pr))));
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
        push_metadata_rows(&mut rows, "PR metadata", &pr.metadata, width);
    }
    push_metadata_rows(&mut rows, "Metadata", &state.resource.metadata, width);
    rows
}

fn push_metadata_rows(
    rows: &mut Vec<ContentRow>,
    heading: &str,
    metadata: &[MetadataItem],
    width: usize,
) {
    if metadata.is_empty() {
        return;
    }
    rows.push(ContentRow::plain(""));
    rows.push(ContentRow::plain(heading));
    for item in metadata {
        rows.push(ContentRow::plain(truncate_ascii(
            &format!("{}: {}", item.label, item.value),
            width,
        )));
    }
}

fn activity_rows(state: &mut AppState, width: usize) -> Vec<ContentRow> {
    let mut rows = vec![
        ContentRow::plain(format!(
            "Activity ({} entries)",
            state.resource.activity.len()
        )),
        ContentRow::plain(horizontal_rule(width as u16)),
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
            rows.push(ContentRow::target(
                if expanded { "[less]" } else { "[more]" },
                HitTarget::ToggleBlock(block),
            ));
        }
        rows.push(ContentRow::plain(""));
    }
    rows
}

fn commits_rows(state: &AppState, width: usize) -> Vec<ContentRow> {
    let resource = &state.resource;
    let Some(pr) = &resource.pull_request else {
        return vec![ContentRow::plain(
            "Commits are only available for pull requests.",
        )];
    };
    let mut rows = vec![
        ContentRow::plain(format!("Commits ({})", pr.commits.len())),
        ContentRow::plain(horizontal_rule(width as u16)),
    ];
    for commit in &pr.commits {
        let block = BlockId::Commit(commit.oid.clone());
        let expanded = state.block_expanded(&block);
        let marker = if expanded { "[less]" } else { "[more]" };
        rows.push(ContentRow::target(
            format!(
                "{} {} [{}] {}",
                truncate_ascii(&commit.oid, 8),
                truncate_ascii(&commit.message, width.saturating_sub(25)),
                commit.status.label(),
                marker
            ),
            HitTarget::ToggleBlock(block.clone()),
        ));
        rows.push(ContentRow::plain(format!(
            "@{} {}",
            commit.author, commit.committed_at
        )));
        if expanded {
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
                let (visible, truncated) = markdown::visible_prefix(&wrapped, 14, false);
                rows.extend(visible.into_iter().map(ContentRow::plain));
                if truncated {
                    rows.push(ContentRow::plain("[body truncated]"));
                }
            }
            rows.push(ContentRow::target("[less]", HitTarget::ToggleBlock(block)));
            rows.push(ContentRow::plain(""));
        }
    }
    rows
}

fn checks_rows(state: &AppState, width: usize) -> Vec<ContentRow> {
    let resource = &state.resource;
    let Some(pr) = &resource.pull_request else {
        return vec![ContentRow::plain(
            "Checks are only available for pull requests.",
        )];
    };
    let counts = pr.check_counts();
    let mut rows = vec![
        ContentRow::plain(format!("Checks: {}", checks_summary(resource))),
        ContentRow::plain(horizontal_rule(width as u16)),
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
    push_check_group(
        &mut rows,
        "Failing",
        &pr.checks,
        CheckStatus::Failure,
        &state.expanded_blocks,
        width,
    );
    push_check_group(
        &mut rows,
        "Pending",
        &pr.checks,
        CheckStatus::Pending,
        &state.expanded_blocks,
        width,
    );
    push_check_group(
        &mut rows,
        "Passing",
        &pr.checks,
        CheckStatus::Success,
        &state.expanded_blocks,
        width,
    );
    push_check_group(
        &mut rows,
        "Neutral",
        &pr.checks,
        CheckStatus::Neutral,
        &state.expanded_blocks,
        width,
    );
    push_check_group(
        &mut rows,
        "Skipped",
        &pr.checks,
        CheckStatus::Skipped,
        &state.expanded_blocks,
        width,
    );
    push_check_group(
        &mut rows,
        "Unknown",
        &pr.checks,
        CheckStatus::Unknown,
        &state.expanded_blocks,
        width,
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
    expanded_blocks: &std::collections::HashSet<BlockId>,
    width: usize,
) {
    let matching = checks
        .iter()
        .filter(|check| check.status == status)
        .collect::<Vec<_>>();
    if matching.is_empty() {
        return;
    }
    rows.push(ContentRow::plain(format!("{label} ({})", matching.len())));
    for check in matching {
        let summary = check.summary.clone().unwrap_or_default();
        let block = BlockId::Check(format!("{}:{}", check.status.label(), check.name));
        let expanded = expanded_blocks.contains(&block);
        let marker = if expanded { "[less]" } else { "[more]" };
        let prefix = format!("[{}] ", check.status.label());
        let reserved = prefix.len() + marker.len() + 1;
        rows.push(ContentRow::target(
            format!(
                "{}{} {}",
                prefix,
                truncate_ascii(&check.name, width.saturating_sub(reserved)),
                marker
            ),
            HitTarget::ToggleBlock(block.clone()),
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
            rows.push(ContentRow::target("[less]", HitTarget::ToggleBlock(block)));
            rows.push(ContentRow::plain(""));
        }
    }
    rows.push(ContentRow::plain(""));
}

fn linkable_check_url_row(url: &str) -> ContentRow {
    if let Ok(id) = ResourceId::parse(url) {
        ContentRow::target(format!("details: {url}"), HitTarget::Navigate(id))
    } else {
        ContentRow::plain(format!("details: {url}"))
    }
}

fn files_rows(state: &AppState, width: usize) -> Vec<ContentRow> {
    let Some(pr) = &state.resource.pull_request else {
        return vec![ContentRow::plain(
            "Files are only available for pull requests.",
        )];
    };
    files_rows_for_pr(pr, width, &state.expanded_blocks)
}

fn files_rows_for_pr(
    pr: &crate::domain::PullRequest,
    width: usize,
    expanded_blocks: &std::collections::HashSet<BlockId>,
) -> Vec<ContentRow> {
    let mut rows = vec![
        ContentRow::plain(format!("Files changed ({})", pr.files.len())),
        ContentRow::plain(horizontal_rule(width as u16)),
    ];
    for file in &pr.files {
        let block = BlockId::File(file.path.clone());
        let expanded = expanded_blocks.contains(&block);
        let marker = if expanded { "[less]" } else { "[more]" };
        rows.push(ContentRow::target(
            format!(
                "+{:<4} -{:<4} {:<8} {} {}",
                file.additions,
                file.deletions,
                file.change_type,
                truncate_ascii(&file.path, width.saturating_sub(27)),
                marker
            ),
            HitTarget::ToggleBlock(block.clone()),
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
                    rows.push(ContentRow::target(
                        if patch_expanded {
                            "[less patch]"
                        } else {
                            "[more patch]"
                        },
                        HitTarget::ToggleBlock(patch_block),
                    ));
                }
            } else {
                rows.push(ContentRow::plain("patch: not loaded"));
            }
            rows.push(ContentRow::target("[less]", HitTarget::ToggleBlock(block)));
            rows.push(ContentRow::plain(""));
        }
    }
    rows
}

fn links_rows(resource: &Resource, width: usize) -> Vec<ContentRow> {
    let mut rows = vec![
        ContentRow::plain("Links"),
        ContentRow::plain(horizontal_rule(width as u16)),
    ];
    let mut seen = std::collections::HashSet::new();
    for id in &resource.related_resources {
        if seen.insert(id.canonical_name()) {
            rows.push(ContentRow::target(
                truncate_ascii(&id.canonical_name(), width),
                HitTarget::Navigate(id.clone()),
            ));
        }
    }
    for token in linked_resource_tokens(resource) {
        if let Some((display, id)) = parse_link_token(token, resource) {
            if seen.insert(id.canonical_name()) {
                rows.push(ContentRow::target(
                    truncate_ascii(&display, width),
                    HitTarget::Navigate(id),
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

fn parse_link_token(token: &str, resource: &Resource) -> Option<(String, ResourceId)> {
    let clean = token.trim_matches(|c: char| {
        matches!(
            c,
            ')' | '(' | ',' | '.' | ';' | ':' | '"' | '\'' | '[' | ']'
        )
    });
    if let Ok(id) = ResourceId::parse(clean) {
        return Some((clean.to_string(), id));
    }
    if let Some(number) = clean.strip_prefix('#') {
        if number.chars().all(|ch| ch.is_ascii_digit()) {
            let id =
                ResourceId::relative_to_repo(&resource.id.owner, &resource.id.repo, number).ok()?;
            return Some((
                format!("{}#{}", resource.id.repo_name_with_owner(), number),
                id,
            ));
        }
    }
    None
}

fn linkable_text_row(text: String, resource: &Resource) -> ContentRow {
    if let Some((_display, id)) = text
        .split_whitespace()
        .find_map(|token| parse_link_token(token, resource))
    {
        ContentRow::target(text, HitTarget::Navigate(id))
    } else {
        ContentRow::plain(text)
    }
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, state: &mut AppState) {
    let controls = [
        ("[refresh]", HitTarget::Refresh),
        ("[open]", HitTarget::OpenCurrent),
        ("[quit]", HitTarget::Quit),
        ("[help]", HitTarget::Help),
    ];
    let mut x = area.x;
    for (label, target) in controls {
        state.hit_areas.push(HitArea::new(
            Rect::new(x, area.y, label.len() as u16, 1),
            target,
        ));
        x = x.saturating_add(label.len() as u16 + 1);
    }

    let controls = "[refresh] [open] [quit] [help]";
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
    let footer = format!("{controls} | {message}");
    Paragraph::new(truncate_ascii(&footer, area.width as usize)).render(area, frame.buffer_mut());
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
    if input.chars().count() <= max_width {
        return input.to_string();
    }
    if max_width <= 3 {
        return ".".repeat(max_width);
    }
    let prefix = input.chars().take(max_width - 3).collect::<String>();
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
        assert!(content.contains("Checks: PASS"));
        assert!(content.contains("Assignees: @osolmaz"));
        assert!(content.contains("Reviewers: @maintainer"));
        assert!(content.contains("[more]") || content.contains("Problem: senseaudio"));
        assert!(!content.contains("┌"));
        assert!(!content.contains("│"));
    }

    #[test]
    fn renders_resource_and_pr_metadata() {
        let backend = TestBackend::new(120, 36);
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
    fn render_registers_footer_action_hit_areas() {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new(pr_resource());

        terminal
            .draw(|frame| render_app(frame, &mut state))
            .unwrap();
        let content = format!("{:?}", terminal.backend().buffer());

        assert!(content.contains("[refresh] [open] [quit] [help]"));
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
        assert!(content.contains("[more]"));
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
        assert!(content.contains("[less]"));
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
        assert!(content.contains("[more patch]"));
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
        assert!(content.contains("[less patch]"));
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

        assert!(content.contains("[more]"));
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
        assert!(content.contains("[less]"));
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

        assert!(content.contains("[more]"));
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

        assert!(content.contains("[more]"));
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
        assert!(content.contains("[less]"));
    }

    #[test]
    fn truncation_does_not_split_emoji_bytes() {
        assert_eq!(truncate_ascii("rating: shrimp", 10), "rating:...");
        assert_eq!(
            truncate_ascii("rating: 🦐 gold shrimp", 12),
            "rating: 🦐..."
        );
    }
}

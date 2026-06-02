---
title: Startup Progressive Loading Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-06-02
---

# Startup Progressive Loading Plan

`gzg owner/repo#number` should show a real TUI frame immediately. The current
live startup path waits for the GitHub resource fetch before entering the
alternate screen, so a slow GraphQL/REST path feels like the command hung even
when the fetch is healthy.

## Current bottleneck

- CLI parsing resolves the `ResourceId` quickly.
- Live mode then calls `GithubApiGateway::fetch_resource` before `run_tui`.
- Only after the complete resource is available does Ratatui render the first
  frame.
- Background fetch behavior already exists for refresh, full-depth loading,
  linked-resource navigation, and Backspace navigation. Those paths keep the
  old resource readable and show `Loading |: ...`.

## Chosen solution

Start live TUI mode with a lightweight placeholder resource, then immediately
kick off the normal GitHub fetch in the existing single-flight background fetch
pipeline.

The placeholder should include:

- the parsed owner, repo, number, and a GitHub web URL
- a `LOADING` state so the status band clearly distinguishes startup data from
  loaded data
- a short title and body explaining that GitHub data is loading
- pull request tabs when the input is explicitly a PR URL, or when the input is
  an ambiguous `owner/repo#number`; GitHub issue URLs keep the smaller issue tab
  set

This is not a fake cache. It is an immediate render shell that gets replaced by
the real resource as soon as the API call completes.

## UX rules

- The first frame should include the normal header, status band, tab bar,
  content area, footer controls, and loading indicator.
- The loading detail line should say `Loading |: opening owner/repo#number from
  GitHub` and animate on later frames.
- Settings, help, quit, scrolling, and mouse handling should work during
  startup loading.
- Duplicate refresh/navigation starts should still be ignored while the initial
  fetch is active.
- On success, replace the placeholder with the fetched resource and show
  `loaded owner/repo#number`.
- On failure, keep the placeholder visible and show the recoverable error.
- `--once` should continue to block until the real resource is fetched because
  it prints a static buffer for scripts and tests.
- Offline fixture mode should continue to load the fixture before rendering
  because it is local, fast, and deterministic.

## Later optimization

This change fixes perceived startup latency without changing the API contract.
The next deeper optimization is a true section-level progressive data model:

- fetch a minimal REST summary first
- render title/state/body quickly
- load checks, files, commits, review threads, linked resources, and optional
  metadata as independent sections
- keep section-level loading states so a slow checks/files page does not block
  conversation reading

That would require a larger domain change. The placeholder startup path is the
lowest-risk production improvement because it reuses the existing background
fetch queue and error handling.

## Implementation checklist

- Add an app-layer `loading_resource_placeholder(ResourceId)` helper so the
  domain model stays free of startup/rendering shell concerns.
- Add an initial background fetch action for live TUI startup.
- Enter `run_tui` before the first live GitHub fetch completes.
- Preserve blocking startup for `--once` and offline fixtures.
- Style `LOADING` as a distinct status.
- Add unit/render coverage for the placeholder and initial fetch outcome.
- Update README and verification docs.
- Run SimpleDoc, Rust tests, smoke captures if render snapshots change, and the
  local CI script.

---
title: ghzinga HTTP Transport Resilience Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-06-01
---

# ghzinga HTTP Transport Resilience Plan

`ghzinga` is a monitoring TUI, so GitHub API calls should never make the UI feel
permanently stuck. The app already keeps rendering the previous resource during
background fetches and ignores duplicate fetch starts. The transport also needs
bounded HTTP behavior.

## Rules

- Reuse one reqwest client for all GitHub calls so refreshes and paginated
  enrichment do not rebuild connection pools for every request.
- Put a per-request timeout on every direct GitHub GraphQL and REST call.
- Keep the timeout at the transport boundary, not scattered through GraphQL,
  public REST fallback, or runner code.
- Preserve the mockable `GithubHttpTransport` shape so tests can assert request
  method, URL, token, body, accept header, and timeout without live network I/O.
- Let timeout failures flow through the existing recoverable error and warning
  paths. A failed enrichment should warn; a failed refresh should keep the
  previous rendered resource visible.

## Current Setting

`GITHUB_HTTP_TIMEOUT` is 30 seconds. That is long enough for GitHub API latency
and large REST diff responses, but bounded enough that a bad network path does
not leave refresh or navigation waiting indefinitely.

## Verification

- GraphQL request-shape tests assert the configured timeout.
- GraphQL rate-limit preflight tests assert the configured timeout.
- REST request-shape tests assert the configured timeout.
- Public unauthenticated REST fallback continues to use the same transport
  boundary, so it inherits the timeout without duplicating HTTP code.

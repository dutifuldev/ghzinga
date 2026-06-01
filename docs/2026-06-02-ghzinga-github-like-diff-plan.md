---
title: GitHub-Like Diff Rendering Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-06-02
---

# GitHub-Like Diff Rendering Plan

## Goal

The Files tab should make inline PR diffs feel like GitHub's web diff instead of
raw unified diff text. Change type should be communicated by row styling, not by
visible leading `+` and `-` markers inside the code column.

## Rendering Rules

- Keep file summary rows unchanged: they still show aggregate additions and
  deletions.
- In patch bodies, classify each unified-diff line before rendering:
  addition, deletion, context, hunk header, or metadata.
- Strip only the one-character unified-diff content marker from addition,
  deletion, and context lines.
- Preserve all code whitespace after that marker, including indentation and
  blank changed lines.
- Keep metadata visible as metadata, including `diff --git`, `index`, file
  headers (`---` / `+++`), rename/mode lines, and `\ No newline...` markers.
- Keep hunk headers visible, including their Git range syntax.
- Use green background tint for additions and red background tint for deletions.
  Do not rely on foreground-only red/green.
- Wrap patch lines with a code-oriented display-width wrapper that does not
  collapse or reflow whitespace.

## Implementation Checklist

- Replace the patch-body path that currently uses prose markdown wrapping.
- Add explicit diff line classification helpers so `+++ b/file` and `--- a/file`
  are not mistaken for added or deleted code.
- Add renderer tests for stripped markers, preserved indentation, context-line
  marker removal, metadata styling, hunk styling, and long patch expansion.
- Refresh tmux capture evidence if rendered diff transcripts change.

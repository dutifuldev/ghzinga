---
title: ghzinga Capture Evidence Refresh Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-05-31
---

# ghzinga Capture Evidence Refresh Plan

## Goal

Refresh the tmux capture evidence after the responsive chrome changes and make
the captures easier to audit later. The screenshots and text captures should
prove the current renderer, not an earlier build.

## Current Gap

The repository already contains PR and issue capture directories, but the
capture metadata is too thin for a strict evidence audit:

- size manifests list requested dimensions, but not the actual tmux dimensions
- frame metadata lists output files, but not the command, tab, or keys used
- manifests do not record the git revision being captured
- validation is documented in prose, but there is no repeatable marker check
  command tied to the capture script

After changing responsive chrome, the old captures are also stale. They still
show previous footer/status wrapping behavior.

## Plan

1. Extend `captures/ghzinga-pr-81834/capture_ghzinga.py` so each run records:
   - git commit
   - target resource
   - mode
   - binary path
   - command used for each frame
   - requested and actual tmux size
   - tab and keys used for each frame
   - history capture paths
2. Add a validation mode to the same script:
   - verify every expected frame exists for narrow, medium, and large sizes
   - verify current marker and high-value content text in each frame set
   - verify the footer action surface is present
   - verify each size manifest records actual tmux dimensions
   - verify no app/rendering source paths changed since the recorded capture
     revision unless `--allow-stale-revision` is explicitly passed
3. Regenerate PR captures for `openclaw/openclaw#81834`.
4. Regenerate issue captures for `openclaw/openclaw#88499`.
5. Run validation against both capture roots.
6. Update the UX capture report and verification matrix with the stronger
   evidence path.

## Non-Goals

- Do not change application behavior in this slice.
- Do not add the capture command to regular CI, because it depends on tmux,
  image fonts, live GitHub access, and local auth.

## Expected Result

The capture directories should become reproducible evidence for the current UI:
an auditor can inspect frame text/PNG output, see exactly how each frame was
created, and rerun the marker/content validation without manually reading every
file.
By default, validation should fail when app/rendering code changed after the
recorded capture revision, so UX evidence cannot silently drift behind the code
under review while still allowing capture artifact commits to sit on top.

---
id: 2
title: Preserve observation provenance and freshness
type: feature
status: planned
priority: p1
area: scan
created: 2026-09-18
updated: 2026-09-18
---

## Problem

The first schema records most scanner results as values or optional values. That cannot fully distinguish absent evidence from an unavailable API, a stale cache, a failed probe, or a capability that does not apply to the project's declared intent.

## Proposal

Introduce a generic observation envelope carrying state, source, observation time, freshness, confidence, and a bounded error. Use it at every local, GitHub, Cairn, deployment, and package-registry boundary while keeping the compact values ergonomic for the TUI.

## Acceptance criteria

- [ ] Observations distinguish present, missing, unknown, not-applicable, stale, and error.
- [ ] Every remote value exposes its source and observation time.
- [ ] Score confidence derives from evidence coverage and freshness.
- [ ] JSON snapshots retain errors without turning them into negative project evidence.
- [ ] Cached observations visibly age and can be refreshed independently.

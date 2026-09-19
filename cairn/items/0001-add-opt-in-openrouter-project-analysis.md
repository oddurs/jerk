---
id: 1
title: Add opt-in OpenRouter project analysis
type: feature
status: backlog
priority: p2
area: ai
created: 2026-09-18
updated: 2026-09-18
---

## Problem

The dashboard exposes the facts that describe a project, but it does not yet turn their relationships into a concise narrative about risks, momentum, or the highest-leverage next step.

## Proposal

Add an opt-in OpenRouter analysis panel behind a small provider interface. Read the key from `OPENROUTER_API_KEY` or OS credential storage, send only the bounded metrics snapshot by default, cache by snapshot hash, and make the exact payload inspectable before sending.

## Acceptance criteria

- [ ] No source code or file content is sent by default.
- [ ] The outbound snapshot can be previewed.
- [ ] Keys never enter repository configuration or logs.
- [ ] Offline and provider failures leave every existing view usable.
- [ ] The summary distinguishes observations from recommendations.

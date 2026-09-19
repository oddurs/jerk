# The project-effectiveness schema

The unit shown by jerk is a **project**, not necessarily a repository. A project may contain a source repository, a documentation site, one or more deployments, release artifacts, package registries, distribution taps, and supporting repositories. Treating the repository as the project loses exactly the delivery information this dashboard exists to reveal.

The schema is versioned with `schema = 1` in a committed `.jerk.toml`. Everything can be inferred, but declarations take precedence because a filesystem cannot determine whether quiet software is complete or abandoned.

## The model

```text
Portfolio
└── Project
    ├── Intent: kind, lifecycle, tier, outcomes
    ├── Components: repositories, sites, services, packages, data
    ├── Expectations: which evidence is applicable
    ├── Observations: value, source, age, confidence, error
    ├── Assessments: readiness, vitality, delivery, reliability, reach
    └── Work: Cairn flow, milestones, blockers, aging
```

Every observation needs more than a value. Its complete shape is:

```text
value + state + source + observed_at + freshness + confidence
```

`state` is one of `present`, `missing`, `unknown`, `not-applicable`, `stale`, or `error`. This distinction prevents an unauthenticated GitHub request from becoming a failed CI check and prevents a command-line tool with no website from losing deployment points.

## Intent

These fields are the smallest useful human declaration:

| Field | Values | Why it exists |
|---|---|---|
| `kind` | `cli`, `tui`, `web`, `library`, `service`, `research`, `creative`, `simulation`, `theme`, `infrastructure`, `configuration`, `distribution`, `learning`, `application` | Selects sensible default expectations. |
| `lifecycle` | `incubating`, `active`, `maintenance`, `complete`, `paused`, `archived` | Makes inactivity interpretable. |
| `tier` | `flagship`, `supported`, `experimental`, `personal`, `archive` | Describes the maintenance promise. |
| `intent` | one sentence | Says what effectiveness means for this project. |
| `outcomes` | assertions | Defines success without confusing output with impact. |

Kind, lifecycle, and tier form the evaluation profile. An active flagship TUI should have tests, CI, releases, documentation, and a roadmap. A complete creative work should remain healthy without recent commits, an issue queue, or release automation. A paused experiment should invite a decision, not fabricate a quality failure.

## Evidence axes

The dashboard should keep these axes separate. A single roll-up may summarize them, but must never replace them.

### Readiness

- description and explicit intent;
- README, license, contributor guide, changelog, and reference docs;
- reproducible build and install instructions;
- tests, linting, formatting, and CI presence;
- dependency update policy and security policy;
- repository topics and discoverability metadata.

### Vitality

- last meaningful change, commits over 7/30/90 days, and active days;
- worktree/upstream state and unreleased commits;
- contributor count and concentration;
- dependency freshness and maintenance debt;
- lifecycle-relative staleness rather than raw inactivity.

### Delivery

- current version and latest release age;
- artifact availability, checksums, signatures, and package channels;
- release-asset downloads and version drift between channels;
- site/deployment existence and current deployed revision;
- time from merge to release or deployment.

### Reliability

- required default-branch checks rather than merely the latest workflow;
- deployment state plus independent HTTP health and latency;
- recent failure rate and time since last green build;
- branch rules, protected environments, dependency alerts, and known vulnerabilities;
- smoke-test and rollback evidence where the project promises service.

### Work and flow

- Cairn counts by semantic category, not hard-coded status names;
- blocked age, active-item age, ready work, and unscheduled work;
- milestone completion, due-date risk, and scope change;
- throughput and cycle time from first active state to done;
- work in progress relative to the declared tier.

### Reach and adoption

- unique repository visitors and clones with their 14-day window attached;
- release-asset downloads, package downloads, and installer-channel usage;
- site traffic when a project has a site;
- stars, forks, external contributors, and inbound issues as secondary context;
- outcome-specific measures declared by the project.

Stars are intentionally not the primary effectiveness measure. A repository can have meaningful cloning and release consumption before it has a social audience.

## Portfolio-level signals

Some of the most important facts do not belong to any one repository:

- active projects and new projects created per 7/30/90 days;
- concurrent work in progress and context-switch pressure;
- projects whose inferred lifecycle needs a human decision;
- unarchived stale repositories;
- duplicated ideas and families of related repositories;
- distribution repositories whose versions trail their source project;
- cloud-only repositories versus local-only worktrees;
- total maintenance surface by tier and language.

## Committed declaration

```toml
schema = 1

[project]
kind = "tui"
lifecycle = "active"
tier = "flagship"
intent = "Make project state legible from one terminal."
outcomes = ["A portfolio can be triaged in under a minute."]

[expect]
documentation = true
tests = true
ci = true
packaging = true
releases = true
site = false
deployment = false
planning = true
collaboration = true

[[component]]
name = "source"
kind = "repository"
target = "https://github.com/OWNER/REPO"

[[component]]
name = "homebrew"
kind = "distribution"
target = "https://github.com/OWNER/homebrew-tap"

[[healthcheck]]
name = "documentation"
url = "https://example.com/"
expect_status = 200
```

The TUI infers a profile when this file is absent and labels that profile as inferred. Declared expectations control applicability in the effectiveness score. `jerk --json` emits the versioned local snapshot that future AI analysis can consume without reading source code.


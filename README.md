# jerk

[![CI](https://github.com/oddurs/jerk/actions/workflows/ci.yml/badge.svg)](https://github.com/oddurs/jerk/actions/workflows/ci.yml)

`jerk` is a terminal dashboard for the question every projects directory eventually asks: **which of these things is actually alive, complete, and effective?**

It scans local Git repositories immediately, then enriches the selected project and portfolio with GitHub and deployment data in the background. It understands the conventions already present in these repositories, including [Cairn](https://github.com/oddurs/cairn) roadmaps.

## What it shows

- Git momentum: recent and total commits, contributors, branches, tags, churn, worktree state, upstream drift, and a 12-week activity graph.
- Project readiness: README, license, docs, changelog, tests, CI, packaging, release automation, and dependency automation.
- GitHub effectiveness: open, merged, and closed pull requests; issues; stars; forks; 14-day unique views and clones; release downloads; latest release; and workflow/default-branch checks.
- Website delivery: source detection, deployment URL discovery, HTTP health, status code, and latency.
- Cairn overview: roadmap completion, open/active/blocked/done work, milestones, current work, and what is next.
- Portfolio overview: local versus cloud inventory, lifecycle mix, repository creation velocity, standards coverage, public-repository hygiene, an actionable stale-repository review queue, and total work in progress.
- A transparent 100-point effectiveness score built from those signals.
- Lifecycle-aware project profiles from an optional committed `.jerk.toml`.

The GitHub layer uses the authenticated `gh` CLI. Everything local still works without it. Deployment health uses `curl`, and failures stay informational rather than preventing the dashboard from opening.

## Install and run

jerk supports macOS, Linux, and Windows. It requires Rust 1.88 or newer and Git. The authenticated GitHub layer is optional and uses [`gh`](https://cli.github.com/); deployment probes are optional and use `curl`. If either tool is missing, the local dashboard still works.

Install directly from GitHub:

```sh
cargo install --locked --git https://github.com/oddurs/jerk
jerk ~/Code
```

Or build this checkout:

```sh
cargo install --path .
jerk ~/Code
```

On Windows PowerShell, for example:

```powershell
cargo install --locked --git https://github.com/oddurs/jerk
jerk "$HOME\Code"
```

Tagged releases publish native archives for Intel Linux, Intel and Apple Silicon macOS, and 64-bit Windows.

Pass one repository to inspect it, or a directory to scan its immediate child repositories:

```sh
jerk .
jerk ~/Code
jerk ~/Code --plain
jerk ~/Code --json
```

Inside the TUI, use `j`/`k` or the arrow keys to select projects, `g`/`G` to jump to the first or last project, `h`/`l` or left/right to change views, `/` to filter projects, `s` to cycle between name, attention, and recency ordering, `r` to rescan locally, `R` to refresh remote data, and `q` to leave. Press `?` for the in-app keyboard guide. Filters match names, descriptions, and GitHub slugs. Startup and rescans happen in the background, so the interface remains responsive while Git is working.

## Terminal-native colour

The default theme never paints a background and never embeds RGB colours. It uses `Color::Reset` plus the terminal's ANSI roles, so Ghostty, SSH sessions, light themes, and live palette changes remain authoritative. Selection uses reverse video for the same reason. Set `NO_COLOR=1` for a monochrome interface.

## How the score works

The score is deliberately legible rather than “AI magic”:

| Area | Points | Signals |
|---|---:|---|
| Project shape | 15 | repository, remote, description, history |
| Documentation | 15 | README, license, docs, contributor/change records |
| Momentum | 20 | activity, recency, contributors, tags, clean state |
| Delivery | 20 | CI, tests, package, releases, site and health |
| Planning | 20 | Cairn presence, completion, active and unblocked work |
| Community | 10 | PRs, branch checks, releases, manageable review queue |

Remote-only points appear after GitHub finishes loading. The UI shows individual signals beside the score so a number never hides the reason behind it.

The score is profile-aware: evidence marked not applicable does not count as failure, and intentionally complete or paused projects are not penalized for inactivity. The accompanying evidence percentage shows how much of the assessment is based on declared intent and currently available remote data. See [the schema](docs/schema.md) for the full model and `.jerk.toml` format.

## AI analysis later

The roadmap includes an OpenRouter-backed summary, but this first release intentionally collects deterministic facts only. The future integration will be opt-in, send a bounded project snapshot rather than source code, and read the key from the environment or OS credential storage—never from a committed config file.

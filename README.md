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
- Opt-in AI insight: a concise effectiveness brief, separated observations and risks, and one high-leverage next action from a previewable metrics-only snapshot.
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

### Personal dashboard settings

`jerk` keeps personal preferences separate from repository `.jerk.toml` files. Run this once to create an editable template in the platform’s normal config directory:

```sh
jerk --init-config
```

The template lets you set the identity shown in the header, the default scan root, the first view and project ordering, a pinned GitHub owner for portfolio stats, and an ANSI accent role that still inherits the active Ghostty or parent-shell palette. Use `--config PATH` or `JERK_CONFIG` when you want a different profile, such as a work dashboard:

```toml
[dashboard]
brand = "ODDURS"
workspace = "studio"
root = "~/Code"
default_view = "portfolio"
default_sort = "attention"
github_owner = "your-handle"
accent = "cyan"
```

A command-line directory always takes precedence over the configured root. Missing or invalid personal config never prevents local scanning; the warning appears in the dashboard footer.

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
jerk . --ai-payload
```

Inside the TUI, use `j`/`k` or the arrow keys to select projects, `g`/`G` to jump to the first or last project, `h`/`l` or left/right to change views, `/` to filter projects, `s` to cycle between name, attention, and recency ordering, `r` to rescan locally, `R` to refresh remote data, and `q` to leave. Press `?` for the in-app guide. Filters match names, descriptions, and GitHub slugs. Startup, rescans, and AI requests happen in the background, so the interface remains responsive.

Mouse input works alongside the keyboard: click a tab, project, or footer command; use the wheel to move through projects; and click a modal to dismiss it. Mouse capture is released whenever `jerk` exits, including error paths.

The interface is intentionally dense: one-line navigation and project rows, compact evidence cards, and lightweight section rules keep a large portfolio scannable without turning the terminal into a wall of boxes.

## Terminal-native colour

The default theme never paints a background and never embeds RGB colours. It uses `Color::Reset` plus the terminal's ANSI roles, so Ghostty, SSH sessions, light themes, and live palette changes remain authoritative. Selection and progress indicators use foreground-only emphasis, avoiding reverse-video blocks that fight dark or tinted backgrounds. Set `NO_COLOR=1` for a monochrome interface.

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

## Opt-in AI insight

The `6:Insight` view can ask [OpenRouter](https://openrouter.ai/) to interpret the collected evidence. It never runs automatically: press `p` first to inspect the complete metrics snapshot, then `a` to send it. Results are cached in memory until those metrics change. Provider failures never interrupt local scanning or the other views.

The snapshot contains numeric Git, documentation, delivery, GitHub, Cairn, and scoring signals. It excludes local paths, repository URLs and slugs, descriptions, commit messages, source code, file contents, and Cairn item titles. `jerk . --ai-payload` prints the exact snapshot without making a network request.

Set the key for the current shell on every supported operating system:

```sh
export OPENROUTER_API_KEY='...'
```

```powershell
$env:OPENROUTER_API_KEY = '...'
```

On macOS, `jerk` also checks the login Keychain for service `dev.jerk.openrouter`, so the key does not need to live in a shell profile. Environment variables take precedence. The default model is the free router, `openrouter/free`; set `OPENROUTER_MODEL` to any compatible model slug to override it.

# Contributing

Contributions are welcome on macOS, Linux, and Windows. jerk's local scanner must remain useful without GitHub authentication or network access, and its interface must keep the terminal's own foreground, background, and ANSI palette authoritative.

## Development setup

Install Rust 1.88 or newer and Git, then run:

```sh
cargo build --locked
cargo test --all-targets --locked
cargo clippy --all-targets --locked -- -D warnings
cargo fmt --check
```

PowerShell accepts the same Cargo commands. GitHub enrichment additionally needs the optional GitHub CLI (`gh`); live deployment probes additionally need `curl`.

When changing the interface, check at least 80×24 and one wider terminal. Also check `NO_COLOR=1` and avoid hard-coded RGB colors or backgrounds.

## Pull requests

Keep changes focused, add tests for parsing and scoring behavior, and update `docs/schema.md` when the serialized contract changes. Do not include API keys, private repository data, or captured `--json` output that has not been reviewed.

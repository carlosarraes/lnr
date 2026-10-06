# lnr

Requires Rust 1.94 or newer.

A Rust CLI for Linear workflows used by agents and humans. Read issue context,
search work, update issues, add comments, manage relations, and inspect projects
without writing GraphQL.

```sh
cargo install --path . --locked
lnr auth import-linear --workspace YOUR_WORKSPACE
lnr issue list --assignee me --state started --json
lnr issue context ENG-123 --json
lnr project list --mine --json
```

Alternatively, provide `LINEAR_API_KEY` through your shell's secret management.
Do not combine that variable with `--workspace`. Import reads the existing
schpet Linear CLI credentials and stores a copy in the OS keyring under `lnr`.
Keys never appear in command output or configuration files.

The CLI is the first phase. Bare `lnr` currently shows help. The planned Ratatui
interface will use the same services for an overview, project cards, progress,
and involvement filters.

## Agent usage

Piped output defaults to JSON. Use `--json` explicitly when invoking from an
agent's terminal. `lnr schema` describes commands, flags, output shapes, and exit
codes without authentication. Nothing prompts for input.

```sh
lnr issue list --team ENG --search timeout --limit 20 --json
lnr issue context ENG-123 --limit 10 --json
lnr issue update ENG-123 --state 'In Progress' --assignee me --json
lnr issue comment add ENG-123 --body-file report.md --json
lnr issue relation add ENG-123 --blocks ENG-456 --json
lnr project issues PROJECT_UUID --assignee me --all --json
```

Use file inputs for long text; `--body-file -` and `--description-file -` read
stdin without changing line breaks. Lists expose cursors; `--all` explicitly
fetches every page. Context has independent cursors for comments, children,
outgoing relations, and incoming relations.

See [CLI contracts and examples](docs/cli.md) for authentication, field clearing,
error recovery, and raw API access.

## Development

The justfile follows the build and sync commands used in our other Rust CLIs:

```sh
just build           # Build and install into ~/.local/bin
just sync            # Also install on mac, building there when OS/arch differ
just sync HOST       # Install on another SSH host
just check           # Formatting, Clippy, and tests
just release 0.1.0   # From clean, pushed main: verify, tag, push, publish via CI
```

Sync requires SSH, rsync, and Rust on the destination when building there.
Release requires Python 3 and the `upstream` Git remote. It updates Cargo's
package version and lockfile, then pushes the release commit and tag atomically.
Failed checks leave local changes available to inspect. Release archives contain
binaries for Linux x86_64 and macOS Apple Silicon and Intel.

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
python3 scripts/measure.py
```

Integration tests run the real binary against local HTTP fixtures. They never
modify a Linear workspace. Built-in GraphQL documents compile against a checked-in
schema. `src/service` contains reusable asynchronous workflows; `src/cli` owns
argument parsing and presentation.

The reference schema's source and attribution are in [graphql/README.md](graphql/README.md).

See [verification results and known limits](docs/verification.md) for the Linux/macOS
checks, independent review, and repeatable measurements.

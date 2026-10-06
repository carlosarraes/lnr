# lnr CLI and future TUI

Status: approved by the user on 2026-10-06. Implementation planning follows.

## Intent

Build a Rust CLI named `lnr` that lets agents perform common Linear workflows
without constructing GraphQL or parsing terminal output. After the CLI works,
add a Ratatui interface launched by bare `lnr`, with an overview, project cards,
progress, and filters for projects and the authenticated user's involvement.
The CLI is the first deliverable. Visual exploration of the TUI comes later.

## Evidence collected on 2026-10-06

The Mac's `/opt/homebrew/bin/linear` points to Homebrew Linear 2.0.0.
Its installation receipt identifies `schpet/tap`.
The reference repository is https://github.com/schpet/linear-cli.
It is cloned at `/home/carraes/github/schpet-linear-cli`, revision `9590d2bd`.
The existing `/home/carraes/github/linear-cli` is a different repository,
`Finesssee/linear-cli`, and was left intact.

The schpet checkout is already Rust, version 3.0.0-alpha.2, with typed GraphQL,
reference resolution, pagination, JSON modes, and relation commands. Its core
client and command modules are private to the CLI library; command execution
uses a CLI context containing output, terminal state, and a runtime. It is useful
reference material, but is not currently a ready-made shared TUI service library.
This inspection does not establish that lnr will be faster than upstream.

A remote scan read 7,195 JSONL files under `.claude/projects`, `.codex/sessions`,
and `.codex/archived_sessions`. It inspected Claude tool-use inputs and Codex
function/custom-tool inputs, deduplicating available call IDs. There were 20,470
matching tool-call inputs. Raw conversations and credentials were not copied.

| Command text | Occurrences |
| --- | ---: |
| linear issue view | 7,919 |
| linear api | 2,930 |
| linear issue update | 1,866 |
| linear issue create | 1,109 |
| linear issue query | 632 |
| linear issue comment | 590 |
| linear issue relation | 367 |
| Direct API URL in input | 287 |
| linear issue list | 263 |

Within inputs containing `linear api` or the Linear API URL, field/operation
matches included `issue` 1,665, `issues` 1,545, `comments` 805, `project` 620,
`commentCreate` 233, `issueUpdate` 202, `issueRelationCreate` 87, and
`issueCreate` 67. These categories overlap and include nested fields.

These are lexical usage signals, not successful request counts. Shell commands
may contain examples, help requests, or several operations. Session copies with
different call IDs can remain duplicated. No claim about the exact percentage
of workflows requiring the API follows from these counts.

## Options

1. Build a focused standalone Rust core and CLI, using upstream as a reference.
   Recommended: command contracts and shared UI services can fit this project.
   Cost: maintain our own API operations and credential integration.
2. Fork the current upstream Rust rewrite and extract reusable services.
   Gains broad coverage immediately, but couples this project's scope to a large
   existing command implementation and future merge work.
3. Wrap the installed executable. Fastest initial prototype, but adds subprocess
   parsing and version dependencies and does not establish the desired core.

## Architecture

Start with one package containing a library and thin binary. Keep modules for
configuration/authentication, API transport and operations, entity resolution,
workflow services, and CLI formatting. Add a TUI adapter after CLI acceptance.
Split crates only when there is a concrete dependency or build benefit.

Use clap for commands, Tokio and reqwest for asynchronous requests, serde for
output, and schema-checked GraphQL operations for built-in commands. The shared
services accept typed arguments and return typed data; they never print, prompt,
read terminal state, or spawn the CLI. Both future interfaces call these services.

One configured HTTP client per invocation or TUI session reuses connections.
Resolve IDs directly; resolve names within explicit workspace/team/project scope.
Ambiguous names return candidates instead of selecting the first match.
Memoize resolutions during an invocation. Defer persistent caching until measured
request counts justify its freshness and invalidation complexity.

## First CLI release

Proposed command forms:

```sh
lnr auth status
lnr issue list --assignee me --project PROJECT --state started --json
lnr issue list --search 'timeout' --team ENG --json
lnr issue view ENG-123 --json
lnr issue context ENG-123 --json
lnr issue create --team ENG --title 'Fix timeout' --description-file body.md
lnr issue update ENG-123 --state 'In Progress' --assignee me
lnr issue comment list ENG-123 --json
lnr issue comment add ENG-123 --body-file comment.md
lnr issue relation add ENG-123 --blocks ENG-456
lnr issue relation list ENG-123 --json
lnr issue relation remove RELATION_ID
lnr project list --json
lnr project view PROJECT --json
lnr project issues PROJECT --assignee me --json
lnr team list --json
lnr user list --json
lnr state list --team ENG --json
lnr label list --team ENG --json
lnr api --query-file query.graphql --variables-file variables.json
```

`issue context` returns the issue description, parent, children, relations,
comments, and project summary. It exposes completeness and pagination metadata
for each collection. Bounded defaults prevent a large project from exhausting
agent context. Additional pages are explicit. It does not recursively fetch the
entire dependency graph or every issue in the project.

Create/update support project, parent, labels, priority, and description inputs.
Update distinguishes omitted values from explicit clearing. Body-file input
accepts `-` for stdin. Ordinary writes do not require interactive prompts.
Project writes, documents, initiatives, uploads, git branch management, and bulk
mutation orchestration are outside the first release unless subsequent evidence
changes their priority. Raw API access remains an escape hatch.

## Agent contract

- Explicit `--json` always produces machine-readable output. Piped output defaults
  to JSON; terminal output defaults to a readable summary. `--output` overrides.
- JSON success uses a versioned envelope with `data` and `meta`. Collections
  include cursors and `has_more`. Context includes per-collection completeness.
- Errors use a stable code, message, retryability, and structured details.
  Diagnostics go to stderr. JSON failures emit one error envelope on stdout.
- Exit codes distinguish success, invalid input, authentication, missing/ambiguous
  references, rate limits, network failures, and API failures. Freeze the numeric
  mapping in the implementation plan and test it as a public contract.
- Never prompt in agent/piped use. Missing required input returns actionable help.
- Lists default to a bounded page. `--limit`, `--after`, and explicit `--all`
  control retrieval. No silent truncation and no automatic workspace-wide scan.
- Provide `lnr schema` for machine-readable command/input/output discovery.
- Initially bare `lnr` shows help. In the TUI phase it launches Ratatui only in an
  interactive terminal; noninteractive use continues to return help.

## Authentication and failures

Support `LINEAR_API_KEY` and explicitly selected workspace credentials. Add an
explicit import path for the installed CLI's credentials after inspecting its
storage format without exposing values. Do not silently fall back to another
workspace. Redact credentials from logs and errors.

Validate GraphQL errors even on HTTP 200 and check mutation success fields.
Expose partial read results as incomplete with a nonzero exit status. Use request
deadlines and bounded read retries for transient failures, respecting rate-limit
responses. Never automatically retry a mutation with an uncertain outcome.
Validate raw API operation type before applying any read-only retry behavior.

## Optimizations and verification

Filter on the server, select only required fields, and batch related context
within GraphQL complexity limits. Paginate nested collections independently.
Bound concurrency for independent requests. Measure request counts, response
bytes, cold startup, and context retrieval time before claiming improvements.

Test CLI invocations against a local HTTP fixture server, covering filters,
cursor traversal, ambiguous names, authentication selection, GraphQL partial
errors, rate limits, timeouts, and uncertain mutation outcomes. Test that context
retrieval reports incomplete nested collections and avoids N+1 request growth.
Verify stdout is valid JSON with no prompts or progress text under redirection.
Build on Linux and macOS. Live smoke testing should use read-only operations;
fixture tests cover writes without altering real workspaces.

## Later TUI

Ratatui calls the same workflow services. Provide an overview and selectable
project cards/list showing available progress, status, lead, and target date.
Allow all-project browsing and filters for project, team, status, and involvement.
The proposed default for “mine” is the union of projects the viewer leads,
belongs to, or has assigned issues in, pending the user's preference.
Keep each involvement criterion independently selectable.

Use cancellable background reads, explicit loading/error states, visible last
refresh time, and bounded refreshes. Define progress from documented API values
or label any computed counts precisely. Empty and inaccessible data must not
look like zero progress. Explore layouts visually after CLI acceptance.

## Sources

- Upstream: https://github.com/schpet/linear-cli
- API/auth/errors: https://linear.app/developers/graphql
- Pagination: https://linear.app/developers/pagination
- Rate limits: https://linear.app/developers/rate-limiting

Linear documents server-side filtering, cursor pagination, and GraphQL errors
within HTTP 200 responses. These inform the transport and completeness contract.

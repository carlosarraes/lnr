# CLI verification

Validated on 2026-10-06. The CLI phase is implemented on `feat/agent-cli`.
The Ratatui phase has not started.

## Results

| Check | Linux x86_64 | macOS arm64 |
| --- | --- | --- |
| Rust | 1.98.1 | 1.94.0 |
| `cargo test --locked` | 57 passed | 56 passed |
| `cargo clippy --locked --all-targets -- -D warnings` | Passed | Passed |
| `cargo build --release --locked` | Passed | Passed |
| `cargo fmt --check` | Passed | Formatting shared from Linux |

The additional Linux test verifies upstream Secret Service credential lookup
attributes using an isolated command fixture. The tests execute real subprocesses
against local HTTP servers, including partial responses, cursor failures,
ambiguous names, JSON contracts, rate limits, interrupted responses, deadlines,
and non-retried writes. Emitted issue-list, project-list, and context envelopes
validate against the schemas advertised by `lnr schema`.

Read-only live checks on the Mac passed after the review fixes:

- Authentication and workspace identity.
- Assigned issue listing, bounded to one issue.
- Issue context, bounded to two items per nested collection.
- Projects filtered by involvement, bounded to three projects.
- Team listing, bounded to two teams.

Credentials stayed on the Mac. Validation did not create or update any Linear
entities, import keys into lnr's keyring, or change the existing Linear CLI.
The Mac validation checkout is `~/github/lnr-validation`.

## Measurements

Release build, Linux, `python3 scripts/measure.py`:

| Measurement | Result |
| --- | ---: |
| Fresh-process `--help`, median of 10 launches | 1.642 ms |
| Context against local fixture, including startup | 3.461 ms |
| Context HTTP requests | 1 |
| GraphQL request body | 1,810 bytes |
| Fixture response | 457 bytes |
| CLI output | 579 bytes |

These measure local process and fixture behavior, not live API latency. No
comparison with upstream was performed. The script is repeatable; timings vary
with machine load and caches.

## Independent review and fixes

A separate reviewer inspected the completed branch and reproduced bugs with real
CLI invocations. No Critical findings were reported. All six Important findings
were fixed and covered by regression tests:

1. Preserve prior and current partial nodes when pagination or GraphQL fails.
2. Read upstream Linux credentials from Secret Service using its `service` and
   `account` attributes; store lnr keys in persistent Secret Service.
3. Include workspace-wide labels alongside the selected team's labels, retaining
   ambiguity detection.
4. Return typed, consistent snake_case issue/project/comment/relation/context
   DTOs and preserve context metadata on partial failures.
5. Retry interrupted query responses within the deadline while sending mutations
   once.
6. Describe per-command output schemas, pagination, argument conflicts, required
   choices, and numeric constraints in discovery.

The review regression suite went from 0/9 passing to 9/9. The interrupted-query
regression failed before its fix; the interrupted-mutation check confirms no
retry. Additional checks cover authenticated identity output, total deadlines,
and JSON Schema validation.

The Mac suite exposed a fixture portability issue. macOS accepted sockets
inherited nonblocking mode, unlike Linux; explicitly selecting blocking mode
fixed truncated reads/writes. A separate regression verifies that a connection
closed before HTTP headers does not stop the fixture server.

## Decisions and limits

- Work remained in the dedicated project checkout on a feature branch. A second
  worktree added no isolation from existing product code because the repository
  initially held only the approved documents. Concurrent unrelated edits would
  require coordination.
- GraphQL operations use graphql_client rather than the proposed cynic. Both
  validate against the checked-in schema; generated types add compile overhead.
- Transport and initial read workflows shared a commit because their checked
  operations and service boundary were developed together. This makes history
  less granular. Most tests followed red/green execution; the initial project
  test invocation hit an unrelated compile error before a behavioral failure
  could be observed. Do not treat that task as a demonstrated red/green cycle.
- State UUID filtering was raised from Minor to Important because IDs are the
  unambiguous path used by agents. A regression now proves UUID filters work
  without team lookup. Without the fix, valid agent commands would fail.
- Native keyring writes and live Linear mutations were deliberately not exercised.
  Credential-store permissions and real workspace write acceptance therefore
  remain outside the live smoke evidence. Writes were tested against fixtures.
- No upstream speed advantage is claimed because an equivalent comparative
  benchmark was not run.
- The plan's proposed single acceptance-test file became focused subprocess
  integration suites. Schema validation and deadline checks have dedicated suites.
  The original planning checklist is retained as a historical reference; this
  document records the checks actually performed.

One Minor finding remains deferred: the cross-team `--state duplicate` shortcut
is unsupported. Use a state UUID or raw GraphQL for that type.

## Next phase

Design the Ratatui layout visually, then implement bare interactive `lnr` as the
TUI entry point. Reuse the typed asynchronous services for overview, project cards,
progress, and filters. Noninteractive bare invocation should keep showing help.

## Agent feedback and OAuth follow-up, 2026-10-06

Addressed all six items in the first-use feedback. Issue view fetches all comment
pages by default, sorts oldest first, and supports `--no-comments`. Failed later
pages retain the issue and available discussion with incomplete metadata.
Workspace selection supports a single-workspace fallback and `auth default`.
Plain `auth status` is an offline inventory, including when `LINEAR_API_KEY` is
set; `--check` explicitly validates the active credential. Help and schema now
explain input semantics. Build/sync install a `linear` symlink alongside `lnr`.

Added OAuth authorization-code login with S256 PKCE, state validation, bounded
loopback callback handling, Bearer authorization, and rotating refresh tokens in
the existing OS keyring. A cross-process file lock serializes credential changes
and refresh. The client ID and callback port are retained from the legacy Zig
implementation. Protocol details were checked against
[Linear's OAuth documentation](https://linear.app/developers/oauth-2-0-authentication).

Verification:

- Linux: formatting, Clippy with warnings denied, 75 tests, release build.
- macOS: Clippy with warnings denied, 74 tests, release build.
- OAuth fixtures cover the RFC PKCE vector, invalid callbacks, callback HTTP
  handling, refresh rotation/failure, token validation, Bearer headers, redaction,
  and timeout without credential changes.
- Comment fixtures cover populated multi-page results, ordering, opt-out, and
  partial failures. Live read-only Mac checks verified issue view and opt-out;
  the six sampled assigned issues had no comments.
- Minimal noninteractive zsh environments found both `lnr` and `linear` on both
  hosts. Mac inventory reported `mondrio` as the default.
- Independent review caught environment credentials bypassing offline inventory;
  fixed with populated/empty environment regression cases.

The user confirmed unlocking the macOS login keychain resolved the original
import error. The native backend remains in use, with more actionable errors.
Live browser consent and OAuth keyring persistence/refresh have not been exercised
against the user's account; the user must complete browser consent. No workspace
mutations or public releases were made during verification.

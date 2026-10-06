# CLI contract

## Output

`--json` or `--output json` emits one JSON value, with schema version 1, `data`,
and `meta`. Errors also contain `error.code`, `message`, `retryable`, and
`details`. Partial failures retain available data and set `meta.complete=false`.
Do not treat HTTP 200 as proof of success; the CLI checks GraphQL errors and
mutation success values. Use process exit status before consuming the result.

`--output text` explicitly selects readable output. Help/version and bare `lnr`
are informational text with exit 0. OAuth login prints a browser authorization URL
on stderr and waits for approval; other commands do not prompt.

| Exit | Meaning |
| --- | --- |
| 0 | Success |
| 2 | Invalid input |
| 3 | Authentication |
| 4 | Missing entity |
| 5 | Ambiguous name; use a candidate UUID |
| 6 | Rate limited |
| 7 | Network failure or read timeout |
| 8 | API failure, partial result, or uncertain mutation |
| 9 | Local I/O or configuration failure |

## Credentials

Use browser OAuth, set `LINEAR_API_KEY`, or import an existing Linear CLI workspace:

```sh
lnr auth login
lnr auth import-linear --workspace YOUR_WORKSPACE
lnr auth status
lnr auth default YOUR_WORKSPACE
lnr auth status --workspace YOUR_WORKSPACE
```

Import validates the key's organization before storing it. A different existing
lnr key requires `--replace`. Configuration follows `XDG_CONFIG_HOME`, falling
back to `~/.config`. macOS stores secrets in `lnr/credentials.json` with mode
`0600` in a `0700` directory, independent of TTY and Keychain state. Linux defaults
to persistent Secret Service. `LNR_CREDENTIAL_STORE=file|keyring` overrides the
backend for import, login, reads, and refresh. `auth status` reports the active store.
Existing Mac installs must re-run `auth import-linear --workspace SLUG` once to
populate the file store. Writes are atomic and serialized across agent processes.
Linux imports of
upstream keyring credentials require `secret-tool`, matching upstream storage. On headless systems without an
available keyring, provide `LINEAR_API_KEY`. An explicit `--workspace` conflicts
with the environment key, preventing accidental cross-workspace actions.

OAuth login uses PKCE and a loopback callback, requests `read,write` as the user,
and stores rotating tokens in the selected credential store. It refreshes within five minutes of
expiry and serializes credential updates across processes. `--no-browser` prints
the URL without launching a browser; SSH users can forward the callback port.
Use `--replace` to replace an imported API key. `--client-id` / `LNR_CLIENT_ID`
and `--port` configure your own registered OAuth app. Default client ID and
callback port 8484 are retained from the original Zig lnr implementation.
The browser consent flow still needs to be completed by the user.

`auth status` without an explicit workspace returns a local
inventory (`default_workspace`, `workspaces`, `import_default`,
`importable_workspaces`, `credential_store`). With `--workspace`, it verifies that credential against
Linear. Use `auth status --check` to verify the active credential, including
`LINEAR_API_KEY` when set. Plain inventory does not read or validate that variable. The first
import/login becomes the default, and a sole configured workspace is selected
even if older configuration has no default. `auth default SLUG` changes it.

## Read workflows

- `issue list`: filters for team, assignee, project, state, and searchable content.
- `issue view REF`: issue details and all comments, oldest first and newest last.
  `--no-comments` omits discussion. Issue identifiers such as `ENG-123` work directly.
  Comment pagination is automatic; `meta.collections.comments` reports completeness.
  Partial failures preserve fetched comments and issue details with a nonzero exit.
- `issue context REF`: details, parent, project summary, children, comments, and relations.
- `issue comment list REF`: paged comments.
- `issue relation list REF`: separate outgoing and incoming relations.
- `project list`: all accessible projects by default; filter by team, status name, or involvement.
- `project view REF`: project progress and details.
- `project issues REF`: the same issue filters scoped to one project.
- `team list`, `user list`, `state list --team REF`, `label list --team REF`: discover identifiers.

Names resolve by exact matching within the applicable scope. Duplicates fail
with candidates. For issue lists, state types such as `started` work across
teams; state names such as `In Review` require `--team`. Updates resolve a state
name using the issue's team. UUIDs avoid name resolution.

`project list --mine` includes projects you lead, belong to, or have assigned
issues in. `--involvement lead,member` chooses specific criteria. The OR filter
runs on the server, so projects matching several criteria are not duplicated.
Progress is Linear's API `progress` value, a fraction from 0 to 1. Missing values
remain unavailable rather than becoming zero.

## Pagination

Lists default to 50 items; `--limit` accepts 1 through 250. Continue with
`--after CURSOR`, or explicitly use `--all`. The latter conflicts with `--limit`.
`meta.page.has_more` and `end_cursor` make bounded results visible. A bounded
page with more results is successful; a failed later page is an error and
retains previously fetched items.

Context defaults to 20 items per collection and accepts a limit up to 100.
Use `--comments-after`, `--children-after`, `--relations-after`, and
`--inverse-relations-after` independently. `meta.collections` contains each
collection's completeness and next cursor. Context never recursively expands
relations or fetches every issue in the project.

Relation lists use `--after` for outgoing and `--inverse-after` for incoming
relations. `--all` finishes both directions independently.

## Writes

```sh
lnr issue create --team ENG --title 'Fix timeout' --description-file issue.md
lnr issue update ENG-123 --title 'Handle slow responses' --priority 2
lnr issue update ENG-123 --clear-project --unassign
lnr issue update ENG-123 --labels Bug,Backend
lnr issue comment add ENG-123 --body-file comment.md
lnr issue relation add ENG-123 --blocks ENG-456
lnr issue relation add ENG-123 --related ENG-789
lnr issue relation add ENG-123 --duplicate-of ENG-321
lnr issue relation remove RELATION_UUID
```

Omitted fields stay unchanged. Clear nullable values explicitly with
`--clear-description`, `--clear-project`, `--clear-parent`, `--clear-labels`, or
`--unassign`. Set/clear conflicts fail locally. Labels replace the issue's label
set. Priority accepts 0 through 4. Empty updates and empty comments are errors.
`--blocks` means the source issue blocks the target.

Writes are never automatically retried. `uncertain_mutation` means the server
may have applied the operation before the response was lost. Read the current
issue/comments/relations before retrying to avoid duplicate writes.

## Raw GraphQL

```sh
lnr api --query-file request.graphql --variables-file variables.json
lnr api --query-file request.graphql --operation-name ReadIssues
```

Select an operation when a document has several. Variables must be a JSON
object. Only one input may use stdin. Subscriptions are unsupported. Built-in output uses snake_case fields consistently across lists, views, and
mutations. Raw output
preserves GraphQL field names and is wrapped in the standard envelope.

## Scope and limits

This release does not provide project mutations, documents, initiatives,
uploads, bulk writes, branch management, or a TUI. Use raw GraphQL for API
operations without dedicated commands. There is no persistent cache or daemon.
A request and its retries share a 30-second budget; replies are capped at 16 MiB.
`LNR_API_URL` can select an alternate HTTPS endpoint for development, or HTTP on
loopback for fixture tests. It receives the selected credential, so set it only
to an endpoint you control.

The cross-team state-type shortcut currently excludes `duplicate`; use its state
UUID or raw GraphQL. UUID filters do not require a team lookup.

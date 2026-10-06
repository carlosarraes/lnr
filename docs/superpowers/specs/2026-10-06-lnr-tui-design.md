# lnr project overview TUI

Status: visual direction approved by Carlos on 2026-10-06. This document records
the approved layout and the proposed behavior for the first implementation.

## Purpose

Running `lnr` in an interactive terminal opens an overview of accessible Linear
projects. Carlos can inspect progress across all projects, narrow the view to
projects involving him, and open a project's issues. Existing CLI commands and
JSON contracts remain usable by agents.

The first TUI is read-only. Issue editing remains available through the CLI.

## Approved screen

The approved visual reference is
`.superpowers/brainstorm/963410-1791313054/content/project-overview-v7-quiet.html`.
It uses sample data and is a disposable design reference. Ship a Rust/Ratatui
terminal application, not the browser mockup.

- One quiet header: application name, workspace, viewer.
- Scope: All / Mine, with All initially selected and a loaded-project count.
- One spacious list: project name, status, progress bar and percentage, target date.
- A muted highlight spans the selected row. No surrounding card borders.
- A small area below a single divider shows the selected project's name, team,
  lead, viewer involvement, and description. Avoid repeating the list's metadata.
- One short footer: move, open, search, mine, filter, help. Additional bindings
  belong in the help overlay.
- Filters are hidden until requested. Applied filters and search appear in one
  compact summary line only when active.

No cards/list switch is needed in the first implementation. Preserve the approved
spacing on roomy terminals; reduce row spacing on short terminals. At narrow
widths, hide target date, then shorten the progress bar, then hide the status
column. Keep project names and progress readable. Below 60 columns or 18 rows,
show a resize message while retaining quit/help handling and application state.
Resizing preserves selection and filters. Descriptions wrap and can scroll.

My projects means the union of projects the viewer leads, belongs to, or has an
assigned issue in. Filters combine team, project status, and involvement. Values
within each group use OR; groups combine with AND. Mine additionally intersects
with these filters. Project statuses and teams come from workspace data.

The filter overlay displays one group at a time. Tab switches team/status/role;
arrows move the cursor; Space toggles a choice; Enter applies; Escape cancels.
All/Any clears the group's selections. Edits remain a draft until applied.

## Implementation boundary

Keep the existing CLI/service boundary. Add a `src/tui/` module with separate
application state, rendering, data loading, and terminal lifecycle responsibilities.
The renderer consumes state and does no networking. Input updates state and emits
read requests. Background results return through a channel and are accepted only
for the current request generation.

Extend existing project service queries and models for the authoritative fields
and server-side search/multiple filter values this screen needs. Preserve existing
CLI flag semantics and JSON contracts; additions must remain compatible. Keep
version 0.0.1. Use Ratatui and Crossterm compatible with the project's Rust 1.94
minimum, and retain Linux/macOS support.

## Navigation

`j`/`k` and arrows move selection. `m` switches All/My projects. `/` opens
a search line; Enter applies the query and Escape restores it. `f` opens the
filter overlay. Enter opens
project issues; Escape returns or dismisses a picker. `r` refreshes. `?` shows
help. `q` exits from the overview; Ctrl-C always restores the terminal and exits.

Project issues show identifier, title, status, assignee, and priority. Opening an
issue shows its description and discussion through the existing service layer.
Long content scrolls with PageUp/PageDown. Pagination is visible and explicit:
`n` loads the next page, appending results and retaining selection. Show a loaded
count and “more available” until the last page; never imply a loaded count is the
workspace total. Escape returns issue → project issues → overview.

## Data and failures

The Ratatui adapter calls Rust services directly. It never invokes the CLI as a
subprocess and never parses formatted terminal output. Reuse the authenticated
HTTP client, credential-store selection, and existing error contracts.

Reads run outside the input/render loop. Changing a filter cancels superseded
work; generation identifiers prevent stale results replacing the current view.
Initial reads are paged, with a visible action to fetch the next page. Every
project remains accessible; there is no silent result truncation. Search should
apply server-side to the result set, not only to rows already loaded.

The UI distinguishes initial loading, refresh in progress, empty results,
authentication failure, and partial data. Refresh failures preserve the last
successful result and show its age. No automatic polling is required initially;
manual refresh avoids repeatedly loading a large workspace in the background.

Progress uses Linear's reported project progress. Missing progress is shown as
unavailable, never as zero. Do not invent health, issue counts, or involvement
badges from fields absent in the API response. Add only the query fields needed
to display authoritative values.

Plain `lnr` with redirected input/output continues to show help. An explicit
`--json` does not start an interactive screen. Missing credentials produce an
actionable authentication message without trapping the user in a broken TUI.
Raw mode and the alternate screen must be restored on normal exit, error, and
panic. Untrusted issue text must not inject terminal control sequences.

## Verification

Use Ratatui's test backend for layout and navigation checks, including empty
results, missing progress, a narrow terminal, Unicode, and long content. Fixture
HTTP tests cover filters, pagination, failures, and stale response handling.
PTY tests cover interactive launch, input, and terminal restoration; pipe tests
protect existing noninteractive behavior. Run the existing CLI suite on Linux
and macOS. Finish with read-only live smoke tests on the Mac.

## Acceptance

Plain interactive `lnr` opens the approved list with real workspace data. Carlos
can switch All/Mine, combine filters, search projects, load later pages, and inspect
project issues and discussion without leaving the terminal. Inputs remain responsive
during requests. Existing agent CLI invocations retain their output and exit behavior.

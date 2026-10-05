# 0004: Workspace tabs and read-only consoles

Status: accepted and implemented for the experimental MySQL/MariaDB slice; final owner-run, current-main CI and native validation remain gates.

## Context

Daily database work requires multiple table views and a scoped SQL console without losing filters, scroll, requests or results when switching. The user wants DataGrip-like database workflows, not DataGrip visual styling, Files/Git UI or a general editor. Arbitrary SQL cannot safely be treated as read-only using prefixes or semicolon splitting. A console also needs explicit transaction ownership, cancellation and sensitive-text retention boundaries before completion/history or guarded writes.

## Decision

### Bounded, retained workspace

Use a pure registry for session-only tab identity/navigation, capped at 32 total table/console tabs. Table keys are `(SourceUUID, database, table name)`; reopening one activates its retained tab even at capacity. Consoles have unique session IDs and monotonically increasing Console N labels. Active close selects the left neighbor or first remaining tab; inactive close leaves selection intact.

Each tab owns its model/view, requests/abort handles/generations, filters, grid scroll and immutable result snapshot. The root model retains shared profile/catalog/settings ownership and an in-memory credential map with on-demand Keychain access. Closing a tab cancels only owned work; connection edits/removal invalidate affected tabs, while cosmetic names/colors preserve state. No new tab or credential store is introduced.

### Compact database-only UI

Render a flat 28 px Carbonfox strip with table/query icons, names, close x and meaningful nonempty-draft/running indicators. The strip plus uses active-tab context. A seventh explorer toolbar query icon uses the explorer source/database or profile default, otherwise No default database; the toolbar scrolls horizontally at its 200 px minimum width. A cached database chooser does not start discovery. No native New Query Console menu entry is claimed.

Cmd-Shift-N creates a console; Cmd-Alt-Left/Right navigates tabs. Cmd-W closes an active workspace tab, preserving the shell close-window fallback with no tabs. Native OS window-close behavior is unchanged. Nonempty console drafts always require Keep Open/Discard tab confirmation, regardless of execution; Keep Open has default focus and Enter/Space activation.

### Scoped native SQL input

Use native GPUI multiline input/IME/selection/clipboard, with Menlo 13 px, 22 px lines, 44 px gutter and viewport-visible shaping. Limit drafts to 64 KiB and undo to 100 states. Tab inserts four spaces, Shift-Tab unindents, newline autoindents. Run submits selected text or the whole draft; there is no cursor-statement detection, script splitting or autoexecution/cancel on edit.

The SQL editor is an attributed adaptation analogous to the existing GPUI input, not application/editor code imported from another product. Existing notices retain source provenance and Apache-2.0 attribution; this does not select the project's own license. No generic viewer/files, syntax highlighting, completion or persistent history is added.

### Parser allowlist and run-owned sessions

Pin sqlparser 0.62.0 with visitor/MySqlDialect. Tokenize and parse exactly one query with at most one trailing semicolon; comments/quoted semicolons are handled structurally. Validate every nested AST construct, permitting a restricted SELECT/CTE/UNION subset and curated unqualified built-in functions. Reject DDL/DML/CALL/SHOW/EXPLAIN/session commands, INTO/OUTFILE, variables/assignments/locks, executable comments/hints, write CTEs, unknown/stored/UDF/qualified functions and unsupported AST forms before connection.

Protect parsing/traversal with 64 KiB SQL, 4,096 meaningful tokens, parenthesis/CASE nesting 32, 256 operators/recursive constructs and a separate 256-node set-body guard. This is neither full dialect support nor authorization.

Every Run opens a fresh physical connection, sets MySQL MAX_EXECUTION_TIME = 20000 ms or MariaDB max_statement_time = 20 seconds and starts a read-only transaction. Local timeout is 20 seconds. No persistent transaction affinity, autocommit/commit/rollback UI or parallel transaction semantics are offered. Cancel drops the local socket/relay; no server KILL acknowledgment is claimed and work may continue until the deadline.

### Honest results and sensitive data

Do not rewrite submitted SQL for pagination. Retain up to 100 UI rows (backend maximum 200), 512 columns and existing preview-byte/cell bounds, exposing `has_more` warnings and `next_offset = None`. Share immutable `Arc<TablePage>` snapshots through the virtual grid. Disable query header sorting and exclude generic table filters. Loaded CSV uses existing fresh/nontruncated guards and exports only loaded rows, never a full query.

Track in-flight SQL separately from the last successful result SQL. Failed/canceled runs retain correctly labeled stale rows, original elapsed time/warnings and a visible sanitized error. Tab/request generations prevent cross-tab or late installation. Draft edits do not relabel results.

Tabs, SQL and results stay in memory, with no files, SQLite history, restoration, telemetry or automatic ACP sharing. Existing source JSON, metadata SQLite and opt-in Keychain remain unchanged.

## Security consequences

Require least-privilege SELECT roles and trusted server objects. The classifier cannot analyze functions inside server view definitions; a read-only transaction and client allowlist are not an absolute sandbox against privileged accounts or an untrusted server. Finite client retention and parser budgets do not guarantee total process memory or bounded server work. Local cancellation is not confirmed server termination/rollback.

Positive native-protocol tests use owned disposable MySQL/MariaDB fixtures; rejected writes are never sent to prove policy. Simulated input/virtualization tests do not replace native IME/accessibility, manual visual or performance review. See [testing](../testing.md#workspace-tabs-and-query-consoles).

## Alternatives and future work

A single rebuilt table view loses independent state. Prefix classification and semicolon splitting miss nested/dialect/comment constructs. A persistent transaction/session pool would add affinity and uncertain-outcome semantics not yet exposed safely. Importing a full application editor or persisting history would overscope database-only input and retention policy.

Syntax highlighting, completion, history/deletion, saved scripts, cursor/current-statement execution, dedicated transaction sessions, guarded writes and streaming full-query export require explicit follow-up scope. PostgreSQL/Redis need their own parser/policy contracts. See the [query guide](../query-consoles.md), [architecture](../architecture.md#workspace-tabs-and-query-console-ownership) and [roadmap](../roadmap.md#completed-multi-tab--read-only-console-slice).

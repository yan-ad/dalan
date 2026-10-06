# Workspace tabs and read-only query consoles

Status: experimental MySQL/MariaDB database workspace. This is a restricted read console, not arbitrary SQL, a write editor or a full DataGrip replacement. See [ADR 0004](adr/0004-workspace-tabs-and-read-only-consoles.md), [MySQL source setup](mysql-sources.md) and [testing](testing.md#workspace-tabs-and-query-consoles).

## Open and switch tabs

Open a BASE TABLE in Database Explorer to create its table tab. Identity is `(SourceUUID, database, table name)`, not just the display name. Reopening the same table activates the existing tab, preserving filters, requests, grid scroll and results. Different sources/databases can have identically named tables without colliding. Views remain listed but unavailable in the table browser.

Use **New Query Console** (query icon) in the explorer toolbar or **Cmd-Shift-N**. With tabs open, the strip's plus creates a console from the active tab's source/database context. The explorer action uses the clicked/selected source or database, with the source profile's database as fallback; otherwise it starts with **No default database**. With no default, use fully qualified table names. The console database chooser offers cached names and does not trigger discovery; changing it is guarded while running. No native New Query Console menu item is claimed.

All table/console tabs share a **32-tab maximum**. At capacity, opening another table/console reports an error; activating an existing table still works. Consoles have unique session identities and monotonically numbered **Console N** labels; closing one does not recycle its name in the current workspace.

The compact Kit tab strip uses table/query icons, names, close x, a nonempty-draft dot and a real running indicator, with overflow scrolling. The explorer has three toolbar icons and horizontal scrolling at its 200 px minimum width. Kit's default theme replaces Carbonfox runtime styling. This is database-only chrome, not Files/Git/version-control UI or a visual copy of DataGrip.

## Shortcuts and editing

| Action | Shortcut / behavior |
| --- | --- |
| New console | Cmd-Shift-N; explorer query icon or populated-strip plus |
| Previous / next tab | Cmd-Alt-Left / Cmd-Alt-Right |
| Close active tab | Cmd-W or its close x |
| Run | Cmd-Enter or play icon |
| Cancel run | Cmd-Period or stop icon |
| Indent / unindent | Kit editor bindings; verify current native behavior |
| Newline | Kit editor behavior |
| Copy / cut / paste | Native selection/clipboard behavior |
| Undo / redo | Kit editor history/actions; no inherited 100-state guarantee |

The thin SQL adapter uses Kit's rope-backed EditorState for native selection, clipboard, IME, scrolling, undo/redo and SQL Tree-sitter highlighting. Each tab retains its draft in memory. The application SQL policy remains **64 KiB UTF-8**: oversized programmatic loads are rejected; interactive edit/IME/paste limit enforcement is a current regression gate, not a claimed atomic rejection guarantee. Native IME/accessibility still needs real-session verification.

Run submits the **selected text**, or the **whole draft if there is no selection**. It does not detect a statement under the cursor, split scripts, run multiple statements or automatically execute on edit. Editing a draft neither executes nor cancels an existing run. SQL highlighting is implemented; database-aware completion, persistent history, saved script/file workflows and a generic code viewer are not. Grammar support does not implement PostgreSQL/MongoDB/Redis executors or broaden the MySQL/MariaDB read-only policy.

## Close and discard

Closing an active tab selects its left neighbor, or the first remaining tab if there is no left neighbor. Closing an inactive tab does not change active selection. Closing a tab cancels only that tab's owned requests; late completions cannot install results into another tab.

Any **nonempty console draft is unsaved**, even if it has already run successfully. Close x/Cmd-W asks **Keep Open** or **Discard**. Keep Open has default focus; both choices support mouse and keyboard Enter/Space activation. An empty console closes without draft confirmation. No SQL files are written.

Cmd-W closes an active workspace tab. With no workspace tab, Cmd-W closes the window. The native OS window-close control is unchanged; this tab confirmation is not a promise of restored drafts or a new native-window shutdown guard. Closing the application loses in-memory SQL/results.

## Accepted SQL and rejections

The backend uses **sqlparser 0.62.0**, its visitor support and **MySqlDialect**, not prefix matching or manual semicolon splitting. It accepts exactly **one supported SELECT query**, with at most one optional trailing semicolon. Ordinary comments and semicolons inside quoted strings are handled by tokenization. Nested SELECT, CTE and UNION are accepted only if every nested construct passes the same policy.

For example, using your own trusted schema:

```sql
SELECT 1 AS example;
```

```sql
WITH example AS (SELECT 1 AS n)
SELECT n FROM example
UNION ALL
SELECT 2;
```

Functions are a curated unqualified built-in allowlist, not all server functions. The current list is COUNT, SUM, MIN, MAX, AVG, COALESCE, IFNULL, NULLIF, IF, LENGTH, CHAR_LENGTH, CHARACTER_LENGTH, LOWER, UPPER, TRIM, LTRIM, RTRIM, CONCAT, CONCAT_WS, SUBSTRING, SUBSTR, LEFT, RIGHT, REPLACE, ROUND, ABS, CEIL, CEILING, FLOOR, MOD, POWER, SQRT, DATE, YEAR, MONTH, DAY, DAYOFMONTH, DATE_FORMAT, DATEDIFF, NOW, CURRENT_TIMESTAMP, CURRENT_DATE, CURRENT_TIME, UTC_TIMESTAMP, ROW_NUMBER, RANK, DENSE_RANK, LAG, LEAD, FIRST_VALUE, LAST_VALUE, JSON_EXTRACT, JSON_UNQUOTE, JSON_LENGTH and JSON_VALID. This is still subject to parser and construct validation, not a guarantee every syntax form works on every server version.

Rejected before connecting:

- DDL, DML, CALL, SHOW, EXPLAIN and session SET/control statements.
- SELECT INTO/OUTFILE, variable references/assignment and locking clauses.
- MySQL/MariaDB executable comments and optimizer hints, including hints that could change execution limits.
- Write-containing CTEs, table functions/special table sources, unsupported SELECT modifiers and other unsupported AST forms.
- Unknown, stored/UDF, quoted or qualified function calls.
- Multiple statements, empty input and SQL exceeding the budgets below.

**SHOW is not supported in the console currently.** Explorer metadata discovery remains a separate app-generated operation and is unaffected. Unsupported syntax produces a visible error rather than falling back to unvalidated SQL. MySQL dialect parsing is not complete MySQL/MariaDB feature parity.

## Limits and session ownership

| Boundary | Current limit |
| --- | --- |
| SQL text | 64 KiB |
| Meaningful tokens (excluding whitespace/comments) | 4,096 |
| Parenthesis/CASE nesting | 32 |
| Operators/recursive constructs | 256 |
| Set-body traversal guard | 256 nodes |
| Result columns | 512 |
| UI retained rows | 100; backend request maximum 200 |
| Display cell / retained preview | Existing 4 KiB / 2 MiB bounds |
| MySQL server execution limit | MAX_EXECUTION_TIME = 20000 ms |
| MariaDB server execution limit | max_statement_time = 20 seconds |
| Local client timeout | 20 seconds |

Each Run opens a **fresh physical connection**, applies the engine's execution-time limit and starts a **read-only transaction**. It uses the current source profile's existing transport/TLS policy and on-demand credential access. There is no persistent console transaction, cross-run session affinity, autocommit toggle, commit/rollback button or promised parallel-transaction support.

The submitted SQL is not rewritten with LIMIT/OFFSET. Results are capped locally, with `has_more` and a visible warning when rows/preview bytes are omitted; `next_offset` is always None. There is **no query pagination**. Client caps do not bound total server work, process allocations or malicious server behavior.

Cancel disposes the owned socket and any local relay/tunnel work. Closing a tab invalidates its generation too. Neither action sends a confirmed server KILL or proves instant server termination; server work may continue until its deadline. Do not confuse cancellation with rollback confirmation.

## Result provenance and export

Current result bodies use the same production **canvas grid** as tables: quads and cached shaped lines, no per-cell Divs, grapheme-safe capped/fitted previews and a 44 px pinned row-number gutter. Native headers remain interactive elements but query-result sorting is disabled, including explicit table-sort glyph actions. Table WHERE/ORDER BY inputs are not present in consoles and their narrow grammar does not change this guide's SELECT policy.

Global workspace retention is estimated 16 MiB/eight loaded pages with inactive LRU eviction, **not tab/draft eviction**; all 32 tab identities and their SQL/scroll state remain. Active/busy/export/save-protected pages can exceed the best-effort budget; metadata/drafts/GPU/temporary export memory is excluded. On an evicted console, submitted-query metadata remains identifiable but the old result is labeled unavailable with a rerun message. Activation **does not automatically run SQL**. Evicted tables may refresh their applied clauses asynchronously. Views release shaping/text caches when model page is None. See [retention](table-browser.md#workspace-result-retention) and [current evidence](testing.md#rich-canvas-table-browser).

The result grid shares an immutable `Arc<TablePage>` snapshot, preserving two-axis virtualization. Query headers cannot sort and generic table filters are not applied. Scroll/results remain independent per tab. Draft changes never relabel existing rows as if they came from the edited SQL.

The model tracks **query_running_sql** for the in-flight request and **query_submitted_sql** only for the last successfully installed result. Failed/canceled runs retain previous rows with a stale notice, the original successful SQL, elapsed time and warnings, plus a readable sanitized error. Old rows are not labeled with a failed new query. No cross-tab/page late result can become the current page.

Use the existing **Export loaded CSV** action only for fresh, nonbusy, nonstale, nontruncated loaded results. It exports the captured loaded page, not a rerun, full query or whole table. Existing native-picker generation guards, spreadsheet-safe text handling and no-overwrite publication still apply. See [CSV contract](mysql-sources.md#export-loaded-csv).

## Security and persistence

Use a **SELECT-only server role** and **trusted schemas/views/server objects**. The AST classifier cannot analyze view definitions or functions invoked within views. A read-only transaction plus client allowlist is defense in depth, not an absolute sandbox for a broadly privileged account or untrusted server objects.

Profiles remain password-free version 1 JSON; opt-in macOS Keychain and the shared session credential map are unchanged. SQLite caches metadata names/kinds only. Tabs, SQL drafts, results and history are **not persisted**, and no telemetry or automatic ACP sharing is added. SQL clipboard operations are intentional and are distinct from password-input copy/cut suppression. Connection-affecting edits/removal invalidate affected tabs only; cosmetic name/color changes preserve their models/results and update labels.

## Evidence and remaining scope

Current verification passed 93 headless, 132 simulated UI and four bundle tests, plus 23 unique live database/transport cases and strict lint/build checks. See [testing](testing.md#workspace-tabs-and-query-consoles) for evidence and the pending hosted/native gates. Positive query protocol fixtures use owned disposable MySQL/MariaDB containers on existing direct/TLS/CONNECT/SSH routes; rejected writes are never executed. Structural virtual-editor/grid tests do not establish native FPS, IME or accessibility quality.

Syntax highlighting, completion, history retention/deletion, saved scripts, current-statement execution, dedicated transaction sessions, guarded writes, full-query streaming export, column resizing and cell inspection/copy remain separately scoped. There is no fake completion/history/transaction support in this slice.


Changing a source’s connection settings invalidates its affected tabs and cancels their work. Reopening an invalidated table rebuilds it from the updated source while keeping its tab ID; healthy duplicate opens retain their filters/results/scroll state. An invalidated console keeps its SQL draft but refuses execution until the draft is copied into a new console with the new source context. Removing a source invalidates its tabs without changing another source’s tabs.

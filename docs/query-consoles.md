# Workspace tabs and read-only query consoles

Status: experimental MySQL/MariaDB database workspace. This is a restricted read console, not arbitrary SQL, a write editor or a full DataGrip replacement. See [ADR 0004](adr/0004-workspace-tabs-and-read-only-consoles.md), [MySQL source setup](mysql-sources.md) and [testing](testing.md#workspace-tabs-and-query-consoles).

## Open and switch tabs

Open a BASE TABLE in Database Explorer to create its table tab. Identity is `(SourceUUID, database, table name)`, not just the display name. Reopening the same table activates the existing tab, preserving filters, requests, grid scroll and results. Different sources/databases can have identically named tables without colliding. Views remain listed but unavailable in the table browser.

Use **New Query Console** (query icon) in the explorer toolbar or **Cmd-T** (Cmd-Shift-N remains an alias). With tabs open, the strip's plus creates a console from the active tab's source/database context. The explorer action uses the clicked/selected source or database, with the source profile's database as fallback; otherwise it starts with **No default database**. With no default, use fully qualified table names. The console database chooser offers cached names and does not trigger discovery; changing it is guarded while running. No native New Query Console menu item is claimed.

All table/console tabs share a **32-tab maximum**. At capacity, opening another table/console reports an error; activating an existing table still works. Consoles have unique session identities and a monotonically allocated console number; closing one does not recycle its number in the current workspace. Visible query labels use **Source@Database · N** (or No database), while table labels use **Table@Database**.

The compact Kit tab strip uses provider/driver prefix icons so context labels remain visible (Kit `Tab.icon` suppresses labels). Tabs use the saved source color, or a deterministic source-UUID color when none is configured, as a 9% inactive/24% active tint with an active colored underline. Kit text colors remain unchanged. Tabs share 100–280 px bounds with ellipsis/full source/database/console tooltips, a nonempty-draft dot and a real running indicator, with overflow scrolling. Small ghost close buttons stop propagation, so closing an inactive tab does not activate it. The explorer has three toolbar icons and horizontal scrolling at its 200 px minimum width. Kit's default theme replaces Carbonfox runtime styling. This is database-only chrome, not Files/Git/version-control UI or a visual copy of DataGrip.

The database chooser uses actual Kit `ComboboxState<SearchableVec<DbChoice>>` with search and a virtual list, including the 1,000-name fixture. `Option<String>` values distinguish no default from a real database named “No default database”; selection uses values rather than stale indices. Catalog-unchanged busy/result notifications retain the same state and menu. Running guards restore selection rather than change execution context. Search is cached-only and never discovers databases.

## Shortcuts and editing

| Action | Shortcut / behavior |
| --- | --- |
| New console | Cmd-T (Cmd-Shift-N alias); explorer query icon or populated-strip plus |
| Previous / next tab | Cmd-Alt-Left / Cmd-Alt-Right |
| Close active tab | Cmd-W or its close x |
| Run | Cmd-Enter or play icon |
| Cancel run | Cmd-Period or stop icon |
| Indent / unindent | Kit editor bindings; verify current native behavior |
| Newline | Kit editor behavior |
| Copy / cut / paste | Native selection/clipboard behavior |
| Undo / redo | Kit editor history/actions; no inherited 100-state guarantee |

The thin SQL adapter uses Kit's rope-backed EditorState for native selection, clipboard, IME, scrolling, undo/redo and SQL Tree-sitter highlighting. Each tab retains its draft in memory. The application SQL policy remains **64 KiB UTF-8**: oversized programmatic loads are rejected; interactive edit/IME/paste limit enforcement is a current regression gate, not a claimed atomic rejection guarantee. Native IME/accessibility still needs real-session verification.

Run submits the **selected text**, or the **whole draft if there is no selection**. It does not detect a statement under the cursor, split scripts, run multiple statements or automatically execute on edit. Editing a draft neither executes nor cancels an existing run. SQL highlighting is implemented; database-aware completion, persistent history, a persistent script library and a generic code viewer are not. Explicit bounded SQL file open/new-file save is available as described below. Grammar support does not implement PostgreSQL/MongoDB/Redis executors or broaden the MySQL/MariaDB read-only policy.

Run immediately refocuses the actual Kit editor so typing can continue; asynchronous completion does not refocus and steal a deliberately focused result. Regressions exercise actual widget rendering and the native UTF-16 selection interface, not a substitute editor or unsupported text-test API. Preview is rebuild-and-restart, not hot reload: drafts, results and session-only passwords are lost, and server reads may continue until their deadline without a cancellation acknowledgement. See [development caveats](development.md#bacon-live-preview).

## DBX-style safe toolbar tools

The [staged DBX parity plan](dbx-toolbar-parity.md) distinguishes implemented Stage 1 from full future parity. The primary **Run/Stop** control switches with busy state; separate **Cancel** and Cmd-Period are retained. Whole-document **Format** adjusts existing token gaps and recognized keyword case; its per-console uppercase/lowercase preference affects formatting only. **Compress** preserves original token adjacency, comments, literal text and newline-bearing gaps instead of blindly deleting whitespace. Unsafe/unlexable input remains unchanged. These are conservative tools, not a complete dialect-aware formatter.

Kit **Soft Wrap** is remembered per console in memory; **Unfold All** is available. No public Kit Fold All API is integrated, so no Fold All button/placeholder is advertised. **Paste as IN condition** converts clipboard newline/tab cells into escaped quoted strings, preserving spaces/empty cells within the SQL size budget; it inserts, never executes. Editor transformations use Kit undo and preserve active caret/focus behavior.

**Open SQL** accepts one UTF-8 file up to 64 KiB, strips an optional UTF-8 BOM and normalizes CRLF/CR outside literals. Literal carriage returns that Kit cannot preserve are refused. A draft changed during the picker/read is not replaced. **Save SQL** captures the draft to a new file with private Unix creation modes and no overwrite/symlink paths; it is not a SQL library, overwrite workflow or automatic recovery.

The right-side source picker retargets the retained console rather than resetting its tab identity/draft; tab source/database descriptors follow the model on each render. The searchable database chooser remains cached-only. **Clear Database** changes only console scope. **Set Default** persists the selected source's default through root profile storage without auth access or resetting the console target (selecting the existing default toggles it off). URL-only sources can select their effective database, but Set Default is refused: edit the URL in connection settings. Target/tool changes are guarded while busy.

At 800 px available toolbar width, safe tools move into **More** and source selection becomes compact. This tiered layout is not DBX's exact measured overflow/hysteresis. **Export loaded CSV** retains its existing contract as an extra action. New result tabs, explain/analyze, archives, multi-database execution, transaction controls, LSP/diagnostic hints and script library remain unimplemented.

## Close and discard

Closing an active tab selects its left neighbor, or the first remaining tab if there is no left neighbor. Closing an inactive tab does not change active selection. Closing a tab cancels only that tab's owned requests; late completions cannot install results into another tab.

Any **nonempty console draft is unsaved**, even if it has already run successfully. Close x/Cmd-W asks **Keep Open** or **Discard**. Keep Open has default focus; both choices support mouse and keyboard Enter/Space activation. An empty console closes without draft confirmation. Closing never writes a SQL file automatically; explicit Save SQL creates a new file and does not mark the console draft as persisted.

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

Profiles remain password-free version 1 JSON; passwords are session-only unless SaveForever explicitly opts into separate local unencrypted `dalan.auth` storage. The current backend does not use Keychain (see [security](security.md#credentials-and-persistence)). SQLite caches metadata names/kinds only. Tabs, SQL drafts, results and history are **not persisted**, and no telemetry or automatic ACP sharing is added. SQL clipboard operations are intentional and are distinct from password-input copy/cut suppression. Connection-affecting edits/removal invalidate affected tabs only; cosmetic name/color changes preserve their models/results and update labels.

## Evidence and remaining scope

Historical console verification passed 93 headless, 132 simulated UI and four bundle tests, plus 23 unique live database/transport cases and strict lint/build checks. Final local development-fix totals await primary confirmation; hosted CI is explicitly skipped for this task. See [current evidence boundary](testing.md#native-multiline-result-crash-and-development-fixes). Positive query protocol fixtures use owned disposable MySQL/MariaDB containers on existing direct/TLS/CONNECT/SSH routes; rejected writes are never executed. Structural editor/grid tests do not establish native FPS, IME or accessibility quality.

SQL syntax highlighting is implemented. Completion, history retention/deletion, saved scripts, current-statement execution, dedicated transaction sessions, guarded writes, full-query streaming export, column resizing and cell inspection/copy remain separately scoped. There is no fake completion/history/transaction support in this slice.


Changing a source’s connection settings invalidates its affected tabs and cancels their work. Reopening an invalidated table rebuilds it from the updated source while keeping its tab ID; healthy duplicate opens retain their filters/results/scroll state. An invalidated console keeps its SQL draft but refuses execution until the draft is copied into a new console with the new source context. Removing a source invalidates its tabs without changing another source’s tabs.

### Caret and Run dispatch

Focus restoration uses Kit’s `EditorState::focus` so lazy editor initialization starts the caret blink lifecycle. `Cmd-Enter` captures Kit’s secondary Enter action within the focused SQL editor before the inner input consumes it, then calls the same safe Run path as the toolbar. Ordinary Enter still inserts a newline; selection and undo are retained. Completion notifications never steal focus from results. See [caret/shortcut evidence](testing.md#sql-caret-initialization-and-cmd-enter-regression) for test scope and the separate long-line horizontal-scroll limitation.

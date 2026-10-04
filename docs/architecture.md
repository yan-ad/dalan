# Technical architecture

Status: proposed architecture unless marked implemented. See [product scope](product-plan.md) and [ADRs](adr/README.md).

## Workspace boundaries

```text
dalan-app ──> dalan-core
           ├─> dalan-drivers ──> dalan-core
           └─> dalan-acp ──> official ACP SDK

GPUI entities/views live in app only.
Engine adapters never depend on GPUI.
ACP never owns a database connection.
```

Implemented: these four crates and dependency directions. The app has a diagnostic binary, headless-tested shell layout state, and a feature-gated macOS GPUI shell. The shell implements a custom native titlebar, one collapsible/resizable Database Explorer sidebar without an activity rail, a layout menu, keyboard actions, a status strip with a 28 px bot-message-square icon toggle (AI · ACP tooltip/focus help), an optional disconnected ACP panel, and a separate native-menu About Dalan window; main content contains the database browser/read-only table view, while source setup uses a dedicated dialog window. Core provides engine identity, proposed default limits, and a decision table over caller-declared risk. Drivers provide an experimental MySQL/MariaDB adapter and a future engine catalog. ACP exposes SDK v1 schemas, default capabilities, and absolute-path validation. MySQL/MariaDB network reads, profile persistence and opt-in native macOS Keychain credentials exist; no AI connection exists. See [MySQL sources](mysql-sources.md) for the current contracts and evidence matrix.

The workspace is database-only. Do not add a generic Files explorer, code viewer, Git UI, build/run integrations, generic terminal, or plugin/toolbox chrome. Database query consoles, SQL scripts, and database-focused import/export remain in scope. The current app has a read-only browser/table workspace in the main area and a source form in a separate dedicated dialog window, plus an explicitly permitted optional database-focused ACP right panel, not generic tools. No ACP transport, agent launch, text prompt input, or BYOK/provider settings are implemented. Database Explorer defaults to 320 px, bounded to 200–480 px; the body has 0 px outer padding and a 4 px divider hit area (1 px visible line), with no reserved rail width. Main content retains at least 240 px. The ACP panel starts closed and prefers 300 px capped by available space. Compact layout may temporarily hide Database Explorer while ACP is visible without changing retained visibility/width preferences; closing ACP restores them. Layout has four rows: toggle Database Explorer, narrow, widen, and reset. macOS uses Cmd-B, Cmd-Alt-0, and Cmd-Shift-A for ACP, not Ctrl bindings. About Dalan is a separate 420 × 280 nonresizable GPUI macOS window opened by the native application menu, displaying Cargo version, the Javanese meaning “ways,” and database-workspace scope without external libraries.

Do not add a crate for each planned feature. Add modules inside these boundaries first; split storage or platform services only when a tested vertical slice needs an independently owned lifecycle.

## Icons and distribution notices

The desktop entry point installs the embedded Lucide `IconAssets` source. Nineteen SVGs are pinned to `500620a2e8123f8d1db191538886dc0c223f69a9`; no runtime fetch or icon font is required. BotMessageSquare denotes agent communication rather than decorative sparkle or app branding. Root [third-party notices](../THIRD_PARTY_NOTICES.md) contain the complete Lucide ISC/retained Feather MIT notices and adapted GPUI input Apache-2.0 attribution. macOS bundle Resources carry those notices and `lucide-LICENSE.txt`; none select Dalan's project license.

Database Explorer now has only a 28 px toolbar with six 28 px icon actions: Add, Manage, Refresh, Remove, Expand Loaded and Collapse All. The Database Explorer title header is removed, as are the activity rail and header hide/minimise controls. The bottom-left 28 px panel-left toggle is unchanged; Cmd-B and native View/Layout alternatives remain. Closing the explorer retains that preference until toggled or reset. Selection actions have busy/save guards; Add is disabled during saving. At 720 px with ACP closed, the sidebar clamps to at most 476 px. The no-source main state centers Connect to a Source, opening the same form by mouse or Enter/Space even when the explorer is hidden.

Source rows retain names and engine text, with abstract Lucide database/MySQL and database-zap/MariaDB icons rather than vendor branding. Color is an optional marker only, not an execution-risk role. The form places manual hex and labeled presets below Name. Custom colors are not guaranteed AA. Local trash-2.svg aliases unchanged upstream trash.svg at the same pin; [asset provenance](../crates/app/assets/README.md) records the alias and full ISC/Feather MIT notice requirements.

## Shared theme ownership (implemented)

`desktop/theme.rs` owns compiled **Carbonfox - opaque** RGB tokens and compact dimensions for shell, browser, form, About and native text inputs. Every window explicitly uses opaque appearance; upstream alpha-bearing toolbar, selection and border colors are composited over PANEL, not passed through as transparency. There is no runtime JSON loader, blur, theme switch or light mode. Normal status shows the theme name; existing focused-control help overrides it.

The complete exact variant from `cange/nightfox.zed` commit `3511a6f1f665455c70a24d14fd5d2de0eaab58fa`, `themes/nvim-nightfox.json`, is [vendored for reference/tests](../crates/app/assets/themes/carbonfox-opaque.json). Both upstream MIT notices and separate original-project license pin are recorded in [theme provenance](../crates/app/assets/themes/README.md). `include_str!` appears only in the token mapping test; runtime colors are compiled constants. Bundle Resources contain the root third-party notices with both full MIT texts and the icon license, not a required runtime theme JSON/license-file loader. No Zed editor/source/assets beyond this independently licensed theme reference are imported, and no affiliation is claimed.

The user explicitly rejects DataGrip visual style. Zed-like compact UI is the direction; DataGrip informs database workflows only, not button-heavy layout/chrome. Shared 34 px titlebar, 28 px headers/toolbar/status/controls, 22 px tree/grid rows, 3 px control radius and square flush panes preserve workflow ownership. System UI type is 13 px and browse data 12 px, with no external fonts or implemented monospace SQL editor. Inputs use shared background/border, opaque selection, focus cursor and muted placeholders, not independent hardcoded colors. Disabled labels use MUTED, not opacity. Blue outlines mark focus/active state; Save/Connect primary fills use blue with dark text, other controls remain neutral. Semantic error text belongs on panel/input backgrounds, not hover-filled controls; user marker colors stay unchanged and do not establish AA or risk semantics.

The normal-state contrast checks are pure token tests, not native visual evidence. See [palette/metrics](../DESIGN.md#carbonfox-provenance-and-tokens) and [verification](testing.md#carbonfox-opaque-revision). Theme changes do not change database/runtime, metadata cache, SSH/TLS, password or persistence contracts.

## UI and runtime ownership

Implemented for this slice: GPUI owns foreground UI entities; a managed two-worker Tokio runtime owns async database work. Typed bounded results return to the foreground with generation checks and cancellation so superseded requests do not overwrite current state. mysql_async 0.37.1 was chosen instead of SQLx for the native async backend and preservation of database TLS hostname identity through relays. GPUI's executor is not a Tokio runtime. Filter changes retain a labeled stale previous page with pagination disabled until successful refresh; changing sources clears prior rows. SSH has an optional backward-compatible `known_hosts_file`: None keeps OpenSSH user/system defaults; a selected absolute existing file sets `UserKnownHostsFile` and disables global trust via `GlobalKnownHostsFile=/dev/null`, always retaining strict host checking. Actual selected-trust SSH transport reads and rejection cases passed, including paths with spaces. Trusted custom-CA database TLS direct/HTTP CONNECT passed; positive system-trusted HTTPS proxy success remains unverified. CONNECT is wire tunneling, not an implemented database-query HTTP API. Broader session/execution contracts below remain proposals.

- Start a single runtime service at application startup; retain its handle for the application lifetime.
- Send typed requests and bounded event streams across the bridge. Return plain domain data, not GPUI entity handles, across worker threads.
- Apply updates on the GPUI foreground executor using weak view references; ignore responses for closed views and superseded request generations.
- Bound queues, row chunks, payload bytes, and parallel requests. UI virtualization alone does not limit memory use.
- Cancellation and shutdown must stop admission, request cancellation, drain bounded work, terminate owned agent processes, and join tasks within a chosen deadline.

ACP SDK 2.2.0 uses runtime-neutral `futures::io` byte streams. Confirm whether the SDK-owned subprocess helper or a Tokio compatibility adapter fits the launcher requirements before integrating it. Do not copy an older SDK's trait-based example into this version.

### Lazy explorer ownership (implemented)

`explorer_tree.rs` holds UI-independent tree identities, flattening and navigation. Catalog state is keyed by source UUID and database; expansion/cached metadata remain separate from table filters/pages and survive switching sources. `desktop/source_browser.rs` rebuilds the flattened tree on model notifications, not on wheel scrolling, and uses a viewport-backed uniform list to render only its visible range. Explicit width-constrained names, ellipses/full-name tooltips and collision-safe IDs preserve nonblank Unicode/quoted names. Rows are 22 px, and a single focus handle owns guarded tree navigation; disclosure clicks stop propagation.

`SourceModel` separates explorer-selected source from `selected_source`, the identity owning the current table page. Manage/removal of another source cannot rebind that page; delete confirmation captures a stable UUID. Explorer Refresh requires a valid explicitly selected explorer source and transactionally replaces its complete catalog on success, not the current table view. It retains the previous tree on failure and has no current-table-source fallback. Branch status/error dots carry full sanitized tooltip messages; default global errors remain available with nonsecret endpoint context.

Startup restores persisted complete metadata without network/Keychain calls. Branch expansion uses cached metadata first; source/database expansion can still fetch missing metadata. Tables auto-opens, Views remains collapsed initially and unavailable for queries. Catalog tasks have their own per-key generation/abort lifecycle, independent of page/form jobs, with a maximum of two active requests and newest-over-oldest supersession. Collapsing a branch cancels only that branch's catalog work. Expand Loaded expands cached branches without network fan-out; Collapse All preserves metadata. Bounds remain 100 profiles, 1,000 databases per source and 1,000 tables per database. The 1,000-database painted-range regression establishes bounded rendering structure, not FPS, latency or native smoothness.

### Source dialog ownership (implemented)

`desktop/source_dialog.rs` owns the resizable normal GPUI **Data Sources · Dalan** window (1040 × 760 initially, minimum 780 × 560), separate from `source_workspace.rs`'s browser/table UI. Window lookup enforces one application-wide source window, not one per model. Add/Manage/center actions reuse it and preserve the draft. The dialog owns its `SourceForm` and observes `SourceModel`: `form_generation` changes rebuild/focus the form for loaded draft/password updates, while ordinary notifications retain edits. Model dismissal after successful Save removes the window, then starts metadata-only refresh without browsing data.

Cancel, Escape, Cmd-W and native close call the model's draft-close/cancellation path. Native close and Cmd-W reject dismissal during credential/JSON saving. This is not an OS modal sheet and imposes no main-window focus trap. The source store and credential contracts are unchanged. No new dependencies or assets are required. Compact form styling adds 8 px gaps, 16 px scroll padding and footer padding 8 px vertical/16 px horizontal; 28 px inputs, 30 px endpoint parents/candidates and the 18 px Keychain indicator retain existing focus/traversal/guard APIs. A max-720 px natural-height form body and nonshrinking endpoint/candidate rows sit inside a scrolling viewport; inline Host/Port layout preserves native input traversal. See [UI geometry](ui-foundation.md#dedicated-source-dialog-window) and [verification](testing.md#source-dialog-window-and-windows-fixture-fix).

## Proposed domain model

Source profiles already have stable UUIDs. Future console, session and execution identifiers remain separate concepts to introduce. Keep these separate: a profile can have multiple sessions; a console pins one transaction session; an execution belongs to a session and a console.

| Model | Required contract |
| --- | --- |
| Connection profile | Engine, endpoint, database/schema, TLS options, environment label, credential reference. No raw password in persisted configuration. |
| Session | Owning engine, connection identity, autocommit/transaction state, lifecycle generation, active operation. |
| Execution | Immutable target/session generation, statement or command, limits, risk/approval state, timestamps, terminal outcome. |
| Result event | Start, column metadata, bounded typed row chunks, warnings, affected rows, completion/error/uncertain outcome. |
| Grid draft | Qualified table, primary-key identity, original values/concurrency predicate, proposed values, review state. |
| Redis result | Raw key/argument bytes, native response/value shape, type, TTL, cursor/range, truncation information. |

Preserve lossless type identity: NULL differs from empty strings; decimals and large integers must not become floating point; timezone semantics, bytes, JSON, arrays, and unknown database types require explicit decoding/display rules. Display text is not the source of truth for later writes.

## Adapter contracts

Use engine-specific adapters with explicit capabilities, not a universal SQL facade. Proposed operations are connect/test, discover metadata, open/close session, execute, cancel, and bounded browse. SQL-specific transaction/grid operations and Redis key/TTL operations remain distinct.

The current MySQL/MariaDB slice exposes typed test/discovery/column/browse operations, not arbitrary SQL execution. Choose broader async trait/enum dispatch only after real streaming, cancellation and session ownership spikes. Do not freeze a public plugin ABI or a `query(String) -> Vec<Row>` contract now. That contract loses output events and accumulates unbounded results.

Maintain explicit engine identity even when MySQL and MariaDB use the same wire driver. Missing capability means an unavailable action, not silent fallback. No third-party dynamic driver loading in the first release.

## SQL sessions and execution

- Ordinary read sessions can use a pool where appropriate; explicit transactions need a reserved physical connection.
- Database/schema changes and session variables are observable session state. A pooled query cannot pretend to share that state.
- Parse execution boundaries with dialect-aware tooling; never use semicolon splitting or prefix-only safety checks.
- Selection/current statement/full script are different commands. Record sequential results and partial failure; scripts are not implicitly atomic.
- A requested cancel is not a confirmed server abort. A dropped future can leave server-side work running. Verify per-engine cancellation and decide whether to discard affected connections.
- Disconnection after write submission can produce unknown outcome. Do not retry writes automatically or claim rollback without evidence.
- Pin approvals to immutable target, operation, and session generation; editing SQL or switching connections invalidates approval.

## Result browsing and export

Separate user-query execution from generated table browsing. Bound user-query retrieval without quietly appending SQL that changes its meaning. Generated browse queries can use engine-aware paging and explicit ordering. Offset paging is acceptable initially with documented mutation/ordering caveats; consider keyset paging when identity is available.

Implemented sorting uses typed metadata-validated column/direction, identifier quoting and bound filter values, with available primary-key tie-breakers. UI sort changes retain filters and reset the offset; stale pages cannot be exported.

Implemented loaded CSV lives in `crates/app/src/table_export.rs`, separate from driver fetching. A native save picker defaults to `Dalan-loaded-page.csv` without enforcing an extension. Generation is checked before writing; afterward the captured page is the export, even if selection changes. Encoding rejects truncation and bounds output to 8 MiB; no additional rows are fetched. The blocking worker syncs a private `0600` same-directory staging file and publishes by hard link without overwriting an existing file/symlink. Unsupported hard links and cleanup errors remain explicit. Feedback shows success/cancel/error. See [CSV semantics and limits](mysql-sources.md#export-loaded-csv).

Distinguish loaded rows, total known rows, and estimates. Planned user-requested full export requires a separate bounded streaming pipeline with disk backpressure and cancellation; it must not require retaining the whole dataset. Snapshot consistency is not implied. Exporting only loaded data must say so.

### Retained two-axis grid (implemented)

`desktop/data_grid.rs` is a separate retained GPUI entity owned by `SourceBrowser`. The browser no longer builds eager row/cell elements or loops over `CellValue::display` for a full page. The old path rendered every row/column, allocated all display strings and deeply cloned `TablePage` on redraw; a 100 × 512 page represented 51,200 cells even in a small viewport.

`SourceModel.page` is `Option<Arc<TablePage>>`. Grid, browser counters and CSV export capture the same immutable snapshot via `Arc::clone`, not a deep row copy; replacement creates a new snapshot. Page pointer or selected-target changes reset scrolling and clear visible caches. Busy/saving/error notifications retaining the pointer preserve stale view/offsets and disable sorting. CSV still consumes the complete bounded loaded page, never only visible cells or extra server rows. `serde`'s `rc` feature permits shared snapshots in serialization tests; it does **not** enable row persistence. No new crate is added; SQL, schema/catalog caches, profile JSON, SQLite metadata and credentials are unchanged.

`grid_viewport.rs` is pure, GPUI-independent fixed-size geometry with seven tests: 180 px columns, 22 px rows, 28 px header, clamped two-axis offsets, two-cell overscan and scrollbar mapping with a 20 px minimum thumb. Header/body use one horizontal offset. The grid measures actual layout bounds in a canvas prepaint and uses `cx.defer` only when bounds change; it does not wait for a one-frame callback that may never arrive in initial simulated layout. Stable wheel redraws schedule no measurement updates. Cell shaping uses display-only 128-grapheme previews; underlying values and loaded export are unchanged. Visible-range text is reused across small scrolls and evicted outside the viewport; clipped overscan headers are excluded from Tab stops.

A `HashMap<(row, column), SharedString>` retains only text in current visible/overscan ranges, and a separate cache retains only corresponding header strings. Departed entries are dropped; new snapshots clear both. Small scrolls that leave ranges unchanged format zero new cells. Formatting preserves `CellValue::display` behavior for numbers, NULL and bytes; the complete native typed snapshot remains the source of truth. Virtualization bounds UI work, not backend snapshot retention or absolute process memory. No active-cell selection, resizing, inspection/copy, general code viewer or screen-reader completion is implied. See [regressions](testing.md#wide-grid-performance-revision).

## Persistence

Implemented source profiles use version 1 JSON at `~/Library/Application Support/Dalan/sources.json`, stable UUIDs and no passwords. Optional `color: Option<String>` uses `serde(default)` and accepts only `#RRGGBB`, preserving letter case. The version stays 1; legacy profiles load color None without automatic rewrite. The source-field whitelist now includes color, with password rejection unchanged. Invalid color loads fail without rewriting the file; invalid saves preserve existing bytes. Color is local metadata and does not change Keychain behavior. macOS Keychain password saving is explicit; otherwise credentials are session-only and require Edit/re-entry after restart. JSON and Keychain are not an atomic cross-resource transaction; compensation failures remain visible. Source JSON is bounded to 1 MiB and 100 profiles; failed loads block saves. Session-only Save is tested without Keychain calls and one generated native Keychain round-trip passed with cleanup. Layout preferences remain in-memory. Broader preference persistence is planned. Separate embedded SQLite metadata caching is implemented as described below. Query history and session recovery are not implemented or selected; neither the cache nor its dependency adds a user-facing SQLite driver.

Use OS credential services behind a narrow platform interface: macOS Keychain first, Linux Secret Service next, Windows Credential Manager later. Credential store failures must remain explicit with no plaintext fallback. Settings/history migrations need atomic writes, backup/recovery tests, permissions, and secret-free fixtures. Persisted query text and agent conversations are sensitive and must have clear retention/delete controls.

### Persistent metadata cache (implemented)

`app::schema_cache` owns synchronous SQLite persistence, invoked on blocking workers, never the foreground UI. `rusqlite = 0.40.2` disables default features and enables only `bundled`; no external database service, libSQL/cloud client or credential storage is involved. The path is `~/Library/Application Support/Dalan/metadata.sqlite3`, separate from version 1 profile JSON and native Keychain. Directory sync/private Unix `0700` directory and `0600` database creation, DELETE journaling, two-second busy timeout, 128 MiB file cap and 8 MiB encoded metadata bound are enforced. Portable path checks do not defend against hostile concurrent directory replacement.

Schema version 1 is recorded in SQLite `user_version`; application ID is `0x44414c4e` (1145130062). Normalized tables are `source_state(id, identity, generation, fetched_at)`, singleton `cache_clock(id, generation)`, `cached_databases(source_id, name, position)` and `cached_tables(source_id, database_name, name, kind, position)`. Foreign keys cascade child deletion. Persisted driver structs are `CatalogSnapshot { databases: Vec<DatabaseCatalog> }`, `DatabaseCatalog { name: String, tables: Vec<TableInfo> }`, `TableInfo { name: String, kind: String }`. No columns, indexes, DDL or rows are stored. Unsupported versions, foreign application IDs and corruption fail visibly without destructive reset.

The connection identity explicitly serializes engine/host/port/username/optional database/transport/TLS/CA path, not secrets, names, colors or save-password policy. Registration atomically clears old metadata on identity change. Startup loads JSON, registers/prunes and restores snapshots without network or Keychain access. Only a successful credential/profile save launches automatic full-catalog discovery; it closes the dialog and never auto-browses data. Discovery uses one owned connection/tunnel serially, 1,000 databases, 1,000 objects per database, 50,000 total tables/views (separate from databases), 120 seconds overall and 20 seconds per connect/query step.

`RefreshTicket { source_id, identity, generation }` publication matches all three fields in one transaction. A global monotonically increasing cache epoch prevents deleted/re-added IDs and edits away/back from matching old tickets. A per-model Tokio mutex serializes ticket admission; its owned guard moves into the blocking worker so an aborted queued worker cannot invalidate a newer ticket. This is correctness ownership, not a throughput claim. Failed discovery/replacement retains the previous snapshot. Cancellation can leave an already-running blocking write to persist a valid catalog; UI generations ignore its late completion, and ticket guards prevent profile/identity resurrection.

Cache failures are nonfatal `metadata_notice` warnings: unreadable SQLite does not block loading valid source JSON; cache trouble after commit reports “saved” plus a warning rather than falsifying profile-save failure. Deletion failure can leave an orphan until next startup's prune, but never a displayed phantom source. Textual Cached/Stale/Refreshing markers, full Unix-seconds fetch timestamps and sanitized error tooltips distinguish cached metadata from server authorization or offline row availability.

## Platform boundary

The desktop entry point intentionally builds on macOS only. Headless core tests are portable but do not prove desktop support. Linux needs GPUI backend/dependency selection, font and clipboard behavior, credential service and packaging validation. Windows needs native text/window paths, process invocation/quoting, credential service and installer validation. Do not embed POSIX-only path assumptions into domain types.

GPUI 0.2.2 is pinned as a bootstrap baseline, not a permanent commitment. Current Zed main has different `gpui_platform` initialization. Evaluate release/revision upgrades as coherent dependency changes with build, keyboard, rendering, and accessibility regression checks.

## Implementation risks

Editor/completion, broader result-grid behavior beyond the implemented bounded browse viewport, SQL grammar coverage, screen-reader support, cancellation semantics, and desktop portability need executable spikes. GPUI does not provide DataGrip's editor/database features merely by rendering a window. Importing Zed's editor code requires a separate license review; GPUI's Apache-2.0 package license does not cover all Zed crates.

[Drivers](drivers.md) · [ACP](acp.md) · [Security](security.md) · [Testing](testing.md)

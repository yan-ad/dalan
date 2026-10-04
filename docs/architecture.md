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

Implemented: these four crates and dependency directions. The app has a diagnostic binary, headless-tested shell layout state, and a feature-gated macOS GPUI shell. The shell implements a custom native titlebar, one collapsible/resizable Database Explorer sidebar without an activity rail, a layout menu, keyboard actions, a status strip with a 28 px bot-message-square icon toggle (AI · ACP tooltip/focus help), an optional disconnected ACP panel, and a separate native-menu About Dalan window; main content now contains a source form and read-only table view. Core provides engine identity, proposed default limits, and a decision table over caller-declared risk. Drivers provide an experimental MySQL/MariaDB adapter and a future engine catalog. ACP exposes SDK v1 schemas, default capabilities, and absolute-path validation. MySQL/MariaDB network reads, profile persistence and opt-in native macOS Keychain credentials exist; no AI connection exists. See [MySQL sources](mysql-sources.md) for the current contracts and evidence matrix.

The workspace is database-only. Do not add a generic Files explorer, code viewer, Git UI, build/run integrations, generic terminal, or plugin/toolbox chrome. Database query consoles, SQL scripts, and database-focused import/export remain in scope. The current app has a source form and read-only table view in the main area and an explicitly permitted optional database-focused ACP right panel, not generic tools. No ACP transport, agent launch, text prompt input, or BYOK/provider settings are implemented. Database Explorer defaults to 320 px, bounded to 200–480 px; the body has 6 px outer padding on both sides and a 6 px divider gap, with no reserved rail width. Main content retains at least 240 px. The ACP panel starts closed and prefers 300 px capped by available space. Compact layout may temporarily hide Database Explorer while ACP is visible without changing retained visibility/width preferences; closing ACP restores them. Layout has four rows: toggle Database Explorer, narrow, widen, and reset. macOS uses Cmd-B, Cmd-Alt-0, and Cmd-Shift-A for ACP, not Ctrl bindings. About Dalan is a separate 420 × 280 nonresizable GPUI macOS window opened by the native application menu, displaying Cargo version, the Javanese meaning “ways,” and database-workspace scope without external libraries.

Do not add a crate for each planned feature. Add modules inside these boundaries first; split storage or platform services only when a tested vertical slice needs an independently owned lifecycle.

## Icons and distribution notices

The desktop entry point installs the embedded Lucide `IconAssets` source. Fourteen SVGs are pinned to `500620a2e8123f8d1db191538886dc0c223f69a9`; no runtime fetch or icon font is required. BotMessageSquare denotes agent communication rather than decorative sparkle or app branding. Root [third-party notices](../THIRD_PARTY_NOTICES.md) contain the complete Lucide ISC/retained Feather MIT notices and adapted GPUI input Apache-2.0 attribution. macOS bundle Resources carry those notices and `lucide-LICENSE.txt`; none select Dalan's project license.

The explorer renders one 32 px title header and one 32 px toolbar with four 28 px Add, Manage, Refresh and Remove icon buttons. Selection actions are disabled without a source or while busy/saving; Add is disabled during saving. The bottom-left 28 px panel-left toggle replaces rail/header hiding, retains the closed preference and leaves status help unchanged. Cmd-B and View/Layout alternatives remain. At 720 px with ACP closed, the sidebar clamps to at most 462 px. The no-source main state centers Connect to a Source, opening the same form by mouse or Enter/Space even when the explorer is hidden.

Source rows retain names and engine text, with abstract Lucide database/MySQL and database-zap/MariaDB icons rather than vendor branding. Color is an optional marker only, not an execution-risk role. The form places manual hex and labeled presets below Name. Custom colors are not guaranteed AA. Local trash-2.svg aliases unchanged upstream trash.svg at the same pin; [asset provenance](../crates/app/assets/README.md) records the alias and full ISC/Feather MIT notice requirements.

## UI and runtime ownership

Implemented for this slice: GPUI owns foreground UI entities; a managed two-worker Tokio runtime owns async database work. Typed bounded results return to the foreground with generation checks and cancellation so superseded requests do not overwrite current state. mysql_async 0.37.1 was chosen instead of SQLx for the native async backend and preservation of database TLS hostname identity through relays. GPUI's executor is not a Tokio runtime. Filter changes retain a labeled stale previous page with pagination disabled until successful refresh; changing sources clears prior rows. SSH has an optional backward-compatible `known_hosts_file`: None keeps OpenSSH user/system defaults; a selected absolute existing file sets `UserKnownHostsFile` and disables global trust via `GlobalKnownHostsFile=/dev/null`, always retaining strict host checking. Actual selected-trust SSH transport reads and rejection cases passed, including paths with spaces. Trusted custom-CA database TLS direct/HTTP CONNECT passed; positive system-trusted HTTPS proxy success remains unverified. CONNECT is wire tunneling, not an implemented database-query HTTP API. Broader session/execution contracts below remain proposals.

- Start a single runtime service at application startup; retain its handle for the application lifetime.
- Send typed requests and bounded event streams across the bridge. Return plain domain data, not GPUI entity handles, across worker threads.
- Apply updates on the GPUI foreground executor using weak view references; ignore responses for closed views and superseded request generations.
- Bound queues, row chunks, payload bytes, and parallel requests. UI virtualization alone does not limit memory use.
- Cancellation and shutdown must stop admission, request cancellation, drain bounded work, terminate owned agent processes, and join tasks within a chosen deadline.

ACP SDK 2.2.0 uses runtime-neutral `futures::io` byte streams. Confirm whether the SDK-owned subprocess helper or a Tokio compatibility adapter fits the launcher requirements before integrating it. Do not copy an older SDK's trait-based example into this version.

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

## Persistence

Implemented source profiles use version 1 JSON at `~/Library/Application Support/Dalan/sources.json`, stable UUIDs and no passwords. Optional `color: Option<String>` uses `serde(default)` and accepts only `#RRGGBB`, preserving letter case. The version stays 1; legacy profiles load color None without automatic rewrite. The source-field whitelist now includes color, with password rejection unchanged. Invalid color loads fail without rewriting the file; invalid saves preserve existing bytes. Color is local metadata and does not change Keychain behavior. macOS Keychain password saving is explicit; otherwise credentials are session-only and require Edit/re-entry after restart. JSON and Keychain are not an atomic cross-resource transaction; compensation failures remain visible. Source JSON is bounded to 1 MiB and 100 profiles; failed loads block saves. Session-only Save is tested without Keychain calls and one generated native Keychain round-trip passed with cleanup. Layout preferences remain in-memory. Broader preference persistence is planned. Use a local SQLite store for history/session recovery only if those workflows require it; SQLx's SQLite driver would then serve app storage, not a promised new user driver. SQLite history/recovery is not implemented or selected.

Use OS credential services behind a narrow platform interface: macOS Keychain first, Linux Secret Service next, Windows Credential Manager later. Credential store failures must remain explicit with no plaintext fallback. Settings/history migrations need atomic writes, backup/recovery tests, permissions, and secret-free fixtures. Persisted query text and agent conversations are sensitive and must have clear retention/delete controls.

## Platform boundary

The desktop entry point intentionally builds on macOS only. Headless core tests are portable but do not prove desktop support. Linux needs GPUI backend/dependency selection, font and clipboard behavior, credential service and packaging validation. Windows needs native text/window paths, process invocation/quoting, credential service and installer validation. Do not embed POSIX-only path assumptions into domain types.

GPUI 0.2.2 is pinned as a bootstrap baseline, not a permanent commitment. Current Zed main has different `gpui_platform` initialization. Evaluate release/revision upgrades as coherent dependency changes with build, keyboard, rendering, and accessibility regression checks.

## Implementation risks

Editor/completion, result-grid virtualization, SQL grammar coverage, screen-reader support, cancellation semantics, and desktop portability need executable spikes. GPUI does not provide DataGrip's editor/database features merely by rendering a window. Importing Zed's editor code requires a separate license review; GPUI's Apache-2.0 package license does not cover all Zed crates.

[Drivers](drivers.md) · [ACP](acp.md) · [Security](security.md) · [Testing](testing.md)

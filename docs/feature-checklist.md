# Feature checklist

Status: experimental macOS read-only workspace. A checked item means the scoped implementation exists, not release readiness or full DataGrip/Compass compatibility. User direction is to complete database features one by one without adding non-database tools. See [testing](testing.md) for evidence and remaining verification limits.

## Confirmed scoped completions

- [x] Rust workspace and macOS GPUI dark shell, native titlebar/menu, Database Explorer visibility/resizing and keyboard controls.
- [x] About Dalan and optional ACP panel with honest **Not connected** state. No agent process, transport, prompt or provider-key integration.
- [x] Main open-source icon set: twenty-eight embedded Lucide SVGs pinned to `500620a2e8123f8d1db191538886dc0c223f69a9`, including hard-drive, triangle-alert, loader-circle, lock-keyhole and chevron-left. Bottom-right 28 px bot-message-square retains AI · ACP tooltip and Cmd-Shift-A. No new app/brand icon.
- [x] Complete Lucide ISC and retained Feather MIT notices, with adapted GPUI input Apache-2.0 attribution in [third-party notices](../THIRD_PARTY_NOTICES.md), plus bundle resource wiring. Project license remains undecided.
- [x] Experimental MySQL/MariaDB source form, version 1 password-free profile storage and optional database discovery. Test, Save and Connect remain separate actions.
- [x] Session-only passwords and opt-in native macOS Keychain storage; explicit failure/compensation boundaries.
- [x] Direct TCP, strict SSH and anonymous HTTP/HTTPS CONNECT transport configuration; database VerifyIdentity TLS with optional CA and no insecure retry. Trusted system-CA HTTPS proxy success remains unverified.
- [x] BASE TABLE discovery/browse, seven bound-value filters, bounded configurable 1–200-row UI pages (default 100), typed display and honest stale/error/cancel states. Views are listed but cannot be browsed.
- [x] Column header sorting by click or Enter/Space: ascending, descending, none; metadata validation, filter retention, offset reset and available primary-key tie-breakers. Pages are not snapshots and can be unstable without keys.
- [x] Loaded-page CSV with native save picker, fresh complete rows only, visible success/cancel/error and no overwrite. Default name `Dalan-loaded-page.csv`; no extension enforcement. UTF-8/CRLF/quoted fields, explicit NULL syntax, default spreadsheet-safe text protection and private same-filesystem hard-link publication. No full-query or whole-table export. See [exact export limits](mysql-sources.md#export-loaded-csv).
- [x] Headless, simulated GPUI, bundle-helper and disposable live transport coverage. Historical icon/sorting/export results: 48 headless, 42 simulated UI, four bundle-helper, one generated Keychain and 22 live checks passed, including sorting on actual routes.

- [x] Compact explorer: no title header; one horizontally scrolling 28 px toolbar with seven 28 px Add/Manage/Refresh/Remove/Expand Loaded/Collapse All/New Query Console icons, tooltips and selection/busy/save guards; titlebar database toggle retained; no rail or inactive advanced tools.
- [x] Virtual lazy tree for large catalogs: 22 px rows, cached expansion, viewport-only rendering, readable ellipsized names/full-name tooltips, Tables/Views groups and unavailable view leaves.
- [x] Selection-safe explorer actions independent of the current table page; UUID-pinned removal confirmation, complete-source metadata Refresh with retained old snapshot on failure and per-branch catalog cancellation/errors.
- [x] Single-focus tree keyboard navigation and guarded toolbar Enter/Space activation; scoped 71-test simulated suite includes 1,000-database visible-range, scroll-to-900 and End regressions. No native smoothness/FPS claim.
- [x] Compact-explorer local verification: 57 headless, 71 UI and four Python tests passed with formatting, strict lint and bundle verification; current hosted CI and native review remain pending. See [testing](testing.md#compact-lazy-explorer).
- [x] Titlebar 28 px database toggle beside traffic lights retaining the closed preference; Cmd-B and View/Layout alternatives unchanged. Flush body edges have 0 px outer padding and a 4 px divider hit area/1 px visible line; 320 px preferred sidebar, 200–480 px bounds and 476 px maximum at 720 px retain main 240 px and compact ACP behavior.
- [x] Source rows with readable names and abstract Lucide database/MySQL and database-zap/MariaDB cues, not vendor logos or duplicate engine badges; engine and known counts in full-name tooltips. Optional marker color, manual hex and labeled Default/Blue/Green/Amber/Red/Purple form presets; no color-only risk meaning or arbitrary-color AA claim.
- [x] Backward-compatible version 1 color metadata with absent None, case-preserving hex, unchanged password rejection and no automatic rewrite/Keychain migration. Scoped migration and malformed-color no-overwrite tests passed.
- [x] Working centered no-source Connect to a Source action using the same dedicated source dialog window by mouse/Enter/Space, including with explorer hidden; no demo/trial welcome.
- [x] Historical explorer redesign verified: 54 headless, 57 simulated UI and four Python tests passed, including legacy color migration and centered connection with the sidebar hidden; formatting/lint/bundle checks passed. See [testing](testing.md#database-explorer-redesign-verified).

Asset provenance documents trash-2.svg as unchanged upstream trash.svg at the same pin, with full ISC and retained Feather MIT notices. No invented branding, dependencies or generic tools are added.

- [x] Dedicated resizable Data Sources · Dalan window (1040 × 760 initially, minimum 780 × 560), separate from the main browser/table workspace; one reused application-wide draft, Name focus, generation-aware refresh, discard/test cancellation and saving close guards. Not an OS modal sheet.
- [x] Inline Host/Port rows and nonshrinking scrolling body/key-list layout, with draw-bound regressions at 1040 × 760 and 850 × 600. Local suites passed 54 headless, 63 simulated UI and four Python tests; native visual/accessibility review is not implied.
- [x] Windows SSH-key fixture portability fix: control-character filenames are Unix-only; Windows metadata-discovery assertions and CI workflow remain intact. See [historical run and pending main verification](testing.md#hosted-ci-historical-failure-and-pending-main-verification).

## Persistent metadata slice

- [x] Separate version 1 embedded SQLite cache using pinned rusqlite 0.40.2 with only bundled enabled; normalized database/table/view names and kinds, no columns/indexes/DDL/rows or passwords. Profile JSON and native Keychain remain separate.
- [x] Offline startup register/prune/restore without network or Keychain calls; cached-first expansion and fixed 18 px Cached/hard-drive, Stale/triangle-alert and Refreshing/static loader-circle markers with meaning/timestamp/error tooltips on unchanged virtual 22 px rows.
- [x] Successful profile/credential commit closes the dialog before automatic full metadata-only discovery; all visible databases or explicit database scope, one serial owned connection/tunnel, no automatic data browse.
- [x] Transactional bounded refresh, identity invalidation and globally monotonic ticket guards against stale/deleted/re-added/edit-away-back completions; old snapshots remain on refresh failure.
- [x] Explicit explorer-selection-only Refresh, independent of table-page selection/paging; nonfatal disk-cache/save/delete warnings without suppressing profile JSON or inventing phantom sources.
- [x] Historical metadata suites passed 67 headless, 83 simulated UI and four Python tests, formatting/lint/bundle checks and 7 direct/auth/CONNECT + 10 TLS + 6 SSH live cases. Commit `4c8af09` passed all five hosted jobs in [run 37201696519](https://github.com/yan-ad/dalan/actions/runs/37201696519).
- [ ] Current icon-led-revision hosted CI; native macOS offline/source-save/refresh/error/keyboard/accessibility review. Historical theme CI is recorded below; no native Keychain rerun or production endpoint success is claimed.

## Carbonfox compact foundation

- [x] User-selected **Carbonfox - opaque** default and compact Zed-like UI. DataGrip visual styling/button-heavy layout explicitly rejected; database UX/workflows only remain references.
- [x] Full exact variant vendored from Nightfox Zed port commit `3511a6f1f665455c70a24d14fd5d2de0eaab58fa`, with both full MIT licenses and separate original-project license provenance. Runtime uses compiled tokens, not JSON; no affiliation or other Zed source/editor asset import. See [theme record](../crates/app/assets/themes/README.md).
- [x] Shared opaque main/source/About/input theme; alpha toolbar/selection/border composited over PANEL; no blur/transparency/toggle/fake light. Status spacer has a Carbonfox - opaque tooltip only; no persistent theme/focus-help text or extra Theme button.
- [x] Compact 34 px titlebar, 28 px headers/toolbars/status/controls, 22 px tree/grid, 3 px controls/0 px panes, flush 0 px outer padding and 4 px divider hit area/1 px line. System UI 13 px/data 12 px, no external font assets; the SQL editor now uses system Menlo 13 px with 22 px lines.
- [x] Neutral controls/hover/selection, blue focus/active and Save/Connect primary with dark text; readable disabled MUTED labels without opacity; shared opaque input selection, cursor/placeholder and visible input borders. User source markers remain unchanged, separately labeled and not AA-guaranteed.
- [x] Compact form gaps/scroll/footer styling while retaining source window size, APIs, focus/saving guards, 28 px inputs, 30 px endpoint parents/candidates and 18 px Keychain indicator. Compact table header/filter/footer without fake content; no DB/cache/SSH/password migration.
- [x] Pure token mapping/compositing and contrast coverage; computed normal-state minima 13.04:1 primary, 7.22:1 secondary, 6.10:1 focus and 3.44:1 input boundary. Semantic labels supported on panel/input, not arbitrary hover controls. No native pixel/accessibility claim.
- [x] Historical theme verification: 67 headless, 88 simulated UI and four Python tests, formatting/lint/build/bundle checks. Theme commit `0ca0221` passed all five hosted jobs in [run 37204370849](https://github.com/yan-ad/dalan/actions/runs/37204370849). This is historical evidence for that commit; the wide-grid revision needs its own next-main CI run.
- [ ] Native screenshot/manual/accessibility review remains open; simulated contrast/layout checks do not close it.

## Wide-grid rendering slice

- [x] Retained separate GPUI `DataGrid`: both-axis viewport virtualization with two-cell overscan, fixed 180 px columns/22 px rows, pinned 28 px header sharing horizontal offset; no eager full-page cell elements or formatting.
- [x] Shared immutable `Option<Arc<TablePage>>` for model/grid/counters/export; no deep page copy on redraw. New snapshot/target resets scroll/cache; same-page busy/saving/error updates retain stale scroll and disable sorting. No persisted row cache, SQL/schema-cache/password change or new crate.
- [x] Wheel/trackpad/Shift-wheel, grid-focused arrows/PageUp/PageDown/Home/End/Ctrl-or-Cmd-Home/End, two visible draggable/clickable tracks with minimum 20 px thumbs. Visible/overscan header Tab sorting remains guarded; this is not active-cell selection or screen-reader completion.
- [x] Visible cell/header `SharedString` caches with bounded eviction and snapshot invalidation; changed-bounds-only deferred canvas measurement, no frame callback dependency or stable-wheel bounds updates.
- [x] Combined simulated workspace subset passed: 1,000 databases, 100 × 512 cells, 950 × 574.5 viewport at 1280 × 720, 310/51,200 materialized cells (~0.61%), 32 sidebar rows, projection rebuild delta 0 and same page pointer. Sidebar scrolling leaves grid/model unchanged; not native FPS/latency evidence.
- [x] Wide-grid verification: 74 headless, 99 simulated UI and four Python tests passed, including bounded text reuse, visible-only header focus and released-drag cancellation; formatting/lint/bundle checks passed. Hosted CI remains a separate gate. See [evidence](testing.md#wide-grid-performance-revision).
- [ ] Native user retry, screenshot/manual review and high-DPI/GPU profiling on the latest macOS runtime-shader build; current-revision hosted CI, release/offline-Metal and accessibility gates remain open. Column resizing and cell inspection/copy remain next, not completed.

## Icon-led chrome verification

- [x] Dalan-only titlebar, 28 px Layout icon opening the existing four-row popover, preserved shortcuts/focus outlines/action tooltips and right ACP trigger.
- [x] No Table browser/read-only header without a selected table or page; saved sources show small-icon Select a table, empty profiles retain working Connect to a Source. Actual qualified title and passive 28 px lock tooltip appear for selected/loaded tables only.
- [x] Apply/check, Clear/minus, Export/download and Previous/Next chevron 28 px controls retain IDs, keyboard activation and guards; actual filter column/operator/value remain visible. Minimal loaded-range footer with loaded-count/has_more tooltip, no fabricated totals. Errors/stale feedback/cache warnings remain text; Tables/Views grouping names remain meaningful.
- [x] Icon-led local verification: 74 headless, 106 simulated UI and four Python tests passed, plus formatting/lint/build/bundle checks. Current hosted CI and native visual/accessibility remain separate gates. Historical credential fix `9b3d3d8` passed all five jobs in run 37212641100. See [testing](testing.md#icon-led-chrome-revision).

## Next read-workflow slices, in order

1. [ ] Column resizing and bounded cell inspection/copy, with keyboard/focus and value-fidelity tests. These are next, not already done.
2. [x] Scoped read-only query console, with [ADR 0004](adr/0004-workspace-tabs-and-read-only-consoles.md), MySQL-dialect AST allowlist, fresh run-owned sessions and local cancellation. No semicolon splitting or prefix-only safety classification; broader dialect support remains future work.
3. [ ] SQL editor completion and history, with scoped dialect support, sensitive-text retention/deletion and distinct selection/current-statement/script execution semantics.
4. [ ] Dedicated transaction sessions and guarded staged writes, primary-key identity, conflict/affected-row checks, immutable approvals and unknown-outcome handling. Do not turn browse/export completion into write authorization.
5. [ ] Separate bounded streaming full-query/whole-table export pipeline, disk backpressure and cancellation. Do not imply snapshot consistency.

## Remaining platform and release work

- [ ] Verify all five CI jobs on the actual next main push. Historical run 37192402473 failed the Windows fixture; fix `3c4fdae` passed all five jobs in run 37194669634. Compact-tree `8dc3d27` passed run 37198333660 and metadata `4c8af09` passed run 37201696519; theme `0ca0221` passed all five jobs in run 37204370849. Credential fix `9b3d3d8` passed all five jobs in [run 37212641100](https://github.com/yan-ad/dalan/actions/runs/37212641100). The current icon-led revision needs its own run. Live database and generated Keychain reruns are not required for this UI-only change; their historical evidence remains preserved.
- [ ] Native macOS source/browse/sort/save-picker keyboard and error/cancel interaction, visual review, VoiceOver and scaled text. Simulated tests do not close these gates.
- [ ] Broader MySQL/MariaDB auth/TLS/server matrix and positive trusted HTTPS proxy fixture.
- [ ] PostgreSQL adapter and workflow slice; Redis engine-native key/type/TTL/binary workflows. Neither adapter is working yet.
- [ ] Real ACP suggestion/insertion lifecycle and context consent. External agents own authentication/billing; no application BYOK or autonomous database execution.
- [ ] macOS release packaging, minimum OS/Intel decisions, signing/notarization, measured performance/accessibility gates; light/system support is an unselected future proposal, not a shipped mode.
- [ ] Linux second, Windows third, each with platform-specific GPUI, credential, input and distribution validation.
- [ ] MongoDB/Compass-style workflows after the first release.
- [ ] User-authorized commit and push, with actual revision/remote verification by the primary workflow. Documentation does not claim this has happened.

## Scope boundary

Database explorer, query consoles, data views, results/sessions and optional ACP only. No generic Files explorer, code viewer, Git UI, build tools, general terminal or plugin/toolbox chrome. First-release engine goals are not present-day compatibility claims. Complete and verify each bounded slice before expanding scope.

[Overview](../README.md) · [Roadmap](roadmap.md) · [MySQL sources](mysql-sources.md) · [UI foundation](ui-foundation.md) · [Architecture](architecture.md) · [Security](security.md) · [Testing](testing.md)

## Workspace tabs and read-only consoles

- [x] Table identity `(SourceUUID, database, table)`, duplicate activation, 32-tab total cap, unique monotonic Console N labels and active-close neighboring selection.
- [x] Retained per-tab models, requests, filter state, grid scroll and immutable result snapshots; cancellation/generation isolation on close and affected-source connection changes/removal. Cosmetic labels/colors preserve state.
- [x] Flat 28 px Carbonfox table/query strip, close x, meaningful draft/running indicators, populated-strip plus and seventh explorer query icon with narrow-width horizontal scrolling. Cmd-Shift-N and Cmd-Alt-Left/Right controls, with context-specific source/database selection.
- [x] Native GPUI multiline SQL selection/IME/clipboard, Menlo 13 px/22 px lines/44 px gutter, visible shaping, 64 KiB and 100 undo-state bounds; Tab four spaces, Shift-Tab unindent and newline autoindent.
- [x] Run selected SQL or whole draft by play/Cmd-Enter and local cancel by stop/Cmd-Period; one parsed SELECT with optional trailing semicolon, nested CTE/UNION and curated functions. Unsupported statements/constructs fail visibly before connection.
- [x] Fresh physical read-only transaction per Run, 20-second MySQL/MariaDB server/client limits, finite parser/result budgets and no submitted-SQL pagination rewriting. No server KILL confirmation, persistent transaction/autocommit or write UI.
- [x] Query grid sorting disabled, table filters excluded, result cap warning and no next offset; existing guarded loaded-only CSV, not full-query export.
- [x] Successful SQL provenance separate from in-flight SQL; failure retains prior SQL/elapsed/warnings and stale results with visible sanitized error. Draft edits do not autoexecute/cancel.
- [x] Nonempty-draft tab-close Keep Open/Discard confirmation regardless of prior execution; Keep Open default focus with mouse/Enter/Space. No SQL/result/tab persistence, telemetry or new credential storage.
- [x] Local/live verification: 93 headless, 132 simulated GPUI, four Python and 23 unique live cases passed with strict lint and signed bundle checks. Current hosted CI and native IME/visual/accessibility/performance review remain separate gates. Historical `cf24e48` passed all five jobs in [run 37215973701](https://github.com/yan-ad/dalan/actions/runs/37215973701); do not treat it as current evidence.
- [ ] Syntax highlighting, completion, persistent history and deletion policy; saved scripts, cursor/current-statement execution, full-query export and dedicated transaction/write workflows remain separately scoped.

- [x] Multi-table/query-console verification: 93 headless, 132 simulated UI, four bundle tests and 23 live database/transport cases passed; formatting/lint/signed bundle checks passed. Current hosted CI and native visual/accessibility review remain separate gates.

## Source-manager redesign scope

- [x] General/Options/SSH/SSL/Schemas titlebar tabs, real driver/authentication combos, Name/Color, opaque Carbonfox body and traffic-light reservation; independent source window reuse/saving guards.
- [x] Default, explicit local Unix Socket and credential-free URL-only with generated URL synchronization and static rejected-credential diagnostics; no arbitrary JDBC properties.
- [x] No Auth supplies no credentials, skips Keychain and clears remembered credentials on Save with compensation; password-free version 1 JSON compatibility.
- [x] Applied Connect 1–60 seconds, Query 1–120 seconds and Page size 1–200; defaults 10/20/100, unchanged global catalog cap.
- [x] Independent reusable SSH manager/repository, named selection and Custom fallback; strict host checking, agent/key only, explicit Parse config warning, owned-child Test cancellation and in-use removal guard.
- [x] Four explicit TLS modes, optional CA picker and paired driver-wired PEM identity paths; no encrypted TLS-key passphrase/Java truststore controls.
- [x] Searchable exact-name Schemas checkbox visibility filter, complete allowed SQLite snapshots and unchanged full console database choices.
- [ ] Final owner-run confirmation of expected 105 headless, 141 simulated UI, four Python and expanded 29 unique live cases (11 direct, 12 TLS, 6 SSH); no unverified counts marked passed here.
- [ ] Positive live mutual TLS, live successful SSH-manager remote `true`, native visual/window/accessibility review and positive system-trusted HTTPS proxy.
- [ ] Current hosted CI; historical `9ecae4c` passed all five jobs in run 37255792022, not this revision.

See [source management](source-management.md) and [evidence](testing.md#source-manager-redesign). Existing 23-case query-console coverage remains historical. No new icons, packages or licensing choices are part of this slice.

- [x] Source-manager redesign verification: 105 headless unit tests plus one native No Auth wire test, 141 simulated UI, four Python and 29 unique live cases passed with formatting, strict lint and signed bundle checks. Hosted CI and native visual/accessibility remain separate gates.

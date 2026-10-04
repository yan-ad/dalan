# Feature checklist

Status: experimental macOS read-only workspace. A checked item means the scoped implementation exists, not release readiness or full DataGrip/Compass compatibility. User direction is to complete database features one by one without adding non-database tools. See [testing](testing.md) for evidence and remaining verification limits.

## Confirmed scoped completions

- [x] Rust workspace and macOS GPUI dark shell, native titlebar/menu, Database Explorer visibility/resizing and keyboard controls.
- [x] About Dalan and optional ACP panel with honest **Not connected** state. No agent process, transport, prompt or provider-key integration.
- [x] Main open-source icon set: nineteen embedded Lucide SVGs pinned to `500620a2e8123f8d1db191538886dc0c223f69a9`. Bottom-right 28 px bot-message-square replaces text in the ACP trigger, retaining AI · ACP tooltip/focus help and Cmd-Shift-A. No new app/brand icon.
- [x] Complete Lucide ISC and retained Feather MIT notices, with adapted GPUI input Apache-2.0 attribution in [third-party notices](../THIRD_PARTY_NOTICES.md), plus bundle resource wiring. Project license remains undecided.
- [x] Experimental MySQL/MariaDB source form, version 1 password-free profile storage and optional database discovery. Test, Save and Connect remain separate actions.
- [x] Session-only passwords and opt-in native macOS Keychain storage; explicit failure/compensation boundaries.
- [x] Direct TCP, strict SSH and anonymous HTTP/HTTPS CONNECT transport configuration; database VerifyIdentity TLS with optional CA and no insecure retry. Trusted system-CA HTTPS proxy success remains unverified.
- [x] BASE TABLE discovery/browse, seven bound-value filters, bounded 100-row UI pages, typed display and honest stale/error/cancel states. Views are listed but cannot be browsed.
- [x] Column header sorting by click or Enter/Space: ascending, descending, none; metadata validation, filter retention, offset reset and available primary-key tie-breakers. Pages are not snapshots and can be unstable without keys.
- [x] Loaded-page CSV with native save picker, fresh complete rows only, visible success/cancel/error and no overwrite. Default name `Dalan-loaded-page.csv`; no extension enforcement. UTF-8/CRLF/quoted fields, explicit NULL syntax, default spreadsheet-safe text protection and private same-filesystem hard-link publication. No full-query or whole-table export. See [exact export limits](mysql-sources.md#export-loaded-csv).
- [x] Headless, simulated GPUI, bundle-helper and disposable live transport coverage. Historical icon/sorting/export results: 48 headless, 42 simulated UI, four bundle-helper, one generated Keychain and 22 live checks passed, including sorting on actual routes.

- [x] Compact explorer: no title header; one 32 px toolbar with six 28 px Add/Manage/Refresh/Remove/Expand Loaded/Collapse All icons, tooltips and selection/busy/save guards; bottom-left toggle unchanged; no rail or inactive advanced tools.
- [x] Virtual lazy tree for large catalogs: 22 px rows, cached expansion, viewport-only rendering, readable ellipsized names/full-name tooltips, Tables/Views groups and unavailable view leaves.
- [x] Selection-safe explorer actions independent of the current table page; UUID-pinned removal confirmation, source-root-only Refresh and per-branch catalog cancellation/errors.
- [x] Single-focus tree keyboard navigation and guarded toolbar Enter/Space activation; scoped 71-test simulated suite includes 1,000-database visible-range, scroll-to-900 and End regressions. No native smoothness/FPS claim.
- [x] Compact-explorer local verification: 57 headless, 71 UI and four Python tests passed with formatting, strict lint and bundle verification; current hosted CI and native review remain pending. See [testing](testing.md#compact-lazy-explorer).
- [x] Bottom-left 28 px panel-left toggle retaining the closed preference; Cmd-B and View/Layout alternatives unchanged. Both body sides have 6 px padding; 320 px preferred sidebar, 200–480 px bounds and 462 px maximum at 720 px retain main 240 px and compact ACP behavior.
- [x] Source rows with readable name/engine labels and abstract Lucide database/MySQL and database-zap/MariaDB cues, not vendor logos. Optional marker color, manual hex and labeled Default/Blue/Green/Amber/Red/Purple form presets; no color-only risk meaning or arbitrary-color AA claim.
- [x] Backward-compatible version 1 color metadata with absent None, case-preserving hex, unchanged password rejection and no automatic rewrite/Keychain migration. Scoped migration and malformed-color no-overwrite tests passed.
- [x] Working centered no-source Connect to a Source action using the same dedicated source dialog window by mouse/Enter/Space, including with explorer hidden; no demo/trial welcome.
- [x] Historical explorer redesign verified: 54 headless, 57 simulated UI and four Python tests passed, including legacy color migration and centered connection with the sidebar hidden; formatting/lint/bundle checks passed. See [testing](testing.md#database-explorer-redesign-verified).

Asset provenance documents trash-2.svg as unchanged upstream trash.svg at the same pin, with full ISC and retained Feather MIT notices. No invented branding, dependencies or generic tools are added.

- [x] Dedicated resizable Data Sources · Dalan window (1040 × 760 initially, minimum 780 × 560), separate from the main browser/table workspace; one reused application-wide draft, Name focus, generation-aware refresh, discard/test cancellation and saving close guards. Not an OS modal sheet.
- [x] Inline Host/Port rows and nonshrinking scrolling body/key-list layout, with draw-bound regressions at 1040 × 760 and 850 × 600. Local suites passed 54 headless, 63 simulated UI and four Python tests; native visual/accessibility review is not implied.
- [x] Windows SSH-key fixture portability fix: control-character filenames are Unix-only; Windows metadata-discovery assertions and CI workflow remain intact. See [historical run and pending main verification](testing.md#hosted-ci-historical-failure-and-pending-main-verification).

## Next read-workflow slices, in order

1. [ ] Column resizing and bounded cell inspection/copy, with keyboard/focus and value-fidelity tests. These are next, not already done.
2. [ ] Read-oriented query console. Write a new ADR for dialect-aware parsing/execution boundaries, cancellation and explicit session ownership before implementation. No semicolon splitting or prefix-only safety classification.
3. [ ] SQL editor completion and history, with scoped dialect support, sensitive-text retention/deletion and distinct selection/current-statement/script execution semantics.
4. [ ] Dedicated transaction sessions and guarded staged writes, primary-key identity, conflict/affected-row checks, immutable approvals and unknown-outcome handling. Do not turn browse/export completion into write authorization.
5. [ ] Separate bounded streaming full-query/whole-table export pipeline, disk backpressure and cancellation. Do not imply snapshot consistency.

## Remaining platform and release work

- [ ] Verify all five CI jobs on the actual next main push. Historical run 37192402473 failed the Windows fixture; fix `3c4fdae` passed all five jobs in run 37194669634. The current compact-tree revision requires a separate run. Live database and generated Keychain reruns are not required for this window/layout/test-fixture-only change; their historical evidence remains preserved.
- [ ] Native macOS source/browse/sort/save-picker keyboard and error/cancel interaction, visual review, VoiceOver and scaled text. Simulated tests do not close these gates.
- [ ] Broader MySQL/MariaDB auth/TLS/server matrix and positive trusted HTTPS proxy fixture.
- [ ] PostgreSQL adapter and workflow slice; Redis engine-native key/type/TTL/binary workflows. Neither adapter is working yet.
- [ ] Real ACP suggestion/insertion lifecycle and context consent. External agents own authentication/billing; no application BYOK or autonomous database execution.
- [ ] macOS release packaging, minimum OS/Intel decisions, signing/notarization, light/system themes and measured performance/accessibility gates.
- [ ] Linux second, Windows third, each with platform-specific GPUI, credential, input and distribution validation.
- [ ] MongoDB/Compass-style workflows after the first release.
- [ ] User-authorized commit and push, with actual revision/remote verification by the primary workflow. Documentation does not claim this has happened.

## Scope boundary

Database explorer, query consoles, data views, results/sessions and optional ACP only. No generic Files explorer, code viewer, Git UI, build tools, general terminal or plugin/toolbox chrome. First-release engine goals are not present-day compatibility claims. Complete and verify each bounded slice before expanding scope.

[Overview](../README.md) · [Roadmap](roadmap.md) · [MySQL sources](mysql-sources.md) · [UI foundation](ui-foundation.md) · [Architecture](architecture.md) · [Security](security.md) · [Testing](testing.md)

# Feature checklist

Status: experimental macOS read-only workspace. A checked item means the scoped implementation exists, not release readiness or full DataGrip/Compass compatibility. User direction is to complete database features one by one without adding non-database tools. See [testing](testing.md) for evidence and remaining verification limits.

## Confirmed scoped completions

- [x] Rust workspace and macOS GPUI dark shell, native titlebar/menu, Database Explorer visibility/resizing and keyboard controls.
- [x] About Dalan and optional ACP panel with honest **Not connected** state. No agent process, transport, prompt or provider-key integration.
- [x] Main open-source icon set: eight embedded Lucide SVGs pinned to `500620a2e8123f8d1db191538886dc0c223f69a9`. Bottom-right 28 px bot-message-square replaces text in the ACP trigger, retaining AI · ACP tooltip/focus help and Cmd-Shift-A. No new app/brand icon.
- [x] Complete Lucide ISC and retained Feather MIT notices, with adapted GPUI input Apache-2.0 attribution in [third-party notices](../THIRD_PARTY_NOTICES.md), plus bundle resource wiring. Project license remains undecided.
- [x] Experimental MySQL/MariaDB source form, version 1 password-free profile storage and optional database discovery. Test, Save and Connect remain separate actions.
- [x] Session-only passwords and opt-in native macOS Keychain storage; explicit failure/compensation boundaries.
- [x] Direct TCP, strict SSH and anonymous HTTP/HTTPS CONNECT transport configuration; database VerifyIdentity TLS with optional CA and no insecure retry. Trusted system-CA HTTPS proxy success remains unverified.
- [x] BASE TABLE discovery/browse, seven bound-value filters, bounded 100-row UI pages, typed display and honest stale/error/cancel states. Views are listed but cannot be browsed.
- [x] Column header sorting by click or Enter/Space: ascending, descending, none; metadata validation, filter retention, offset reset and available primary-key tie-breakers. Pages are not snapshots and can be unstable without keys.
- [x] Loaded-page CSV with native save picker, fresh complete rows only, visible success/cancel/error and no overwrite. Default name `Dalan-loaded-page.csv`; no extension enforcement. UTF-8/CRLF/quoted fields, explicit NULL syntax, default spreadsheet-safe text protection and private same-filesystem hard-link publication. No full-query or whole-table export. See [exact export limits](mysql-sources.md#export-loaded-csv).
- [x] Headless, simulated GPUI, bundle-helper and disposable live transport coverage. Current results: 48 headless, 42 simulated UI, four bundle-helper, one generated Keychain and 22 live checks passed, including sorting on actual routes.

## Next read-workflow slices, in order

1. [ ] Column resizing and bounded cell inspection/copy, with keyboard/focus and value-fidelity tests. These are next, not already done.
2. [ ] Read-oriented query console. Write a new ADR for dialect-aware parsing/execution boundaries, cancellation and explicit session ownership before implementation. No semicolon splitting or prefix-only safety classification.
3. [ ] SQL editor completion and history, with scoped dialect support, sensitive-text retention/deletion and distinct selection/current-statement/script execution semantics.
4. [ ] Dedicated transaction sessions and guarded staged writes, primary-key identity, conflict/affected-row checks, immutable approvals and unknown-outcome handling. Do not turn browse/export completion into write authorization.
5. [ ] Separate bounded streaming full-query/whole-table export pipeline, disk backpressure and cancellation. Do not imply snapshot consistency.

## Remaining platform and release work

- [ ] Final current-revision test/lint rerun, bundle resource/signature verification and optional generated Keychain rerun. Expected counts are not final pass claims.
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

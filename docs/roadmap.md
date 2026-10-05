# Dalan roadmap

Status: sequencing proposal, not a delivery commitment. Foundation and the experimental [MySQL/MariaDB read-only slice](mysql-sources.md) exist. No full phase is complete. There are no invented dates or staffing assumptions. The user requires Rust + GPUI, macOS first, first-release PostgreSQL/MySQL/MariaDB/Redis, MongoDB after first release, Linux second, and Windows third. Other scope and phase boundaries are proposals.

MySQL/MariaDB-first is now explicit user direction and supersedes the earlier PostgreSQL-first sequence. PostgreSQL and Redis remain future work; MongoDB remains post-first-release. Implemented does not mean a full phase or release gate is complete.

## One-by-one workflow completion

Track confirmed experimental completions and remaining gates in the [feature checklist](feature-checklist.md). Current priority is zero-version workflow parity one slice at a time, not a claim of DataGrip/Compass parity or a reason to overscope. After sorting and loaded CSV, improve column resizing and cell inspection/copy. The scoped read-only query console and [ADR 0004](adr/0004-workspace-tabs-and-read-only-consoles.md) are now implemented; SQL syntax highlighting/completion/history remain next editor slices, with explicit sensitive-text retention and execution semantics. Guarded transactions and staged writes follow later. Commit/push is authorized by the user, but remains an execution gate until the primary workflow performs and verifies it.

## Proposed sequence and exit criteria

### 1. Foundation

- Establish Rust workspace, pinned GPUI revision, macOS Apple Silicon packaging path, configuration model, and engine capability interfaces.
- Maintain the database-only shell and keyboard/focus model: one database rail, one collapsible/resizable Database Explorer sidebar, native titlebar/traffic lights, four-row Layout menu, status strip, source form and read-only table view, and an initially closed, explicitly permitted database-focused ACP right panel. About Dalan and the disconnected AI · ACP panel are tested shell features only; [validation](ui-foundation.md#modules-and-verification) does not imply working agent connections. Planned database consoles, results/output/sessions, and real ACP sessions have separate milestones.
- Keep generic Files explorer, code viewer, Git UI, build/run integrations, generic terminal, and plugin/toolbox chrome out of scope. Preserve SQL parsing, query consoles, database script workflows, and database-focused import/export requirements.
- Validate the 320 px preferred sidebar width, 200–480 px bounds, 36 px rail, 6 px divider gap, 6 px right padding, and at least 240 px content. Compact layout clamps one pane and preserves preference. Layout rows toggle, narrow, widen, and reset Database Explorer; macOS shortcuts are Cmd-B and Cmd-Alt-0, not Ctrl variants. Revalidate layout and GPUI controls, including absence of file selectors; current suite counts and boundaries are recorded in [testing](testing.md).
- Spike GPUI rendering, accessibility, clipboard, native menus, and virtualized grids. Do not assume framework support guarantees screen-reader compatibility.
- Spike SQL parsing/completion and Redis binary argument representation before committing to libraries.
- Define credential boundaries, cancellation/outcome states, test fixtures, minimum macOS target, Intel gate, and supported server matrix.

Exit: documented architecture/ADRs, working internal shell proof of concept, explicit risk list, and reproducible performance/accessibility test approach. A prototype is not a finished UI.

Reason: session semantics and platform constraints are harder to change after drivers depend on them.

### 2. MySQL and MariaDB first

Implemented experimentally: stable UUID profiles in version 1 JSON without passwords, opt-in native macOS Keychain, Test/Save/Connect separation, database discovery, BASE TABLE reads, seven bound-value filters, bounded pages, metadata-validated column sorting and loaded-page CSV export. Direct TCP, SSH and anonymous HTTP/HTTPS CONNECT carry the database protocol; they are not a SQL-over-HTTP gateway. The main UI is no longer blank. See [source setup and scope](mysql-sources.md).

Verified on MySQL 8.4.11 and MariaDB 11.4.13: six direct/HTTP CONNECT smoke tests, ten verified database TLS/proxy rejection tests, and six SSH reads/host-key/identity rejection tests. Native Keychain generated-item validation passed. Historical source-slice counts were 37 headless and 27 simulated UI tests, plus four bundle tests. Latest expected inventory is 48 headless and 42 simulated UI tests, pending final rerun confirmation. The 22 live cases were rerun with sorting on actual transport routes. Positive system-trusted HTTPS proxy success remains unverified; see [testing](testing.md).

Remaining: wider auth/TLS/capability matrix, native keyboard/accessibility checks, broader SQL dialect coverage, transaction affinity, full-query/whole-table streaming export, staged edits, uncertain outcomes and broader cancellation evidence. Read-only transactions do not replace least-privilege roles. A restricted SELECT console is implemented; arbitrary SQL and writes are not.

Exit: independently verified full-release workflows on both selected server matrices, with unsupported capabilities explicit. An experimental read slice does not close this phase.

### 3. PostgreSQL vertical slice

- Connect with verified TLS and optional credential persistence.
- Discover schemas/metadata and add SQL console/script scope with dialect-aware parsing.
- Validate dedicated transactions, bounded browsing/export, cancellation and primary-key staged edits.
- Exercise real-server failure/reconnect, credential failures and uncertain writes.

Exit: end-to-end acceptance scenarios on the selected PostgreSQL matrix. Reuse proven boundaries without assuming MySQL semantics transfer.

### 4. Redis

- Add incremental SCAN browser with binary-safe key identity, duplicate/mutation handling, type and TTL display.
- Provide bounded string/hash/list/set/sorted-set/stream inspection and explicit truncation.
- Add guarded writes/deletes/TTL operations and command console with a defined restricted policy for blocking/admin commands.
- Validate real-server authentication/TLS and supported topology boundaries.

Exit: binary keys round-trip, large/mutating keyspaces remain bounded, TTL states are correct, and restricted commands cannot bypass policy through alternate input paths. No SQL transaction/grid parity is claimed.

Reason: Redis requires a different data and safety model, not a SQL driver wrapper.

### 5. ACP and hardening

- Add optional external-agent ACP lifecycle, transcript/suggestion review, explicit insertion, and opt-in metadata context.
- Exclude direct provider adapters, application BYOK, autonomous execution, and automatic row sharing.
- Explain that the agent owns auth/billing and runs under its own privileges, not a Dalan sandbox.
- Complete security failure tests, keyboard/focus checks, both theme contrast checks, actual accessibility audit, measured performance budgets, export robustness, and packaging/signing validation.

Exit: AI consent scenarios and cross-engine release gates pass; security and accessibility gaps receive explicit disposition. Core non-AI use works when the agent is unavailable.

Reason: AI is optional and must not weaken the core tool or its execution boundary.

### 6. First release

Release only the verified PostgreSQL, MySQL, MariaDB, and Redis scope on the declared macOS architecture/OS matrix. Publish known limitations and unsupported server capabilities. Resolve Intel inclusion explicitly rather than implying support. Do not claim MongoDB, Linux, Windows, or measured quality results before validation.

Exit: [product-plan release gates](product-plan.md#proposed-release-gates) and [testing](testing.md) are satisfied, distribution is verified, and documentation describes actual shipped behavior rather than this proposal.

Reason: a narrow truthful support contract is more useful than an incomplete compatibility list.

### 7. MongoDB after first release

- Design document/collection browsing, query/filter semantics, BSON fidelity, indexes, bounded results/export, and guarded writes as a separate capability set.
- Validate the user's MongoDB Compass replacement needs rather than promising full Compass parity.
- Select MongoDB server versions/authentication/topologies and test against real servers.

Exit: a separately reviewed scope and acceptance suite; no retroactive inclusion in first-release promises.

Reason: the user explicitly placed MongoDB after the first release.

### 8. Linux

Validate GPUI platform support, packaging, credential storage, input methods, clipboard, fonts, TLS, accessibility, and measured performance on a declared distribution/window-system matrix. Adapt conventions without changing engine safety semantics.

Exit: declared Linux configurations pass platform and cross-engine gates.

Reason: Linux is the second platform, not an assumed byproduct of Rust portability.

### 9. Windows

Validate GPUI readiness, credential storage, installer/signing, native input conventions, fonts, accessibility, TLS, and cross-engine behavior on a declared Windows matrix.

Exit: declared Windows configurations pass the same safety gates and Windows-specific validation.

Reason: Windows is third and depends on verified toolkit/platform readiness.

## Track and dependency notes

The default sequence is foundation -> MySQL/MariaDB -> PostgreSQL -> Redis -> ACP and hardening -> first release -> MongoDB -> Linux -> Windows. After the first release, MongoDB and Linux may run as separate tracks if resources permit; MongoDB must remain post-first-release and Linux must remain ahead of Windows in platform ordering. This is not a promise of parallel staffing.

Other drivers, named HTTP query gateways, expanded Redis topologies, administration tooling and richer AI require new scope decisions. SSH and CONNECT transports exist experimentally but need their outstanding live verification gates.

## Decisions needed before commitment

License and visual identity; minimum macOS and Intel gate; server matrix; broader SQL parser coverage/completion scope; GPUI pre-1.0 upgrade policy and accessibility findings; Redis command and payload bounds; trusted-CA/SSH/HTTPS positive-path evidence; post-release track allocation. The app name `Dalan` is confirmed. Performance numbers in the product plan are proposed measurement targets, not existing results.

[Overview](../README.md) · [Product plan](product-plan.md) · [UX](ux.md) · [Design](../DESIGN.md) · [Architecture](architecture.md) · [Drivers](drivers.md) · [ACP](acp.md) · [Security](security.md) · [Development](development.md) · [Testing](testing.md) · [ADRs](adr/README.md)

## Completed multi-tab / read-only-console slice

Qualified table deduplication, 32-tab capacity including uniquely numbered consoles, independent tab models/requests/filter/grid-scroll/results, neighboring close selection and keyboard navigation are implemented. The 28 px Carbonfox strip and seventh explorer query icon remain compact database chrome, not DataGrip styling or Files/Git UI. The native multiline editor supports selection/IME/clipboard/bounded undo; Run executes selected text or the whole draft, never automatically a cursor statement.

The console backend is a guarded MySQL-dialect SELECT AST subset with fresh read-only sessions, curated functions, finite complexity/result bounds, configured server/client query limits (20 seconds by default) and local cancellation. No write, session/autocommit control, persistent transaction, query pagination or full-query export is shipped. Drafts/results remain in memory with nonempty-draft close confirmation. See [guide](query-consoles.md), [ADR](adr/0004-workspace-tabs-and-read-only-consoles.md) and [testing](testing.md#workspace-tabs-and-query-consoles).

Next scope remains column resizing/cell inspection-copy, syntax highlighting, completion and explicitly designed history/retention; saved SQL scripts and current-statement/script execution need separate semantics. Dedicated transaction affinity, guarded writes and streaming export remain later phases. Historical `cf24e48` passed all five jobs in run 37215973701; the actual new main run and native review are still gates, not presumed green.

## Implemented source-manager subset and exit gates

Tabbed source setup, real engine/authentication choices, Default/Unix Socket/credential-free URL-only, applied Options, searchable schema visibility, four TLS modes and paired PEM identity paths are implemented. Reusable SSH metadata has its own manager/repository with strict trust, opt-in local OpenSSH config, references resolved to current settings and cancellable remote-true Test. This is not exact DataGrip property parity, SOCKS, SSH password authentication, PuTTY keys or Java truststores. The compact Carbonfox visual direction and database-only scope are unchanged.

Exit gates: final owner-run confirmation of expected 105 headless/141 UI/four Python and 29 unique live cases, current hosted CI, native titlebar/window/keyboard/accessibility review, positive mTLS and live manager remote-true verification. Positive trusted HTTPS proxy remains open. Historical `9ecae4c` passed all five jobs in run 37255792022, not the new worktree. Keep column/cell tools, SQL editor improvements, PostgreSQL/Redis and broader sessions/writes in their existing sequence; source-management completion is not a release claim. See [guide](source-management.md) and [testing](testing.md#source-manager-redesign).

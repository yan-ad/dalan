# dalan roadmap

Status: sequencing proposal, not a delivery commitment. No phase is implemented or complete. There are no invented dates or staffing assumptions. The user requires Rust + GPUI, macOS first, first-release PostgreSQL/MySQL/MariaDB/Redis, MongoDB after first release, Linux second, and Windows third. Other scope and phase boundaries are proposals.

## Proposed sequence and exit criteria

### 1. Foundation

- Establish Rust workspace, pinned GPUI revision, macOS Apple Silicon packaging path, configuration model, and engine capability interfaces.
- Prototype the compact shell and keyboard/focus model: left data sources, center tabs, bottom results/output/sessions, optional right AI.
- Spike GPUI rendering, accessibility, clipboard, native menus, and virtualized grids. Do not assume framework support guarantees screen-reader compatibility.
- Spike SQL parsing/completion and Redis binary argument representation before committing to libraries.
- Define credential boundaries, cancellation/outcome states, test fixtures, minimum macOS target, Intel gate, and supported server matrix.

Exit: documented architecture/ADRs, working internal shell proof of concept, explicit risk list, and reproducible performance/accessibility test approach. A prototype is not a finished UI.

Reason: session semantics and platform constraints are harder to change after drivers depend on them.

### 2. PostgreSQL vertical slice

- Connect with validated TLS and Keychain-backed optional persistence.
- Browse metadata; open a SQL console; correctly split supported scripts; execute with explicit scope.
- Add dedicated transaction sessions, bounded result browsing/export, cancellation states, and primary-key-only staged edits with conflict handling.
- Exercise real-server success, failure, reconnect, credential failure, and uncertain-write outcomes.

Exit: end-to-end acceptance scenarios pass on the selected PostgreSQL matrix; unsupported cases are explicit rather than silently approximated.

Reason: one full workflow establishes abstractions with actual database behavior.

### 3. MySQL and MariaDB

- Implement and test both engines, not a shared label that implies unverified parity.
- Cover metadata/capability differences, supported delimiter/stored-program parsing, autocommit, DDL implicit commits, charset behavior, TLS/authentication, and storage-engine transaction limits.
- Reuse the shell while verifying result paging, export, staged edits, and transaction affinity independently.

Exit: real-server coverage for each selected engine/version and all applicable SQL acceptance scenarios; dialect gaps documented.

Reason: familiar SQL syntax does not make session or transaction behavior interchangeable.

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
- Explain that the agent owns auth/billing and runs under its own privileges, not a dalan sandbox.
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

The default sequence is foundation -> PostgreSQL -> MySQL/MariaDB -> Redis -> ACP and hardening -> first release -> MongoDB -> Linux -> Windows. After the first release, MongoDB and Linux may run as separate tracks if resources permit; MongoDB must remain post-first-release and Linux must remain ahead of Windows in platform ordering. This is not a promise of parallel staffing.

Other drivers, SSH tunnels, expanded Redis topology support, administration tooling, and richer AI actions require new scope decisions. SSH tunnels are proposed deferred from the MVP; an externally managed tunnel is the initial workaround, subject to user-workflow validation.

## Decisions needed before commitment

License and visual identity; minimum macOS and Intel gate; server matrix; SQL parser/completion spike result; GPUI pre-1.0 upgrade policy and accessibility findings; Redis command and payload bounds; SSH defer confirmation; post-release track allocation. The app name `dalan` is confirmed. Performance numbers in the product plan are proposed measurement targets, not existing results.

[Overview](../README.md) · [Product plan](product-plan.md) · [UX](ux.md) · [Design](../DESIGN.md) · [Architecture](architecture.md) · [Drivers](drivers.md) · [ACP](acp.md) · [Security](security.md) · [Development](development.md) · [Testing](testing.md) · [ADRs](adr/README.md)

# dalan product plan

Status: planning proposal. No application functionality or finished UI is implemented or claimed by this document. Explicit user requirements below are constraints; other choices are proposals pending validation. The confirmed app name is `dalan`, meaning “ways” in Javanese.

## Intent and confirmed constraints

Build a Rust + GPUI native daily database tool that can replace the user's DataGrip workflow and, after the first release, MongoDB Compass workflow. The visual direction is Zed-like restraint with DataGrip-like information architecture and interaction patterns, not a copy of either product.

- macOS first, prioritizing Apple Silicon. Intel support is a release gate decision still to be made. Linux second; Windows third.
- First release: PostgreSQL, MySQL, MariaDB, and Redis. MongoDB is explicitly after the first release. Other drivers follow later.
- Compact, keyboard-first IDE: left data-source tree, central tabbed consoles and table views, bottom results/output/sessions, optional right AI panel.
- AI integration uses Agent Client Protocol (ACP) only. No direct provider adapters or application BYOK settings. An external agent owns authentication and billing and may itself use API keys.
- Initial AI is suggestion-only. Users review or insert suggestions and explicitly execute database operations through normal application controls. There is no autonomous database execution.

Reason: constrain the first release to the daily workflows and supported integrations the user actually requested.

## Intended first-release workflow

A user configures a connection, securely stores credentials if requested, browses metadata, opens a console or data view, executes an operation deliberately, inspects results, and exports or stages supported edits. Connection identity and operation state stay visible throughout. SQL and Redis share application chrome, not false feature parity.

Reason: a familiar layout should not hide engine-specific semantics or dangerous operations.

## Proposed scope by engine

Every entry is planned, not implemented. Supported server versions and edge cases must be selected and recorded in [drivers](drivers.md) before release.

| Capability | PostgreSQL | MySQL / MariaDB | Redis |
| --- | --- | --- | --- |
| Connection and discovery | Databases/schemas, tables, columns, keys, indexes, views; refresh and explicit failure states | Databases, tables, columns, keys, indexes, views; distinguish server capabilities and dialects | Selected logical database where supported, key namespace browser, type and TTL; no SQL schema fiction |
| Console execution | SQL statements and scripts split using dialect-aware parsing; explicit selection/current statement/script actions | SQL statements and scripts with dialect-aware splitting, including delimiter and stored-program cases within the supported grammar | Command console with binary-safe argument model; allowlisted normal commands, explicit restrictions on blocking/admin commands |
| Sessions and transactions | Dedicated pinned sessions for explicit transactions; visible begin/commit/rollback and disconnect behavior | Dedicated pinned sessions; show autocommit and engine-specific transaction limitations, including implicit DDL commits | No SQL transaction controls; MULTI/EXEC workflow is proposed deferred rather than simulated |
| Read views | Bounded pages, explicit ordering and refresh semantics; streaming where supported without implying a snapshot | Bounded pages with ordering and refresh semantics; engine-specific behavior documented | Incremental SCAN, duplicates tolerated, no full-keyspace KEYS; bounded inspection of string, hash, list, set, sorted set, and stream values |
| Export | Export selected/loaded results or explicitly requested full query stream with visible scope and cancellation | Same export contract, tested separately for both engines | Export explicitly fetched keys/values only, binary-safe encoding and bounds stated; no implied whole-database export |
| Writes | Staged grid edits only for rows with an unambiguous primary key; parameterized operations, review, affected-row checks | Same primary-key-only rule; honor storage-engine transaction support and concurrent-change checks | Guarded type-specific write/delete and TTL changes with key identity, preview, and confirmation; no SQL grid-edit emulation |
| Safety and limits | Read-only connection mode, destructive-action review, configurable result limits | Same, with capability differences visible | TTL distinguishes expiry, persistent keys, and missing keys; binary key round-trip; explicit size limits and truncation; dangerous/blocking/admin commands denied or guarded by an explicit limited policy |

Proposed exclusions: SSH tunnels in the MVP, database administration suites, ER diagram editors, routine debugging, collaborative editing, autonomous AI, direct AI provider configuration, SQL grid writes without primary keys, Redis Cluster/Sentinel topology management unless later admitted by the server-matrix decision. SSH tunnels are explicitly proposed deferred; required workflows would initially use an externally managed tunnel.

Reason: bounded data inspection and explicit execution are achievable release units; broad administration features are not prerequisites for a daily client.

### SQL execution contract

Do not split scripts on semicolons alone. A parsing spike must establish the supported handling of strings, comments, PostgreSQL dollar quoting, and MySQL/MariaDB delimiter directives and stored programs. If an input is unsupported, report that fact before partial execution rather than guessing. Show which statements ran, failed, remain pending, or have uncertain outcomes. A script is not automatically atomic. Dedicated transaction sessions cannot silently migrate to a different pooled connection.

Reason: incorrect statement boundaries or session reuse can change the meaning of a user's operation.

### Redis execution contract

SCAN cursors are incremental, not stable pages or exact counts. Deduplicate display where practical and describe concurrent-mutation effects. Key display must preserve raw bytes separately from a readable escaped representation. Values and collection members are bounded by bytes and item count; truncation is explicit. Use bounded operations such as HSCAN/SSCAN/ZSCAN and bounded list/stream ranges as appropriate, without claiming a consistent snapshot. A console tokenizer must preserve byte arguments and must not interpret display strings as key identity. Blocking commands, scripting, configuration, flush, shutdown, and other administrative commands need a documented allow/deny policy; the proposed initial default denies unsafe classes rather than presenting an unrestricted console.

Reason: keyspaces and payloads can be large or binary, and an innocent-looking command can block a session or destroy data.

## Proposed AI contract

The optional panel connects to an external ACP agent and exposes connection/permission state. Metadata context is opt-in for a clearly identified connection and scope. Query results and row data are never included automatically. Explicit data sharing, if later supported, needs a separate review showing exactly what leaves the application. Secrets are excluded from application-built context.

Suggested SQL or Redis commands arrive as text for review or insertion. Insertion does not execute. The application does not grant an agent a database execution tool in the first release. The agent runs with its own host/process privileges, not an application sandbox; disabling application execution tools cannot prevent an independently configured agent from accessing databases or files. Explain this boundary before launch and disclose agent launch arguments without exposing secrets.

Reason: useful assistance must not be confused with a security boundary the application does not provide.

## Proposed release gates

- Test against real supported PostgreSQL, MySQL, MariaDB, and Redis servers, including authenticated and TLS configurations. Mocks alone are insufficient. Define exact server versions and features before claiming support.
- Validate TLS peer and hostname verification by default; any insecure override is explicit, scoped, and visibly marked. Exercise certificate failure and reconnect behavior.
- Test macOS Keychain save/read/update/delete failures. No plaintext credential fallback in configuration, logs, crash reports, or exported settings. Session-only use is an explicit alternative, not silent persistence.
- Exercise cancellation, timeout, disconnect, and reconnect during reads and writes. Retain an **unknown outcome** when the server may have applied a write; do not label it rolled back or retry it automatically.
- Verify transaction session affinity, staged edit predicates and affected-row handling, binary Redis keys, TTL cases, collection bounds, SCAN mutation behavior, and command restrictions.
- Measure UI responsiveness with reproducible hardware/data fixtures. Proposed targets, not results: common local key-to-visible-response p95 <= 50 ms; interactive scrolling should target a 16.7 ms frame budget on a 60 Hz reference display; initial shell usable <= 2 seconds on the chosen Apple Silicon fixture. Record reference hardware, sample sizes, cold/warm conditions, and failures before adopting these as gates. Remote query latency is reported separately.
- Test dark and light themes for WCAG AA text contrast, keyboard reachability, visible focus, and non-color status cues. Audit GPUI/platform accessibility APIs and actual assistive-technology behavior. No screen-reader support claim until tested; gaps require remediation or an explicit release decision, not assumed support.
- Resolve platform support, packaging/signing, license, parser spike, and GPUI pre-1.0 compatibility risk. Ship only after supported paths pass [testing](testing.md) and security checks.

Reason: release confidence must come from measured behavior and failure-path tests, not planned feature lists.

## Acceptance scenarios

1. **SQL script boundaries:** A script containing quoted semicolons, comments, PostgreSQL dollar-quoted content, or a supported MySQL/MariaDB delimiter construct executes the intended statements in order. Unsupported syntax is rejected clearly; partial execution is visible.
2. **Transaction affinity:** Start an explicit transaction, modify a row, open another console, and roll back in the original console. The transaction retains its session; the other console does not inherit it. Loss of the original session produces an explicit state requiring reconciliation.
3. **Staged edits:** Editing a primary-key row changes only the draft until Apply. Review shows connection, table, keys, and values. Concurrent change or unexpected affected-row count produces a conflict, not silent success. A view without an eligible primary key is read-only.
4. **Cancellation uncertainty:** Cancel a write while dropping the connection after send. Show unknown outcome, preserve the operation record, and offer inspection/reconciliation rather than automatic retry.
5. **Redis safety:** Browse a large changing keyspace without KEYS, inspect a binary key and all six supported value types within bounds, distinguish TTL -1 and -2, and reject a restricted blocking/admin command. Delete review identifies the exact raw key safely.
6. **Credential failure:** With Keychain inaccessible, persistence fails visibly and no plaintext credential appears on disk. The user may explicitly continue for the current session.
7. **AI consent:** An ACP suggestion does not run. Metadata is not shared until opted in, rows are not automatically shared, and insertion still requires a normal explicit execute action. Agent launch explains independent privileges.
8. **Keyboard and appearance:** Complete connect, browse, query, inspect, export, and staged edit review without a pointer. Focus stays visible in both tested themes; canceling an edit does not cancel a query accidentally.

## Open decisions

- License, visual identity, and distribution model.
- Minimum macOS version and Intel inclusion or explicit exclusion for the first release.
- Supported server-version matrix, TLS/authentication methods, and Redis standalone/managed-service boundaries.
- SSH tunnels: proposed deferred; validate whether that blocks the user's daily workflow.
- SQL completion/parsing implementation: spike libraries, dialect coverage, latency, and licensing before choosing.
- GPUI pre-1.0 risk: pin a known revision, validate accessibility and platform APIs, budget upgrades, and define fallback/release-blocking criteria.
- Exact Redis allowed commands, editable types, size budgets, and persistence of local query history.
- Whether MongoDB and Linux work can overlap after first release, based on staffing and platform risk.

## Related planning documents

[Project overview](../README.md) · [Roadmap](roadmap.md) · [UX](ux.md) · [Design direction](../DESIGN.md) · [Architecture](architecture.md) · [Drivers](drivers.md) · [ACP](acp.md) · [Security](security.md) · [Development](development.md) · [Testing](testing.md) · [Decisions](adr/README.md)

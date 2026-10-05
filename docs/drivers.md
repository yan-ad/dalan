# Driver design and compatibility

Status: MySQL/MariaDB have experimental read-only adapters. PostgreSQL and Redis remain planned, with MongoDB after the first release. MySQL/MariaDB now come first by explicit user direction, superseding the earlier PostgreSQL-first sequence. Experimental is not a general support or release claim.

## Backend choices

| Engine | Library | State |
| --- | --- | --- |
| MySQL | mysql_async 0.37.1 | Implemented experimental reads |
| MariaDB | mysql_async 0.37.1 | Shared wire backend, distinct engine identity |
| PostgreSQL | SQLx PostgreSQL driver | Proposed, no adapter |
| Redis | redis-rs | Proposed, no adapter |
| MongoDB | Official MongoDB Rust driver, evaluate later | Post-first-release |
| Other | Not chosen | Requires a concrete workflow and compatibility gate |

mysql_async was chosen instead of SQLx for its native async backend and preservation of database TLS identity through relays. Database operations run on a managed two-worker Tokio runtime, with bounded results, request-generation checks and cancellation before GPUI foreground updates. Reverify versions, features, trust roots and licenses when adding future backends. See [architecture](architecture.md).

## Implemented MySQL/MariaDB contract

### Current generated browse clause compiler

Production UI sends WHERE/ORDER BY drafts through pinned sqlparser 0.62.0/MySqlDialect validation and compilation, not a full user statement. WHERE allows metadata-validated unqualified/backtick columns, literal comparisons (`= != <> < <= > >=`), AND/OR/NOT, LIKE, BETWEEN, IN up to 256 literals and null checks. Native Int/UInt and decimal bytes remain exact bound values; no float rounding is introduced. Functions/subqueries/qualified identifiers/comments/statements/extra clauses are rejected outside quoted literals. Each fragment has 16 KiB, 1,024 token, 24 nesting and 128 operator budgets. ORDER BY admits at most eight unique real columns with ASC/DESC, rejecting ordinals, expressions/functions and NULLS directives.

Compilation emits a fixed SELECT using quoted metadata identifiers and bound literals/LIMIT/OFFSET. Syntax/budget validation precedes connection for direct callers; actual metadata membership requires discovery. Production UI validates against current page columns before credentials/network. Empty clauses use available PK order; explicit clauses append ascending PK columns not already included, without snapshot guarantees. Legacy internal typed filter/sort requests remain supported for older callers, not exposed as production toolbar controls. This policy is narrower than console SELECT validation and does not allow console functions/subqueries. See [table-browser contract](table-browser.md#where-and-order-by). Existing row/cell/page/packet bounds and typed value/CSV scope remain unchanged.

Test, connect/discovery, columns, generated read-only table browsing and restricted SELECT console execution are implemented. There is no unrestricted arbitrary SQL execution, write or transaction-control UI; loaded-page CSV export lives in the app, not the driver. Optional database None uses `SHOW DATABASES`; selecting a database discovers tables. Views are listed but browsing accepts BASE TABLE only. Read-only transactions depend on server capabilities and do not replace least-privilege roles.

Pages default to 100 rows; Options applies 1–200 rows to browse and console results, with a fixed backend maximum of 200. Limits are 4 KiB per displayed cell, 2 MiB retained page data, 512 columns, 1,000 databases/tables and 8 MiB per protocol packet. The packet limit is real but not an absolute process-memory bound. Primary-key ordering is used when available; otherwise pagination can be unstable. Offset paging is never a cross-page snapshot. Next offset counts rows actually retained. Binary data is hex, MySQL JSON text, and decimal/large integer values strings.

Filters bind values and quote verified identifiers. Operators are Contains, Equals, NotEquals, greater than, less than, is null and is not null. Contains escapes LIKE metacharacters with `!`. Stale retained results are labeled and cannot be paginated until refreshed.

Direct TCP, system SSH and anonymous HTTP/HTTPS CONNECT transports are implemented. VerifyIdentity is the default database TLS mode with optional CA file; no automatic insecure retry. The original database hostname survives a relay. SSH optionally selects an absolute existing known-host file, with strict checking and `GlobalKnownHostsFile=/dev/null` to make that file authoritative; absent/None keeps OpenSSH user/system known-host defaults. CONNECT is a wire-protocol proxy, not a database-query HTTP gateway; a gateway requires a named API and is not implemented. See [MySQL sources](mysql-sources.md) for source setup, credentials, SSH defaults and local relay risks.

### Complete metadata snapshots

`discover_catalog` returns `CatalogSnapshot { databases: Vec<DatabaseCatalog> }`, `DatabaseCatalog { name: String, tables: Vec<TableInfo> }`, and `TableInfo { name: String, kind: String }`. Database None means all account-visible schemas; a configured database restricts scope. It is a table/view-name and kind index, not columns, indexes, DDL or row data. Browse still discovers columns separately and rejects view execution.

Complete discovery serializes metadata-only SQL on one owned native-protocol session and transport/tunnel, without per-database fan-out. Limits are 1,000 databases, 1,000 objects per database, 50,000 tables/views total (databases counted separately), 120 seconds overall, configured Connect timeout (1–60 seconds, default 10) and configured metadata Query timeout (1–120 seconds, default 20) per query step. Failure discards the incomplete discovery result; app persistence retains the prior snapshot. Startup reads the app cache without driver network or Keychain work. Successful Save and explicit source Refresh can fetch complete metadata, never automatically browse data.

The app, not the network driver, owns `rusqlite 0.40.2` with only bundled SQLite enabled, 8 MiB encoded metadata and 128 MiB cache-file bounds, atomic publication and connection-identity/ticket checks. It does not add a user-facing SQLite driver, daemon, libSQL or cloud backend. See [architecture](architecture.md#persistent-metadata-cache-implemented) and [license notices](../THIRD_PARTY_NOTICES.md).

Historical metadata-revision live scripts were rerun successfully: 7 direct/CONNECT/authentication, 10 TLS and 6 SSH cases, with full-catalog assertions across the supported fixture routes. Test-only serialized SSH configuration makes the fixture deterministic; production runtime/transport configuration is unchanged. Positive system-trusted HTTPS proxy and actual private VPN/edge connectivity remain unverified.

## Compatibility evidence

| Case | Evidence |
| --- | --- |
| MySQL 8.4.11 | Live disposable fixture verified |
| MariaDB 11.4.13 | Same fixture verified independently |
| Direct TCP / anonymous HTTP CONNECT | Live success verified |
| Seven filters and fixture values | Live verified on both servers |
| Views rejected for browsing | Live verified |
| Default TLS rejects untrusted server | Live verified |
| Trusted custom CA database TLS | Direct and HTTP CONNECT reads passed; wrong-hostname/untrusted-CA rejection passed |
| SSH actual transport | Reads, wrong-host-key and wrong-identity rejection passed for both engines; selected trust path with spaces verified |
| HTTPS CONNECT | Untrusted proxy rejection passed; trusted system-CA proxy success unverified |
| Native macOS Keychain generated round-trip | One passed with generated-item cleanup |
| Other server versions/plugins/topologies | Not established |

Six live database smoke tests, ten secure-transport tests and six actual SSH-transport tests passed. The historical source-slice suites also passed 37 default headless Rust tests, 27 simulated GPUI tests and four Python bundle-helper tests. These fixtures are not blanket authentication, TLS or managed-service compatibility claims. That historical run did not establish Unix-socket or client-certificate support. Current Unix-socket implementation and validation are described below; positive mTLS, cloud-specific authentication and proxy authentication remain unverified or unsupported.

Legacy sorting uses typed column/direction metadata; current ORDER BY fragments use the compiler above, never raw execution. Identifiers are validated and quoted; filter values remain bound. Available primary-key columns break ties, otherwise equal sort values can produce unstable pages. Sorting retains filters and resets the UI offset. Live sorting checks were rerun across the 22 direct/CONNECT/TLS/SSH fixture cases. Loaded CSV is an app-owned captured-page operation, not a full-query or whole-table export. Latest expected test counts and pending final gates are in [testing](testing.md).

## Future engine rules

PostgreSQL must preserve schemas, qualified identifiers, native types and session search path. Investigate wire cancellation rather than assuming a backend exposes all controls. EXPLAIN ANALYZE executes operations. Choose and lock runtime/TLS features when implementing SQLx, not by copying old examples.

MySQL/MariaDB still need wider authentication, SQL mode, generated-column, charset and storage-engine coverage before full release. DDL can implicitly commit, and nontransactional tables cannot promise rollback. Shared protocol does not imply interchangeable capabilities.

Redis needs binary-safe keys/arguments, native RESP values, type/TTL inspection and bounded reads. SCAN can duplicate keys and is not a snapshot or exact count; no browser KEYS. TTL -1 means persistent and -2 absent, with inspection races. Normal multiplexed requests and blocking commands need separate admission/lifecycle policies. Retries of mutations are not automatically safe. Cluster/Sentinel remain deferred unless separately admitted.

## Adding an adapter

1. Specify the workflow and engine capability table.
2. Lock a compatible dependency and review licenses.
3. Implement bounded read-only behavior before writes.
4. Test real servers for auth/TLS, types, metadata, error/cancel and session behavior.
5. Add guarded writes only with immutable identity, approvals and uncertainty tests.
6. Publish tested scope and exclusions, not mocked success or catalog-only compatibility.

[MySQL sources](mysql-sources.md) · [Architecture](architecture.md) · [Product plan](product-plan.md) · [Testing](testing.md)

## Read-only query-console adapter (implemented)

The MySQL/MariaDB `query` path uses pinned sqlparser 0.62.0 with visitor and MySqlDialect. It accepts exactly one supported SELECT query, including nested SELECT, CTE and UNION plus curated unqualified built-ins. It rejects write/session/admin statements, SHOW/EXPLAIN, SELECT INTO/OUTFILE, variables, locks, executable comments/optimizer hints and unknown/stored/UDF/qualified functions before opening a connection. This is an allowlisted subset, not complete dialect support or authorization. See the [precise guide](query-consoles.md#accepted-sql-and-rejections).

Validation bounds are 64 KiB SQL, 4,096 meaningful tokens, 32 parenthesis/CASE nesting, 256 operators/recursive constructs and a separate 256-node set-body guard. Each Run opens a fresh physical connection, sets MySQL MAX_EXECUTION_TIME in milliseconds or MariaDB max_statement_time in seconds from the configured Query timeout, then starts a read-only transaction. The same configured client deadline bounds local work (20 seconds by default). Cancellation disposes the socket/owned relay; there is no server KILL acknowledgment or persistent transaction/session UI.

User SQL is submitted without LIMIT/OFFSET rewriting. Results use typed immutable `TablePage` snapshots, up to 512 columns and the configured Page size (default 100; backend 1 through 200), existing cell/preview-byte bounds, `has_more` plus warnings and `next_offset = None`. Query headers cannot sort and table filters do not mutate query results. Loaded CSV remains loaded-only and guarded against busy/stale/truncated pages. Failure retains the prior successful SQL/elapsed/warnings with an explicit stale notice, not new SQL mislabeled over old rows.

Least-privilege SELECT roles and trusted server views are essential: the client cannot inspect code inside view definitions or prove server-side functions harmless. Consoles share the source profile, transports/TLS, on-demand credential policy and session credential map; No Auth bypasses Keychain as described below. Tabs/SQL/results are not persisted. Future PostgreSQL/Redis drivers require their own dialect/policy and session semantics, not this MySQL allowlist reused blindly.

## Source-manager backend contract

`SourceProfile` keeps version 1 compatibility via defaults for endpoint mode, authentication, schema selection, Options, client-certificate/key paths and reusable SSH UUID reference. Default resolves host/port/database; URL-only resolves a credential-free mysql/mariadb URL (optional jdbc: prefix), rejecting userinfo, query parameters and fragments without echoing input. Unix sockets require Unix, Direct and explicitly Disabled TLS, never silent fallback. No Auth supplies driver username/password None regardless of supplied session secrets; credential policy belongs to the app.

Four TLS modes are wired: VerifyIdentity validates chain/hostname; VerifyCA skips hostname validation; Required encrypts without chain/hostname validation; Disabled has no database TLS. Paired PEM client-identity paths are validated and driver-wired, but live positive mTLS is unverified and encrypted TLS keys have no supported passphrase path. Database Rustls uses built-in webpki roots, not macOS Keychain; HTTPS proxy trust separately uses native roots. No Java/IDE truststore or SOCKS support is implied.

App-level reusable SSH references must resolve to current settings before driver calls. Custom inline settings remain compatible. Parse config defaults false (`-F /dev/null`); explicit opt-in can run configured local ProxyCommand/Match exec and still requires strict known-host verification. Positive driver forwarding is distinct from manager remote `true` testing. See [source management](source-management.md) and [current test gate](testing.md#source-manager-redesign); earlier 23-case console/transport runs remain historical, not the expanded 29-case run.

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

Test, connect/discovery, columns and generated read-only table browsing are implemented. There is no arbitrary SQL execution, write or transaction-control UI; loaded-page CSV export lives in the app, not the driver. Optional database None uses `SHOW DATABASES`; selecting a database discovers tables. Views are listed but browsing accepts BASE TABLE only. Read-only transactions depend on server capabilities and do not replace least-privilege roles.

Pages are 100 rows in the UI, capped at 200 in the backend. Limits are 4 KiB per displayed cell, 2 MiB retained page data, 512 columns, 1,000 databases/tables and 8 MiB per protocol packet. The packet limit is real but not an absolute process-memory bound. Primary-key ordering is used when available; otherwise pagination can be unstable. Offset paging is never a cross-page snapshot. Next offset counts rows actually retained. Binary data is hex, MySQL JSON text, and decimal/large integer values strings.

Filters bind values and quote verified identifiers. Operators are Contains, Equals, NotEquals, greater than, less than, is null and is not null. Contains escapes LIKE metacharacters with `!`. Stale retained results are labeled and cannot be paginated until refreshed.

Direct TCP, system SSH and anonymous HTTP/HTTPS CONNECT transports are implemented. VerifyIdentity is the default database TLS mode with optional CA file; no automatic insecure retry. The original database hostname survives a relay. SSH optionally selects an absolute existing known-host file, with strict checking and `GlobalKnownHostsFile=/dev/null` to make that file authoritative; absent/None keeps OpenSSH user/system known-host defaults. CONNECT is a wire-protocol proxy, not a database-query HTTP gateway; a gateway requires a named API and is not implemented. See [MySQL sources](mysql-sources.md) for source setup, credentials, SSH defaults and local relay risks.

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

Six live database smoke tests, ten secure-transport tests and six actual SSH-transport tests passed. The historical source-slice suites also passed 37 default headless Rust tests, 27 simulated GPUI tests and four Python bundle-helper tests. These fixtures are not blanket authentication, TLS or managed-service compatibility claims. Unix sockets, client certificates, cloud-specific authentication and proxy authentication are not established support contracts.

Sorting uses typed column/direction metadata, not raw SQL fragments. Identifiers are validated and quoted; filter values remain bound. Available primary-key columns break ties, otherwise equal sort values can produce unstable pages. Sorting retains filters and resets the UI offset. Live sorting checks were rerun across the 22 direct/CONNECT/TLS/SSH fixture cases. Loaded CSV is an app-owned captured-page operation, not a full-query or whole-table export. Latest expected test counts and pending final gates are in [testing](testing.md).

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

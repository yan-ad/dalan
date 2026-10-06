# Native drivers and support matrix

Status: experimental read-only implementations, not release certification. Dalan now exposes five built-in engines: MySQL, MariaDB, PostgreSQL, MongoDB and Redis. PostgreSQL/MongoDB/Redis are native protocol adapters. A separate [optional JDBC bridge and curated installer](jdbc-drivers.md) is now implemented; it requires an explicitly supplied Java 17+ executable and trusted installed JARs, with vendor/server qualification unrun. A JDBC-shaped native URL prefix alone does not select that bridge or install Java. Broader external workers remain later work.

## Bundled library versions and selection

The Drivers page distinguishes **driver library versions** from **database server versions**. It offers **Latest bundled** and an exact pin for each available backend, persisted in `dalan.config` through Apply/OK. Latest means the newest backend **included in this Dalan build**, not an upstream download. There is currently **one bundled version per engine**, so the two choices execute the same backend. Older alternate native backends and native package installation are not implemented; JDBC versions are managed separately by the opt-in curated installer, not these native preferences; adding another version requires an independently compiled, routed and tested adapter, not just another dropdown entry.

| Engine | Bundled implementation | Exact selection ID | Server-version qualification |
| --- | --- | --- | --- |
| MySQL | `mysql_async` 0.37.1 | `mysql-async-0.37.1` | No certified range; actual-server matrix pending |
| MariaDB | `mysql_async` 0.37.1 | `mysql-async-0.37.1` (separate engine preference) | No certified range; actual-server matrix pending |
| PostgreSQL | `tokio-postgres` 0.7.18 / `postgres-native-tls` 0.5.3 | `tokio-postgres-0.7.18` | PostgreSQL v3 wire; server-version matrix pending |
| MongoDB | Official `mongodb` 3.7.0 | `mongodb-3.7.0` | Official compatibility must be qualified against actual servers |
| Redis | Dalan bounded RESP2 codec, revision 1 | `dalan-resp2-1` | RESP2; no Cluster/RESP3/streams |

Missing preferences resolve to Latest bundled. Unknown engines/versions are rejected before backend use and are not silently substituted. Changing theme preserves driver preferences; driver Apply preserves appearance and does not read credentials, rewrite sources or reconnect existing operations. Cancel discards unapplied choices. The page uses Kit tables for exact library/status and capability/transport/TLS details, with correctly spelled centered labels. Provider SVGs are decoded as cached color images, not theme-tinted masks; original brand fills are unchanged across light/dark modes.

## Current support

| Boundary | MySQL / MariaDB | PostgreSQL | MongoDB | Redis |
| --- | --- | --- | --- | --- |
| Backend | `mysql_async` | `tokio-postgres` + `postgres-native-tls` | Official Rust driver 3.7.0, rustls, BSON 2 | Dalan bounded RESP2 codec |
| Transport | Direct TCP, Unix socket, strict SSH, anonymous HTTP/HTTPS CONNECT | Direct TCP, Unix socket, shared strict SSH and HTTP/HTTPS CONNECT relay | Direct TCP only | Direct TCP only |
| TLS | Disabled, Required, VerifyCa, VerifyIdentity | All four modes; native-tls, optional CA and PKCS#8 client identity | Disabled, Required, VerifyIdentity; **VerifyCa rejected** | All four modes; native-tls, optional CA and PKCS#8 client identity |
| Catalog | Databases, tables, views | Actual databases; schema-qualified objects | Databases and collections | INFO keyspace databases; bounded SCAN key catalog |
| Console | Restricted single SELECT | Restricted single SELECT, PostgreSQL dialect | Restricted JSON find object | Allowlisted JSON command array |
| Browse | Typed table grid; WHERE/ORDER BY | Shared typed preview grid; structured backend filter/sort only | Single Extended JSON document column | Shared grid for key/value previews |
| Header sorting | Tables only, not console results | Metadata-validated table sorting | Disabled | Disabled |

All new operations own their connection/client and are routed through `drivers::native`; the shared source, catalog, table-page and query-result models are reused. Engine identity includes MongoDB and the static catalog contains five experimental entries. This is not a stable plugin ABI or universal SQL contract.

## Source setup and TLS

The New Source driver selector and built-in Drivers information page cover all five engines. Use engine-specific connection settings; visible shared controls do not promise every transport or property works on every backend. Unsupported combinations fail closed. Connector **imports remain MySQL/MariaDB-only**, including native interchange import; adding these drivers does not widen importer support.

URL-only accepts credential-free engine schemes: `postgres://` / `postgresql://`, `mongodb://`, and `redis://` / `rediss://`, alongside existing MySQL/MariaDB schemes. Query parameters, fragments, embedded credentials and arbitrary JDBC properties are rejected; use explicit source fields. MongoDB SRV URLs, replica-set configuration and multiple-host URLs are not supported. Redis database scope must be a numeric index **0–65535** (a server can allow fewer); no scope defaults to 0.

PostgreSQL opens the actual selected database for each operation, rather than treating database names as SQL qualifiers. Optional scope falls back to `postgres` for the initial connection/discovery. Catalog table names are two separately SQL-quoted identifiers, such as `"public"."orders"`, preserving dots and quotes in identifiers. A Unix endpoint may be a socket directory or a `.s.PGSQL.<port>` path; it is not a generic file transport. Shared SSH/CONNECT relays retain original-host TLS identity checks.

MongoDB's optional database scope falls back to `admin`; credential-free MongoDB URLs support `directConnection`, `authSource`, and `tls`/`ssl`. URI authSource defaults to the URI database when present, otherwise `admin`; structured profiles keep the prior `admin` default. Pasting a credential-bearing URI in the source form extracts user/password into masked authentication fields and switches User & Password; persisted URLs never retain credentials. The official rustls backend cannot independently omit hostname verification while retaining CA verification, so VerifyCa is rejected rather than weakened. Required enables explicitly unverified TLS; prefer VerifyIdentity. A client identity requires a combined certificate/key PEM: **both client-path fields must name the same file**. CA and client identity settings do not imply a tested X.509 authentication matrix.

Disabled is plaintext; Required encrypts without peer verification; VerifyCa verifies certificate trust without hostname matching; VerifyIdentity verifies trust and hostname. Do not silently retry a failed TLS connection insecurely. Passwords remain session-only unless SaveForever explicitly opts into local unencrypted `dalan.auth`; no Keychain backend is reintroduced.

## PostgreSQL read policy

Each operation owns a fresh connection and `BEGIN READ ONLY`, applies transaction-local `statement_timeout` plus an outer client deadline, and cleans up its connection/relay. Console validation reuses the bounded sqlparser AST visitor with **PostgreSqlDialect**, exactly one supported SELECT and the same conservative unqualified built-in function allowlist used by MySQL/MariaDB. This is intentionally not complete PostgreSQL SQL/function coverage. Rejected constructs do not fall back to arbitrary SQL.

Shared previews retain at most **4,096 bytes per cell / 2 MiB per page**, with typed values and explicit truncation. The official PostgreSQL driver has **no configurable hard wire-message allocation cap** here. Preview limits are not a protocol, total-memory or server-work guarantee.

Structured backend filters support NULL tests and text comparisons. Non-NULL comparison operands use `column::text`: greater/less comparisons are **lexical, not native numeric comparisons**. Contains is a text operation. Sort columns are metadata-validated and quoted. Raw WHERE/ORDER BY fragments are unsupported; the production WHERE/ORDER BY toolbar remains MySQL/MariaDB-only. Do not advertise PostgreSQL fragment-parser parity.

## MongoDB read policy

Catalog discovery lists databases and collections. Browsing renders one document per row in an Extended JSON column, preserving BSON-specific values in the preview rather than inventing relational columns.

The console accepts one bounded JSON object with only `find`, `filter`, `sort`, `projection`, `skip` and `limit`:

```json
{"find":"orders","filter":{"status":"open"},"sort":{"_id":1},"limit":100}
```

Filters are restricted, not arbitrary MongoDB query expressions. Sort values are 1/-1; projection is inclusion/exclusion, not expressions. Skip is at most 1,000,000; limit is 1–200. JavaScript, `$where`, aggregation pipelines, writes and unrestricted `runCommand` are unavailable. SQL table fragments and generic header sorting are disabled.

Reads use server `maxTimeMS` and an outer operation deadline. Each operation owns a client pool; normal completion shuts it down, and cancellation drops cursors and schedules immediate asynchronous `Client::shutdown`. This is cleanup, not a confirmed server-side cancellation acknowledgment. The driver can decode BSON documents/batches larger than the retained preview: **there is no hard BSON/batch wire-allocation cap** established by the 4 KiB/2 MiB display limits.

## Redis read policy

Setup may send AUTH (username/password or password-only) and SELECT for the validated numeric database. User console commands cannot send AUTH or SELECT. Commands are a single JSON array of strings, for example:

```json
["GET","order:1"]
```

The allowlist is PING, DBSIZE, GET, MGET, EXISTS, TYPE, TTL, PTTL, STRLEN, HLEN, HGETALL, HGET, HMGET, SCARD, SMEMBERS, LLEN, LRANGE, ZCARD, ZRANGE, SCAN, HSCAN, SSCAN and ZSCAN, with validated argument shapes. KEYS, EVAL/scripts, writes, blocking and administration commands are rejected. Text is at most 64 KiB, with 1–128 arguments and 4,096 bytes per argument.

Unlike the official PostgreSQL/MongoDB clients, the custom codec bounds RESP2 replies before declared-size allocation: **8 MiB, 100,000 nodes, nesting depth 32**. Retained previews still have 4 KiB/cell and 2 MiB/page bounds. Oversize/malformed responses produce errors, not silently accepted partial protocol data. Cancellation closes the owned socket; operations have outer deadlines.

Catalog discovery uses INFO keyspace and SCAN, bounded to 50,000 objects; binary/non-UTF-8 or oversized key names are skipped rather than misidentified as text keys. Key values retain binary preview identity. SCAN is not a snapshot: duplicates and changing order are possible. Hash/set/sorted-set browse offsets are **opaque SCAN cursors**, not row numbers. COUNT is advisory; oversized browse batches produce an explicit error rather than silently dropping entries and advancing the cursor. Lists/ranges have their own semantics. Cluster routing, RESP3 and stream inspection are not implemented; discovering a stream key or accepting a SCAN type filter is not stream-browse support.

## Shared UI and evidence boundaries

Existing source targeting, metadata cache, independent tabs, canvas grid, stale-result provenance and loaded-page CSV contracts are reused for document/key reads. MongoDB/Redis use JSON highlighting, not SQL highlighting. SQL Format/Compress are disabled for **every non-MySQL/MariaDB engine, including PostgreSQL**, because the existing tokenizer is not safe for their dialects. Shared toolbar presence is not SQL compatibility or transaction support.

Owned loopback framed-protocol fixtures exist for all three native additions, with validator, typed-value, capped-preview, protocol/deadline and cancellation/cleanup coverage at their scoped boundaries. These fixtures are not an actual PostgreSQL/MongoDB/Redis server/auth/TLS/topology matrix. **The full native multi-database server/TLS matrix has not been run.** Final suite totals belong to primary verification and are not asserted here. No hosted CI or private-server test is claimed by this documentation update. Native UI/IME/accessibility/performance and release signing remain separate gates.

Use least-privilege SELECT/read-only roles or Redis ACLs and trusted server objects. Client validation is defense in depth, not server authorization or a sandbox. Cancellation does not prove instant server termination; preview caps do not bound all server work. See [query consoles](query-consoles.md), [source management](source-management.md), [security](security.md) and the [roadmap](../ROADMAP.md).

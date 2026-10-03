# Driver design and compatibility

Status: proposed adapters. `dalan-drivers` currently contains a planned catalog only. No engine is implemented or supported by a working client.

## Proposed dependency choices

| Engine | Library | Notes |
| --- | --- | --- |
| PostgreSQL | SQLx PostgreSQL driver | Pure Rust async; explicit Tokio runtime and TLS feature configuration |
| MySQL | SQLx MySQL driver | Separate dialect/metadata adapter |
| MariaDB | SQLx MySQL driver | Shares wire backend, retains MariaDB identity and capability checks; no `mariadb` Cargo feature |
| Redis | `redis` crate (redis-rs) | Async Tokio support, runtime-specific TLS configuration |
| MongoDB | Official MongoDB Rust driver, evaluate later | After first release, document model and aggregation UX required |
| Other | Not chosen | Add only after concrete daily workflow and compatibility gates |

Research observed SQLx 0.9.0 and `redis` 1.7.1 in current documentation. They are not workspace dependencies yet. Reverify release versions, features, minimum Rust versions, TLS roots, and licenses when implementing adapters; add locked versions then. See [sources](references.md).

For SQLx, disable unnecessary defaults, choose `runtime-tokio`, engine features, and a tested TLS backend. Native TLS and Rustls have different trust-store/provider behavior. Prefer verified TLS. Current SQLx separates non-TLS MySQL RSA authentication into `mysql-rsa`; do not enable insecure connectivity as a workaround. Use runtime query APIs for user SQL, not compile-time schema-dependent `query!` macros. `AnyPool` does not erase dialect or session semantics and is not the proposed abstraction.

For Redis, `tokio-comp` enables async APIs; async TLS needs the correct `tokio-rustls-comp` or `tokio-native-tls-comp` feature. Verify root loading and crypto-provider setup. Multiplexed connections can serve normal concurrent requests; isolate blocking commands if admitted later. Reconnection does not make retries of mutating commands safe. Avoid older 0.x examples against Redis crate 1.x.

## Engine capability rules

PostgreSQL: discover schemas and qualified identifiers; preserve native type information and session search path. Investigate cancellation API support without assuming SQLx exposes all wire-protocol controls. EXPLAIN ANALYZE executes the operation and is not a harmless metadata request.

MySQL/MariaDB: distinguish server/version, catalog naming, identifier quoting, SQL modes, generated columns, storage engine, and authentication plugins. DDL can implicitly commit; nontransactional tables cannot offer rollback guarantees. Test both servers independently despite a shared driver.

Redis: share profiles/tree chrome but not SQL sessions/grids. Preserve byte keys, represent integer/bulk/array/map/native RESP replies, inspect type and TTL, and bound reads. SCAN may duplicate keys and is not a snapshot or exact page count. No browser KEYS command. Cursor cancellation stops client browsing, not a server-side snapshot. TTL -1 means persistent and -2 means absent; deletion/expiry can race inspection. Document allowed command classes before exposing execution.

## Proposed support matrix to settle before adapter claims

| Item | PostgreSQL | MySQL | MariaDB | Redis |
| --- | --- | --- | --- | --- |
| Exact tested server versions | TBD | TBD | TBD | TBD |
| Auth methods/plugins | TBD | TBD | TBD | Password/ACL proposal, tests TBD |
| TLS / custom CA / client certificates | Select and test | Select and test | Select and test | Select and test |
| Deployment topology | Direct single-server proposal | Direct single-server proposal | Direct single-server proposal | Standalone proposal; Cluster/Sentinel deferred |
| OS credential integration | macOS first | macOS first | macOS first | macOS first |
| Cancellation guarantees | Spike required | Spike required | Spike required | Command-class-specific; spike required |

Managed services are supported only when their actual protocol/auth/TLS requirements pass the matrix, not because they advertise engine compatibility. Unix sockets, SSH tunnels, proxies, client certificates, and cloud-specific auth remain explicit decisions. Externally managed SSH tunnels are the proposed initial workaround, not an implemented feature.

## Adding an adapter

1. Specify the user workflow and engine capability table.
2. Choose a compatible dependency with locked version and license review.
3. Implement a bounded read-only vertical slice before writes.
4. Add real-server fixtures covering auth/TLS, metadata, native types, empty/error/cancel states, and session behavior.
5. Add guarded writes only with explicit identity, approvals, and uncertainty tests.
6. Mark a driver implemented in the catalog only when the UI workflow exists and passes its compatibility matrix.

The current catalog deliberately has no `Supported` status and no mocked successful connection.

[Architecture](architecture.md) · [Product plan](product-plan.md) · [Testing](testing.md)

# Technical architecture

Status: proposed architecture unless marked implemented. See [product scope](product-plan.md) and [ADRs](adr/README.md).

## Workspace boundaries

```text
dalan-app ──> dalan-core
           ├─> dalan-drivers ──> dalan-core
           └─> dalan-acp ──> official ACP SDK

GPUI entities/views live in app only.
Engine adapters never depend on GPUI.
ACP never owns a database connection.
```

Implemented: these four crates and dependency directions. The app has a diagnostic binary and feature-gated macOS bootstrap. Core provides engine identity, proposed default limits, and a decision table over caller-declared risk. Drivers provide a planned catalog. ACP exposes SDK v1 schemas, default capabilities, and absolute-path validation. No network services, database sessions, credential persistence, or AI connection exists.

Do not add a crate for each planned feature. Add modules inside these boundaries first; split storage or platform services only when a tested vertical slice needs an independently owned lifecycle.

## UI and runtime ownership

Proposal: GPUI owns UI entities on its foreground executor. A managed Tokio runtime owns database sockets, background metadata requests, timers, and driver tasks. GPUI's executor is not a Tokio runtime; do not poll SQLx or Redis work there without an explicit runtime bridge.

- Start a single runtime service at application startup; retain its handle for the application lifetime.
- Send typed requests and bounded event streams across the bridge. Return plain domain data, not GPUI entity handles, across worker threads.
- Apply updates on the GPUI foreground executor using weak view references; ignore responses for closed views and superseded request generations.
- Bound queues, row chunks, payload bytes, and parallel requests. UI virtualization alone does not limit memory use.
- Cancellation and shutdown must stop admission, request cancellation, drain bounded work, terminate owned agent processes, and join tasks within a chosen deadline.

ACP SDK 2.2.0 uses runtime-neutral `futures::io` byte streams. Confirm whether the SDK-owned subprocess helper or a Tokio compatibility adapter fits the launcher requirements before integrating it. Do not copy an older SDK's trait-based example into this version.

## Proposed domain model

The next vertical slice should introduce identifiers for connection profile, console, session, and execution. Keep these separate: a profile can have multiple sessions; a console pins one transaction session; an execution belongs to a session and a console.

| Model | Required contract |
| --- | --- |
| Connection profile | Engine, endpoint, database/schema, TLS options, environment label, credential reference. No raw password in persisted configuration. |
| Session | Owning engine, connection identity, autocommit/transaction state, lifecycle generation, active operation. |
| Execution | Immutable target/session generation, statement or command, limits, risk/approval state, timestamps, terminal outcome. |
| Result event | Start, column metadata, bounded typed row chunks, warnings, affected rows, completion/error/uncertain outcome. |
| Grid draft | Qualified table, primary-key identity, original values/concurrency predicate, proposed values, review state. |
| Redis result | Raw key/argument bytes, native response/value shape, type, TTL, cursor/range, truncation information. |

Preserve lossless type identity: NULL differs from empty strings; decimals and large integers must not become floating point; timezone semantics, bytes, JSON, arrays, and unknown database types require explicit decoding/display rules. Display text is not the source of truth for later writes.

## Adapter contracts, deferred implementation

Use engine-specific adapters with explicit capabilities, not a universal SQL facade. Proposed operations are connect/test, discover metadata, open/close session, execute, cancel, and bounded browse. SQL-specific transaction/grid operations and Redis key/TTL operations remain distinct.

Choose concrete async trait/enum dispatch after the PostgreSQL spike establishes streaming, cancellation, and session ownership. Do not freeze a public plugin ABI or a `query(String) -> Vec<Row>` contract now. That contract loses output events and accumulates unbounded results.

Maintain explicit engine identity even when MySQL and MariaDB use the same wire driver. Missing capability means an unavailable action, not silent fallback. No third-party dynamic driver loading in the first release.

## SQL sessions and execution

- Ordinary read sessions can use a pool where appropriate; explicit transactions need a reserved physical connection.
- Database/schema changes and session variables are observable session state. A pooled query cannot pretend to share that state.
- Parse execution boundaries with dialect-aware tooling; never use semicolon splitting or prefix-only safety checks.
- Selection/current statement/full script are different commands. Record sequential results and partial failure; scripts are not implicitly atomic.
- A requested cancel is not a confirmed server abort. A dropped future can leave server-side work running. Verify per-engine cancellation and decide whether to discard affected connections.
- Disconnection after write submission can produce unknown outcome. Do not retry writes automatically or claim rollback without evidence.
- Pin approvals to immutable target, operation, and session generation; editing SQL or switching connections invalidates approval.

## Result browsing and export

Separate user-query execution from generated table browsing. Bound user-query retrieval without quietly appending SQL that changes its meaning. Generated browse queries can use engine-aware paging and explicit ordering. Offset paging is acceptable initially with documented mutation/ordering caveats; consider keyset paging when identity is available.

Distinguish loaded rows, total known rows, and estimates. User-requested full export uses a separate bounded streaming pipeline with disk backpressure and cancellation; it must not require retaining the whole dataset. Snapshot consistency is not implied. Exporting only loaded data must say so.

## Persistence proposal

Use versioned local settings for profiles, preferences, and layout; choose JSON/TOML in the storage spike. Use a local SQLite store for history/session recovery only if those workflows require it; SQLx's SQLite driver would then serve app storage, not a promised new user driver. Neither storage format is implemented or selected yet.

Use OS credential services behind a narrow platform interface: macOS Keychain first, Linux Secret Service next, Windows Credential Manager later. Credential store failures must remain explicit with no plaintext fallback. Settings/history migrations need atomic writes, backup/recovery tests, permissions, and secret-free fixtures. Persisted query text and agent conversations are sensitive and must have clear retention/delete controls.

## Platform boundary

The desktop entry point intentionally builds on macOS only. Headless core tests are portable but do not prove desktop support. Linux needs GPUI backend/dependency selection, font and clipboard behavior, credential service and packaging validation. Windows needs native text/window paths, process invocation/quoting, credential service and installer validation. Do not embed POSIX-only path assumptions into domain types.

GPUI 0.2.2 is pinned as a bootstrap baseline, not a permanent commitment. Current Zed main has different `gpui_platform` initialization. Evaluate release/revision upgrades as coherent dependency changes with build, keyboard, rendering, and accessibility regression checks.

## Implementation risks

Editor/completion, result-grid virtualization, SQL grammar coverage, screen-reader support, cancellation semantics, and desktop portability need executable spikes. GPUI does not provide DataGrip's editor/database features merely by rendering a window. Importing Zed's editor code requires a separate license review; GPUI's Apache-2.0 package license does not cover all Zed crates.

[Drivers](drivers.md) · [ACP](acp.md) · [Security](security.md) · [Testing](testing.md)

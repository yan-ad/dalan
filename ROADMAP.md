# Dalan roadmap

Status: product and technical plan, not a release promise. This is the canonical forward roadmap. [Implemented features](docs/feature-checklist.md) and [test evidence](docs/testing.md) remain separate records. Milestones below have dependency and acceptance gates, not invented dates.

## Product contract

- Rust + GPUI database workspace. DataGrip is the workflow reference; Zed-like compact UI and **Carbonfox - opaque** remain the visual direction.
- macOS first, Linux second, Windows third. Portable headless CI does not establish desktop support.
- MySQL/MariaDB are the first implemented engines. PostgreSQL and Redis remain first-release targets; MongoDB follows the first release, as previously agreed. Other engines enter through tested driver capabilities, not compatibility labels.
- AI connects only through **Agent Client Protocol (ACP)**. No application BYOK, direct provider adapters, provider-specific CLI protocols, or automatic database execution. External agents own model authentication and billing.
- Plugins extend database workflows, not a generic IDE or infrastructure toolbox. No general file explorer, Git UI, build tools, Kubernetes console, or general-purpose terminal.
- Preserve optional database scope, Keychain/session credential choices, strict connectivity, offline schema cache, independent table/console tabs, lossless values, bounded results, and working keyboard controls.

## Where we are now

| Area | Implemented in Dalan | Not implemented yet |
| --- | --- | --- |
| Drivers | Experimental MySQL/MariaDB through `mysql_async`, direct/Unix socket/URL/SSH/CONNECT routes, verified TLS options | Extensible driver registry, PostgreSQL, Redis, third-party driver workers |
| Source management | General/Options/SSH-SSL/Schemas, reusable SSH configurations, optional Keychain, per-source menus | Complete authentication/version/topology matrix, release-grade signing |
| Catalog | SQLite database/table/view-name cache, offline restoration, selected-source refresh, virtualized tree | Cached columns/keys/indexes/routines, metadata-aware completion |
| Workspace | Independent table tabs and restricted read consoles; WHERE/ORDER BY; canvas grid; loaded CSV; result-retention budget | Cell inspection/copy, column resizing, query history, scripts, transactions, staged writes |
| Plugins | No plugin host, SDK, package installer, or marketplace | All plugin milestones below |
| AI | ACP SDK boundary and disconnected panel | Agent launch, negotiation, authentication, sessions, streaming, permissions |

These are scoped implementations, not DataGrip/Compass parity. The current query console accepts one restricted read statement; this plan does not silently broaden it to arbitrary SQL.

## DBX reference and reuse strategy

Cloned [t8y2/dbx](https://github.com/t8y2/dbx) for source inspection at **`38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad`**. The local research checkout is `target/research/dbx`, intentionally ignored and excluded from Dalan distribution. It is removable build/research data, not a submodule or runtime dependency.

DBX is a source of reusable database and extension logic, not Dalan's product shell. Its concrete driver pools, generated descriptors, dialect utilities, metadata provenance, query-cancellation registry, plugin RPC, and package lifecycle are useful references. Its Vue/Tauri UI, CodeMirror integration, direct AI providers, proprietary CLI adapters, web deployment, broad middleware tools, and large binary/message budgets are not adopted.

**First adaptation implemented:** `crates/drivers/src/catalog.rs` adapts DBX's descriptor/capability separation and registry lookup/validation patterns to Dalan's four existing engines. It adds stable IDs, dialect/runtime intent, optional capability metadata and validation without importing DBX's full engine list, generated manifests or runtime. No driver executor, plugin host or AI provider was copied. Root/package Apache-2.0 permits adaptation subject to applicable notices and modified-file requirements; third-party/vendor components and external plugin artifacts require separate review. Dalan's own project license remains an owner decision before distribution. See [source-backed reuse notes](docs/dbx-reuse.md).

## Now: consolidate the foundation before adding breadth

### N1. Establish the DBX port boundary

Completed starting point: attributed static catalog, distinct MySQL/MariaDB identities, optional-database/Unix-socket capability metadata for experimental drivers, and duplicate/invalid/planned-capability tests. Planned PostgreSQL/Redis entries expose no implemented capabilities. This descriptive catalog does not change connection behavior or establish a plugin ABI.

1. Record the exact upstream file, revision, license/notices, intended behavior, Dalan changes, and tests for every port.
2. Start with descriptor validation, capability categories, metadata availability, and pure identifier/qualification logic. Adapt to Dalan's typed values and explicit catalog identities.
3. Keep database I/O independent of GPUI and credential storage independent of driver/plugin/AI code. Split existing crates only when ownership justifies it.
4. Do not depend on all of `dbx-core`, inherit its global state, or copy its patched dependency graph automatically.

**Exit:** one isolated port lands with provenance, unit tests, strict lint, and unchanged MySQL/MariaDB workflows. A reference clone alone does not complete this milestone.

### N2. Stabilize the daily DataGrip workflow

- Profile native wide-grid scrolling/frame time and memory on representative hardware; keep two-axis canvas rendering and retained-result limits. Structural render counts are not latency measurements.
- Add cell inspection/copy and column resizing/auto-fit one by one, preserving exact NULL/binary/decimal semantics.
- Add SQL highlighting, metadata-aware completion, and explicit opt-in history/draft recovery with retention/delete controls.
- Introduce richer object metadata with `supported / unsupported / unknown` availability rather than fabricated fields.
- Design pinned physical-session ownership before transactions/scripts/writes. Add cancel acknowledgements and uncertain outcomes before retry or write controls.

**Exit:** reproducible native performance evidence, keyboard/focus and error-state tests, lossless-value regressions, and isolated-tab cancellation/recovery. No unused toolbar placeholders.

### N3. Define internal driver contract v1

A proposed Dalan-owned contract separates:

- stable driver ID, engine identity, dialect family, runtime kind and version;
- connection fields/secrets, TLS/transports, authentication and topology support;
- metadata scope and available object capabilities;
- typed row/document/key values and bounded batches/cursors;
- execution, physical-session/transaction ownership, cancellation and terminal/unknown outcomes;
- validated browsing/filter/sort, plan/export/edit capabilities.

Use an internal Rust trait/enum appropriate to the first adapters. This is **not** a stable dynamically loaded Rust ABI. Model SQL, Redis and documents as different capability sets, not one universal `query(String) -> Vec<Row>` facade.

**Exit:** MySQL and MariaDB migrate behind the contract without losing existing profiles/cache identities or safety controls; fake adapters and real-server conformance tests cover unsupported capabilities and failures.

## Next: grow three ecosystems in parallel behind that foundation

### Driver ecosystem

| Order | Milestone | Acceptance gate |
| --- | --- | --- |
| D1 | PostgreSQL read vertical slice | Verified TLS/auth matrix; databases/schemas/objects; exact NUMERIC, arrays, UUID, bytea, timestamps and special floats; table/console/cache isolation; real cancel tests |
| D2 | Redis native workspace | SCAN without KEYS; binary-safe keys; bounded string/hash/list/set/sorted-set/stream inspection; correct TTL states; command policy; no false SQL grid/transaction parity |
| D3 | First-release native driver set | PostgreSQL/MySQL/MariaDB/Redis pass declared server/OS matrices and release gates, not merely compilation |
| D4 | MongoDB after first release | Collection/document/aggregation UX, BSON fidelity, indexes, bounded reads/export and explicit later-write policy |
| D5 | Additional native engines | Rank SQLite/DuckDB, SQL Server, ClickHouse and others by actual daily need, dependency/license cost and completed conformance tests |

Each driver ships a descriptor, tested capabilities, migration rules and fixtures. Adding a catalog entry does not claim support. PostgreSQL NUMERIC decoding is a promising isolated DBX port; preserve Dalan's native type identity rather than DBX's JavaScript-safe JSON representation.

Optional external/JDBC drivers are a later, explicitly installed worker tier. They must not force a JVM, proprietary client, webview, or unrelated native library into the default GPUI build. Database-worker RPC is separate from ACP AI transport.

### Plugin ecosystem

**P1: one narrow SDK and sample plugin.** Propose a versioned manifest and stdio protocol, compatibility negotiation, structured errors, lifecycle/cancellation and bounded payloads. First sample: a loaded-result/schema export extension rendered with app-owned GPUI controls. Prove installation, invocation and cleanup before a marketplace.

**P2: host-owned permissions and packaging.** Add scoped/revocable grants, a clean environment allowlist, dedicated plugin storage, request/byte/queue/process budgets, safe archive extraction, trusted signatures, atomic activation/update/rollback and permission-expansion review. Unsigned development mode must be explicit and unavailable for normal store installs.

**P3: database-focused contributions.** Add validated declarative connection forms, table actions, result viewers and approved read-only metadata/data APIs. Ordinary tools use opaque source/session references and host-mediated operations, not passwords or driver objects. Arbitrary plugin HTML/Vue cannot be used as native GPUI widgets.

Native connection-provider plugins are a separate privileged tier: they may need credentials and must disclose that boundary. Do not let a plugin's manifest grant itself access. Package signatures establish provenance/integrity, not safety. Sidecars are not OS-sandboxed unless platform confinement is implemented and tested.

**Exit:** protocol conformance, malformed/oversized frames, cancellation/timeout distinction, child-process cleanup, grant denial/revocation, secret-redaction, archive traversal/symlink/tampering, update/downgrade and rollback tests. No generic filesystem/infrastructure contribution surfaces by default.

### AI integrations, ACP only

**A1: real ACP client.** Launch an explicitly trusted configured executable over stdio; negotiate stable protocol/capabilities; handle agent-managed authentication, session creation, streaming, cancellation, crash/restart and supplied permission-option IDs. Advertise only implemented capabilities. Database use works with no agent.

**A2: reviewed database context.** Opt-in schema/context preview, no automatic rows or credentials. Support explain/draft/fix suggestions and user-controlled insertion into a query console. Insertion changes the draft only; normal Run and validation still apply.

**A3: optional scoped tools, separately reviewed.** Only after the client and host policy mature, consider a Dalan-owned read-only MCP tool service or negotiated extension. ACP is agent communication; MCP is a separate tool interface. Per-source grants, limits, revocation, exact target context and auditing apply on the host. Do not copy DBX's direct provider/BYOK layer or app-owned autonomous tool loop.

**Exit:** deterministic fake-agent conformance plus a declared real-agent/version/auth matrix; no direct-provider fallback; context-consent changes, denied permissions, cancelled turns, unsupported capabilities, crashes and oversize output tested. External agents retain their own OS privileges; ACP permission UI is not a sandbox.

## Future: mature the ecosystem without bloating the workspace

- **Driver workers:** optional installed adapters with typed, versioned RPC, platform-specific artifacts, bounded paging/streams, crash isolation, explicit credential consent and compatibility fixtures. No universal DBX-agent or plugin binary compatibility promise.
- **Database writes:** pinned transactions, primary-key staged edits, generated SQL preview, immutable approval/target binding, conflict and affected-row checks, rollback limitations, unknown outcomes and no automatic write retry. The read-only console policy changes only through an explicit decision record.
- **Metadata tools:** view/routine/index/source inspection, estimated plans, explain/lineage, schema diff and database-focused transfer/import/export. Actual plans and DDL execution require separate risk gates.
- **Plugin registry:** curated, signed database extensions with publisher/key continuity, offline install, revocation, trust review and tested SDK versions. Marketplace breadth follows host quality, not the reverse.
- **AI tools:** narrowly scoped read-only operations first. Any write tool needs a separate design and fresh exact-operation confirmation; no plugin/agent can bypass Dalan's execution authority.
- **Platforms:** macOS distribution signing/notarization and accessibility first; Linux desktop/credential/input/packaging matrix second; Windows desktop/signing/credential/process-tree matrix third. Continue headless CI on all three throughout.

## Dependency order and concrete next task

```text
DBX provenance + internal capabilities/typed outcomes
    -> MySQL/MariaDB contract migration + richer metadata
    -> PostgreSQL, Redis and shared conformance fixtures
    -> optional driver workers

host boundaries + context/permission model
    -> plugin SDK + one export extension
    -> signed installer/rollback + scoped contributions
    -> registry and privileged driver-plugin tier

ACP transport + fake-agent tests
    -> real-agent/auth compatibility
    -> consented suggestions into consoles
    -> separately reviewed read-only tool interface
```

**Next implementation:** extend the initial catalog into a metadata-availability and typed execution contract, then migrate the existing MySQL/MariaDB executors behind it with conformance tests. PostgreSQL is the next new engine. The first plugin and ACP transport can develop in parallel once their host boundaries are defined.

## Release evidence and unresolved decisions

Every milestone must record commands, actual outcomes, exact engine/agent/plugin versions, failure paths and remaining exclusions. Upstream README feature counts, executable size and sandbox language are not evidence about Dalan. Preserve native performance/assistive-technology gates, dependency/license review, migration rollback and credential redaction.

Owner decisions still needed: project/distribution license, minimum macOS and Intel matrix, exact server support versions, first optional worker engines, plugin signing authority/trust model, concrete process-confinement targets, and which real ACP agents/auth methods are supported. Do not invent a delivery date to resolve these.

[DBX reuse assessment](docs/dbx-reuse.md) · [Architecture](docs/architecture.md) · [Product scope](docs/product-plan.md) · [Feature checklist](docs/feature-checklist.md) · [ACP](docs/acp.md)

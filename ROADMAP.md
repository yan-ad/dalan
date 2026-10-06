# Dalan roadmap

Status: product and technical plan, not a release promise. This is the canonical forward roadmap. [Implemented features](docs/feature-checklist.md) and [test evidence](docs/testing.md) remain separate records. Milestones below have dependency and acceptance gates, not invented dates.

## Product contract

- Rust + GPUI Kit database workspace. DataGrip is the workflow reference; compact layout, **Kit standard controls and Kit's default theme** are the visual direction.
- macOS first, Linux second, Windows third, followed by Android feasibility and mobile implementation. All public platform releases are **coming soon**; portable headless CI does not establish native desktop or mobile support.
- MySQL/MariaDB were the first implemented engines. PostgreSQL, MongoDB and Redis now have experimental native read-only adapters: the earlier immediate priority was native three-engine support; a first optional JDBC bridge and curated installer now follows. This brings restricted MongoDB reads forward from the earlier post-first-release plan; full Compass-style workflows remain later. Release support still requires tested driver capabilities and declared server matrices, not compatibility labels.
- AI connects only through **Agent Client Protocol (ACP)**. No application BYOK, direct provider adapters, provider-specific CLI protocols, or automatic database execution. External agents own model authentication and billing.
- Plugins extend database workflows, not a generic IDE or infrastructure toolbox. No general file explorer, Git UI, build tools, Kubernetes console, or general-purpose terminal.
- Preserve optional database scope, session-only credentials with explicit SaveForever local plaintext opt-in, strict connectivity, offline schema cache, independent table/console tabs, lossless values, bounded results, and working keyboard controls.

## Where we are now

| Area | Implemented in Dalan | Not implemented yet |
| --- | --- | --- |
| Drivers | Five experimental native engines: MySQL/MariaDB, PostgreSQL, official-driver MongoDB and bounded-RESP2 Redis; engine-specific transport/TLS restrictions; first opt-in Java 17+ JDBC bridge and curated installer | Declared native/JDBC server/auth/TLS matrices, general extensible registry and broader third-party workers |
| Source management | General/Options/SSH-SSL/Schemas, reusable SSH configurations, session/SaveForever local plaintext auth, per-source menus; imports still MySQL/MariaDB-only | Complete authentication/version/topology matrix, release-grade signing |
| Catalog | SQLite database/table/view-name cache, offline restoration, selected-source refresh, virtualized tree and cached-only search through collapsed branches | Cached columns/keys/indexes/routines, metadata-aware completion |
| Workspace | Independent table/document/key tabs and restricted SQL/JSON read consoles; MySQL/MariaDB-only WHERE/ORDER BY fragments; canvas grid with cell selection/copy; loaded CSV; result-retention budget; bounded staged SQL base-table writes | Details drawer, column resizing, query history, scripts, reusable transactions and full DBX editing parity |
| Plugins | No plugin host, SDK, package installer, or marketplace | All plugin milestones below |
| AI | ACP SDK boundary and disconnected panel | Agent launch, negotiation, authentication, sessions, streaming, permissions |

These are scoped implementations, not DataGrip/Compass parity. SQL consoles execute individually validated SELECTs, with prevalidated sequential batches, MongoDB a restricted JSON find object, Redis an allowlisted JSON command array; this plan does not broaden them to arbitrary SQL, shell or write execution. See the [native support matrix](docs/native-drivers.md), including direct-only MongoDB/Redis, MongoDB VerifyCa rejection, and official-driver wire-cap limitations. Owned loopback fixtures are not the unrun actual native multi-database server/TLS matrix; final totals await primary verification.

## Platform coverage and release plan

**Dalan is coming soon on Mac, Linux, Windows and Android.** These are planned release targets, not currently available installers or a promise that every platform will ship together. No delivery dates are committed.

| Order | Platform | Current status | Release gates |
| --- | --- | --- | --- |
| 1 | **Mac (macOS)** | Coming soon; experimental local native app build exists | Signed/notarized distribution, declared OS/architecture matrix, native input/accessibility/performance checks, and actual-server driver qualification. |
| 2 | **Linux** | Coming soon; headless test coverage only | Native desktop builds and packages, declared distributions/display-server matrix, input/accessibility checks, TLS and local-auth permissions, and driver/process lifecycle qualification. |
| 3 | **Windows** | Coming soon; headless test coverage only | Native desktop builds and signed installer, declared OS/architecture matrix, input/accessibility checks, local-auth ACL enforcement/validation, and driver/process-tree qualification. |
| 4 | **Android** | Coming soon; planned, not implemented | Confirm the GPUI/native runtime and dependency feasibility; design touch navigation and an on-screen-keyboard-friendly workspace; validate app-private credential storage, lifecycle/background-network behavior, driver compatibility, and signed mobile packaging. |

Keep shared database logic portable, but qualify each native shell independently. Android is a separate mobile milestone—not a claim that the current desktop layout, SSH process model, experimental JDBC bridge or all native drivers already work on mobile. Update the [README platform table](README.md#installation) only when corresponding build and release evidence exists.

## DBX reference and reuse strategy

Cloned [t8y2/dbx](https://github.com/t8y2/dbx) for source inspection at **`38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad`**. The local research checkout is `target/research/dbx`, intentionally ignored and excluded from Dalan distribution. It is removable build/research data, not a submodule or runtime dependency.

DBX is a source of reusable database and extension logic, not Dalan's product shell. Its concrete driver pools, generated descriptors, dialect utilities, metadata provenance, query-cancellation registry, plugin RPC, and package lifecycle are useful references. Its Vue/Tauri UI, CodeMirror integration, direct AI providers, proprietary CLI adapters, web deployment, broad middleware tools, and large binary/message budgets are not adopted.

**First adaptation implemented:** `crates/drivers/src/catalog.rs` adapts DBX's descriptor/capability separation and registry lookup/validation patterns to Dalan's initial four engine identities (now extended to five with MongoDB). It adds stable IDs, dialect/runtime intent, optional capability metadata and validation without importing DBX's full engine list, generated manifests or runtime. No driver executor, plugin host or AI provider was copied. Root/package Apache-2.0 permits adaptation subject to applicable notices and modified-file requirements; third-party/vendor components and external plugin artifacts require separate review. Dalan's own project license remains an owner decision before distribution. See [source-backed reuse notes](docs/dbx-reuse.md).

## Now: consolidate the foundation before adding breadth

### N0. Adopt the GPUI Kit component foundation

Production now aliases `gpui` to `gpui-kit = 0.7.1` with `tree-sitter-sql`, on one coordinated `gpui-pre 0.3.8` family; the old GPUI 0.2 family is removed. Bootstrap/init and Base Root cover main/source/About/SSH windows. All standard controls use Kit and its default theme; Carbonfox runtime overrides are dropped while historical assets/notices remain. Thin InputState/privacy and rope SQL Editor adapters replace bespoke editing engines. Preserve app-owned models, credentials, cache, query policies and tabs.

Kit inputs/buttons/checkboxes/tabs/dropdown and popup menus/tooltips are implemented; native dropdown alternatives are not claimed Combobox/Select usage. SQL Tree-sitter highlighting is enabled selectively (JSON is also in the graph), not all language bundles. Benchmark Kit DataTable before replacing the specialized shipping canvas; stateless Table is not the wide-result solution. Database-aware completion/history/per-statement execution remain pending Dalan features. See [migration guide](docs/gpui-kit-migration.md) and [ADR 0006](docs/adr/0006-gpui-kit-migration.md).

**Completed foundation:** production Kit adoption, default-theme ownership, password/privacy/editor adapters and coordinated dependencies. The isolated experiment and its job are removed; earlier pilot counts remain historical, not canvas parity. Main-workspace fixes cover multiline-safe canvas previews, visible prefixed tab labels, immediate Run focus restoration, searchable console database choices, cached-only explorer search and top-right ACP/default-theme controls. Final local validation counts await primary confirmation. **Remaining gates:** native visual/accessibility and representative performance evidence; hosted CI is explicitly skipped for this task. Production adoption is implemented, not native certification, history or live ACP. New native adapters are independently implemented; they do not establish release certification.

### N1. Establish the DBX port boundary

Completed starting point: attributed static catalog, distinct MySQL/MariaDB identities, optional-database/Unix-socket capability metadata for experimental drivers, and duplicate/invalid/planned-capability tests. PostgreSQL/Redis entries and the added MongoDB entry now advertise scoped experimental native connection capabilities; all five preserve distinct identities. The original planned-entry state is historical. This descriptive catalog does not change connection behavior or establish a plugin ABI.

1. Record the exact upstream file, revision, license/notices, intended behavior, Dalan changes, and tests for every port.
2. Start with descriptor validation, capability categories, metadata availability, and pure identifier/qualification logic. Adapt to Dalan's typed values and explicit catalog identities.
3. Keep database I/O independent of GPUI and credential storage independent of driver/plugin/AI code. Split existing crates only when ownership justifies it.
4. Do not depend on all of `dbx-core`, inherit its global state, or copy its patched dependency graph automatically.

**Exit:** one isolated port lands with provenance, unit tests, strict lint, and unchanged MySQL/MariaDB workflows. A reference clone alone does not complete this milestone.

### N2. Stabilize the daily DataGrip workflow

- Profile native wide-grid scrolling/frame time and memory on representative hardware; keep two-axis canvas rendering and retained-result limits. Structural render counts are not latency measurements.
- Build on cell selection/copy and bounded staged SQL editing with details inspection and column resizing/auto-fit one by one, preserving exact NULL/binary/decimal semantics.
- Build on implemented Kit SQL highlighting with metadata-aware completion and explicit opt-in history/draft recovery with retention/delete controls.
- Introduce richer object metadata with `supported / unsupported / unknown` availability rather than fabricated fields.
- Consolidate the [bounded SQL write stage](docs/table-editing.md): actual-server transaction/commit fixtures, schema/conflict/uncertain-outcome and original-target reconciliation gates come before release claims or type expansion. No automatic write retry. Design pinned reusable physical-session ownership before general transactions/scripts; do not treat one-batch Apply as transaction-console parity.

**Exit:** reproducible native performance evidence, keyboard/focus and error-state tests, lossless-value regressions, and isolated-tab cancellation/recovery. No unused toolbar placeholders.

### Staged DBX toolbar parity

Full DBX parity is approved **in stages**, not claimed complete. [The detailed parity plan](docs/dbx-toolbar-parity.md) records implemented Stage 1 safe toolbar/editor tools and remaining gaps: Run/Stop plus retained Cancel, MySQL/MariaDB-only whole-document token-gap Format/Compress (disabled for PostgreSQL/MongoDB/Redis), Kit wrap/Unfold, bounded UTF-8 SQL open/new-file save, quoted clipboard IN lists, source/database targeting and persisted source-default controls. Existing loaded CSV export remains an extra. No Fold All placeholder is shipped; compact More uses an 800 px tier, not DBX measured overflow/hysteresis.

Next parity stages are **2: results and explain**, **3: archives/script library, diagnostics and folding/LSP**, **4: multi-database execution and cancellation**, and **5: transaction sessions, permissions and confirmation/rollback**. They depend on explicit capability, execution-policy and session-ownership gates; the current console remains restricted read-only. Stage 1 is independently implemented from conceptual upstream references, not copied toolbar code/assets, and is not 1:1 visual/logic parity. Final local validation totals await primary confirmation; hosted CI is skipped.

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
| D1 | Harden implemented PostgreSQL native read slice | Verified TLS/auth matrix; databases/schemas/objects; exact NUMERIC, arrays, UUID, bytea, timestamps and special floats; table/console/cache isolation; real cancel tests |
| D2 | Harden implemented Redis native read slice | Actual server/TLS/ACL matrix; SCAN cursor/oversize behavior; binary-key catalog limitations; bounded string/hash/list/set/sorted-set inspection; streams/cluster/RESP3 remain unsupported; no false SQL/transaction parity |
| D3 | Release-qualified native driver set | All declared engines pass server/OS/auth/TLS matrices and release gates, not merely compilation or framed loopback fixtures |
| D4 | Harden implemented MongoDB native find slice, then deeper workflows | Direct-only and fixed-admin auth tests; BSON/EJSON fidelity; actual server/TLS matrix before release claims; aggregation/index UX and writes remain later explicit scope |
| D5 | Additional native engines | Rank SQLite/DuckDB, SQL Server, ClickHouse and others by actual daily need, dependency/license cost and completed conformance tests |

The native three-engine implementations already have scoped descriptors, validators and owned loopback fixtures, not completed real-server matrices. PostgreSQL/MongoDB official drivers have no hard message/batch allocation cap here; Redis has an explicit bounded RESP2 decoder. Each release-qualified driver needs a descriptor, tested capabilities, migration rules and fixtures. Adding a catalog entry does not claim support. PostgreSQL NUMERIC decoding is a promising isolated DBX port; preserve Dalan's native type identity rather than DBX's JavaScript-safe JSON representation.

**First JDBC slice implemented:** optional Java 17+ source-launched child per operation, restricted read policy, curated seven-entry discovery/version feed, mandatory-checksum installer and installed-driver source selection. Java remains explicitly supplied, not bundled or managed; JARs are arbitrary trusted user code, not sandboxed. Vendor/server and complete downloaded-JAR install/execution qualification remain unrun. General dependency resolution, arbitrary JAR import, uninstall, publisher signatures and broader worker contracts are later gates. See [JDBC contract and trust limits](docs/jdbc-drivers.md). Database-worker transport remains separate from ACP AI transport.

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
- **Platforms:** follow the [platform release plan](#platform-coverage-and-release-plan): macOS signing/notarization and accessibility first; Linux desktop/credential/input/packaging second; Windows desktop/signing/credential/process-tree third; Android runtime feasibility, touch UX, app-private storage and mobile packaging afterward. Continue headless coverage on the three desktop platforms without treating it as native release certification.

## Dependency order and concrete next task

```text
DBX provenance + internal capabilities/typed outcomes
    -> MySQL/MariaDB contract migration + richer metadata
    -> harden implemented PostgreSQL/MongoDB/Redis + actual-server conformance matrices
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

**Next implementation:** harden the implemented native PostgreSQL/MongoDB/Redis read slices with owned actual-server authentication/TLS/deadline/cancellation matrices and native UI evidence, then extend metadata availability and typed execution/session contracts without broadening read policy. Retain MySQL/MariaDB-only imports until separately designed. Harden the implemented first JDBC bridge/installer with vendor/server and full artifact-install evidence before extending its capabilities; broader worker loading remains later. The first plugin and ACP transport can develop in parallel once their host boundaries are defined.

## Release evidence and unresolved decisions

Every milestone must record commands, actual outcomes, exact engine/agent/plugin versions, failure paths and remaining exclusions. Upstream README feature counts, executable size and sandbox language are not evidence about Dalan. Preserve native performance/assistive-technology gates, dependency/license review, migration rollback and credential redaction.

Owner decisions still needed: project/distribution license, minimum macOS and Intel matrix, exact server support versions, first optional worker engines, plugin signing authority/trust model, concrete process-confinement targets, and which real ACP agents/auth methods are supported. Do not invent a delivery date to resolve these.

[DBX reuse assessment](docs/dbx-reuse.md) · [Architecture](docs/architecture.md) · [Product scope](docs/product-plan.md) · [Feature checklist](docs/feature-checklist.md) · [ACP](docs/acp.md)

Reusable SSH sessions now have an explicit Enable SSH / session picker / Manage SSH Sessions workflow; inline SSH compatibility and source-side key discovery are removed. Name and Color remain in a shared fixed header above connection tabs.

### Native driver version management

Implemented: exact bundled version inventory, Latest bundled/exact-pin preferences in `dalan.config`, validation before native routing, and driver-page tables separating library versions/capabilities from unqualified server-version ranges. Each engine currently has one executable bundled backend; selections are not package downloads. Next: compile and route independently tested alternate versions or qualify the first separately installed JDBC tier or provide broader versioned external workers, with dependency/TLS matrix qualification and safe operation-bound switching. Never advertise a version that cannot execute.

# DBX source assessment and Dalan reuse plan

Status: source investigation, initial attributed adaptation and further port plan. No DBX build or test suite was run. The clone was inspected as external data; its scripts, agent instructions and dependencies were not executed.

## Provenance and license gate

- Requested upstream: https://github.com/t8y2/dbx
- Inspected commit: **`38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad`**.
- Local checkout: `target/research/dbx`, shallow clone, ignored by Dalan's existing `/target/` rule. Not a runtime dependency, distribution artifact or submodule.
- Root LICENSE and inspected original Rust package manifests declare **Apache-2.0**. No root NOTICE was found; selected vendored/imported components have their own licenses/notices.
- Before copying code, preserve the applicable license and notices, record exact paths/revision, mark modifications, inspect dependencies and check artifact-specific licenses. Apache-2.0 does not grant general DBX/vendor trademark use or license all external marketplace/JDBC artifacts.
- Dalan's project license is still undecided. Upstream licensing does not silently choose it.

[Canonical roadmap](../ROADMAP.md) describes the adoption sequence. References below are immutable source links; live documentation can diverge from this revision.

## First adaptation landed

`crates/drivers/src/catalog.rs` adapts descriptor/capability separation and lookup/validation patterns from `dbx-types/src/database_manifest.rs` and `dbx-driver-agent/src/database_capabilities.rs`. Dalan retains its four current engines, stable driver IDs distinct from profile UUIDs, explicit status, runtime/dialect intent, and optional connection capabilities. The existing MySQL/MariaDB backend is unchanged; PostgreSQL and Redis remain planned with unknown/not-implemented capabilities.

The port deliberately excludes DBX's generated large enum, global registries, pool policy and worker installation. Five tests cover lookups, distinct engine identities, invalid/duplicate IDs and prevention of planned-driver support claims. Source attribution and modification scope are in the module; the complete upstream license is retained in [licenses](../licenses/dbx-Apache-2.0.txt) and third-party notices. Metadata provenance, additional dialect helpers, cancellation and external workers remain future ports rather than implemented features.

## What the implementation actually provides

| Component | Inspected source | Dalan decision |
| --- | --- | --- |
| Concrete pool routing | [PoolKind](https://github.com/t8y2/dbx/blob/38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad/crates/dbx-core/src/connection/mod.rs) | Useful routing reference, not a discovered universal driver trait or stable plugin ABI |
| Descriptor generation | [dbx-types build](https://github.com/t8y2/dbx/blob/38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad/crates/dbx-types/build.rs), [manifest](https://github.com/t8y2/dbx/blob/38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad/crates/dbx-types/src/database_manifest.rs), [PostgreSQL descriptor](https://github.com/t8y2/dbx/blob/38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad/plugins/connection-types/postgres.yaml) | Adapt uniqueness/schema validation and capability categories to extensible Dalan IDs; do not copy a giant fixed enum or advertise untested engines |
| Connection capabilities | [database_capabilities](https://github.com/t8y2/dbx/blob/38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad/crates/dbx-driver-agent/src/database_capabilities.rs) | Separate connection scope, runtime, local-file behavior and transport probing from engine names |
| Dialect rules | [descriptor](https://github.com/t8y2/dbx/blob/38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad/crates/dbx-sql-dialect/src/sql_dialect/descriptor.rs), [identifiers](https://github.com/t8y2/dbx/blob/38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad/crates/dbx-sql-dialect/src/sql_dialect/identifiers.rs) | Port isolated tested quoting/qualification rules; avoid global registries, watch services and unrelated dialect exceptions in the first port |
| Metadata provenance | [types](https://github.com/t8y2/dbx/blob/38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad/crates/dbx-types/src/types.rs), [plugin metadata](https://github.com/t8y2/dbx/blob/38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad/crates/dbx-core/src/schema/plugin_metadata.rs) | Adopt supported/unsupported/unknown availability separately from a field's null value |
| PostgreSQL numeric conversion | [postgres driver](https://github.com/t8y2/dbx/blob/38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad/crates/dbx-driver-postgres/src/postgres.rs) | Good later isolated decoder/test port; preserve scale, special values and native type identity |
| Cancellation lifecycle | [query_cancel](https://github.com/t8y2/dbx/blob/38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad/crates/dbx-core/src/query/query_cancel.rs), [execution helpers](https://github.com/t8y2/dbx/blob/38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad/crates/dbx-driver-support/src/execution.rs) | Adapt registration generations, owner scopes and acknowledgement semantics; timeout/drop is not proof of server termination |
| Database-worker RPC | [agent_driver](https://github.com/t8y2/dbx/blob/38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad/crates/dbx-driver-agent/src/agent_driver.rs), [protocol](https://github.com/t8y2/dbx/blob/38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad/crates/dbx-driver-agent/assets/agent-protocol-v2.json) | Future optional worker reference; not ACP AI, and not a binary compatibility promise |
| Plugin manifests/RPC | [manifest](https://github.com/t8y2/dbx/blob/38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad/crates/dbx-plugin-runtime/src/plugins/manifest.rs), [runtime](https://github.com/t8y2/dbx/blob/38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad/crates/dbx-plugin-runtime/src/plugins/runtime.rs) | Adapt small version/handshake/permission/lifecycle pieces behind Dalan-owned wire contracts and smaller interactive budgets |
| Package trust/update | [installer](https://github.com/t8y2/dbx/blob/38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad/crates/dbx-plugin-runtime/src/plugins/installer.rs), [marketplace](https://github.com/t8y2/dbx/blob/38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad/crates/dbx-plugin-runtime/src/plugins/marketplace.rs), [lifecycle](https://github.com/t8y2/dbx/blob/38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad/crates/dbx-plugin-runtime/src/plugins/lifecycle.rs) | Review for a later signed installer with safe extraction, provenance continuity and rollback; SDK/sample first |
| Host-mediated data grants | [plugin_data](https://github.com/t8y2/dbx/blob/38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad/crates/dbx-core/src/query/plugin_data.rs) | Borrow per-plugin/source consent and bounded host operations, without giving ordinary plugins credentials or connections |

## Important differences not to erase

### Result fidelity and resource budgets

DBX query results use `serde_json::Value` rows and JavaScript-safe integer serialization. This is useful for its web boundary but is not a replacement for Dalan's typed `CellValue` and immutable `Arc<TablePage>`. Preserve SQL NULL versus JSON null, exact decimal/bigint representations, bytes, dates and native/unknown types. DBX's nonfinite float JSON conversion can map to null; a port must not inherit that loss.

DBX has materialized result vectors and paged worker APIs. Neither establishes automatic backpressure or total process-memory bounds. Inspected worker/runtime code permits response/frame limits much larger than Dalan's interactive pages, including a 512 MiB agent response line. Dalan keeps explicit row/cell/byte limits, two-axis canvas rendering, source/catalog bounds and tab result retention. Any worker protocol must add bounded batches, in-flight limits and cursor close ownership.

### Plugins and sandbox claims

DBX implements versioned manifests, sidecar JSONL/framed RPC, Go/Rust SDKs, installation checks, signatures and rollback. Its browser UI is an iframe with sandbox/CSP; that cannot be used directly as native GPUI UI.

Native sidecars run with ordinary user-process privileges and inherited-environment considerations. Host permissions govern mediated APIs, not arbitrary local filesystem/network syscalls. Its Windows Job Object supports process-tree cleanup, not an established general OS sandbox; inspected non-Windows cleanup is direct-child based. Dalan must not call native plugins sandboxed before actual platform confinement tests exist.

Signed/store package paths require verification, but DBX also accepts unsigned local development packages. Signatures prove provenance/integrity, not benign behavior. Ordinary export/result plugins should use host-owned scoped data references. Native connection-provider plugins can receive hydrated secrets and require a separately trusted tier.

### AI and tools

DBX's [AI provider crate](https://github.com/t8y2/dbx/tree/38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad/crates/dbx-ai-provider) implements direct provider auth/endpoints and provider-specific CLI dialects. Do not copy it into Dalan: AI remains ACP-only, with external agents owning auth/billing and suggestions reviewed before execution.

Its [MCP server](https://github.com/t8y2/dbx/tree/38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad/crates/dbx-mcp) is a separate database-tool server, not an ACP client. A future Dalan tool service needs its own host policy, per-source grants, audit and revocation design. The initial ACP client does not automatically gain database execution tools.

### Dependencies and excluded breadth

DBX's core/default feature graph includes broad storage/admin/export/AI functionality and optional agent runtimes. Its root workspace patches PostgreSQL/MySQL libraries and vendors native/platform components. Importing `dbx-core` wholesale would bring assumptions, dependencies and behaviors Dalan has not verified.

Do not import Vue/Tauri/CodeMirror UI, general filesystems, infrastructure/message-queue consoles, web/server/cloud-sync products, direct provider APIs, or automatic driver/plugin downloads merely because upstream offers them. Optional Java/JDBC/vendor-client artifacts need separate installation, licensing and runtime-cost decisions.

## Port checklist, applied to each small implementation

1. Pin upstream revision and paths; review original source, tests, package license, applicable NOTICE and dependency patches.
2. Write Dalan's behavior/compatibility contract before changing an existing engine path.
3. Copy only the selected logic/tests, retain notices, mark modifications and record provenance in third-party notices. Do not imply DBX endorsement.
4. Adapt types, errors, credentials, cancellation and resource ceilings to Dalan; no credential-bearing debug/RPC payloads.
5. Run original-behavior fixtures where portable and Dalan regression/conformance tests. Static inspection is not a compatibility test.
6. Remove or explicitly disable unimplemented capabilities rather than showing dead UI.
7. Confirm MySQL/MariaDB source/profile/cache/tab migrations remain safe before enabling a replacement.
8. Record reviewed upstream updates; no automatic floating dependency or wholesale sync.

## Research evidence and limits

Confirmed clean upstream checkout and commit, root/package licensing, workspace architecture, source symbols and linked primary docs. Upstream builds, claimed database coverage, executable size, external plugin inventory, safety of all agents/dependencies, and real sandbox guarantees were not verified. These are not Dalan support/performance claims.

Relevant public pages discovered from the supplied repository: [plugin development](https://dbxio.com/en/docs/plugin-development), [driver management](https://dbxio.com/en/docs/driver-management), [database lab](https://dbxio.com/en/docs/database-lab), [AI assistant](https://dbxio.com/en/docs/ai-assistant), and [MCP](https://dbxio.com/en/docs/mcp). Live pages are not commit-pinned; inspected source wins where they disagree. ACP behavior follows [official initialization](https://agentclientprotocol.com/protocol/v1/initialization) and [Dalan's ACP contract](acp.md).

[Roadmap](../ROADMAP.md) · [Architecture](architecture.md) · [Driver implementation](drivers.md) · [Plugin/AI milestones](../ROADMAP.md#next-grow-three-ecosystems-in-parallel-behind-that-foundation)

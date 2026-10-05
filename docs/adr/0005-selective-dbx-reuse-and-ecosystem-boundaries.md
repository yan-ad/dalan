# 0005: Selective DBX reuse and ecosystem boundaries

Status: adopted reference/port boundary; future driver/plugin/AI contracts remain proposals. The initial static catalog adaptation is implemented.

## Context

The user requested cloning https://github.com/t8y2/dbx, reusing its base logic while retaining Dalan's DataGrip-like database workflows, and creating a root roadmap for driver, plugin and AI ecosystems. Dalan remains Rust + GPUI, compact Carbonfox opaque, database-only, macOS first, and ACP-only AI without app BYOK.

DBX at `38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad` separates Rust driver/types/SQL/platform/plugin crates but its application/core includes broad functionality and its UI uses Vue/Tauri. It has concrete driver routing and versioned workers, not a universal stable native plugin ABI. Its provider-based AI and native-sidecar trust assumptions do not match Dalan's product contract.

## Decision

- Keep the upstream research checkout under ignored `target/research/dbx`, with pinned source links and no upstream build/script execution.
- Adapt small original Apache-2.0 modules and tests only after artifact-level provenance/dependency review. Retain complete applicable license/notices and mark changed files; do not imply DBX affiliation.
- Start with Dalan-owned stable driver IDs and descriptive capability lookup/validation. Do not import all of `dbx-core`, its large enum/build generator, patched dependencies or untested engine declarations.
- Preserve typed results, bounded canvas grid, credential boundaries, cache identity, tab ownership and explicit query policies. JSON/wire normalization is not the internal canonical value model.
- Plan database-only extensions using versioned process RPC and app-owned GPUI UI. Ordinary plugins receive scoped host operations, not credentials. Privileged connection-provider workers need a separate trust tier and review.
- Keep ACP AI transport distinct from database-worker RPC and MCP tool serving. Do not port DBX provider-key/HTTP/CLI dialect code; initial AI produces reviewed suggestions only.
- Root `ROADMAP.md` is the single forward plan. Existing detailed implementation guides/checklist/test records remain authoritative evidence of completed behavior.

## Consequences

The first adaptation is deliberately small and tested; this decision does not implement new database engines, a plugin host or live AI sessions. Subsequent ports require conformance/migration/resource-limit tests and updated provenance. Package signing does not establish harmlessness; sidecar separation and ACP permissions are not OS sandboxing. Rich web-plugin UI, Java/JDBC artifacts and generic infrastructure tooling do not enter the default GPUI build.

Project/distribution license, signing authority, native confinement strategy, supported server/agent versions and platform matrix remain explicit owner decisions. No invented dates or upstream coverage/size claims become Dalan promises.

[Canonical roadmap](../../ROADMAP.md) · [Source assessment](../dbx-reuse.md) · [Architecture](../architecture.md) · [ACP contract](../acp.md)

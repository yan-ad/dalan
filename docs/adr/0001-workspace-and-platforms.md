# 0001: Workspace and platform staging

Status: adopted for the scaffold, not a production compatibility commitment.

## Context

The user requires Rust + GPUI, macOS first, then Linux and Windows. GPUI is pre-1.0 and native builds depend on platform tools. Current Zed main initialization differs from published GPUI 0.2.2.

## Decision

Use a Cargo workspace with app/core/drivers/acp crates, edition 2024, pinned Rust 1.98.1, locked dependencies, and exact GPUI 0.2.2 bootstrap pin. Keep core/driver/ACP boundaries free of GPUI. Default app features are headless with a diagnostic binary; desktop is opt-in and macOS-only initially. Use the published API and macOS `font-kit` feature. A development-only runtime-shader feature is explicit, not a release default.

## Consequences

Core tests can run on macOS/Linux/Windows without native UI dependencies. This does not validate desktop portability. Full Xcode/Metal is required for standard macOS shader builds. Linux/Windows desktop backends, native integrations and packaging need separate milestones. GPUI upgrades require coherent source/API review and regression checks. Editor/accessibility capabilities remain spikes, not assumed framework features.

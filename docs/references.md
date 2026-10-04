# Research references

Primary sources inspected during initial scaffold planning. Upstream documentation changes; pinned version source/rustdoc takes precedence over main-branch examples. These links support dependency/protocol facts, not claims that Dalan implements their features.

## GPUI and desktop

- [GPUI site and examples](https://www.gpui.rs/).
- [Published GPUI 0.2.2 rustdoc](https://docs.rs/gpui/0.2.2/gpui/) and its Cargo registry source/example/build script: bootstrap uses `Application::new()`, `font-kit`, and optional `runtime_shaders`.
- [Current GPUI README in Zed main](https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md): pre-1.0 warning, native dependencies, newer `gpui_platform` initialization.
- [Zed macOS development](https://zed.dev/docs/development/macos): Xcode/Metal setup and troubleshooting.
- [Zed Linux development](https://zed.dev/docs/development/linux): later platform research source.
- [Apple Bundle Programming Guide: macOS application structure](https://developer.apple.com/library/archive/documentation/CoreFoundation/Conceptual/CFBundles/BundleTypes/BundleTypes.html): `Contents/Info.plist`, executable and Resources layout, application identity/version metadata. Used for the local `Dalan.app` bundler, not a distribution/signing certification.

## ACP

- [Documentation index](https://agentclientprotocol.com/llms.txt).
- [Architecture](https://agentclientprotocol.com/get-started/architecture).
- [Stable v1 transport](https://agentclientprotocol.com/protocol/v1/transports).
- [Initialization/capabilities](https://agentclientprotocol.com/protocol/v1/initialization).
- [Authentication](https://agentclientprotocol.com/protocol/v1/authentication).
- [Sessions](https://agentclientprotocol.com/protocol/v1/session-setup).
- [Tool calls and permission outcomes](https://agentclientprotocol.com/protocol/v1/tool-calls).
- [Filesystem methods](https://agentclientprotocol.com/protocol/v1/file-system).
- [Official Rust SDK](https://github.com/agentclientprotocol/rust-sdk) and [versioned 2.2.0 API](https://docs.rs/agent-client-protocol/2.2.0/agent_client_protocol/).

SDK 2.2.0 is not protocol v2. Optional permission requests and client capability withholding do not sandbox agents or guarantee universal tool approval. Use the current versioned API rather than older trait-based tutorials.

## Drivers

- [SQLx repository](https://github.com/transact-rs/sqlx) and [0.9.0 documentation](https://docs.rs/sqlx/0.9.0/sqlx/): PostgreSQL/MySQL/MariaDB support, runtime/TLS features, dynamic SQL caveats.
- [redis-rs repository](https://github.com/redis-rs/redis-rs) and [redis 1.7.1 documentation](https://docs.rs/redis/1.7.1/redis/): async runtime/TLS, multiplexed connections, version-sensitive APIs.

SQLx/Redis versions are research observations, not installed adapter dependencies. Reverify before implementation.

## UX references

- [DataGrip query consoles](https://www.jetbrains.com/help/datagrip/query-consoles.html): attached data-source/session workflows.
- [DataGrip Services tool window](https://www.jetbrains.com/help/datagrip/services-tool-window.html): output/results/session and transaction patterns.
- [DataGrip query results](https://www.jetbrains.com/help/datagrip/viewing-query-results.html): related browsing reference.

The user supplied the Zed-like visual/DataGrip-like UX direction. References are used for behavior planning, not copied assets, affiliation, or licenses for third-party source.

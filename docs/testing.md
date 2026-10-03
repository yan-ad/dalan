# Test strategy and release evidence

Status: scaffold tests exist; server, agent transport, desktop interaction, and release suites below are planned.

## Current tests

- Core: engine names/ports, nonzero default limits, read-only denial and read-write confirmation for write/destructive/unknown risk, reads in both modes.
- Drivers: unique first-release identities, planned status, shared MySQL/MariaDB backend with distinct identities.
- ACP: absolute executable/cwd shape, invalid relative paths, default capability helper, stable version constant.

These test helper contracts, not database safety, real permission handling, or interoperability. Tests must stay headless by default. `cargo test --workspace --locked` does not compile the feature-gated GPUI binary.

## Commands

```sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p dalan-app --bin dalan-doctor --locked
cargo doc --workspace --no-deps --locked

# macOS with full Xcode/Metal:
cargo check -p dalan-app --bin dalan --features desktop --locked
cargo build -p dalan-app --bin dalan --features desktop --locked
cargo clippy -p dalan-app --bin dalan --features desktop --locked -- -D warnings
```

Use `runtime-shaders` instead of `desktop` for a development check without the offline Metal compiler, with the limitations in [development](development.md). Standard shader/release validation remains required.

## Planned coverage

| Layer | Required cases |
| --- | --- |
| Domain | Session generations, approvals bound to immutable target/operation, cancellation/unknown outcomes, draft conflicts, bounded events |
| SQL parsing | Strings/comments/quoted identifiers, PostgreSQL dollar quoting, MySQL/MariaDB delimiter/stored-program boundaries, unsupported syntax, no partial guessing |
| PostgreSQL | Real server auth/TLS, schema refresh, lossless types, multiple results, session affinity, errors, cancellation, write uncertainty |
| MySQL and MariaDB | Independent fixtures for both, modes/auth/plugins, DDL implicit commit, nontransactional tables, metadata/type differences |
| Redis | Raw byte keys/arguments, all six value types, TTL -1/-2/races, collection and byte limits, SCAN duplicates/mutation, denied command classes |
| Credentials | Save/read/update/delete, locked/denied/missing keychain, no plaintext fallback or export/log leakage |
| ACP fake peer | Initialization negotiation, unsupported capabilities/auth, session/prompt streaming, permission supplied IDs, cancelled permission races, stderr/oversize/malformed output, process exit/shutdown/reconnect |
| ACP real agents | Explicit version/auth matrix, context consent, no automatic execution, no direct provider fallback |
| UI | Focus/navigation, empty/loading/error/stale/cancelled/uncertain states, grid virtualization, layout resizing, scaled text, dark/light contrast, actual accessibility API behavior |
| Distribution | macOS debug/release Apple Silicon, Intel decision, signing/notarization, install/uninstall; Linux then Windows separately |

Containerized real databases are proposed test fixtures, not a production requirement or an installed dependency today. Bind locally, use throwaway credentials, pin images/versions once the matrix is settled, isolate tests, and ensure cleanup after failure. Avoid production credentials and destructive tests against arbitrary developer URLs. Integration test commands must opt in explicitly.

## Manual bootstrap smoke test

On a logged-in macOS session, launch the desktop binary, verify the notice renders and window resizes/closes, inspect stderr for errors, and record OS/GPU/GPUI/features. There are no app controls to click in this bootstrap. Do not label this as a database workflow test. A compile check alone cannot establish the window appeared.

## Performance and accessibility evidence

Treat budgets in [product plan](product-plan.md#proposed-release-gates) as proposed targets, not measured results. Record hardware, OS, build/features, fixture sizes, methodology, sample sizes, cold/warm runs, and p50/p95 results. Measure input latency/frame time separately from database/network latency. Assert memory bounds under slow consumers and oversize payloads.

Check every adopted text/background pair, focus/control boundary, state, and theme. Keyboard-only execution and scaled text must be exercised in the real GPUI app. Do not claim screen-reader support based on a layout or library choice; audit actual VoiceOver and later platform tooling.

## Completion definition

A milestone closes only with executable workflow evidence and failure-path tests on its supported targets. Record commands and outcomes, unresolved constraints, and any feature exclusions. Hosted CI, cross-platform UI, real servers, and real agents must not be reported passing until those tests actually run.

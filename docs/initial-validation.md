# Initial scaffold validation

This records checks actually run for the initial planning/scaffold task, not release qualification.

Historical baseline: the bootstrap below was subsequently replaced by the [UI shell foundation](ui-foundation.md). Its newer scope and validation results are recorded there; these initial checks are preserved as history.

## Environment

Apple Silicon (`arm64`), macOS 27.0.1, Rust/Cargo 1.98.1, GPUI 0.2.2, ACP SDK 2.2.0. Active developer directory: `/Library/Developer/CommandLineTools`. Offline `metal` compiler unavailable. No system Xcode installation/selection changes were made.

## Results

| Check | Observed outcome |
| --- | --- |
| `cargo fmt --all --check` | Passed |
| `cargo test --workspace --locked` | Passed: 11 unit tests, no failing tests |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo run -p dalan-app --bin dalan-doctor` | Passed; printed planned driver/AI/platform status |
| `cargo doc --workspace --no-deps --locked` | Passed |
| `cargo check -p dalan-app --bin dalan --features runtime-shaders --locked` | Passed |
| `cargo clippy -p dalan-app --bin dalan --features runtime-shaders --locked -- -D warnings` | Passed |
| `cargo build -p dalan-app --bin dalan --features runtime-shaders --locked` | Passed |
| Launch runtime-shader bootstrap binary | Stayed running with no captured stdout/stderr errors during the brief smoke run; deliberately stopped afterward |
| Standard `desktop` compile check | Blocked: GPUI offline Metal shader build fails without `metal`/full Xcode tooling |
| Markdown local file links | Checked all 17 then-existing Markdown files; passed |
| Bootstrap text contrast | `#e8eaed` on `#202124`: 13.36:1 using installed WCAG contrast script |
| Read-only scaffold/scope audit | No concrete defects found; scope/status/platform boundaries consistent |

The GPUI dependency graph emits Cargo future-incompatibility warnings for `block 0.1.6` and `proc-macro-error2 2.0.1`. They did not prevent builds or strict Clippy on the pinned toolchain. Review them before Rust/GPUI upgrades; no claim of long-term compatibility.

## Not verified

- Actual visible rendering, resize/close interactions, keyboard focus, light theme, or assistive technology. No app buttons or database controls exist in the bootstrap; no click-through claim is made.
- Standard offline-shader or release desktop build on full Xcode.
- Hosted CI or headless builds on Linux/Windows. Workflow definitions are present, not run evidence.
- Database adapters/servers, credentials/TLS, persistence, ACP process/transport/auth/sessions. Those are not implemented.
- Visual identity/license, minimum OS/server matrix, Intel support, SQL parser/editor choice, or proposed performance targets. The user subsequently confirmed the app name `Dalan`, meaning “ways” in Javanese.

The planning/design delivery audit confirms explicit source direction, labeled candidate palette, no fabricated support/performance claims, and no unrequested brand assets. It does not certify a finished UI. The next milestone is the foundation shell/parser/accessibility spike followed by a PostgreSQL vertical slice.

[Testing](testing.md) · [Development](development.md) · [Roadmap](roadmap.md)

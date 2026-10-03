# Development setup

## Current workspace

Rust 1.98.1 is pinned in `rust-toolchain.toml`; edition 2024 and workspace minimum Rust 1.98. GPUI 0.2.2 and ACP SDK 2.2.0 are exact pins. Commit `Cargo.lock` with this application workspace. No project license is selected yet.

Default features are headless. This lets product/domain work compile without native GPUI system dependencies. The diagnostic binary is `dalan-doctor`; the future desktop binary is `dalan` and requires the `desktop` feature. Core checks on Linux/Windows are not a claim of supported desktop apps.

```sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p dalan-app --bin dalan-doctor --locked
cargo doc --workspace --no-deps --locked
```

On first checkout, rustup may download the pinned toolchain/components. Cargo downloads crates. Agent and database packages are not installed or started by these commands.

## macOS desktop

Install full Xcode, launch it to finish component installation, accept its license, and select it as the active developer directory. These system changes require user approval; do not run them automatically.

```sh
xcode-select -p
xcrun --find metal
xcrun --find metallib
# After installing Xcode, when needed:
sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer
sudo xcodebuild -license accept
```

On newer Xcode/macOS versions a separate Metal Toolchain download may be required; follow [Zed's macOS troubleshooting](https://zed.dev/docs/development/macos). Command Line Tools alone can lack `metal`/`metallib`.

```sh
cargo check -p dalan-app --bin dalan --features desktop --locked
cargo run -p dalan-app --bin dalan --features desktop --locked
cargo build -p dalan-app --bin dalan --features desktop --release --locked
```

The bootstrap only displays the app name and a not-implemented notice. It does not preview the planned final layout. A real window requires an interactive logged-in macOS GUI session and working Metal hardware.

### Development-only runtime shaders

Published GPUI 0.2.2 has a `runtime_shaders` feature that builds shader source for compilation by Metal at runtime. The app exposes it as `runtime-shaders` to permit a bootstrap build/check on hosts without the offline Metal compiler:

```sh
cargo check -p dalan-app --bin dalan --features runtime-shaders --locked
cargo run -p dalan-app --bin dalan --features runtime-shaders --locked
```

This is an explicit development escape hatch, not a replacement for release Xcode/Metal validation. It does not solve every native-header/runtime problem and cannot prove rendering from a compile check. Do not enable it by default for distributions.

## Why not use Zed main instructions verbatim?

Published GPUI 0.2.2 constructs `Application::new()`. Current Zed main documents separate `gpui_platform::application()` and platform features. This workspace uses the published version's example/API. Upgrade these dependencies coherently; do not mix main-branch snippets and release crates.

## Adding functionality

Implement [roadmap](roadmap.md) vertical slices. Add a managed Tokio runtime with the first database adapter, then explicit credential/TLS/session/result contracts. SQLx/Redis are researched choices, not installed dependencies. ACP's SDK compiles now, but process launch/auth/session handling remain future work. Do not put provider keys or real database data in fixtures.

## Later platforms

Linux desktop: choose GPUI release/revision and X11/Wayland backend features, confirm system package list on supported distributions, verify fonts/clipboard/IME, Secret Service, and packaging. Use [Zed Linux development docs](https://zed.dev/docs/development/linux) as a source, not a guarantee the full editor dependency list applies to this smaller app.

Windows desktop: choose and validate GPUI native backend, Windows SDK/shader compiler/resource tooling, MSVC toolchain, process quoting/termination, credential storage and packaging. Do not bypass the current macOS-only compile guard until a platform spike works. Windows headless CI is intentionally available now.

## CI and verification boundaries

`.github/workflows/ci.yml` runs headless fmt/tests/lint on all three OSes and a standard macOS desktop check/build. A desktop compilation is not an interactive smoke test; keyboard/accessibility/rendering need a real-session test. CI definitions are not evidence that hosted jobs have run. See [testing](testing.md).

# Development setup

## Current workspace

Rust 1.98.1 is pinned in `rust-toolchain.toml`; edition 2024 and workspace minimum Rust 1.98. GPUI 0.2.2 and ACP SDK 2.2.0 are exact pins. Commit `Cargo.lock` with this application workspace. No project license is selected yet.

Default features are headless. This lets product/domain work compile without native GPUI system dependencies. The diagnostic binary is `dalan-doctor`; the desktop binary is `dalan` and requires the `desktop` feature. Core checks on Linux/Windows are not a claim of supported desktop apps.

```sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p dalan-app --bin dalan-doctor --locked
cargo doc --workspace --no-deps --locked
```

On first checkout, rustup may download the pinned toolchain/components. Cargo downloads crates. Agent and database packages are not installed or started by these commands.

## macOS desktop

### Build and open a debug app bundle

From the repository root, with Rust, Python 3 and Xcode Command Line Tools installed:

```sh
./scripts/macos --open
```

The default command builds a debug `Dalan.app` at `target/debug/bundles/Dalan.app`. Runtime shaders are the debug default, so full Xcode's offline Metal compiler is not required for this path. A logged-in macOS GUI session and Metal-capable hardware are still needed. This bundles the actual native executable with `Contents/Info.plist`, `Contents/MacOS/Dalan`, and a Resources directory.

```sh
./scripts/macos                 # Debug bundle, no launch
open target/debug/bundles/Dalan.app  # Launch existing bundle
./scripts/macos --run           # Foreground launch from bundle, keep stdout/stderr
./scripts/macos --offline-shaders --open  # Debug with full Xcode shader compiler
./scripts/macos --release --open          # Release bundle, offline shaders by default
./scripts/macos --help
```

Release output is `target/release/bundles/Dalan.app`. `--release --runtime-shaders` is available explicitly for local optimized experiments, not the default distribution path. Paths derive from Cargo's actual executable artifact, so `CARGO_TARGET_DIR` or Cargo target configuration is respected; use the printed path when it differs from `target/`.

Quit an older Dalan instance before rebuilding or opening again. `open` may activate an already-running instance rather than start the new build. To rebuild and run with immediate logs, use `--run`; Ctrl-C stops the process. To debug from LLDB, build first, then:

```sh
lldb target/debug/bundles/Dalan.app/Contents/MacOS/Dalan
# At the LLDB prompt:
# run
```

Finder/Dock display name is Dalan. Debug and release use provisional local identifiers `local.dalan.debug` and `local.dalan.release`, generated version metadata from Cargo, high-resolution support, and ad-hoc signatures verified by `codesign`. No custom icon, document/file associations, installer, hardened runtime, Developer ID signature, or notarization is claimed. The icon remains macOS's generic application icon until the user selects artwork. These packages are for local development, not release distribution.

The bundle helper uses Python's standard library and existing macOS tools; it adds no Cargo dependency. It only builds/replaces its generated bundle, does not install into `/Applications`, and never modifies settings/source. A failed Cargo build does not overwrite the existing bundle. Bundle metadata/assembly logic has its own standard-library unit tests.

### Full Xcode and offline shaders

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

The desktop app implements the top bar, Database Explorer, database rail, layout controls, status strip, source form and read-only table view. Experimental MySQL/MariaDB connection setup is implemented. Follow [MySQL sources](mysql-sources.md) for credentials, TLS and transport setup. No general file explorer or code viewer is planned. A real window requires an interactive logged-in macOS GUI session and working Metal hardware. See [UI foundation](ui-foundation.md) for controls.

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

Implement [roadmap](roadmap.md) vertical slices, MySQL/MariaDB first. The existing mysql_async 0.37.1 backend runs on a managed two-worker Tokio runtime with generation-aware cancellation and GPUI foreground updates. Source settings and native macOS Keychain are implemented for this experimental slice. SQLx/PostgreSQL and Redis remain researched future choices. ACP's SDK compiles now, but process launch/auth/session handling remain future work. Do not put provider keys or real database data in fixtures.

## Disposable database fixtures

Run `./scripts/test-databases` with a Docker-compatible runtime. It starts `mysql:8.4` and `mariadb:11.4` on random localhost ports, uses a SELECT-granted disposable fixture account and cleans up its own containers/volumes. It does not use production credentials or remove unrelated resources. See [testing](testing.md) and the [scope matrix](mysql-sources.md#scope-and-evidence-matrix). `./scripts/test-secure-transports` opts into trusted custom-CA database TLS direct/HTTP CONNECT and rejection fixtures; `./scripts/test-ssh-transport` opts into actual SSH reads and host-key/identity rejection. Both use generated disposable trust material and temporary directories, cleaning their owned containers/volumes; SSH also removes its dedicated network and generated SSH image, not pulled database images. They do not install OS CA roots or edit user SSH state. Optional selected SSH trust is authoritative; None still uses OpenSSH default user/system known-host reads. Native Keychain validation is separately opt-in: one generated-item round-trip passed with cleanup. Current evidence includes six database smoke, ten secure-transport and six SSH tests on MySQL 8.4.11/MariaDB 11.4.13. Trusted system-CA HTTPS proxy success remains unverified.

## Later platforms

Linux desktop: choose GPUI release/revision and X11/Wayland backend features, confirm system package list on supported distributions, verify fonts/clipboard/IME, Secret Service, and packaging. Use [Zed Linux development docs](https://zed.dev/docs/development/linux) as a source, not a guarantee the full editor dependency list applies to this smaller app.

Windows desktop: choose and validate GPUI native backend, Windows SDK/shader compiler/resource tooling, MSVC toolchain, process quoting/termination, credential storage and packaging. Do not bypass the current macOS-only compile guard until a platform spike works. Windows headless CI is intentionally available now.

## CI and verification boundaries

`.github/workflows/ci.yml` runs headless fmt/tests/lint on all three OSes and a standard macOS desktop check/build, simulated shell input tests, bundle-helper tests, and debug app bundling. An Ubuntu `database-fixtures` job runs direct/HTTP CONNECT and database-TLS/proxy-rejection fixtures; SSH is local opt-in only. Current recorded results are 37 default headless Rust, 27 simulated GPUI and four Python bundle-helper tests passed. The current source-slice Dalan.app was rebuilt and signature-verified, then relaunched with its bundle identity confirmed by macOS. It was left open; manual source-flow/visual validation is separate. A desktop compilation is not an interactive smoke test; keyboard/accessibility/rendering need a real-session test. CI definitions are not evidence that hosted jobs have run. See [testing](testing.md).

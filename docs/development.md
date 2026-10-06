# Development setup

## Current workspace

Rust 1.98.1 is pinned in `rust-toolchain.toml`; edition 2024 and workspace minimum Rust 1.98. The `gpui` alias pins `gpui-kit = 0.7.1` with `tree-sitter-sql`, coordinated with `gpui-pre = 0.3.8`; the old GPUI 0.2 family is removed. ACP SDK 2.2.0 remains pinned. Commit `Cargo.lock` with this application workspace. No project license is selected yet.

Default features are headless. This lets product/domain work compile without native GPUI system dependencies. The diagnostic binary is `dalan-doctor`; the desktop binary is `dalan` and requires the `desktop` feature. Core checks on Linux/Windows are not a claim of supported desktop apps.

```sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p dalan-app --bin dalan-doctor --locked
cargo doc --workspace --no-deps --locked
```

On first checkout, rustup may download the pinned toolchain/components. Cargo downloads crates. Agent and database packages are not installed or started by these commands.

## Embedded metadata storage

The app pins `rusqlite = 0.40.2` with `default-features = false` and only `bundled`; SQLite is compiled through libsqlite3-sys, so a working target C toolchain is required even for headless app tests. No separately installed SQLite daemon, libSQL service, cloud account or new user database driver is required. The resolved registry package declares MIT, with its full notice and SQLite's public-domain dedication in [third-party notices](../THIRD_PARTY_NOTICES.md).

Local profiles stay in version 1 `~/Library/Application Support/Dalan/sources.json`; passwords stay session-only or in native macOS Keychain. `metadata.sqlite3` beside that JSON stores only catalog names/kinds and connection identity, never credentials or rows. Private Unix directory/database permissions are not encryption; avoid committing or sharing real cache files. Startup restore does not need credentials or a network. The cache is disposable, but do not reset a foreign/unsupported file silently; use reported warnings for explicit local recovery. See [cache contracts](mysql-sources.md#persistent-offline-metadata).

Historical metadata verification passed 67 headless, 83 simulated UI and four Python helper tests plus formatting/lint/bundle checks. Commit `4c8af09` passed all five hosted jobs in [run 37201696519](https://github.com/yan-ad/dalan/actions/runs/37201696519). Theme commit `0ca0221` passed all five hosted jobs in [run 37204370849](https://github.com/yan-ad/dalan/actions/runs/37204370849). This is historical evidence for that commit; the wide-grid revision needs its own next-main CI run. Current wide-grid final totals and checks await owner verification; native Keychain was not rerun.

## Historical Carbonfox foundation maintenance (superseded)

The following palette/contrast instructions and counts describe the earlier revision only. **Do not reintroduce its runtime tokens.** Production now uses Kit's default theme and all Kit standard controls; `desktop/theme.rs` maps active semantic `Hsla` colors for app paint. Historical JSON/licenses remain provenance, not runtime configuration. See [current migration](gpui-kit-migration.md). Current production tests/native validation must replace historical counts; Kit initialization owns initial light/dark selection.

The default is **Carbonfox - opaque**, with compact Zed-like UI, not DataGrip visual styling. DataGrip is a database UX/workflow reference only. Shared compiled [theme constants](../crates/app/src/desktop/theme.rs) drive main/source/About and inputs. Keep all window backgrounds explicitly opaque; do not add blur, transparency, a fake light mode or a nonfunctional theme toggle. Normal status shows the theme name unless existing focus help overrides it.

The exact full variant is [vendored](../crates/app/assets/themes/carbonfox-opaque.json) from `cange/nightfox.zed` commit `3511a6f1f665455c70a24d14fd5d2de0eaab58fa`, `themes/nvim-nightfox.json`. Keep both MIT license texts (2024 Christian Angermann and 2021 James Simpson); the original-project license pin is separate from the unknown source revision used by the port. Follow [theme provenance](../crates/app/assets/themes/README.md), not moving-branch colors. Alpha-bearing upstream toolbar/selection/border entries stay unchanged in the reference, while compiled desktop colors composite over PANEL to opaque RGB. The JSON is test/reference-only, not runtime-loaded. Bundle Resources include root third-party notices with both full MIT texts plus the icon license; raw theme JSON/standalone theme license files are not required Resources.

Use system UI type at 13 px and 12 px browse data, 28 px controls/headers, 22 px rows, 3 px control radius and square flush panes. Inputs use shared tokens (opaque selection, FOCUS cursor, MUTED placeholder), not hardcoded colors. Disabled labels use MUTED without opacity. Blue is reserved for focus/active state and Save/Connect primary fill with dark text; normal controls remain neutral. Semantic error text is allowed on panel/input backgrounds, not filled hover controls. Preserve user marker colors and readable independent name/engine labels.

Source geometry remains 1040 × 760, minimum 780 × 560, with 8 px gaps, 16 px scroll padding, footer 8 px vertical/16 px horizontal and existing input/endpoint/checkbox sizes and APIs. Table header/filter padding is 10 px/6 px, footer 10 px/4 px. This is theme/layout work only, not a DB/cache/SSH/password migration. No external font, generated asset or Zed editor code is introduced. Run pure mapping/contrast tests with `runtime-shaders,ui-tests`; no native screenshot/accessibility pass follows from them. Historical theme verification passed 67 headless, 88 simulated UI and four bundle tests with strict lint/build checks; window capture was blocked. See [testing](testing.md#carbonfox-opaque-revision).

## Wide-grid regression and profiling

Current production canvas replaces body per-cell Divs with row/grid `PaintQuad`s and cached `ShapedLine::paint` calls; retain native header controls, exact painted-vs-cached-vs-shaped counters and grapheme-safe fit/cap logic. Keep caches row/column-range bounded with overscan two, release on model page None, and never deep-clone or format the full page during redraw. The 44 px gutter stays x = 0 and shares y only. WHERE/ORDER BY drafts must not autofetch; UI metadata validation must precede credential/network calls. Keep explicit header glyph propagation tests and busy Clear guards; fix fixtures by canceling owned work, not weakening guards.

Run the existing clause, grid, retention and full regression suites; [testing](testing.md#rich-canvas-table-browser) contains commands and verified counts (114 unit, one wire, 154 UI, four bundle and 29 live tests). The narrow fixture reports 40 painted/cached shaped cells, six header controls and zero subsequent tiny-wheel shapes. Integrated 1280 × 720 evidence uses a 906 × 570 body and 300/51,200 materialized cells, not the older 310-cell count. These are simulated counters, not native FPS. Retention estimates are computed once per Arc identity without budget-owned references or notification row walks; 16 MiB/eight pages is best effort and excludes other allocations. See [table-browser guide](table-browser.md).

Thirty-six Lucide utility assets retain the existing revision and ISC/Feather notices; use the [asset provenance](../crates/app/assets/README.md), including the upstream text-initial alias. No project license or brand icon is selected. Historical `4e389d0` passed all five jobs in run 37266563119; this tree's hosted CI remains pending. Native permission-blocked capture and real visual/high-DPI/GPU/accessibility/user retry remain separate gates, not automated-count claims.

The retained `desktop/data_grid.rs` uses shared immutable `Arc<TablePage>` snapshots and viewport-only cell/header text. Keep redraws free of deep page clones and eager full-page formatting; keep explorer projections independent of grid/sidebar wheel events. `grid_viewport.rs` stays GPUI-independent. `serde` enables `rc` for shared snapshot serialization tests, not persisted rows; no new crate, SQL/cache schema migration or license change is involved.

```sh
cargo test -p dalan-app --lib --locked grid_viewport::tests
cargo test -p dalan-app --bin dalan --features runtime-shaders,ui-tests --locked data_grid::tests
cargo test -p dalan-app --bin dalan --features runtime-shaders,ui-tests --locked integrated_scrolling_keeps_shared_snapshot_and_projection_stable -- --nocapture
```

The combined test prints viewport/materialized-cell/sidebar/projection counts; these are operation counts per simulated frame, not FPS or millisecond timings. Full Cargo elapsed time includes compilation and is not a rendering benchmark. Final owner-run wide-grid full-suite/format/lint/bundle checks remain separate gates; [testing](testing.md#wide-grid-performance-revision) records the evidence.

For native follow-up, rebuild the latest debug bundle with `./scripts/macos --open` (runtime shaders by default), quit the older process, then retry the user's wide table and record viewport, row/column dimensions, high-DPI scale and GPU/frame profiling. Do not store private fixture rows or connection details, change OS capture permissions, or modify profiles/secrets to obtain a screenshot. Synthetic tests use names such as `database_0` only. Native screenshot capture remains unavailable; no latency/FPS or accessibility pass is implied. Full-Xcode/offline-Metal and optimized release profiling remain future gates.

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
./scripts/macos --icon-composer --open    # Optional icon compilation with full Xcode
./scripts/macos --release --open          # Optimized Kit runtime-shader bundle
./scripts/macos --help
```

Release output is `target/release/bundles/Dalan.app`. Both debug and release use Kit's runtime-shader platform; `--runtime-shaders` remains a compatibility spelling and `--offline-shaders` rejects explicitly. Paths derive from Cargo's actual executable artifact, so `CARGO_TARGET_DIR` or Cargo target configuration is respected; use the printed path when it differs from `target/`.

Quit an older Dalan instance before rebuilding or opening again. `open` may activate an already-running instance rather than start the new build. To rebuild and run with immediate logs, use `--run`; Ctrl-C stops the process. To debug from LLDB, build first, then:

```sh
lldb target/debug/bundles/Dalan.app/Contents/MacOS/Dalan
# At the LLDB prompt:
# run
```

Finder/Dock display name is Dalan. Debug and release use provisional local identifiers `local.dalan.debug` and `local.dalan.release`, generated version metadata from Cargo, high-resolution support, and ad-hoc signatures verified by `codesign`. The bundle includes the user-supplied app artwork through a conventional `.icns`, with PNG fallback and optional Icon Composer compilation. No document/file associations, installer, hardened runtime, Developer ID signature, or notarization is claimed. These packages are for local development, not release distribution.

### Supplied app icon

The original `dalan-db.icon` is retained unchanged at `crates/app/assets/brand/Dalan.icon`. The README and portable fallback use its PNG layer resized to 1024 px; `Dalan.icns` contains conventional macOS sizes. The default build copies both fallbacks into Resources and sets `CFBundleIconFile`.

```sh
./scripts/prepare-icons                    # Regenerate PNG/icns using sips + iconutil
./scripts/macos --open                     # Verified conventional icon fallback
./scripts/macos --icon-composer --open     # Requires full Xcode 26+ actool
```

`.icon` source cannot be activated by copying it into a bundle. The optional compiler path produces `Assets.car` and sets `CFBundleIconName` only after receiving valid compiler output, while retaining the `.icns` fallback. Missing/unsupported tooling fails explicitly without replacing the existing app. Real Icon Composer compilation is not verified on the current Command-Line-Tools-only host; unit tests cover orchestration with a simulated compiler, not Apple renderer output. Linux/Windows packaging is still future work and can use the supplied PNG. macOS may cache the previous Dock icon; fully quit older instances before launching the rebuilt bundle. See [artwork provenance](../crates/app/assets/brand/README.md). Artwork redistribution terms remain unspecified.

The bundle helper uses Python's standard library and existing macOS tools; it adds no Cargo dependency. It only builds/replaces its generated bundle, does not install into `/Applications`, and never modifies settings/source. A failed Cargo build does not overwrite the existing bundle. Bundle metadata/assembly logic has its own standard-library unit tests.

### Native toolchain and optional full Xcode

Command Line Tools support the normal Kit build. Full Xcode is optional for Icon Composer and other Apple distribution tooling. Installing/selecting Xcode requires user approval; do not perform system changes automatically.

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

The desktop app implements the top bar, Database Explorer (without an activity rail), layout controls, status strip, a dedicated source dialog window and the main read-only browser/table workspace. Experimental MySQL/MariaDB connection setup is implemented. Follow [MySQL sources](mysql-sources.md) for credentials, TLS and transport setup. No general file explorer or code viewer is planned. A real window requires an interactive logged-in macOS GUI session and working Metal hardware. See [UI foundation](ui-foundation.md) for controls.

### Kit runtime shaders

The coordinated Kit platform enables runtime shaders and font-kit. Full Xcode's offline Metal compiler is not required for regular debug/release builds, but a working native Metal runtime/SDK is still required. The `runtime-shaders` application feature remains a compatibility alias for `desktop`; it is not a separate underlying platform. `--offline-shaders` is rejected instead of falsely claiming a build mode Kit does not expose here. Optional Icon Composer compilation still requires full Xcode 26+.

## Why not use Zed main instructions verbatim?

Production uses Kit's `gpui::application`, `gpui::init` and `open_window`/Base Root across main/source/About/SSH windows. Keep core/platform/component dependencies coordinated; do not mix old GPUI 0.2 or Zed main snippets with the pinned Kit family. Use Kit controls/default theme, not another app-owned input/editor engine. Retain password native privacy delegation and test Unicode/IME, masked clipboard, dropdown focus/dismissal, save guards and current SQL limits.

## Adding functionality

Implement [roadmap](roadmap.md) vertical slices, MySQL/MariaDB first. The existing mysql_async 0.37.1 backend runs on a managed two-worker Tokio runtime with generation-aware cancellation and GPUI foreground updates. Source settings and native macOS Keychain are implemented for this experimental slice. SQLx/PostgreSQL and Redis remain researched future choices. ACP's SDK compiles now, but process launch/auth/session handling remain future work. Do not put provider keys or real database data in fixtures.

## Disposable database fixtures

Run `./scripts/test-databases` with a Docker-compatible runtime. It starts `mysql:8.4` and `mariadb:11.4` on random localhost ports, uses a SELECT-granted disposable fixture account and cleans up its own containers/volumes. It does not use production credentials or remove unrelated resources. See [testing](testing.md) and the [scope matrix](mysql-sources.md#scope-and-evidence-matrix). `./scripts/test-secure-transports` opts into trusted custom-CA database TLS direct/HTTP CONNECT and rejection fixtures; `./scripts/test-ssh-transport` opts into actual SSH reads and host-key/identity rejection. Both use generated disposable trust material and temporary directories, cleaning their owned containers/volumes; SSH also removes its dedicated network and generated SSH image, not pulled database images. They do not install OS CA roots or edit user SSH state. Optional selected SSH trust is authoritative; None still uses OpenSSH default user/system known-host reads. Native Keychain validation is separately opt-in: one generated-item round-trip passed with cleanup. Current metadata-revision live reruns include seven direct/CONNECT/authentication, ten secure-transport and six SSH tests on MySQL 8.4.11/MariaDB 11.4.13. Trusted system-CA HTTPS proxy success remains unverified.

## Later platforms

Linux desktop: choose GPUI release/revision and X11/Wayland backend features, confirm system package list on supported distributions, verify fonts/clipboard/IME, Secret Service, and packaging. Use [Zed Linux development docs](https://zed.dev/docs/development/linux) as a source, not a guarantee the full editor dependency list applies to this smaller app.

Windows desktop: choose and validate GPUI native backend, Windows SDK/shader compiler/resource tooling, MSVC toolchain, process quoting/termination, credential storage and packaging. Do not bypass the current macOS-only compile guard until a platform spike works. Windows headless CI is intentionally available now.

## CI and verification boundaries

`.github/workflows/ci.yml` runs headless fmt/tests/lint on all three OSes and a standard macOS desktop check/build, simulated shell input tests, bundle-helper tests, and debug app bundling. An Ubuntu `database-fixtures` job runs direct/HTTP CONNECT and database-TLS/proxy-rejection fixtures; SSH is local opt-in only. Historical source-dialog results are 54 headless Rust, 63 simulated GPUI and four Python bundle-helper tests passed; metadata baseline is 67/83/four. Historical theme totals are 67/88/four; current wide-grid results are 74 headless, 99 simulated UI and four bundle tests passed with strict lint/build checks. Older source-slice counts remain historical in [testing](testing.md). The current source-slice Dalan.app was rebuilt and signature-verified, then relaunched with its bundle identity confirmed by macOS. It was left open; manual source-flow/visual validation is separate. A desktop compilation is not an interactive smoke test; keyboard/accessibility/rendering need a real-session test. Historical hosted run 37192402473 passed macOS/Linux headless, database fixtures and macOS desktop, but failed the Windows SSH-key unit fixture with OS error 123: a newline filename is invalid on Windows. The fixture now creates newline/carriage-return/tab names only on Unix; all metadata-discovery assertions still run on Windows. No checks are disabled and the workflow is unchanged. Metadata commit `4c8af09` subsequently passed all five jobs in run 37201696519. Theme commit `0ca0221` passed all five jobs in run 37204370849; the current wide-grid revision still needs its own push/run. Historical success is not evidence of a green updated main. See [testing](testing.md).

## Workspace and console implementation boundaries

Read [query consoles](query-consoles.md) and [ADR 0004](adr/0004-workspace-tabs-and-read-only-consoles.md) before changing execution or session scope. The app's pure `workspace_tabs` registry defines qualified table deduplication, unique Console N identity and 32-tab capacity. Desktop `source_workspace`, `query_console` and `sql_editor` retain tab-local entities; root profile/catalog/credential ownership is shared, not duplicated persistence. Driver `query` validates before opening a fresh connection and never adds pagination SQL. Keep tab/job generations independent and preserve prior result metadata on failure.

The SQL editor is a scoped native GPUI input adaptation with attribution in existing third-party notices, not an imported application editor or new generic file viewer. Lucide utility assets remain at the existing pin. sqlparser 0.62.0 enables its visitor feature; check locked cross-platform tests and strict lint when updating it. No project license is selected or new LICENSE file implied.

Current results: 93 headless Rust tests, 132 simulated GPUI tests and four Python bundle-helper tests passed, along with formatting, strict lint and signed debug bundle/resource checks. Live scripts retain 23 unique cases (7 direct/CONNECT/authentication, 10 TLS, 6 SSH), with positive console SQL over actual owned protocol fixtures, not production credentials. Rejected writes are classified before network. Historical `cf24e48` passed all five jobs in [run 37215973701](https://github.com/yan-ad/dalan/actions/runs/37215973701); this revision's main CI is pending. Structural visible-line/grid tests are not native FPS, IME/accessibility or manual visual evidence. See [testing](testing.md#workspace-tabs-and-query-consoles).

## Source-manager development

The current configuration contract is in [source management](source-management.md). Form/dialog/model ownership remains in `desktop/source_*`; `desktop/ssh_manager.rs` owns its independent window and remote-true test lifecycle, while `ssh_config_store.rs` owns reusable credential-free metadata. Use isolated temporary files for repository/reference/no-overwrite/permission tests, not actual Application Support settings or user credentials. Preserve source/cache versions and serde defaults; do not add arbitrary JDBC fields or secrets to JSON.

Options are applied to browse/console and metadata query steps; catalog discovery retains its global 120-second deadline. Do not restore a hardcoded 20-second query cap above a user-selected larger timeout. No Auth must never read Keychain; testing credential behavior requires a substitute credential store, not authorizing a real remembered password. Schema-selection tests must retain complete SQLite snapshots, exact comma-containing names, empty-selection source rows and generation guards.

SSH manager cancellation must kill its owned child on input changes/close; strict remote `true` testing is not a driver forwarding handshake. Default config isolation (`-F /dev/null`) and explicit local-command warnings must stay intact. Simulated native-picker tests are not real Open-dialog interaction. No encrypted-key passphrase UI, SOCKS or IDE truststores are implemented. The existing `url` dependency is declared directly by drivers; no new package/icon/license choice is introduced.

Expected owner-run totals are 105 headless (4 ACP/48 app/5 core/48 drivers), 141 UI and four Python tests, with 29 expanded unique live cases; final confirmation belongs to the primary run, not these documentation edits. Historical `9ecae4c` passed all five hosted jobs in [run 37255792022](https://github.com/yan-ad/dalan/actions/runs/37255792022); current hosted CI and native review remain separate gates. See [testing](testing.md#source-manager-redesign).

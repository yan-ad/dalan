# Test strategy and release evidence

Status: headless and simulated UI tests exist, plus verified disposable MySQL/MariaDB live fixtures. Current recorded suites and generated native Keychain validation passed as detailed below. Agent transport, full release and actual accessibility suites remain future work.

## Current tests and evidence

- Core and ACP retain headless policy/schema/path tests; ACP has no live transport.
- Drivers test experimental MySQL/MariaDB identity, source validation, generated read/filter SQL, value conversion, bounds and relay behavior.
- App headless tests cover layout and versioned source-store/credential failure contracts. One opt-in generated native Keychain round-trip passed and cleaned up its item. Session-only Save tests use no Keychain call; failure tests cover save errors and the failed-load overwrite guard.
- GPUI tests cover shell controls plus source-form/table interactions and stale states. Simulated tests do not establish actual macOS click-through, VoiceOver or text-scaling behavior.
- Bundle helper has four standard-library Python tests, which do not imply app launch.

Historical source-slice results: **37 default headless Rust tests passed** (4 ACP, 13 app, 5 core, 15 driver); **27 simulated GPUI tests passed** (3 native-input, 5 source-form, 4 source-model, 3 browser, 12 shell); **four Python bundle-helper tests passed**. These predate the icon/sorting/export revision and are retained as historical evidence. Default workspace tests remain headless and do not compile the feature-gated GPUI binary.

### Latest icon, sorting and loaded-export revision

Current verified results: **48 default headless tests** (4 ACP, 20 app, 5 core, 19 driver), **42 simulated GPUI tests**, four Python bundle-helper tests and one generated native Keychain round-trip passed. Formatting, both strict Clippy paths, debug bundle build, plist lint, ad-hoc signature verification and bundled license-resource checks passed. The 22 live database/transport tests also passed with sorting coverage. These are separate from manual visual/accessibility, hosted CI and release/offline-Metal verification.

New coverage includes eight embedded Lucide assets and labels; header click/Enter/Space sort cycles, column switches, retained filters and offset reset; metadata-validated quoted SQL and primary-key ties; UTF-8 CSV escaping, NULL syntax, formulas and exact decimal representation; bounds/truncation, private publication, no overwrite/symlink overwrite; picker cancellation, duplicate busy requests, generation changes and unavailable-page rejection. The 22 live cases below were rerun with sorting on actual direct, CONNECT, TLS and SSH routes. No new native manual source/save-picker, visual or accessibility evidence is claimed.

### Verified live database slice

Six smoke tests passed against MySQL **8.4.11** and MariaDB **11.4.13** on the same fixture. Coverage includes direct TCP and anonymous HTTP CONNECT reads, all seven filter operators and fixture value representations, BASE TABLE browsing/view rejection, and default verified TLS rejecting untrusted servers. A separate secure-transport suite passed **ten tests**, five per engine: trusted custom-CA database TLS reads over direct TCP and HTTP CONNECT, wrong database hostname rejection, untrusted database CA rejection and untrusted HTTPS proxy rejection. A separate actual SSH suite passed **six tests**: two positive transport reads, two wrong-host-key rejections and two wrong-identity rejections. The SSH fixture exercises a selected known-host path containing spaces. Trusted system/native-CA HTTPS proxy success remains unverified; database CA selection is not proxy CA configuration. This is not a general auth/plugin/topology or managed-service compatibility claim. See the [scope matrix](mysql-sources.md#scope-and-evidence-matrix).

```sh
# Explicitly opt-in, disposable localhost fixtures only:
./scripts/test-databases
./scripts/test-secure-transports
./scripts/test-ssh-transport
# Generated native Keychain item, requires unlocked macOS Keychain, may prompt:
cargo test -p dalan-app --locked generated_keychain_item_round_trip -- --ignored --nocapture
```

The database helper requires a running Docker-compatible runtime, uses `mysql:8.4` and `mariadb:11.4`, binds random localhost ports and grants the fixture user SELECT. It cleans up only its own containers/volumes, including failure paths. Tags may resolve to new patches, so record exact server versions per run. Never substitute production credentials or an arbitrary developer server. The original six-test smoke fixture disables database TLS for success cases; trusted database-TLS evidence comes from the separate secure suite. Secure and SSH helpers use disposable generated certificates/keys and temporary directories, cleaning their owned containers/volumes on exit; SSH additionally cleans its dedicated network and generated SSH image. Pulled database images are retained. They require local OpenSSL/SSH key tooling as applicable and do not modify user known-host files or OS CA trust. This does not imply that ordinary SSH default mode avoids reading user/system trust.

## Commands

```sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p dalan-app --bin dalan-doctor --locked
cargo doc --workspace --no-deps --locked
python3 -m unittest discover -s scripts/tests -v

# macOS with full Xcode/Metal:
cargo check -p dalan-app --bin dalan --features desktop --locked
cargo build -p dalan-app --bin dalan --features desktop --locked
cargo clippy -p dalan-app --bin dalan --features desktop --locked -- -D warnings
```

Use `runtime-shaders` instead of `desktop` for a development check without the offline Metal compiler, with the limitations in [development](development.md). Standard shader/release validation remains required.

```sh
# macOS only; GPUI test support stays off in default headless builds:
cargo test -p dalan-app --bin dalan --features runtime-shaders,ui-tests --locked
cargo clippy -p dalan-app --bin dalan --features runtime-shaders,ui-tests --all-targets --locked -- -D warnings
```

GPUI's test platform does not request real display frames; resize tests explicitly refresh. Its 0.2.2 debug-bound map retains removed-element entries, so compact-layout checks use the current layout decision and newly painted content geometry rather than stale absence checks.

The current source-slice Dalan.app was rebuilt, plist-linted, ad-hoc signed and signature-verified, then reopened after quitting the older instance. macOS confirmed the Dalan display name and bundle executable; the app was left open. This does not establish manual source-flow, visual or accessibility verification. Hosted CI, full-Xcode/offline-Metal and release checks remain unverified. CI now defines an Ubuntu `database-fixtures` job for direct/HTTP CONNECT and secure database TLS/proxy-rejection fixtures, not SSH fixtures.

## macOS bundle smoke test

```sh
./scripts/macos
plutil -lint target/debug/bundles/Dalan.app/Contents/Info.plist
codesign --verify --strict --verbose=2 target/debug/bundles/Dalan.app
open target/debug/bundles/Dalan.app
```

Use the actual printed output path if Cargo target configuration changes it. Confirm the launched process is inside `Dalan.app/Contents/MacOS/Dalan`, the macOS app name is Dalan, and an older loose-binary instance is not being activated. `./scripts/macos --run` captures foreground diagnostics; LLDB can run the bundle executable. Release/offline-shader smoke testing still requires full Xcode. Ad-hoc signature verification does not establish Gatekeeper/notarization or release readiness.

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

Containerized MySQL/MariaDB fixtures are implemented in `scripts/test-databases`, not a production requirement. Other engine fixtures remain planned. Bind locally, use throwaway credentials, pin images/versions once the matrix is settled, isolate tests, and ensure cleanup after failure. Avoid production credentials and destructive tests against arbitrary developer URLs. Integration test commands must opt in explicitly.

## Manual shell smoke test

On a logged-in macOS session, launch Dalan.app, verify the top bar/native controls/Database Explorer render and the real source form/table view render. Open About Dalan from the native menu and check its Cargo version, meaning, scope, and fixed window size. Inspect the 28 px bot-message-square button and its AI · ACP tooltip/focus help; exercise it, Cmd-Shift-A, its close button, focused Escape/focus return, and Escape outside the panel; confirm honest Not connected copy and no prompt/provider fields. Exercise every control in [UI foundation](ui-foundation.md), resize to 720 × 480 and back with ACP both closed and open, verify retained database preferences after close, inspect stderr, and record OS/GPU/GPUI/features. Simulated GPUI event tests are not manual macOS click-through or VoiceOver evidence. Exercise Name focus on form open and Escape cancellation, Test without save, Save without automatic connect, explicit Connect, optional database discovery, filtering/paging, clickable and Enter/Space ascending/descending/none sort cycles with retained filter and offset reset, loaded CSV native save/cancel/error and no-overwrite behavior, Edit, confirmed Delete, password re-entry after restart, TLS warnings and stale/error/cancel states. Delete must never remove server objects. Distinguish this manual evidence from verified driver fixtures; agent I/O remains unimplemented. General file browsing is excluded by product scope, not deferred.

## Performance and accessibility evidence

Treat budgets in [product plan](product-plan.md#proposed-release-gates) as proposed targets, not measured results. Record hardware, OS, build/features, fixture sizes, methodology, sample sizes, cold/warm runs, and p50/p95 results. Measure input latency/frame time separately from database/network latency. Assert memory bounds under slow consumers and oversize payloads.

Check every adopted text/background pair, focus/control boundary, state, and theme. Keyboard-only execution and scaled text must be exercised in the real GPUI app. Do not claim screen-reader support based on a layout or library choice; audit actual VoiceOver and later platform tooling.

## Completion definition

A milestone closes only with executable workflow evidence and failure-path tests on its supported targets. Record commands and outcomes, unresolved constraints, and any feature exclusions. Hosted CI, cross-platform UI, real servers, and real agents must not be reported passing until those tests actually run.

# Test strategy and release evidence

Status: headless and simulated UI tests exist, plus verified disposable MySQL/MariaDB live fixtures. Current recorded suites and generated native Keychain validation passed as detailed below. Agent transport, full release and actual accessibility suites remain future work.

## Current tests and evidence

- Core and ACP retain headless policy/schema/path tests; ACP has no live transport.
- Drivers test experimental MySQL/MariaDB identity, source validation, generated read/filter SQL, value conversion, bounds and relay behavior.
- App headless tests cover layout and versioned source-store/credential failure contracts. One opt-in generated native Keychain round-trip passed and cleaned up its item. Session-only Save tests use no Keychain call; failure tests cover save errors and the failed-load overwrite guard.
- GPUI tests cover shell controls plus source-form/table interactions and stale states. Simulated tests do not establish actual macOS click-through, VoiceOver or text-scaling behavior.
- Bundle helper has four standard-library Python tests, which do not imply app launch.

Historical source-slice results: **37 default headless Rust tests passed** (4 ACP, 13 app, 5 core, 15 driver); **27 simulated GPUI tests passed** (3 native-input, 5 source-form, 4 source-model, 3 browser, 12 shell); **four Python bundle-helper tests passed**. These predate the icon/sorting/export revision and are retained as historical evidence. Default workspace tests remain headless and do not compile the feature-gated GPUI binary.

### Historical icon, sorting and loaded-export revision

Verified results for that revision: **48 default headless tests** (4 ACP, 20 app, 5 core, 19 driver), **42 simulated GPUI tests**, four Python bundle-helper tests and one generated native Keychain round-trip passed. Formatting, both strict Clippy paths, debug bundle build, plist lint, ad-hoc signature verification and bundled license-resource checks passed. The 22 live database/transport tests also passed with sorting coverage. These are separate from manual visual/accessibility, hosted CI and release/offline-Metal verification.

New coverage includes nine embedded Lucide assets and labels; header click/Enter/Space sort cycles, column switches, retained filters and offset reset; metadata-validated quoted SQL and primary-key ties; UTF-8 CSV escaping, NULL syntax, formulas and exact decimal representation; bounds/truncation, private publication, no overwrite/symlink overwrite; picker cancellation, duplicate busy requests, generation changes and unavailable-page rejection. The 22 live cases below were rerun with sorting on actual direct, CONNECT, TLS and SSH routes. No new native manual source/save-picker, visual or accessibility evidence is claimed.

### Connection UX and uncached authentication revision

The live rerun passed **23 unique tests**: **seven direct/HTTP CONNECT/authentication**, **ten TLS** and **six SSH** cases. A standalone fresh-account test passed as well, then passed again within the seven-test smoke suite; count it once, not twice. MySQL 8.4.11 and MariaDB 11.4.13 remain the recorded fixture versions.

The MySQL test uses a newly provisioned `caching_sha2_password` account for its first login with TLS disabled, exercising mysql_async RSA authentication. The original readiness login used the reader account and warmed its cache; readiness now uses fixture root and creates the fresh account afterward. This is disposable first-login evidence, not universal authentication compatibility or proof of a fix for the reported remote account. A credential-free remote probe connected at TCP level but reset before the MySQL greeting; no credentials were sent. A user retry with the new sanitized diagnostic remains necessary.

Final verification passed **52 headless Rust**, **46 simulated GPUI** and **four Python** tests, plus formatting, both strict Clippy paths, debug bundle build, plist lint and ad-hoc signature verification. Earlier generated Keychain and bundle validation above remain historical; Keychain was not rerun for this revision. New coverage targets typed secret-free diagnostics, metadata-only bounded SSH identity discovery, stored Tab-stop state and visual-order traversal, Unicode/password double-click selection without drag shrink, secret clipboard suppression and native surrounding-text privacy. Simulation is not native manual/IME/accessibility verification. No dependencies or license changes were added.

The rebuilt Dalan.app was reopened after quitting the older instance; macOS confirmed its bundle executable and the app was left open. This verifies launch, not the reported remote login or manual visual behavior.

## Historical CA picker and inline credential controls

The source form now has a native single-file CA picker beside its editable path, a visible square/check and clickable Keychain label beside Password, and no redundant form-title/engine strip. Regression tests cover checkbox position and click/Space/Enter behavior, saving guards, updated Tab/Shift-Tab order, file-only picker options, picked-path editing, cancellation, stale manual edits and dialog errors. GPUI 0.2.2's test platform does not implement native Open dialogs, so tests exercise the shared picker completion handler; native macOS dialog interaction remains a manual check.

This revision passed 52 headless tests, 49 simulated UI tests and four Python bundle-helper tests, formatting, both strict Clippy paths, debug bundle build, plist lint and ad-hoc signature verification. No database transport, TLS policy or Keychain backend changes were made; live database and generated-Keychain results above are historical rather than rerun evidence for this UI-only change. The user identified WireGuard/VPN routing as the reason for the earlier endpoint issue; no private endpoint details are recorded here.

## Database Explorer redesign: verified

Verified results: **54 headless Rust tests** (22 driver, 23 app, 5 core, 4 ACP), **57 simulated GPUI tests** and **four Python bundle-helper tests** passed. Formatting, both strict Clippy paths, debug bundle build, plist lint, signature verification and bundled-license checks passed. The previous CA-picker record of 52 headless and 49 UI remains historical. New UI coverage includes the center action with the explorer hidden.

Scoped migration/color tests passed: legacy version 1 profiles without color load exactly as None, mixed-case `#AB12cd` survives version 1 serialization unchanged, malformed colors fail load/save without overwriting stored bytes, and password fields remain rejected. No automatic legacy rewrite or Keychain migration is introduced.

New regression scope covers fourteen embedded Lucide assets and abstract engine cues, the four compact toolbar actions and disabled states, labeled color presets/manual validation and form traversal, and the centered no-source action with mouse/Enter/Space. Shell coverage removes rail/header-hide selectors, checks the bottom-left toggle and retained closed preference, 6 px padding on both sides, the 462 px clamp at 720 px, and unchanged compact ACP suppression. Formatting, strict lint, suites and bundle/signature/resource checks passed. Earlier live transport and generated Keychain results are historical, not reruns for this redesign. Native keyboard/visual/VoiceOver/scaled-text checks remain unverified.

## Compact lazy explorer

The scoped simulated GPUI suite passed **71 tests**. Coverage includes the removed explorer title header, six 28 px toolbar actions, 22 px rows, nonblank constrained/ellipsized names, Unicode/quoted collision-safe identities, single-list navigation, tree-focus guards and toolbar Enter/Space, disclosure propagation, lazy caching, branch-local cancellation/errors, request generations/concurrency and explorer selection independent of table-page identity. Views stay unavailable and do not issue browse queries; Refresh reloads only the explorer-selected catalog root. Expand Loaded has no network fan-out and Collapse All preserves cache.

The large-catalog regression loads **1,000 databases**, verifies **at most 40 painted rows**, scrolls to **row 900** and exercises **End**. The flattened tree is rebuilt on model notification rather than wheel events, and the uniform list renders the visible range only. These are tested structural/render-bound contracts, not measured FPS, latency or a manual native smoothness pass. The reported blank-button cause was not measured; no font/GPU diagnosis is verified.

Final local verification passed **57 headless tests** (26 app, 22 driver, 5 core, 4 ACP), **71 simulated UI tests** and **four Python bundle-helper tests**, plus formatting, both strict Clippy paths, debug app build, plist lint, ad-hoc signature and bundled-license checks. Live database and generated Keychain results remain historical; no native smoothness/FPS/latency, visual or accessibility claim is implied. Hosted verification of the new revision remains pending.

The current icon set is **nineteen** pinned Lucide assets: ChevronRight, Folder, Table, ExpandTree (`list-tree`) and CollapseTree (`chevrons-down-up`) add five to the existing fourteen at the same fixed revision, with complete existing ISC/Feather MIT license notices. No dependencies or project-license choice are added. The historical green CI result for `3c4fdae` applies only to that revision; the new revision's all-five-job CI gate is pending.

## Source dialog window and Windows fixture fix

Historical source-dialog local suites passed **54 headless Rust**, **63 simulated GPUI** and **four Python bundle-helper** tests. These supersede the explorer redesign's historical 57 UI tests; they do not constitute native visual/accessibility evidence. Live database/transport and generated native Keychain evidence above is retained as historical: those suites were not rerun for this window/layout and test-fixture change. Source storage, credentials and backend transport contracts are unchanged; no new dependencies, icons or license changes are introduced.

Simulated regression coverage includes:

- A single reused application-wide Data Sources · Dalan window, initial 1040 × 760 (minimum 780 × 560), preserving draft edits across Add/Manage/center triggers and focusing Name on creation.
- Generation-aware replacement of a loaded draft inside the same window, without resetting forms on ordinary model notifications; successful-save notification closes/releases the dialog and allows reopening, without auto-connect. Save-completion tests simulate UI state, not a native Keychain transaction.
- Cancel, Escape, Cmd-W and native close discard/cancel the draft/test; saving guards reject action/native close during credential/JSON save. Backend cancellation continues to own its relay lifecycle.
- Inline database/SSH/HTTP/HTTPS Host–Port geometry and real input Tab/Shift-Tab sequence. Defaults are localhost with ports 3306/22/8080/443; Port remains 96 px wide (minimum 80 px).
- Draw-bound regressions at 1040 × 760 and 850 × 600 with long/short values, long key labels, password and color changes. The max-720 px natural-height body scrolls, input/candidate rows retain 28/30 px heights, the 150 px key list ellipsizes labels, and the footer stays intact. This addresses a real flex-shrink defect even if the supplied screenshot showed an older sidebar form; it is not dismissed as screenshot staleness.

This is a separate normal dialog window, not an OS modal sheet or a main-window focus trap. Native manual dialog visual/keyboard/VoiceOver/scaled-text verification remains open. Existing screenshot capture attempts were blocked by Screen Recording permissions; no permission changes or native visual pass are claimed. Simulated drawing bounds are layout evidence only.

### Hosted CI: historical failure and pending main verification

Hosted GitHub Actions run **37192402473** confirmed a failure only in the Windows headless SSH-key unit fixture: creating `id_bad\nname` fails with Windows OS error **123**, because control characters are invalid filenames. The macOS and Ubuntu headless jobs, Ubuntu database fixtures and macOS desktop job passed in that historical run. These are observed historical logs, not new-branch or updated-main pass claims.

The fixture now creates newline, carriage-return and tab filenames only under `cfg(unix)`, where they can be represented and discovery rejection can be tested. Windows still runs the complete metadata-discovery assertions for valid filenames and exclusions. No test checks are disabled, no workflow is changed, and no production filename policy is weakened.

Historical source-dialog fix **3c4fdae** passed all five jobs in [run 37194669634](https://github.com/yan-ad/dalan/actions/runs/37194669634), including Windows and the native offline-Metal macOS bundle. The current compact-tree revision still requires its own all-five-job run; do not infer that result from the prior commit.

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

The database helper requires a running Docker-compatible runtime, uses `mysql:8.4` and `mariadb:11.4`, binds random localhost ports and grants the fixture user SELECT. It cleans up only its own containers/volumes, including failure paths. Tags may resolve to new patches, so record exact server versions per run. Never substitute production credentials or an arbitrary developer server. The direct/CONNECT/authentication smoke fixture disables database TLS for success cases; trusted database-TLS evidence comes from the separate secure suite. Secure and SSH helpers use disposable generated certificates/keys and temporary directories, cleaning their owned containers/volumes on exit; SSH additionally cleans its dedicated network and generated SSH image. Pulled database images are retained. They require local OpenSSL/SSH key tooling as applicable and do not modify user known-host files or OS CA trust. This does not imply that ordinary SSH default mode avoids reading user/system trust.

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

The current source-slice Dalan.app was rebuilt, plist-linted, ad-hoc signed and signature-verified, then reopened after quitting the older instance. macOS confirmed the Dalan display name and bundle executable; the app was left open. This does not establish manual source-flow, visual or accessibility verification. Hosted evidence is recorded above; next-main CI verification, local full-Xcode/offline-Metal and release checks remain open. CI defines an Ubuntu `database-fixtures` job for direct/HTTP CONNECT and secure database TLS/proxy-rejection fixtures, not SSH fixtures.

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

On a logged-in macOS session, launch Dalan.app, verify the top bar/native controls/Database Explorer render and the browser/table workspace renders and Add opens the dedicated source dialog. Open About Dalan from the native menu and check its Cargo version, meaning, scope, and fixed window size. Inspect the 28 px bot-message-square button and its AI · ACP tooltip/focus help; exercise it, Cmd-Shift-A, its close button, focused Escape/focus return, and Escape outside the panel; confirm honest Not connected copy and no prompt/provider fields. Verify the activity rail and header hide/minimise control are absent, exercise the bottom-left panel-left toggle and retained closed preference, and check that the explorer title header is absent and its single 32 px toolbar has six 28 px icons/tooltips/disabled states. Inspect readable engine labels and abstract icons, optional color presets/manual hex and marker-only presentation. With no sources, activate the centered Connect to a Source by mouse/Enter/Space with explorer shown and hidden. Exercise every control in [UI foundation](ui-foundation.md), resize to 720 × 480 and back with ACP both closed and open, verify retained database preferences after close, inspect stderr, and record OS/GPU/GPUI/features. Simulated GPUI event tests are not manual macOS click-through or VoiceOver evidence. Verify the 1040 × 760 source dialog and its 780 × 560 minimum separately from the main window: repeated Add/Manage/center triggers retain one draft/window, Host/Port rows remain readable at 850 × 600 with long values and expanded keys, and main-window interaction remains possible. Exercise Name focus on form open, Cancel/Escape/Cmd-W/native-close draft cancellation and refusal to close while saving, Test without save, Save without automatic connect, explicit Connect, optional database discovery, filtering/paging, clickable and Enter/Space ascending/descending/none sort cycles with retained filter and offset reset, loaded CSV native save/cancel/error and no-overwrite behavior, Edit, confirmed Delete, password re-entry after restart, TLS warnings and stale/error/cancel states. Delete must never remove server objects. Distinguish this manual evidence from verified driver fixtures; agent I/O remains unimplemented. General file browsing is excluded by product scope, not deferred.

## Performance and accessibility evidence

Treat budgets in [product plan](product-plan.md#proposed-release-gates) as proposed targets, not measured results. Record hardware, OS, build/features, fixture sizes, methodology, sample sizes, cold/warm runs, and p50/p95 results. Measure input latency/frame time separately from database/network latency. Assert memory bounds under slow consumers and oversize payloads.

Check every adopted text/background pair, focus/control boundary, state, and theme. Keyboard-only execution and scaled text must be exercised in the real GPUI app. Do not claim screen-reader support based on a layout or library choice; audit actual VoiceOver and later platform tooling.

## Completion definition

A milestone closes only with executable workflow evidence and failure-path tests on its supported targets. Record commands and outcomes, unresolved constraints, and any feature exclusions. Hosted CI, cross-platform UI, real servers, and real agents must not be reported passing until those tests actually run.

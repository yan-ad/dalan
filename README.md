# Dalan

An experimental native database workspace in Rust and GPUI. The goal is to cover daily DataGrip workflows first and MongoDB Compass workflows later, with user-selected Zed-like compact UI and **Carbonfox - opaque**. DataGrip is a database UX/workflow reference only: the user explicitly rejects its visual style and button-heavy layout/chrome.

The name **Dalan** means “ways” in Javanese.

The product name is **Dalan** everywhere it is displayed. Lowercase `dalan-*` Rust package names, executable identifiers, paths, and bundle IDs are technical identifiers, not display branding.

**Status: experimental read-only MySQL/MariaDB sources and UI foundation.** The macOS app can persist profiles, test/connect, discover databases and browse bounded table pages with column sorting and loaded-page CSV export. PostgreSQL and Redis remain planned; agents are not connected yet. The project license and distribution model are undecided.

## Product boundaries

- MySQL/MariaDB come first by user direction, superseding the earlier PostgreSQL-first sequence. First release still targets PostgreSQL, MySQL, MariaDB, and Redis.
- MongoDB comes after the first release. Other drivers follow later.
- macOS first, Linux second, Windows third. Apple Silicon is the initial development target; Intel and minimum OS support remain decisions.
- Database-only workspace: database explorer, query consoles/data views, results and sessions dock, optional ACP panel. No general file explorer, code viewer, Git UI, build tools, or general-purpose terminal.
- AI is available only through **Agent Client Protocol (ACP)**. No application BYOK settings or direct model-provider integrations. External agents handle their own authentication, provider selection, and billing.
- First AI milestone proposes suggestions and explicit insertion, not autonomous database execution. ACP agents are external programs, not sandboxed by this application.

## What exists today

| Area | Current state |
| --- | --- |
| Planning | Product scope, roadmap, UX, technical design, and decision records |
| Core | Engine identities, finite execution limits, conservative declared-risk decisions |
| Drivers | Experimental MySQL/MariaDB read-only adapter using mysql_async; PostgreSQL and Redis planned |
| ACP | Official SDK schema boundary, empty client capabilities, launch-path shape validation; no process or transport |
| App | Diagnostic command and macOS GPUI shell: native titlebar, layout menu, bottom-left explorer toggle, collapsible/resizable Database Explorer, optional disconnected ACP panel, About Dalan window, dedicated source dialog window and read-only table view |
| macOS bundle | Debug/release `Dalan.app` build helper with metadata and local ad-hoc signing |
| CI | Headless formatting/test/lint jobs on macOS, Linux, and Windows; macOS desktop build job; Ubuntu direct/HTTP CONNECT and database-TLS fixture jobs (historical hosted results recorded; next main push verification pending) |

The shell uses the exact Carbonfox - opaque default with shared compiled tokens, flush square panes and explicitly opaque main/source/About windows, not blur or transparency. There is no theme switch or fake light mode. Main content remains the database browser/read-only table workspace; source setup opens in a dedicated resizable dialog window, not inside the main area. There is no SQL editor, write UI or agent session. Profiles use version 1 local JSON without passwords; native macOS Keychain saving is opt-in. See [MySQL sources](docs/mysql-sources.md) for setup, transports, TLS warnings, limits and verified versus untested paths. See [UI foundation](docs/ui-foundation.md) for controls and [development](docs/development.md) for build prerequisites.

The bottom-right 28 px Lucide **bot-message-square** icon button (tooltip/focus-help label **AI · ACP**, Cmd-Shift-A) toggles a scoped right panel that honestly reports **Not connected**: no ACP transport or agent launch is implemented, and there is no text prompt input or BYOK/provider settings. The native macOS **About Dalan** menu opens a separate window showing the Cargo version, the Javanese meaning “ways,” and database-workspace scope. Dalan is the display name; Rust crates, identifiers, executable names, commands, and bundle IDs retain their conventional spelling.

Previous icon/sorting/export validation: 48 headless Rust tests, 42 simulated GPUI tests, four Python bundle-helper tests, one generated native Keychain round-trip and 22 live database/transport checks passed, including sorting on MySQL 8.4.11 and MariaDB 11.4.13. The debug app bundle passed plist/signature/license-resource checks. Trusted system-CA HTTPS proxy success, manual native UI/accessibility and release/offline-Metal validation remain unverified; hosted results are recorded separately below. See [testing](docs/testing.md) for commands and boundaries.

The connection-UX revision fixes source-form Tab traversal and double-click selection, adds an explicit metadata-only SSH identity picker, and provides sanitized typed connection diagnostics. A fresh, uncached MySQL `caching_sha2_password` account passed its first login with TLS explicitly disabled, using RSA authentication through mysql_async. The live suites passed 23 unique cases (7 direct/CONNECT/authentication, 10 TLS, 6 SSH); 52 headless, 46 simulated UI and four Python tests passed, along with formatting, strict lint and debug bundle plist/signature checks. No native Keychain rerun is claimed for this revision. The reported remote connection is **not confirmed fixed**: a credential-free probe connected at TCP level but was reset before the MySQL greeting. Retry with the new diagnostic; see [troubleshooting](docs/mysql-sources.md#connection-troubleshooting). No new dependencies or license changes were introduced.

Source setup now uses one application-wide **Data Sources · Dalan** dialog window (1040 × 760 initially, minimum 780 × 560), separate from the main browser/table workspace. Add, Manage and the centered Connect to a Source action reuse it without resetting an unsaved draft; a new or refreshed form focuses Name. Cancel, Escape, Cmd-W and native close discard the draft and cancel its test, except while credential/JSON saving is in progress. Successful Save closes the window, then automatically refreshes metadata only; it does not browse table data. Host and Port share a row for database, SSH and HTTP/HTTPS endpoints; nonshrinking controls and a scrolling natural-height body prevent the reported narrow-field collapse. This is a normal separate window, not an OS modal sheet or a focus trap for the main window.

Historical local source-dialog evidence: **54 headless**, **63 simulated GPUI** and **four Python bundle-helper** tests passed. Native visual/accessibility interaction remains unverified. Hosted run **37192402473** failed only the Windows SSH-key fixture because its newline filename is invalid on Windows; macOS/Linux headless, database fixtures and macOS desktop jobs passed in that historical run. The fix restricts newline/carriage-return/tab filename fixtures to Unix while preserving Windows metadata-discovery assertions; no checks or workflows are disabled. That fix passed all five jobs on `3c4fdae` in [CI run 37194669634](https://github.com/yan-ad/dalan/actions/runs/37194669634). The current tree revision still needs its own run. See [testing](docs/testing.md#source-dialog-window-and-windows-fixture-fix).

The UI uses nineteen pinned Lucide SVGs, embedded through the desktop asset source. Complete Lucide ISC and retained Feather MIT notices, plus GPUI input Apache-2.0 attribution, are in [third-party notices](THIRD_PARTY_NOTICES.md); bundle resources include the notices and Lucide license. These utility glyphs do not introduce a Dalan app/brand icon or select a project license.

Database Explorer now has only a 28 px toolbar with six 28 px icon actions: Add, Manage, Refresh, Remove, Expand Loaded and Collapse All. The Database Explorer title header is removed, as are the activity rail and header hide/minimise controls. The bottom-left 28 px panel-left toggle is unchanged; Cmd-B and native View/Layout alternatives remain. Closing the explorer retains that preference until toggled or reset. Status focus help and the ACP trigger remain unchanged; normal status shows Carbonfox - opaque. Outer padding is now 0 px, the divider hit area 4 px (1 px visible line), titlebar 34 px and status 28 px. At 720 px the explorer maximum is 476 px while preserving 240 px main content.

Source rows retain readable names and engine labels, with abstract Lucide database (MySQL) and database-zap (MariaDB) cues, not vendor logos. Optional source colors are markers only. Color below Name accepts `#RRGGBB` or labeled Default, Blue, Green, Amber, Red and Purple swatches. Legacy version 1 profiles load with no color and are not automatically rewritten; password rejection and Keychain behavior are unchanged. With no sources, the main area centers a working **Connect to a Source** button that opens the same dedicated source dialog window by mouse, Enter or Space, even while the explorer is hidden. No demo/trial welcome content is added.

Historical explorer redesign verification passed **54 headless Rust tests**, **57 simulated UI tests** and **four Python bundle tests**, plus formatting, strict lint and debug bundle plist/signature/license-resource checks. Migration tests verify exact legacy None, case-preserving hex and malformed-color no-overwrite behavior. See [testing](docs/testing.md#database-explorer-redesign-verified). Native visual/accessibility remains unverified; previous live transport results are historical for this UI/profile-only change. Hosted CI evidence and the pending next-main gate are recorded below. Asset provenance, including the trash filename alias, is in the [asset README](crates/app/assets/README.md).

### Compact lazy explorer

The tree uses 22 px rows: source driver/color/name/engine label with counts only after loading, database disclosure/icon rows, Tables and Views folders with counts when metadata is known, and table-icon leaves. Names are explicitly width-constrained, nonblank and ellipsized, with full-name tooltips; Unicode and quoted identifiers retain collision-safe identities. A single-focus keyboard tree and viewport-only uniform list replace the old per-node buttons/full-hierarchy layout. The reported blank buttons were not traced to a measured root cause; no font or GPU diagnosis is claimed.

Startup restores persisted metadata without network or Keychain calls. Cached expansion survives table pagination, filters and source changes; Tables opens automatically and Views starts collapsed and remains unavailable for reading. Expand Loaded never fans out network work; Collapse All preserves metadata. Explicit Refresh replaces the complete explorer-selected source catalog only after success, retaining the previous tree and table page on failure. Catalog jobs remain independent of table-page/form jobs with generation/abort checks; each complete discovery uses one serial owned connection/tunnel, not per-database fan-out.

Historical compact-tree local verification passed **57 headless tests**, **71 simulated UI tests** and **four Python bundle tests**, plus formatting, strict lint and debug bundle plist/signature/license checks. The large-catalog regression paints at most 40 rows out of 1,000 databases and scrolls to row 900/End; that is structural evidence, not a native FPS/latency measurement. That compact-tree commit `8dc3d27` passed all five hosted jobs in [run 37198333660](https://github.com/yan-ad/dalan/actions/runs/37198333660). The icon subset now has nineteen assets at the same Lucide revision and existing license notices.

### Persistent offline metadata

Saved profiles remain version 1 password-free JSON and credentials remain session-only or opt-in native macOS Keychain. A separate `~/Library/Application Support/Dalan/metadata.sqlite3` cache uses pinned `rusqlite 0.40.2` with bundled SQLite, no external daemon, libSQL or cloud service. It stores visible database names and table/view names/kinds, not columns, indexes, DDL or rows. Successful Save commits credentials/profile first, closes the dialog, then refreshes metadata for all visible databases (or just the configured database); it never automatically browses data.

The 22 px virtual tree restores offline with textual **Cached**/**Stale**/**Refreshing…** source markers and timestamp/error tooltips. Refresh requires an explicit valid explorer selection, not a current-table fallback; failed refresh retains the old snapshot. Disk-cache failures are nonfatal warnings and do not block profile JSON loading or falsely report a successful profile save as failed. Cached metadata is sensitive, unencrypted and can reflect old permissions; it does not grant offline server-data access. See [cache scope and limits](docs/mysql-sources.md#persistent-offline-metadata), [architecture](docs/architecture.md#persistent-metadata-cache-implemented), [security](docs/security.md#persistent-metadata-privacy) and [license notices](THIRD_PARTY_NOTICES.md).

Historical metadata-revision evidence: **67 headless tests**, **83 simulated UI tests**, **four Python bundle tests**, and all disposable database/transport scripts passed (**7 direct/CONNECT/authentication**, **10 TLS**, **6 SSH**). Formatting, strict lint and signed debug bundle checks passed. That metadata commit `4c8af09` passed all five hosted jobs in [run 37201696519](https://github.com/yan-ad/dalan/actions/runs/37201696519); this is historical evidence, not a result for the current Carbonfox revision. Native Keychain was not rerun. The automatic-save regression verifies that discovery starts after Save and that failure preserves the committed source and prior metadata without implicitly enabling Refresh. Cached metadata restores without contacting servers; native UI/accessibility remains unverified.

### Carbonfox compact foundation

The exact complete **Carbonfox - opaque** variant is [vendored](crates/app/assets/themes/carbonfox-opaque.json) from `cange/nightfox.zed` commit `3511a6f1f665455c70a24d14fd5d2de0eaab58fa`, `themes/nvim-nightfox.json`. Both MIT licenses (2024 Christian Angermann, 2021 James Simpson) are retained with [provenance](crates/app/assets/themes/README.md) and [third-party notices](THIRD_PARTY_NOTICES.md). The original project's license pin is separate, not a claim about which palette revision the port used. No affiliation or Zed editor asset import is implied.

Runtime uses compiled opaque tokens, including input colors; upstream alpha toolbar/selection/border entries are composited over the panel. Blue is focus/active state and Save/Connect primary action with dark text; controls stay neutral and disabled labels remain readable without opacity. System UI type is 13 px, browse data 12 px, controls/headers 28 px, tree/grid rows 22 px and control radius 3 px. The source window keeps its sizes, draft/traversal/saving APIs, 28 px inputs, 30 px endpoint parents and 18 px checkbox, with 8 px gaps/16 px scroll padding and compact footer. Optional user source colors remain marker-only and unchanged, with no arbitrary-color AA promise.

Pure token checks calculate minimum normal-state contrast of 13.04:1 primary, 7.22:1 secondary, 6.10:1 focus and 3.44:1 input boundary. The theme revision passed 67 headless tests, 88 simulated UI tests and four bundle tests, plus formatting, strict lint and signed bundle/license checks. Dalan.app was rebuilt and reopened. Native window capture was blocked, so visual/accessibility verification is not claimed; hosted CI requires its own new run. Database/cache/SSH/password persistence is unchanged. Light/system support is an unselected future proposal, not a shipped toggle. See [design](DESIGN.md), [UI foundation](docs/ui-foundation.md) and [testing](docs/testing.md#carbonfox-opaque-revision).

## Start locally

The pinned Rust toolchain is installed through rustup when Cargo runs.

```sh
cargo run -p dalan-app --bin dalan-doctor
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

### Build and open Dalan.app (macOS debug)

```sh
./scripts/macos --open
```

This creates `target/debug/bundles/Dalan.app` and opens it as a macOS application, not a loose executable. Requires Rust, Python 3, and Xcode Command Line Tools. Debug defaults to runtime shaders so the offline Metal compiler is not required. Finder/Dock/app-menu display name is **Dalan**; no custom app icon has been selected.

```sh
./scripts/macos          # Build bundle only
./scripts/macos --run    # Run bundle executable in terminal for debug logs
open target/debug/bundles/Dalan.app  # Open an already-built bundle
./scripts/macos --release --open    # Optimized bundle; requires full Xcode/Metal
```

Quit an older Dalan instance before rebuilding/reopening. These are local ad-hoc signed builds, not notarized distribution packages. Custom Cargo target directories are supported; the script prints the actual bundle location. See [development](docs/development.md) for debugger commands and build options.

## Workspace

```text
crates/
  app/       Diagnostic command and macOS GPUI entry point
  core/      UI-independent engine and execution policy types
  drivers/   Experimental MySQL/MariaDB adapter and future engine catalog
  acp/       ACP SDK boundary, future agent lifecycle
docs/        Product and technical planning
```

Experimental MySQL/MariaDB I/O, source persistence and macOS credential storage exist. Editor behavior, writes, PostgreSQL/Redis adapters and real ACP sessions remain implementation work. Avoid adding speculative crates before a vertical slice needs them.

## Planning map

- [Product plan](docs/product-plan.md): workflows, release scope, exclusions, acceptance scenarios.
- [Roadmap](docs/roadmap.md): incremental milestones and exit gates.
- [Feature checklist](docs/feature-checklist.md): completed experimental slices and the next one-by-one workflow sequence.
- [UX](docs/ux.md) and [design direction](DESIGN.md): Zed/DataGrip references, pane behavior, keyboard and state requirements.
- [Architecture](docs/architecture.md): module boundaries, runtime ownership, sessions, result handling, persistence.
- [Drivers](docs/drivers.md): engine-specific behavior and dependency choices.
- [MySQL sources](docs/mysql-sources.md): experimental source how-to, scope matrix and live-test boundaries.
- [ACP integration](docs/acp.md): protocol lifecycle, authentication, context consent, permission handling.
- [Security](docs/security.md): credentials, TLS, execution safety, external-agent trust.
- [Development](docs/development.md) and [testing](docs/testing.md): setup, commands, CI, and release evidence.
- [Decision records](docs/adr/README.md): requirements, provisional choices, and unresolved decisions.
- [Research sources](docs/references.md): primary documentation and version notes.
- [Initial validation](docs/initial-validation.md): checks run, local Metal limitation, and unverified paths.

## Contributing

Start with [CONTRIBUTING.md](CONTRIBUTING.md). This experimental read-only client is not feature-complete. Planned capabilities are not compatibility claims. Do not submit provider-key UI, direct provider SDKs, or credentials in fixtures.

No project license has been selected. Dependency licenses do not license this repository; choose a project license before public distribution.

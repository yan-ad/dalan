# Dalan

An experimental native database workspace in Rust and GPUI. The goal is to cover daily DataGrip workflows first and MongoDB Compass workflows later, with Zed-inspired editor chrome and database-oriented layout.

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
| App | Diagnostic command and macOS GPUI shell: native titlebar, layout menu, database rail, collapsible/resizable Database Explorer, optional disconnected ACP panel, About Dalan window, source form and read-only table view |
| macOS bundle | Debug/release `Dalan.app` build helper with metadata and local ad-hoc signing |
| CI | Headless formatting/test/lint jobs on macOS, Linux, and Windows; macOS desktop build job; Ubuntu direct/HTTP CONNECT and database-TLS fixture jobs (hosted execution unverified) |

The shell follows the supplied dark IDE reference. Main content now hosts a real source form and read-only table view, not a blank placeholder. There is no SQL editor, write UI or agent session. Profiles use version 1 local JSON without passwords; native macOS Keychain saving is opt-in. See [MySQL sources](docs/mysql-sources.md) for setup, transports, TLS warnings, limits and verified versus untested paths. See [UI foundation](docs/ui-foundation.md) for controls and [development](docs/development.md) for build prerequisites.

The bottom-right 28 px Lucide **bot-message-square** icon button (tooltip/focus-help label **AI · ACP**, Cmd-Shift-A) toggles a scoped right panel that honestly reports **Not connected**: no ACP transport or agent launch is implemented, and there is no text prompt input or BYOK/provider settings. The native macOS **About Dalan** menu opens a separate window showing the Cargo version, the Javanese meaning “ways,” and database-workspace scope. Dalan is the display name; Rust crates, identifiers, executable names, commands, and bundle IDs retain their conventional spelling.

Previous icon/sorting/export validation: 48 headless Rust tests, 42 simulated GPUI tests, four Python bundle-helper tests, one generated native Keychain round-trip and 22 live database/transport checks passed, including sorting on MySQL 8.4.11 and MariaDB 11.4.13. The debug app bundle passed plist/signature/license-resource checks. Trusted system-CA HTTPS proxy success, manual native UI/accessibility, hosted CI and release/offline-Metal validation remain unverified. See [testing](docs/testing.md) for commands and boundaries.

The connection-UX revision fixes source-form Tab traversal and double-click selection, adds an explicit metadata-only SSH identity picker, and provides sanitized typed connection diagnostics. A fresh, uncached MySQL `caching_sha2_password` account passed its first login with TLS explicitly disabled, using RSA authentication through mysql_async. The live suites passed 23 unique cases (7 direct/CONNECT/authentication, 10 TLS, 6 SSH); 52 headless, 46 simulated UI and four Python tests passed, along with formatting, strict lint and debug bundle plist/signature checks. No native Keychain rerun is claimed for this revision. The reported remote connection is **not confirmed fixed**: a credential-free probe connected at TCP level but was reset before the MySQL greeting. Retry with the new diagnostic; see [troubleshooting](docs/mysql-sources.md#connection-troubleshooting). No new dependencies or license changes were introduced.

The UI uses eight pinned Lucide SVGs, embedded through the desktop asset source. Complete Lucide ISC and retained Feather MIT notices, plus GPUI input Apache-2.0 attribution, are in [third-party notices](THIRD_PARTY_NOTICES.md); bundle resources include the notices and Lucide license. These utility glyphs do not introduce a Dalan app/brand icon or select a project license.

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

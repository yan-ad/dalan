# dalan

A planned native database workspace in Rust and GPUI. The goal is to cover daily DataGrip workflows first and MongoDB Compass workflows later, with Zed-inspired editor chrome and database-oriented layout.

The name **dalan** means “ways” in Javanese.

**Status: product planning and initial scaffold.** This repository does not connect to databases or agents yet. The project license and distribution model are undecided.

## Product boundaries

- First release targets PostgreSQL, MySQL, MariaDB, and Redis.
- MongoDB comes after the first release. Other drivers follow later.
- macOS first, Linux second, Windows third. Apple Silicon is the initial development target; Intel and minimum OS support remain decisions.
- Keyboard-first workspace: database explorer, tabbed consoles/data views, results and sessions dock, optional AI panel.
- AI is available only through **Agent Client Protocol (ACP)**. No application BYOK settings or direct model-provider integrations. External agents handle their own authentication, provider selection, and billing.
- First AI milestone proposes suggestions and explicit insertion, not autonomous database execution. ACP agents are external programs, not sandboxed by this application.

## What exists today

| Area | Current state |
| --- | --- |
| Planning | Product scope, roadmap, UX, technical design, and decision records |
| Core | Engine identities, finite execution limits, conservative declared-risk decisions |
| Drivers | Planned engine catalog only; no connection adapters |
| ACP | Official SDK schema boundary, empty client capabilities, launch-path shape validation; no process or transport |
| App | Runnable diagnostic command and opt-in macOS GPUI bootstrap window |
| CI | Headless formatting/test/lint jobs on macOS, Linux, and Windows; macOS desktop build job |

The bootstrap window is a developer build fixture, not the intended database IDE layout. See [development](docs/development.md) for desktop prerequisites and validation limits.

## Start locally

The pinned Rust toolchain is installed through rustup when Cargo runs.

```sh
cargo run -p dalan-app --bin dalan-doctor
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

On macOS with full Xcode and Metal tooling:

```sh
cargo run -p dalan-app --bin dalan --features desktop --locked
```

Command Line Tools alone may lack the Metal compiler. A development-only runtime-shader path is documented in [development](docs/development.md); it is not the release build path.

## Workspace

```text
crates/
  app/       Diagnostic command and macOS GPUI entry point
  core/      UI-independent engine and execution policy types
  drivers/   Planned driver catalog, future engine adapters
  acp/       ACP SDK boundary, future agent lifecycle
docs/        Product and technical planning
```

Database I/O, persistence, credential storage, editor behavior, and real ACP sessions remain implementation work. Avoid adding speculative crates before a vertical slice needs them.

## Planning map

- [Product plan](docs/product-plan.md): workflows, release scope, exclusions, acceptance scenarios.
- [Roadmap](docs/roadmap.md): incremental milestones and exit gates.
- [UX](docs/ux.md) and [design direction](DESIGN.md): Zed/DataGrip references, pane behavior, keyboard and state requirements.
- [Architecture](docs/architecture.md): module boundaries, runtime ownership, sessions, result handling, persistence.
- [Drivers](docs/drivers.md): engine-specific behavior and proposed dependencies.
- [ACP integration](docs/acp.md): protocol lifecycle, authentication, context consent, permission handling.
- [Security](docs/security.md): credentials, TLS, execution safety, external-agent trust.
- [Development](docs/development.md) and [testing](docs/testing.md): setup, commands, CI, and release evidence.
- [Decision records](docs/adr/README.md): requirements, provisional choices, and unresolved decisions.
- [Research sources](docs/references.md): primary documentation and version notes.
- [Initial validation](docs/initial-validation.md): checks run, local Metal limitation, and unverified paths.

## Contributing

Start with [CONTRIBUTING.md](CONTRIBUTING.md). This scaffold is not a feature-complete client. Planned capabilities are not compatibility claims. Do not submit provider-key UI, direct provider SDKs, or credentials in fixtures.

No project license has been selected. Dependency licenses do not license this repository; choose a project license before public distribution.

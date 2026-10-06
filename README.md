<p align="center">
  <img src="crates/app/assets/brand/dalan.png" width="128" alt="Dalan app logo">
</p>

# Dalan

[![CI](https://github.com/yan-ad/dalan/actions/workflows/ci.yml/badge.svg)](https://github.com/yan-ad/dalan/actions/workflows/ci.yml)
![Status: experimental](https://img.shields.io/badge/status-experimental-orange)
![Coming soon](https://img.shields.io/badge/release-coming%20soon-lightgrey)
![Rust + GPUI](https://img.shields.io/badge/built%20with-Rust%20%2B%20GPUI-black)

## Introduction

Dalan means “ways” in Javanese. It is a native Rust + GPUI Kit database workspace with DataGrip-inspired workflows, compact layout, Kit standard controls and Kit's default theme.

MySQL, MariaDB, PostgreSQL, MongoDB and Redis have experimental native adapters with restricted reads, source management, cached catalogs, table/document/key previews, restricted consoles and loaded CSV export. Eligible MySQL/MariaDB InnoDB and PostgreSQL base tables also support [bounded staged writes](docs/table-editing.md) with explicit Apply; actual-server write/commit verification remains unrun. Consoles remain read-only. WHERE/ORDER BY fragments remain MySQL/MariaDB-only; MongoDB/Redis consoles use restricted JSON. See the [native support matrix](docs/native-drivers.md) for transport/TLS limits and unrun real-server gates. JDBC, plugins and live ACP-only AI integration are not implemented. No application BYOK or general-purpose IDE tools.

Appearance defaults to OS-following **System**, cycling System → Light → Dark with a saved `dalan.config` preference and Kit default colors. Passwords are session-only unless **SaveForever** explicitly saves them as local unencrypted plaintext in `dalan.auth`; Keychain saving is removed. See [security and storage boundaries](docs/security.md#credentials-and-persistence).

## Installation

**Dalan is coming soon.** There are no public release installers or mobile packages yet. Platform coverage is planned as follows:

| Platform | Release status | Current coverage |
| --- | --- | --- |
| **Mac (macOS)** | Coming soon · first release target | Experimental local native app build; release signing, notarization and native qualification remain pending. |
| **Linux** | Coming soon · planned | Portable headless tests; native desktop packaging and qualification remain pending. |
| **Windows** | Coming soon · planned | Portable headless tests; native desktop packaging and qualification remain pending. |
| **Android** | Coming soon · planned | Mobile feasibility, touch UI, platform integration and packaging have not been implemented or validated. |

These are roadmap targets, not claims of released or certified support. No release dates are announced. See the [platform roadmap](ROADMAP.md#platform-coverage-and-release-plan).

For the current **macOS development build**, install Rust/rustup, Python 3, and Xcode Command Line Tools, then build and open the local debug app:

```sh
git clone https://github.com/yan-ad/dalan.git
cd dalan
./scripts/macos --open
```

Output: `target/debug/bundles/Dalan.app`. This is a local ad-hoc signed build, not a notarized installer. It uses the supplied PNG/icns icon fallback by default; optional Icon Composer compilation requires full Xcode. [Build and icon details](docs/development.md).

## Development

Optionally install `cargo install bacon --version 3.26.0 --locked`, then run `bacon` for native macOS rebuild-and-restart preview (not hot reload).
See [Bacon setup and restart caveats](docs/development.md#bacon-live-preview); `bacon check`/`test`/`lint` run headless jobs.

```sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
./scripts/macos --run
```

The pinned toolchain is installed by rustup. Headless tests run on macOS, Linux, and Windows; desktop support is macOS-first. See [development setup](docs/development.md) and [test strategy](docs/testing.md) for UI, database fixture, and release commands.

## Contribution

Implement one tested database workflow at a time. Preserve credential safety, lossless values, bounded results, and ACP-only AI. Read [CONTRIBUTING.md](CONTRIBUTING.md) and [ROADMAP.md](ROADMAP.md) before changing architecture or scope.

## Reference

- [Roadmap](ROADMAP.md): drivers, plugins, and ACP AI, now through future.
- [Native driver support matrix](docs/native-drivers.md), [source management](docs/source-management.md), [table browser](docs/table-browser.md), [staged table editing](docs/table-editing.md), and [query consoles](docs/query-consoles.md).
- [Architecture](docs/architecture.md), [security](docs/security.md), and [feature checklist](docs/feature-checklist.md).
- [DBX reuse assessment](docs/dbx-reuse.md), [staged DBX toolbar parity](docs/dbx-toolbar-parity.md), and [design direction](DESIGN.md).

## License

Dalan's project and supplied artwork licenses have not yet been selected; no open-source redistribution grant is implied. Third-party assets and adapted code retain their own licenses in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Resolve project/artwork licensing before public distribution.

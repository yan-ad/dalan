# Contributing

This is a planning-stage scaffold. Read [product plan](docs/product-plan.md), [architecture](docs/architecture.md), and [roadmap](docs/roadmap.md) before changing interfaces.

- Implement one tested vertical slice at a time. Keep placeholders labeled; do not mark a driver supported because its library was added.
- Keep UI state inside the app, domain rules independent of GPUI, and database execution independent of ACP.
- Do not add application BYOK/provider integrations. AI is ACP-only.
- Use secret-free local fixtures. Never point destructive integration tests at production or arbitrary environment URLs.
- Do not choose a project license or import Zed editor/assets without owner approval and license review.
- Update docs/ADRs when scope or an accepted boundary changes. Preserve `Cargo.lock`; inspect dependency changes and platform features.
- Run formatting, tests, and strict lint; run the desktop build when changing the feature-gated entry point. Record what could not be exercised.
- Do not commit changes automatically; commits require an explicit request.

```sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

For desktop setup and platform constraints, see [development](docs/development.md).

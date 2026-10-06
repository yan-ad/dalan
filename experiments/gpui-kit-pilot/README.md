# GPUI Kit migration pilot (independent, not shipped)

This nested workspace probes Kit-native SQL editing, control icons and populated
results. It does not import the application, drivers, database models, executor,
credentials or production GPUI types. Production manifests and lockfile are not
part of this experiment. **Do not pass GPUI 0.2.2 entities into this workspace:**
Kit 0.7.1 uses the coordinated `gpui-pre` 0.3.8 family.

## Run / validate

From the repository root, with Rust 1.98 or later:

```sh
cargo check --locked --manifest-path experiments/gpui-kit-pilot/Cargo.toml
cargo test --locked --manifest-path experiments/gpui-kit-pilot/Cargo.toml -- --nocapture
cargo run --locked --manifest-path experiments/gpui-kit-pilot/Cargo.toml
cargo tree --manifest-path experiments/gpui-kit-pilot/Cargo.toml -i gpui-pre
cargo tree --manifest-path experiments/gpui-kit-pilot/Cargo.toml -e features -i gpui-component
```

Initial supported/validated target: macOS arm64. Native builds require the Apple
SDK/toolchain. Linux/Windows were not validated; Kit's native platform dependency
also enables X11/Wayland on Linux, so Linux system development packages may be
required. No CI or production target is changed.

## What is implemented

- `gpui_kit::application().with_assets(assets::Assets)`, `init`, and facade
  `open_window` (which installs Base `Root`). No private platform construction.
- `EditorState` with `language("sql")`, line numbers and a multiline SELECT.
  The editor pane is a fixed, bounded 200 px. The result pane has `flex_1`,
  `min_h_0`, overflow clipping and maximum 420 px; neither depends on an
  unconstrained content-height column.
- Kit `DataTable` delegate: 100 × 512 strings, 160 px column widths and explicit
  22 px rows. One immutable `Arc<Vec<Vec<String>>>` page caches the labels;
  only `render_td` clones the requested cell string. The delegate and owner
  share the page, not a fresh full-page copy on each frame. Labels include a
  64-bit row/column value, with markers for observation. No DB connection.
- Kit Button/Checkbox with owner-state callbacks and Play icon. `Assets` is the
  **default component bundle**, not `AllAssets`; an asset test checks SVG bytes.
- Kit-owned default theme; no application palette override.

## Source/API audit

Pinned `gpui-kit = "=0.7.1"`; defaults are component + assets. The only explicitly
selected grammar feature is `tree-sitter-sql` (not `tree-sitter-languages`).
Component 0.7.1 resolves SQL to `tree-sitter-sequel` 0.3.11 and also has an
unconditional `tree-sitter-json` 0.24.8 dependency; therefore this is **not** a
claim that SQL is the only grammar present in the dependency graph.

`gpui-component` 0.7.1 `table/state.rs::render_table_row` wraps non-fixed cells in
an `Axis::Horizontal` virtual list and invokes `measure_render_td` only over its
visible range. The table body also uses row virtualization. This is actual source
inspection, not an assumption based on `visible_columns_changed` documentation.
The delegate does not clamp columns or substitute a canvas to make the test pass.

`gpui-pre-macos` 0.3.8 provides `runtime_shaders`; this workspace explicitly enables
it as a platform probe/workaround. **Kit's `gpui-pre-platform` already forwards
that feature to macOS**, so the direct declaration is redundant in this release,
not a newly discovered missing forwarding fix. Everything resolves to one GPUI
pre-release version.

API references: https://docs.rs/gpui-kit/0.7.1/gpui_kit/ and
https://github.com/longbridge/gpui-kit (examples, TESTING.md and published tests).
Small initialization/test patterns are attributed in source and
`THIRD_PARTY_NOTICES.md`; no library source is vendored.

## Validation and limitations

Validated locally on macOS arm64, Rust 1.98.1:

- `cargo check` and native `cargo build` succeeded.
- Three actual headless UI integration tests passed: native typing, multiline
  Enter, Tab, Undo, clipboard Paste, focus and caret; table nonzero bounds and
  first/last visible markers after `scroll_to_col(511)`; pointer-driven Button
  and Checkbox callbacks. A fourth test verifies default asset SVG payloads.
- SQL test parses the editor's actual rope with the configured SQL grammar,
  checks an error-free tree, and verifies highlight spans including SELECT.
- At 1040 × 760, measured `render_td` calls: **328 initial, 146 after scrolling
  to column 511**. The test prints `KIT_TABLE_BUDGET`; both must be positive and
  below 2,000. These include frame preparation/measurement calls, not unique cells
  or milliseconds. This confirms bounded work for this fixture, not a production
  performance parity claim (the existing canvas may perform less work).
- A native process startup smoke probe remained alive for four seconds with no
  stderr before controlled termination. This is **not** a human visual/pixel or
  screenshot approval. Headless UI tests inspect real layout/state, not pixels.

Keep the pilot isolated until full application migration and performance profiling
are approved. Sorting, connection wiring, paging, null/type formatters, selection
parity, custom SQL dialects and production themes are intentionally not ported.
The dependency graph reports an upstream future-incompatibility warning for
`block` 0.1.6; it did not prevent the validated build/tests.

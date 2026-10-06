# Pilot dependency / example attribution

The application initialization and UI test fixture patterns are adapted from the
small public examples in GPUI Kit 0.7.1 (`src/lib.rs`, `tests/common/mod.rs`, and
`tests/input/editor.rs`): https://github.com/longbridge/gpui-kit.
GPUI Kit is licensed Apache-2.0, copyright its respective contributors.
This pilot does not vendor the library or its assets: Cargo resolves them.

GPUI Kit dependencies include GPUI pre-release 0.3.8 (Apache-2.0), GPUI Base and
GPUI Component (Apache-2.0), and the default Kit asset bundle, which embeds Lucide
SVG icons. Lucide icons are ISC licensed (https://lucide.dev/license).
Consult the resolved packages' licenses and notices before redistributing binaries.

The pilot uses GPUI Kit's default theme; no separate theme asset is loaded.

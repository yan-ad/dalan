# Vendored UI icons

The main UI icon set is [Lucide](https://github.com/lucide-icons/lucide), explicitly selected as an open-source icon set. The AI entry point uses **BotMessageSquare** (`bot-message-square.svg`) to communicate an AI agent conversation, not decorative sparkles.

## Provenance

All fourteen SVGs and `lucide-LICENSE.txt` were downloaded unchanged from the single pinned upstream Git revision:

`500620a2e8123f8d1db191538886dc0c223f69a9`

The revision was resolved once using `https://api.github.com/repos/lucide-icons/lucide/commits/main`. Downloads use the immutable base URL `https://raw.githubusercontent.com/lucide-icons/lucide/500620a2e8123f8d1db191538886dc0c223f69a9/`, followed by `icons/<name>.svg` or `LICENSE`. Do not fetch individual assets from a moving branch. At this revision, `trash-2` is a deprecated alias of `trash` in `icons/trash.json`, not a separate upstream SVG: local `trash-2.svg` contains the unchanged bytes of upstream `icons/trash.svg`.

| Rust icon | Upstream SVG |
| --- | --- |
| Database | database.svg |
| MariaDb | database-zap.svg |
| Add | plus.svg |
| Manage | settings-2.svg |
| Refresh | refresh-cw.svg |
| Remove | trash.svg (vendored as trash-2.svg) |
| Layout | panel-left.svg |
| Chevron | chevron-down.svg |
| Hide | minus.svg |
| Ai | bot-message-square.svg |
| SortAscending | arrow-up.svg |
| SortDescending | arrow-down.svg |
| Download | download.svg |
| Check | check.svg |

MySQL uses the abstract **Database** glyph; MariaDB uses the distinct abstract **DatabaseZap** glyph. These are not vendor logos or trademark artwork. Keep the visible engine name as the primary identifier; the glyph is a supplementary cue.

`src/desktop/icons.rs` embeds these bytes with `include_bytes!` and exposes them through GPUI's `AssetSource`. The application must register `IconAssets` using `Application::new().with_assets(IconAssets)`. There are no additional npm/crate dependencies, runtime downloads, external resources, or icon fonts. GPUI renders SVG alpha masks with the requested text color; icons retain the upstream 24×24 viewBox and render at 16×16 UI pixels.

## Licensing and distribution

`lucide-LICENSE.txt` is the complete, unmodified upstream license, including both Lucide's ISC license and the Feather-derived icons' MIT license and copyright notice. Keep the entire file with redistributed assets and include it in the application's bundle resources. The license is also embedded in `IconAssets` as `lucide-LICENSE.txt`.

See the repository's `THIRD_PARTY_NOTICES.md` for notices, including the GPUI-derived text input. These third-party notices do not select or change the license of this project.

## Updating

Resolve one new official upstream commit, download all assets and the license from that same fixed commit, update this provenance record and `THIRD_PARTY_NOTICES.md`, then run the `ui-tests` icon tests. Review the complete upstream license when updating; do not drop the Feather notice.

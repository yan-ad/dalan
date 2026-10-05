# Vendored UI icons

The main UI icon set is [Lucide](https://github.com/lucide-icons/lucide), explicitly selected as an open-source icon set. The AI entry point uses **BotMessageSquare** (`bot-message-square.svg`) to communicate an AI agent conversation, not decorative sparkles.

## Provenance

All thirty-six SVGs and `lucide-LICENSE.txt` were downloaded unchanged from the single pinned upstream Git revision:

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
| ChevronRight | chevron-right.svg |
| Folder | folder.svg |
| Table | table.svg |
| ExpandTree | list-tree.svg |
| CollapseTree | chevrons-down-up.svg |
| Hide | minus.svg |
| Ai | bot-message-square.svg |
| SortAscending | arrow-up.svg |
| SortDescending | arrow-down.svg |
| Download | download.svg |
| Check | check.svg |
| Cached | hard-drive.svg |
| Warning | triangle-alert.svg |
| Loading | loader-circle.svg |
| ReadOnly | lock-keyhole.svg |
| Previous | chevron-left.svg |
| Query | square-code.svg |
| Play | play.svg |
| Stop | circle-stop.svg |
| Close | x.svg |
| ColumnKey | key-round.svg |
| ColumnNumber | hash.svg |
| ColumnText | text-initial.svg |
| ColumnDate | calendar-clock.svg |
| ColumnBinary | binary.svg |
| ColumnJson | braces.svg |
| Filter | list-filter.svg |
| Sort | arrow-down-up.svg |

The main user-facing chrome is icon-led, using functional glyphs rather than a brand icon. **Cached** uses **HardDrive** to mark locally cached metadata, **Warning** uses **TriangleAlert** for warnings, **Loading** uses a static **LoaderCircle** glyph, **ReadOnly** uses **LockKeyhole**, and **Previous** uses **ChevronLeft**. Next reuses **ChevronRight**; Download, Hide (Minus), and Check reuse their existing assets. These assets alone add no buttons, state updates, or animation. At the pinned revision, `icons/triangle-alert.svg` exists under that exact name; `alert-triangle` is only a deprecated alias in `icons/triangle-alert.json`. The five additions were fetched through GitHub's authenticated contents API (`https://api.github.com/repos/lucide-icons/lucide/contents/icons/<name>.svg?ref=500620a2e8123f8d1db191538886dc0c223f69a9`), over verified TLS, then base64-decoded unchanged and checked against the returned Git blob SHA.

The query console uses **Query** (**SquareCode**) to denote application SQL, not a shell terminal; it deliberately does not use SquareTerminal. A new-query toolbar action can combine the existing **Add** (**Plus**) glyph with Query. **Play** denotes running a query, **Stop** uses the available upstream **CircleStop** cancellation glyph (not a danger-colored glyph), and **Close** uses **X** for closing a tab. These four additions were fetched through GitHub's authenticated contents API (`https://api.github.com/repos/lucide-icons/lucide/contents/icons/<name>.svg?ref=500620a2e8123f8d1db191538886dc0c223f69a9`) over verified TLS, base64-decoded unchanged, and verified against the returned Git blob SHA. The existing Plus SVG and complete license were also verified unchanged at that revision. This asset change adds no tab UI or query execution behavior.

The column headers use **ColumnKey** (**KeyRound**) for primary keys, **ColumnNumber** (**Hash**) for numeric values, **ColumnText** (**TextInitial**) for text, **ColumnDate** (**CalendarClock**) for dates/times, **ColumnBinary** (**Binary**) for binary data, and **ColumnJson** (**Braces**) for JSON. Boolean columns reuse **Check**, and unsupported types reuse **Table**. **Filter** uses **ListFilter** for WHERE filtering, and neutral **Sort** uses **ArrowDownUp**; directional sorting keeps ArrowUp/ArrowDown. At this revision, `letter-text` is a deprecated alias in upstream `icons/text-initial.json`, and no `icons/letter-text.svg` exists: ColumnText therefore uses the exact official filename `text-initial.svg` without renaming. All eight additions were downloaded as unchanged bytes from the immutable raw GitHub URLs above over verified TLS and checked against the Git blob SHA returned by GitHub's authenticated contents API for the same revision. The complete upstream license was also downloaded and verified byte-for-byte unchanged, including the Feather MIT notice covering Hash. These abstract functional glyphs are not logos. They retain `currentColor`, use the existing GPUI alpha-mask tint path, and introduce no UI behavior or schema/source-field color changes.

The compact explorer reuses **Database** for schema/database children, **Folder** for the Tables group, and **Table** for table children. **ExpandTree** denotes expanding loaded nodes and **CollapseTree** denotes collapsing all nodes. The upstream `icons/table.svg` exists at this revision and is vendored under the same filename; no table alias or substitution is needed.

MySQL uses the abstract **Database** glyph; MariaDB uses the distinct abstract **DatabaseZap** glyph. These are not vendor logos or trademark artwork. Keep the visible engine name as the primary identifier; the glyph is a supplementary cue.

`src/desktop/icons.rs` embeds these bytes with `include_bytes!` and exposes them through GPUI's `AssetSource`. The application must register `IconAssets` using `Application::new().with_assets(IconAssets)`. There are no additional npm/crate dependencies, runtime downloads, external resources, or icon fonts. GPUI renders SVG alpha masks with the requested text color; icons retain the upstream 24×24 viewBox and render at 16×16 UI pixels.

## Licensing and distribution

`lucide-LICENSE.txt` is the complete, unmodified upstream license, including both Lucide's ISC license and the Feather-derived icons' MIT license and copyright notice. The complete license is retained for all additions, including X (listed among Feather-derived icons) and Play; no separate license text is substituted or omitted. Keep the entire file with redistributed assets and include it in the application's bundle resources. The license is also embedded in `IconAssets` as `lucide-LICENSE.txt`.

See the repository's `THIRD_PARTY_NOTICES.md` for notices, including the GPUI-derived text input. These third-party notices do not select or change the license of this project.

## Updating

Resolve one new official upstream commit, download all assets and the license from that same fixed commit, update this provenance record and `THIRD_PARTY_NOTICES.md`, then run the `ui-tests` icon tests. Review the complete upstream license when updating; do not drop the Feather notice.

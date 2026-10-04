# Carbonfox - opaque reference and attribution

The desktop uses **Carbonfox - opaque**, not the blurred Carbonfox variant or a similarly named theme. `carbonfox-opaque.json` contains the complete, unchanged selected variant (`name`, `appearance`, and `style`, including syntax and players) extracted from `themes/nvim-nightfox.json`, plus an `upstream` provenance object. No other variants are vendored.

## Pinned sources

- Zed port: [cange/nightfox.zed](https://github.com/cange/nightfox.zed), commit `3511a6f1f665455c70a24d14fd5d2de0eaab58fa`, resolved from GitHub's `repos/cange/nightfox.zed/commits/main` endpoint. [Theme source](https://github.com/cange/nightfox.zed/blob/3511a6f1f665455c70a24d14fd5d2de0eaab58fa/themes/nvim-nightfox.json); [MIT license](https://github.com/cange/nightfox.zed/blob/3511a6f1f665455c70a24d14fd5d2de0eaab58fa/LICENSE), copyright (c) 2024 Christian Angermann.
- Original palette: [EdenEast/nightfox.nvim](https://github.com/EdenEast/nightfox.nvim), commit `4dacd3f0185a2227bdf3b6c0975a8f0bf87cac9a`, resolved from GitHub's `repos/EdenEast/nightfox.nvim/commits/main` endpoint. [MIT license](https://github.com/EdenEast/nightfox.nvim/blob/4dacd3f0185a2227bdf3b6c0975a8f0bf87cac9a/LICENSE), copyright (c) 2021 James Simpson. This is a separately pinned original-project license reference, not a claim that the Zed port was generated from that revision.

The full upstream license files are retained unchanged as `nightfox-zed-LICENSE.txt` and `nightfox-nvim-LICENSE.txt`. Distributions must include both notices and permission texts; both are also reproduced in the repository's `THIRD_PARTY_NOTICES.md`. These third-party MIT licenses do not select or change the application's own license.

## Desktop token mapping and adaptations

`crates/app/src/desktop/theme.rs` centralizes the tokens. The upstream variant is preserved verbatim; only desktop tokens are adapted for compact, opaque controls. Each token remains opaque 24-bit RGB, and the window must use an opaque background appearance.

| Token | RGB | Upstream source / adaptation |
| --- | --- | --- |
| `BACKGROUND`, `INPUT_BG` | `#161616` | `background` / `editor.background` |
| `SURFACE`, `PANEL` | `#0c0c0c` | `surface.background` / `panel.background` |
| `CHROME` | `#0c0c0c` | `title_bar.background` / `status_bar.background` |
| `HEADER` | `#1c1c1c` | `toolbar.background` (`#2a2a2a8C`) composited over `PANEL` |
| `HOVER` | `#2a2a2a` | Neutral `ghost_element.hover`, rather than teal `element.hover` |
| `SELECTION`, `TEXT_SELECTION` | `#242424` | Neutral `element.selection_background` (`#52525357`) composited over `PANEL` |
| `BORDER` | `#222222` | Decorative `border` (`#252525E3`) composited over `PANEL`; not a control identification boundary |
| `TEXT` | `#f2f4f8` | `editor.foreground` |
| `MUTED` | `#b6b8bb` | `terminal.dim_foreground`, used without opacity for small muted/disabled labels |
| `FOCUS` | `#78a9ff` | `info` / `terminal.ansi.blue`, replacing low-contrast `border.focused` |
| `INPUT_BORDER` | `#7b7c7e` | `editor.line_number`, replacing decorative borders to ensure >=3:1 against input and control surfaces |
| `WARNING` | `#be95ff` | `warning` (Carbonfox's warning is violet, not yellow) |
| `ERROR` | `#ee5396` | `error` |
| `SUCCESS` | `#25be6a` | `success` |

Neutral selection is intentional: active controls do not use a bright accent fill. Focus remains independently visible via the blue outline. Opaque compositing uses rounded sRGB channels, `round(foreground * alpha + PANEL * (1 - alpha))`. Upstream's alpha-bearing style entries stay unchanged in the reference JSON.

Small text and muted labels meet WCAG AA (>=4.5:1) on background, chrome, panel, header, hover, selection, and input surfaces. The minimum muted-label ratio is 7.22:1 (hover); text is >=13.04:1. Focus is >=6.10:1 and input outlines >=3.43:1 on those surfaces. Warning, error, and success labels meet >=4.5:1 on panel/chrome/input backgrounds; error labels are not intended to be placed directly on a filled hover control. Pure token tests under `ui-tests` check contrast and exact upstream mappings.

Compact metrics: control height 28 px, control radius 3 px, square panes (0 px), titlebar 34 px, panel header/toolbar/status 28 px, and grid row height 22 px. Existing text-input height remains 28 px.

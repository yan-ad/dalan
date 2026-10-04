# Dalan design direction

Status: adopted compact Carbonfox UI foundation, with an experimental read-only database workspace. This is not a finished SQL client, accessibility certification, or completed brand identity. **Dalan** means “ways” in Javanese and remains text-only branding.

## Adopted direction

The user explicitly rejected DataGrip's visual style and selected **Zed-like compact UI with Carbonfox - opaque** as the main default. DataGrip is a database UX/workflow reference only, not permission to copy its button-heavy layout or chrome. No Zed editor/source/assets are imported other than the independently MIT-licensed Nightfox theme reference. No affiliation with Zed, DataGrip, or the theme authors is implied.

Reason: database workflows should be familiar without inheriting another product's visual clutter.

The workspace is database-only: one collapsible/resizable Database Explorer, a browser/read-only table view, a separate source setup window, and an optional database-focused ACP panel. Query consoles, SQL scripts and database import/export remain future scope; generic Files, code viewer, Git, build tools, terminal and plugin/toolbox chrome are excluded. ACP honestly says **Not connected**, with no transport, agent launch, prompt or provider settings.

Reason: quiet chrome leaves room for real database context rather than inactive tools.

## Carbonfox provenance and tokens

The exact selected variant is **Carbonfox - opaque**, from `themes/nvim-nightfox.json` in [cange/nightfox.zed at `3511a6f1f665455c70a24d14fd5d2de0eaab58fa`](https://github.com/cange/nightfox.zed/blob/3511a6f1f665455c70a24d14fd5d2de0eaab58fa/themes/nvim-nightfox.json). The complete variant is retained in [carbonfox-opaque.json](crates/app/assets/themes/carbonfox-opaque.json), including syntax/players, with provenance metadata. Runtime UI uses compiled [theme constants](crates/app/src/desktop/theme.rs), not a JSON theme loader or a query editor.

Both full MIT notices are retained: the Zed port, copyright 2024 Christian Angermann, and original Nightfox palette, copyright 2021 James Simpson. Original-project license revision `4dacd3f0185a2227bdf3b6c0975a8f0bf87cac9a` is a separate pin, not a claim about the original version used by the port. See [theme provenance and licenses](crates/app/assets/themes/README.md) and [third-party notices](THIRD_PARTY_NOTICES.md). Neither selects Dalan's project license.

Reason: exact provenance makes the permitted palette reproducible without borrowing unrelated assets.

| Shared token | Opaque RGB | Role |
| --- | --- | --- |
| CHROME / PANEL / SURFACE | `#0c0c0c` | Flush window chrome and panes |
| BACKGROUND / INPUT_BG | `#161616` | Input/editor-background reference; no query editor yet |
| HEADER | `#1c1c1c` | Compact toolbar/header |
| HOVER | `#2a2a2a` | Neutral pointer feedback |
| SELECTION / TEXT_SELECTION | `#242424` | Neutral opaque selection |
| BORDER | `#222222` | Decorative separator, not input identification |
| TEXT | `#f2f4f8` | Primary labels/data |
| MUTED | `#b6b8bb` | Secondary, placeholder and disabled labels |
| FOCUS | `#78a9ff` | Focus/active outline and primary action fill |
| INPUT_BORDER | `#7b7c7e` | Visible input boundary |
| WARNING | `#be95ff` | Warning with text/icon |
| ERROR | `#ee5396` | Error with text/icon |
| SUCCESS | `#25be6a` | Success with text/icon |

Upstream alpha-bearing toolbar, selection and border colors are composited over PANEL into opaque RGB, while the vendored reference stays unchanged. Main, source and About windows explicitly use opaque backgrounds: no blur, transparency, theme toggle or fake light mode. Normal status identifies **Carbonfox - opaque**; focused-control help temporarily overrides it as before. Inputs share the tokens, opaque text selection, FOCUS cursor and MUTED placeholder rather than hardcoded colors.

Reason: one honest default gives every window the same calm material and readable states.

Blue marks focus/active state and Save/Connect primary actions with dark text. Other controls are neutral with minimal borders and hover feedback. Disabled labels use MUTED, not opacity. Semantic pink error text is supported on panel/input backgrounds, not filled hover controls. User-owned source colors remain unchanged marker-only metadata; name and engine labels remain readable independently, and arbitrary marker colors have no AA guarantee or risk meaning.

Reason: accent should communicate action or focus, not decorate every button.

Pure token tests calculate minimum normal-state contrast: primary text **13.04:1**, secondary **7.22:1**, focus **6.10:1**, input boundary **3.44:1**. Semantic labels meet 4.5:1 on their supported backgrounds. These are computed color-pair checks, not native screenshot pixels, full accessibility compliance or proof of visual matching.

Reason: compact UI requires readable text and boundaries before stylistic claims.

## Compact geometry

| Metric | Adopted value |
| --- | --- |
| Titlebar | 34 px |
| Status / explorer toolbar / ACP header / table header | 28 px |
| Tree / grid rows | 22 px |
| Controls / control radius | 28 px / 3 px |
| Pane radius / outer body padding | 0 px / 0 px |
| Divider hit area / visible line | 4 px / 1 px |
| UI / data-browse type | System UI 13 px / 12 px |

Panes meet flush edges. Explorer prefers 320 px, bounded to 200–480 px, reserving 240 px main content; at 720 px its effective maximum is **476 px** with ACP closed (720 minus 4 minus 240). This supersedes the historical 6 px divider plus 6 px outer padding on each side. ACP starts closed, prefers 300 px and may temporarily suppress the explorer in compact layouts without changing its saved in-memory preference. Four Layout rows and Cmd-B/Cmd-Alt-0/Cmd-Shift-A remain unchanged.

Reason: regular dense rows and flush panes maximize useful context without extra chrome.

Source setup remains one normal **Data Sources · Dalan** window, initial 1040 × 760, minimum 780 × 560, not an OS modal sheet. The 720 px natural-height form scrolls with 16 px viewport padding, 8 px form gaps, and footer padding 8 px vertical/16 px horizontal. Inputs remain 28 px; endpoint parents and SSH candidates remain 30 px; the labeled Keychain indicator remains 18 px with 3 px radius. Host/Port traversal, reuse/draft behavior, focus guards, CA picker and saving close guards are unchanged. Table header/filter padding is 10 px horizontal/6 px vertical; footer is 10 px/4 px. No fake content is introduced.

Reason: tighter spacing must not shrink controls or change familiar source workflows.

System UI fonts require no external font assets. No monospace font or SQL editor is implemented. The nineteen pinned Lucide utility SVGs remain unchanged, dynamically colored through existing rendering, with no generated brand assets or app icon. [Asset provenance](crates/app/assets/README.md) retains complete ISC/Feather MIT attribution.

Reason: existing native typography and permitted utility icons are sufficient for this slice.

## Design dials and remaining validation

- **Energy 1:** quiet surfaces, no gradients or promotional ornament.
- **Rhythm 1:** dense regular rows and predictable geometry.
- **Motion 1:** minimal functional feedback, no decorative animation.

Reason: sustained database work should not compete with interface decoration.

Theme geometry and contrast are covered by unit/simulated checks, not measured native pixels or screenshot replication. Native visual/click-through, VoiceOver, scaled text and measured performance remain open gates; [performance budgets](docs/product-plan.md#proposed-release-gates) are targets, not measurements. No new capture or native visual pass is claimed. Light/system support is an unselected future design proposal, not a working mode or a reason to ship a toggle. Any future theme needs separate state/contrast/native validation.

The theme/layout change does not change database I/O, cache, SSH/TLS, passwords, profile persistence or Keychain semantics. Historical theme local verification passed 67 headless, 88 simulated UI and four bundle tests, formatting, strict lint and signed bundle/license checks. The rebuilt app is open. Window capture was blocked, so no native visual/accessibility pass is claimed. Theme commit `0ca0221` passed all five hosted jobs in [run 37204370849](https://github.com/yan-ad/dalan/actions/runs/37204370849). This is historical evidence for that commit; the wide-grid revision needs its own next-main CI run. Current wide-grid validation remains a separate gate; evidence is recorded in [testing](docs/testing.md).

## Wide-table data view

A retained `DataGrid` virtualizes both axes of the database page, with fixed 180 px columns, unchanged 22 px rows, 28 px pinned headers and 12 px browse text. Headers and cells share a pixel-calculated horizontal offset; the body alone scrolls vertically. Two-axis tracks have draggable minimum 20 px thumbs and track-click support. This is not screenshot-matched native evidence.

Wheel/trackpad, Shift-wheel and grid-focused arrows/PageUp/PageDown/Home/End/Ctrl-or-Cmd-Home/End scroll the viewport, not an active-cell selection. Only mounted visible/overscan headers enter Tab sorting traversal; offscreen headers are not focus stops. An unchanged stale page retains scroll during busy/saving/error updates with sorting disabled; new snapshots or selected targets reset it. Column resizing and cell inspection/copy remain future work. Carbonfox styling and other geometry are unchanged. See [UI foundation](docs/ui-foundation.md#two-axis-data-grid) and [operation counts](docs/testing.md#wide-grid-performance-revision).

[Overview](README.md) · [UI foundation](docs/ui-foundation.md) · [Architecture](docs/architecture.md) · [Development](docs/development.md) · [Testing](docs/testing.md) · [Feature checklist](docs/feature-checklist.md)

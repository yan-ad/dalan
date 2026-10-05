# Dalan design direction

Status: adopted compact Carbonfox UI foundation, with an experimental read-only database workspace. This is not a finished SQL client, accessibility certification, or completed brand identity. **Dalan** means “ways” in Javanese and remains text-only branding.

## Adopted direction

The user explicitly rejected DataGrip's visual style and selected **Zed-like compact UI with Carbonfox - opaque** as the main default. DataGrip is a database UX/workflow reference only, not permission to copy its button-heavy layout or chrome. No Zed editor/source/assets are imported other than the independently MIT-licensed Nightfox theme reference. No affiliation with Zed, DataGrip, or the theme authors is implied.

Reason: database workflows should be familiar without inheriting another product's visual clutter.

The workspace is database-only: one collapsible/resizable Database Explorer, a browser/read-only table view, a separate source setup window, and an optional database-focused ACP panel. Read-only query consoles and loaded-page CSV are implemented; persistent SQL scripts and broader database import/export remain future scope; generic Files, code viewer, Git, build tools, terminal and plugin/toolbox chrome are excluded. ACP honestly says **Not connected**, with no transport, agent launch, prompt or provider settings.

Reason: quiet chrome leaves room for real database context rather than inactive tools.

## Carbonfox provenance and tokens

The exact selected variant is **Carbonfox - opaque**, from `themes/nvim-nightfox.json` in [cange/nightfox.zed at `3511a6f1f665455c70a24d14fd5d2de0eaab58fa`](https://github.com/cange/nightfox.zed/blob/3511a6f1f665455c70a24d14fd5d2de0eaab58fa/themes/nvim-nightfox.json). The complete variant is retained in [carbonfox-opaque.json](crates/app/assets/themes/carbonfox-opaque.json), including syntax/players, with provenance metadata. Runtime UI uses compiled [theme constants](crates/app/src/desktop/theme.rs), not a JSON theme loader. The query editor uses the same opaque UI tokens without syntax highlighting.

Both full MIT notices are retained: the Zed port, copyright 2024 Christian Angermann, and original Nightfox palette, copyright 2021 James Simpson. Original-project license revision `4dacd3f0185a2227bdf3b6c0975a8f0bf87cac9a` is a separate pin, not a claim about the original version used by the port. See [theme provenance and licenses](crates/app/assets/themes/README.md) and [third-party notices](THIRD_PARTY_NOTICES.md). Neither selects Dalan's project license.

Reason: exact provenance makes the permitted palette reproducible without borrowing unrelated assets.

| Shared token | Opaque RGB | Role |
| --- | --- | --- |
| CHROME / PANEL / SURFACE | `#0c0c0c` | Flush window chrome and panes |
| BACKGROUND / INPUT_BG | `#161616` | Input and query-editor background |
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

Upstream alpha-bearing toolbar, selection and border colors are composited over PANEL into opaque RGB, while the vendored reference stays unchanged. Main, source and About windows explicitly use opaque backgrounds: no blur, transparency, theme toggle or fake light mode. The status spacer exposes **Carbonfox - opaque** only in a tooltip; no persistent theme name or focused-control help is painted. Inputs share the tokens, opaque text selection, FOCUS cursor and MUTED placeholder rather than hardcoded colors.

Reason: one honest default gives every window the same calm material and readable states.

Blue marks focus/active state and Save/Connect primary actions with dark text. Other controls are neutral with minimal borders and hover feedback. Disabled labels use MUTED, not opacity. Semantic pink error text is supported on panel/input backgrounds, not filled hover controls. User-owned source colors remain unchanged marker-only metadata; source names remain readable and tooltips identify engines independently of color. Arbitrary marker colors have no AA guarantee or risk meaning.

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

Source setup remains one normal independent window, initial 1040 × 760, minimum 780 × 560, not an OS modal sheet. Its General, Options, SSH/SSL and Schemas tabs occupy a 34 px transparent native titlebar with 84 px reserved for traffic lights; an empty native title hides the duplicate “Data Sources · Dalan” caption. The body stays Carbonfox - opaque. The 720 px natural-height form scrolls with 16 px viewport padding, 8 px form gaps, and footer padding 8 px vertical/16 px horizontal. Inputs remain 28 px; endpoint parents and SSH candidates remain 30 px; the labeled Keychain indicator remains 18 px with 3 px radius. Host/Port traversal, reuse/draft behavior, focus guards, CA picker and saving close guards are unchanged. Table header/filter padding is 10 px horizontal/6 px vertical; footer is 10 px/4 px. No fake content is introduced.

Reason: tighter spacing must not shrink controls or change familiar source workflows.

System UI fonts require no external font assets. The in-memory SQL editor uses system Menlo at 13 px, 22 px lines and a 44 px gutter, with visible-line shaping. Twenty-eight pinned Lucide utility SVGs are dynamically colored through existing rendering: the previous nineteen plus hard-drive, triangle-alert, loader-circle, lock-keyhole and chevron-left. No generated brand assets or app icon are added. [Asset provenance](crates/app/assets/README.md) retains complete ISC/Feather MIT attribution; Carbonfox and SQLite notices and bundle-resource contracts are unchanged.

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

## Icon-led chrome

The custom titlebar omits duplicate **Dalan** branding; the macOS application menu and About dialog retain the name, and a database toggle occupies that space beside the traffic lights; a 28 px Layout icon opens the existing four-row popover with unchanged shortcuts/focus return. The explorer toggle is in the titlebar, with no bottom-left duplicate; ACP remains bottom-right. Action labels live in tooltips, not persistent status help; visible focus outlines remain. No extra Theme button or debug status is introduced.

Source rows show their glyph, marker and name, not a duplicate engine badge. Engine/full name and real known counts are in tooltips; Tables and Views keep their meaningful grouping labels. Fixed 18 px hard-drive, warning-triangle and static loader-circle markers mean Cached, Stale and Refreshing, with timestamp/sanitized error in tooltips. Important connection errors, stale-page feedback and cache warnings stay visible text.

With saved sources but no selected/loaded table, center a small table icon and **Select a table**, without Table browser/read-only chrome or a giant welcome. With no profiles, retain the working **Connect to a Source** action. A selected/loaded table shows its actual qualified title and passive 28 px read-only lock tooltip, not an edit button. Apply/check, Clear/minus, Export/download and Previous/Next chevrons are 28 px controls with unchanged IDs, guards and keyboard actions. Filter column names (without a Column prefix), operators and values remain visible. The footer's `1–100` range exposes loaded count and `has_more` in a tooltip, not a fabricated total.

This UI-only slice preserves virtual tree/grid rendering, `Arc<TablePage>`, cached rows and connection/selection/credential behavior. Historical credential fix `9b3d3d8` passed all five jobs in [run 37212641100](https://github.com/yan-ad/dalan/actions/runs/37212641100). Expected current totals are 74 headless, 106 simulated UI and four Python tests, **pending owner-run validation and this revision's own main CI**. No native screenshot, manual/VoiceOver or performance pass is claimed; capture was previously blocked. Existing native source values must not become documentation fixtures or published screenshots.

[Overview](README.md) · [UI foundation](docs/ui-foundation.md) · [Architecture](docs/architecture.md) · [Development](docs/development.md) · [Testing](docs/testing.md) · [Feature checklist](docs/feature-checklist.md)

## Workspace tabs and console design

The database workspace now retains one model/view per tab, up to 32 total. Source UUID/database/table identity deduplicates table tabs; consoles use unique session identities and monotonic Console N names. The 28 px flat Carbonfox strip uses table/query icons, readable name, close x, nonempty-draft dot and an in-flight indicator. These carry actual state, not decorative controls copied from DataGrip. Closing the active tab selects the left neighbor, or the first remaining tab; closing an inactive tab does not change selection.

The explorer adds a seventh New Query Console icon and horizontal toolbar scrolling at its 200 px minimum width. The strip plus uses active-tab context; explorer creation uses explorer-selected source/database or the source profile default, otherwise No default database. A cached-name database chooser performs no discovery itself. No native New Query Console menu entry is claimed. Cmd-W is tab close with an active workspace tab; native OS window close remains unchanged. The empty-workspace fallback belongs to the shell close-window handler, not a swallowed no-op.

The console editor is database-only text entry with native GPUI input/IME and clipboard, not a generic code/file viewer. It has 64 KiB text and 100 undo-state bounds, four-space Tab, Shift-Tab unindent and newline autoindent. Run/Cancel are play/stop icons with shortcuts; there is no syntax highlighting, completion, saved history, cursor-statement splitting, write control or persistent transaction UI. Nonempty drafts always require close confirmation, even after successful execution; Keep Open has default focus and Enter/Space activation. SQL and results stay in memory. See [guide](docs/query-consoles.md) and [ADR](docs/adr/0004-workspace-tabs-and-read-only-consoles.md).

## Source-manager controls

General keeps Name/Color and real driver/authentication combos; endpoint modes are Default, Unix Socket and credential-free URL-only. The Password row keeps its right-side labeled Save in Keychain checkbox. Options are applied backend limits, not decorative properties. SSH/SSL names transport honestly (Direct/SSH/HTTP or HTTPS CONNECT, not SOCKS); SSL warnings distinguish encryption, chain verification and hostname verification. Schemas is a searchable checkbox visibility filter, not authorization. The separate SSH manager owns selection, draft, Test cancellation and guarded Apply/Use/Cancel semantics. Native pickers complement manual paths; no vendor logos, passphrase UI, IDE truststore or arbitrary JDBC properties are introduced.

The [source-management guide](docs/source-management.md) is the current interaction contract. This is a functional subset informed by database workflows, not an exact DataGrip-native-property or screenshot match. Simulated geometry/titlebar tests do not prove native appearance; window capture/manual review remain unverified.

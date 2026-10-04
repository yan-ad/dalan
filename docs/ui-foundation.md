# UI foundation

Implemented scope: macOS desktop shell with the user-selected Zed-like compact **Carbonfox - opaque** default. The user explicitly rejects DataGrip visual styling; DataGrip remains a database UX/workflow reference only, not button-heavy layout/chrome to copy. Main content hosts the experimental MySQL/MariaDB browser/read-only table workspace; source setup uses a separate dedicated dialog window. This is not a completed SQL client or accessibility/theme system.

## Composition and purpose

```text
[ native window controls | Dalan                         Layout icon ]
[ Database Explorer ][ database browser / table view ][ optional ACP ]
[ panel-left | spacer (Carbonfox - opaque tooltip) | bot-message-square ]
```

- Native macOS traffic lights remain real OS controls. The native titlebar is integrated with the custom top bar; every GPUI window explicitly has an opaque background (no blur/transparency); its Dalan title region drags the window, and double-click requests the native zoom behavior.
- A 34 px top bar, 28 px pane/table headers, single 28 px explorer toolbar without a title header, and 28 px status strip establish compact proportions. Main content has database browsing and bounded table results, but no SQL editor, welcome cards or gradients.
- The single Database Explorer sidebar defaults to 320 px, bounded to 200–480 px. Opaque panes are square (0 px radius), with flush edges, no outer body padding, and a 4 px divider hit area around a 1 px visible line; no floating cards or reserved rail space. The optional scoped right ACP panel is initially closed; there is no Files rail.
- System UI typography is a calm, dense 13 px, with 12 px secondary copy and an 11 px status strip. No font assets or logo are introduced.
- Twenty-four pinned Lucide SVGs provide utility glyphs: database, database-zap, plus, settings-2, refresh-cw, trash-2, panel-left, chevron-down, minus, bot-message-square, arrow-up, arrow-down, download, check, chevron-right, folder, table, list-tree, chevrons-down-up, hard-drive, triangle-alert, loader-circle, lock-keyhole and chevron-left. Local trash-2.svg is the unchanged upstream trash.svg alias from the same pin; see the [asset README](../crates/app/assets/README.md). No vendor-brand artwork, decorative AI sparkle or icon-font dependency.
- Database Explorer shows real saved sources and discovered databases/tables. No fabricated sources or results.
- The status strip has an empty spacer with a Carbonfox - opaque tooltip, not persistent theme/focus-help text or an extra Theme control. It does not imply a connected database or agent and offers no theme switch. Its bottom-right 28 px bot-message-square icon button, with AI · ACP tooltip, toggles the right panel. The panel says **Not connected**, with no ACP transport or agent launch implemented, no text prompt input yet, and no BYOK/provider settings.

Database Explorer has only a 28 px toolbar with six 28 px icon actions: Add, Manage, Refresh, Remove, Expand Loaded and Collapse All. The Database Explorer title header is removed, as are the activity rail and header hide/minimise controls. The bottom-left 28 px panel-left toggle is unchanged; Cmd-B and native View/Layout alternatives remain. Closing the explorer retains that preference until toggled or reset. All six actions have action-specific tooltips; no inactive DDL, console or advanced-option icons are shown. Source rows show name and abstract Lucide database/MySQL or database-zap/MariaDB cue, not duplicate engine badges or vendor logos; tooltips identify the engine. Optional color affects only the small marker; selection/focus and names stay independently readable.

With no sources, the main area centers a working Connect to a Source action, not a demo/trial welcome screen. It opens the same dedicated source dialog window by mouse, Enter or Space and remains available when Database Explorer is hidden. Loading and sanitized errors remain visible in the appropriate source/browse state.

## Controls

| Control | Behavior |
| --- | --- |
| Bottom-left 28 px panel-left | Toggle Database Explorer; retain the closed preference until toggled or reset |
| Explorer Add / plus | Open/reuse the dedicated source dialog window; disabled during saving |
| Explorer Manage / settings-2 | Edit selected source; disabled without selection or while busy/saving |
| Explorer Refresh / refresh-cw | Refresh complete explicitly explorer-selected source metadata without deleting the old snapshot first; disabled during source refresh/saving, not table pagination; no current-table fallback |
| Explorer Remove / trash-2 | Confirm removal of a captured stable source UUID; another source's current page is preserved; selection/busy/save guards apply |
| Explorer Expand Loaded / list-tree | Expand cached branches only; no network fan-out |
| Explorer Collapse All / chevrons-down-up | Collapse branches and cancel their catalog work, preserving cached metadata |
| Main Connect to a Source | With no sources, open/reuse the dedicated source dialog window by mouse or Enter/Space, even with explorer hidden |
| Top-bar 28 px Layout / panel-left icon | Open/close the existing four-row layout popover; titlebar shows Dalan only |
| Menu toggle row | Toggle Database Explorer; show requested Shown/Hidden preference |
| Menu narrow/widen rows | Adjust Database Explorer preference by 32 px, bounded to 200–480 px |
| Reset layout | Show Database Explorer, restore its 320 px width, close ACP |
| Pane divider drag | Resize Database Explorer; dragging right widens it |
| Focused divider Left/Right | Move divider by 16 px; keyboard alternative to dragging |
| Escape | Close menu and return focus to Layout; stop active resize. Close ACP only when focus is inside that panel, returning focus to AI · ACP |
| Tab / Shift-Tab | Navigate controls; while menu is open, cycle only through its rows |
| Enter / Space | Activate focused buttons |
| Cmd-B | Toggle Database Explorer |
| Cmd-Alt-0 | Reset layout |
| Bot-message-square (AI · ACP label) / Cmd-Shift-A | Toggle the scoped ACP panel |
| ACP panel close button | Close panel and return focus to AI · ACP |
| Column header click / focused Enter or Space | Cycle ascending, descending, no explicit sort; retain filter and reset offset |
| Export loaded CSV / download | Native save picker for the fresh complete loaded page; visible success/cancel/error; no overwrite |
| Native About Dalan menu | Open the separate About window |
| Cmd-W / Cmd-Q | Close window / quit app |
| Native View menu | Toggle Database Explorer, toggle AI panel, reset layout |

Layout has exactly four rows: toggle Database Explorer, narrow Database Explorer, widen Database Explorer, and reset layout. macOS bindings use Command, not Control.

Controls retain action-specific hover tooltips and a high-contrast focus border/resize indicator, without persistent status focus-help text. The menu focuses its first row on open and returns to the trigger on dismissal. Source add/edit/delete and connection controls are implemented; no generic search or version-control controls are added. AI · ACP exposes only panel visibility, not a working agent feature. Simulated focus and tooltip tests do not establish screen-reader support.

## Resize behavior

The window minimum is 720 × 480. Compact layout clamps Database Explorer toward its 200 px minimum while reserving at least 240 px for main content. At 720 px with ACP closed, the maximum effective sidebar width is 476 px: 720 minus a 4 px divider hit area and 240 px content. The ACP panel prefers 300 px, capped by available space; its initial visibility is false. While ACP is visible, a compact window may temporarily hide Database Explorer. Database visibility and width preferences survive this temporary suppression and are restored after closing ACP or when space allows. Layout preferences are in-memory only; source profiles have separate versioned persistence.

A pane toggle changes the user's visibility preference, independently of width clamping. The bottom-left toggle's selected state reflects actual visibility; the menu's Shown/Hidden text reflects the retained preference.

## About Dalan

The native macOS application menu opens **About Dalan** in a separate 420 × 280 nonresizable GPUI window. It displays the Cargo package version, the name’s Javanese meaning “ways,” and database-workspace scope. It uses existing GPUI/native integration, not external libraries. The titlebar brand is capitalized **Dalan**; no brand/app icon artwork is introduced. Utility icons are distinct from app identity.

## Adopted Carbonfox - opaque tokens

The shared compiled [theme](../crates/app/src/desktop/theme.rs) is used by the main, source and About windows and inputs. See [design](../DESIGN.md#carbonfox-provenance-and-tokens) for the full RGB table and reasons. CHROME/PANEL/SURFACE are `#0c0c0c`, input/editor-background reference `#161616`, HEADER `#1c1c1c`, HOVER `#2a2a2a`, opaque SELECTION `#242424`, decorative BORDER `#222222`, TEXT `#f2f4f8`, MUTED `#b6b8bb`, FOCUS `#78a9ff`, INPUT_BORDER `#7b7c7e`, WARNING `#be95ff`, ERROR `#ee5396`, SUCCESS `#25be6a`.

The exact complete variant is [vendored](../crates/app/assets/themes/carbonfox-opaque.json) from Nightfox's Zed port commit `3511a6f1f665455c70a24d14fd5d2de0eaab58fa`, `themes/nvim-nightfox.json`. The [provenance record](../crates/app/assets/themes/README.md) retains both MIT licenses: 2024 Christian Angermann (port), 2021 James Simpson (original palette). The original-project license is pinned separately at `4dacd3f0185a2227bdf3b6c0975a8f0bf87cac9a`; the port's original palette revision is unknown. No affiliation is implied. Compiled tokens, not JSON, drive runtime rendering; the full reference is used by tests only.

Upstream alpha toolbar/selection/border entries are composited over PANEL into opaque RGB, not translucent GPUI fills. No blur, transparency, light imitation or theme toggle is shipped. Focus/active state uses blue outlines, and Save/Connect primary fills use blue with dark text. Neutral controls have minimal borders and hover feedback. Disabled text uses MUTED without opacity. Inputs share opaque selection, FOCUS cursor and MUTED placeholder; no independent hardcoded palette remains.

Pure color-pair tests calculate minimum primary **13.04:1**, secondary **7.22:1**, focus **6.10:1**, input boundary **3.44:1** across normal surfaces. Semantic labels meet 4.5:1 on panel/input backgrounds, not arbitrary hover controls; pink error text must not be put on filled hover controls. This is computational evidence, not native screenshot pixel measurement, visual replication, VoiceOver or scaled-text validation.

## Modules and verification

- `crates/app/src/shell_state.rs`: GPUI-free database/ACP visibility, width, and compact-layout rules; headless regression coverage.
- `crates/app/src/desktop.rs`: view composition, control focus, input handlers, native titlebar/menu, and ACP panel.
- `crates/app/src/desktop/about.rs`: About Dalan window, Cargo version, dismiss and reuse behavior.
- `crates/app/src/desktop/theme.rs`: adopted tokens and chrome dimensions.
- `crates/app/src/desktop/icons.rs`: embedded Lucide SVG asset source, wired in the desktop entry point; explicit arrow-up/arrow-down sort direction.
- `crates/app/src/desktop/tests.rs`: opt-in GPUI event simulation for controls, menus, focus, resizing, compact windows, ACP toggling/dismissal, and About behavior.

```sh
cargo test --workspace --locked
# macOS development path without offline Metal compiler:
cargo test -p dalan-app --bin dalan --features runtime-shaders,ui-tests --locked
./scripts/macos --open
```

`ui-tests` enables GPUI's own `test-support` dependency graph and stays off by default. Simulated event tests are separate from real macOS/VoiceOver/manual rendering evidence. Standard desktop builds still require full Xcode/Metal.

## Historical validation: initial two-sidebar iteration

These results apply only to the previous iteration, not the newer database-only shell.

- Default workspace: 17 tests passed. Opt-in GPUI input suite: seven tests passed. Formatting, headless/UI strict Clippy, and runtime-shader desktop build passed.
- Simulated click-through: both rail toggles and header hide buttons; all seven Layout rows; trigger open/close, Escape and outside dismissal. Simulated keyboard: complete forward/reverse Tab sequence, menu focus cycle, Enter/Space activation, pane/reset shortcuts, divider arrows, and Cmd-W. Divider drag and compact/expanded viewport geometry also passed.
- Latest real desktop binary launched on local macOS and stayed running without captured startup errors. Window-only screenshot capture was attempted but macOS returned `could not create image from window`; no Screen Recording/Accessibility permissions were changed. Visual matching, native traffic-light/titlebar interaction, manual click-through, actual accessibility, and 200% text scaling are not claimed verified.
- Hosted CI and standard offline-Metal builds were not run in this iteration; the known full Xcode/Metal prerequisite remains. Existing dependency future-Rust warnings for `block` and `proc-macro-error2` remain unchanged.

## Historical validation: database-only and app-bundle iteration

These recorded results predate the About Dalan and optional ACP-panel changes; they do not validate those additions.

- Rewritten single-sidebar checks passed: 17 headless workspace tests, seven GPUI input tests, and four Python bundle-helper tests. Formatting, headless/UI strict Clippy, and runtime-shader debug build passed.
- Simulated controls passed: database rail/header hide, all four Layout rows, open/close/Escape/outside dismissal, Tab/Shift-Tab focus, Enter/Space, Cmd-B/Cmd-Alt-0/Cmd-W, divider drag/arrows, and 720 px clamp/restored preferred width. Regression assertions verify no Files pane/rail/hide/resize controls or selectors exist.
- `./scripts/macos` produced `target/debug/bundles/Dalan.app`. Plist lint and strict ad-hoc signature verification passed. Executable dependencies inspected with `otool -L` are macOS system libraries/frameworks, with no external runtime library paths observed.
- `./scripts/macos --open` successfully launched the bundle through macOS Launch Services. `NSRunningApplication` reported display name `Dalan`, bundle ID `local.dalan.debug`, the bundle path, and executable `Dalan.app/Contents/MacOS/Dalan`. The app was left open for the user.
- This verifies app identity/launch, not visual matching, native titlebar click-through, VoiceOver, scaled text, hosted CI, notarization, or release/offline-Metal builds. Those remain separate checks. Local ad-hoc signing is not a release-distribution guarantee.

## Historical validation: About Dalan and ACP-panel iteration

- Passed 19 headless Rust tests, 11 simulated GPUI interaction tests, and four Python bundle-helper tests (34 total). Formatting, both strict Clippy paths, runtime-shader build, plist lint, and ad-hoc bundle signature verification passed.
- ACP tests check bottom-right button bounds, click/Enter/Space/Cmd-Shift-A activation, close/focus return, panel-focused Escape versus Escape outside the panel, compact layout, and restoration of Database Explorer. Tab order includes the ACP trigger. Main content remains blank.
- About tests dispatch the same action used by the macOS menu, verify a single reused 420 × 280 window, and exercise Done, Escape, and Cmd-W close paths. The window displays Cargo's package version; no duplicated version constant or additional dependency was added.
- Rebuilt Dalan.app, gracefully quit the older development instance, and reopened the bundle. macOS reported display name Dalan and executable Dalan.app/Contents/MacOS/Dalan. The updated app was left open.
- Actual menu click-through, visual comparison, VoiceOver/scaled text, hosted CI, notarization, and release/offline-Metal builds remain unverified. ACP transport/authentication, agent launch, and prompts remain unimplemented; the panel explicitly says Not connected. Existing upstream future-Rust warnings are unchanged.

Generic Files explorer, generic code viewer, Git, build/run integrations, generic terminal, and plugin/toolbox chrome are excluded. The optional database-focused ACP panel is explicitly permitted, not an exception allowing generic tools. SQL query consoles, database script workflows, and database-focused import/export remain valid later scope.

The historical blank-main state above is superseded by the source slice below. SQL editor work requires its own scoped milestone. Text scaling and native accessibility remain release gates; light/system support is an unselected future proposal, not a shipped mode.

## Compact lazy explorer

Rows are 22 px high. Sources have an abstract driver icon, optional color marker and readable name, with engine name and loaded database count in the tooltip. Databases have disclosure and database icons. Tables and Views retain their folder icons and meaningful names; known counts are in full-name tooltips rather than trailing numbers. Table/view leaves use Table icons. Tables automatically opens after metadata arrives; Views starts collapsed and its leaves remain unavailable, issuing no browse query. Unread metadata must not imply view availability or a zero count.

Names have explicit constrained text layout, nonblank ellipses and full-name tooltips. Unicode and quoted names remain distinct through collision-safe IDs. This replaces per-node full-hierarchy buttons; the reported blank state has no measured root cause, and the change is not evidence of a font or GPU defect.

The tree has one focusable list, with Up/Down and Home/End navigation. Right expands or enters a branch; Left collapses or moves to the parent. Enter/Space toggles branches; Enter on an available table leaf browses it. Toolbar Enter/Space invokes the focused action, not the tree; tree-only keys require tree focus. Clicking a row's disclosure chevron stops propagation so it does not also trigger row activation.

The flattened tree is rebuilt on model notifications, not wheel events. A uniform-list viewport renders only the requested visible range, keeping offscreen rows out of the painted hierarchy. Cached database expansions survive filters, table pagination and source changes. Expand Loaded opens cached branches without network fan-out; Collapse All keeps metadata. Branch loading/error status dots expose full sanitized errors in tooltips; default global-error feedback remains available. Explorer selection is independent of the source owning the current table page, so Manage/Remove on another source preserve that page. Refresh fetches complete metadata for an explicitly selected explorer source, retaining the old tree/page on failure; no table-source fallback is allowed.

Scoped simulated GPUI coverage passed **71 tests**, including 1,000 databases with at most 40 painted rows, scrolling to row 900 and End navigation. No native manual smoothness, FPS, latency, VoiceOver or scaled-text result is implied. Full workspace/bundle validation passed 57 headless, 71 UI and four Python tests with formatting, strict lint and bundle checks; hosted CI for the new revision remains pending; see [testing](testing.md#compact-lazy-explorer).

### Persistent metadata feedback

The same virtual 22 px rows restore offline metadata without startup network or Keychain calls. Source rows expose fixed 18 px hard-drive (**Cached**), triangle-alert (**Stale**) and static loader-circle (**Refreshing…**) markers; their tooltips retain meaning, full Unix-seconds fetched timestamp and sanitized error instead of persistent labels. Color is not the sole signal. Cached Tables/Views groups expand locally first. Refresh requires an explicit valid explorer selection, is disabled during source refresh/profile saving, and stays available during table pagination. A failed or disconnected refresh retains the old database/table tree and displayed page instead of clearing them. Cached names may reflect old privileges and are not offline row access.

A nonfatal 22 px `metadata_notice` strip with full tooltip warns about unavailable/corrupt disk cache, cache-save failure after a successfully committed profile, or orphaned metadata after failed cache deletion. Valid profile JSON still loads; no blank explorer or false profile-save failure is implied. Historical metadata local verification passed **67 headless tests**, **83 simulated UI tests** and **four Python bundle tests**, plus formatting, strict lint and signed bundle checks, including cached labels, warning visibility, offline startup, retained rows and explicit-selection gating. This is simulation, not native visual/accessibility, actual Keychain or production save-to-network end-to-end evidence; historical hosted gates are recorded in [testing](testing.md#persistent-metadata-revision).

## Dedicated source dialog window

**Data Sources · Dalan** is one application-wide, resizable normal GPUI window: initial 1040 × 760, minimum 780 × 560. It is not an OS modal sheet and does not trap or block the main window. Add, Manage and the centered connection action activate/reuse it, preserving draft edits. A new form or model `form_generation` refresh (including loaded passwords) replaces the form inside the existing window and focuses Name; ordinary notifications do not reset it. Cancel/Escape/Cmd-W/native close discard the draft and cancel testing. Native close and Cmd-W refuse dismissal while credential/JSON saving is active. Model-observed successful Save closes the window, then starts metadata-only refresh, without automatic row browsing.

The form starts at Engine, without a redundant internal title strip. Compact styling uses 8 px form gaps, 16 px scrolling viewport padding and footer padding 8 px vertical/16 px horizontal; controls are 28 px with 3 px radius. Endpoint/candidate parent rows remain 30 px, and the Keychain indicator remains 18 px with 3 px radius. Existing APIs, traversal and focus/saving guards are unchanged. Database, SSH, HTTP and HTTPS Host/Port fields share horizontal rows, defaulting to localhost and 3306/22/8080/443 respectively. Port is 96 px wide, minimum 80 px; native-input Tab still moves Host → Port. The explicit body maximum is 720 px; zero minimum widths allow text truncation, while nonshrinking rows retain 28 px input and 30 px candidate heights. The viewport scrolls a natural-height body instead of vertically compressing it. The key list is capped at 150 px and long labels ellipsize.

The supplied screenshot may predate the full-width form, but it exposed a real flex-shrink defect, not merely an old sidebar layout. Draw-bound regressions at 1040 × 760 and 850 × 600 exercise long values, key labels and password/color changes, proving fields no longer collapse to single-character widths in simulation. Historical source-dialog local suites passed 54 headless, 63 simulated UI and four Python tests. Native visual/accessibility review remains unverified; existing capture attempts were blocked by Screen Recording permissions, which were not changed. See [testing](testing.md#source-dialog-window-and-windows-fixture-fix) for CI evidence and the pending next-main gate.

## Current source and read-view slice

Source setup has no separate Data Source/repeated-engine header. Password and its labeled Keychain checkbox share a row; the checkbox has an 18 px visible indicator and Lucide check rather than a Unicode glyph. CA path accepts both manual edits and a native single-file Browse action. Cancel preserves the path, stale selections do not replace newer manual edits, and focus returns to the path after the dialog. Native Open-dialog behavior remains a manual macOS check; shared completion and form control behavior have regression tests.

The main area remains the browser/table workspace. The dedicated Add/Edit Source dialog has engine, name, endpoint, user, optional database and password fields, plus transport/TLS choices. Database defaults to None, host/port to localhost:3306 and user to root; recommend a least-privilege account. Opening Add/Edit focuses Name; Escape cancels. SSH exposes an optional Known hosts file field requiring an absolute existing file. Test does not save. Save commits profile/credentials, closes the dialog and automatically refreshes metadata only; Connect on the selected source can discover missing metadata. Delete is confirmed and removes local settings/Keychain only, not server objects.

Passwords are session-only unless saved explicitly to native macOS Keychain. After restart, Edit/re-enter a session-only password. Profiles use stable UUIDs and version 1 JSON without passwords. JSON/Keychain changes are not atomic across resources and compensation failures remain visible. Session-only Save is tested without a Keychain call, save failures are visible and a failed profile load blocks overwriting settings. The file limit is 1 MiB with at most 100 profiles.

Source-form traversal uses a persistent root whose stored Tab-stop state is false, a single tracked focus handle per input, and a form Tab group in visual order. All visible controls participate; keyboard focus can leave inputs instead of being trapped by duplicated nested handles. Database followed by Transport is intentional. Double-click (two or more clicks) selects all Unicode or password text; the ensuing drag does not shrink that selection. Password copy/cut is suppressed and native surrounding-text requests do not expose the secret. These protections do not establish native accessibility/IME validation.

SSH has an explicit background metadata-only identity picker for likely `$HOME/.ssh` files, preserving manual paths with no automatic selection and an explicit Use SSH agent choice. Candidate filenames are not proof of key format. There is no new passphrase prompt; unlock encrypted keys with `ssh-add` outside Dalan. Strict known-host checking is unchanged. See [MySQL sources](mysql-sources.md) for discovery limits and troubleshooting.

The table view uses 28 px headers and 22 px grid rows, 12 px data-browse typography, header/filter padding 10 px horizontal/6 px vertical and footer padding 10 px/4 px, without fake content. The table view offers 100-row pages and a column-cycle filter with Contains, Equals, NotEquals, greater than, less than, is null and is not null. Views are listed but unavailable for browsing. Stale rows remain visibly labeled on errors or filter changes, retaining the labeled previous page with pagination disabled until refresh succeeds. Source changes clear old rows. Primary-key order where available is not a cross-page snapshot. Column headers now cycle ascending/descending/none by click or Enter/Space, retaining filters and resetting offset. Loaded CSV uses a native save picker (default `Dalan-loaded-page.csv`) and fresh nontruncated rows only; visible feedback reports success, cancellation or errors. No arbitrary SQL, writes, full-query export or whole-table export are exposed. Transport, TLS, representation and backend bounds are documented in [MySQL sources](mysql-sources.md).

The input control adapts GPUI Apache-2.0 code with attribution. Lucide SVGs are explicitly permitted open-source utility assets pinned to `500620a2e8123f8d1db191538886dc0c223f69a9`. Complete ISC and retained Feather MIT notices are in [third-party notices](../THIRD_PARTY_NOTICES.md), alongside GPUI input Apache-2.0 attribution. The bundle includes the root notices (including both full Nightfox MIT texts) and Lucide license in Resources. Theme reference JSON and standalone theme license files are not runtime-loaded or separately required bundle resources. The bot/message glyph represents chat-agent communication, not app branding. No reference advanced-options panel, marketplace, visual tools or vendor-brand artwork are copied. Dependency attribution does not choose a project license; that remains undecided.

## Two-axis data grid

### Icon-led browser controls

Without a selected table or loaded page, no Table browser/read-only header is painted. Saved sources, including offline caches with no source selected, show centered **Select a table** with a small table glyph; empty profiles retain the working **Connect to a Source** action. A selected/loaded table displays its qualified title and a passive 28 px lock-keyhole tooltip: **Read-only preview. Editing isn't implemented.** The lock is neither clickable nor a promised edit feature.

Apply/check, Clear/minus, Export/download, Previous/chevron-left and Next/chevron-right use 28 px icon buttons, action tooltips and existing IDs (`apply-filter`, `clear-filter`, `export-loaded-page`, `previous-page`, `next-page`), keyboard activation and guards. Actual filter column names, operators and values remain visible; no Column prefix is needed. Footer ranges such as `1–100` have a tooltip reporting loaded rows and `has_more`, never a total count. Loading/errors, stale-page feedback, truncation and cache warnings remain visible text. This changes chrome only, not virtualized rendering, shared snapshots, cached rows or selection/connection behavior. Native Open-dialog and tooltip rendering remain manual checks, not GPUI title-string assertions or an accessibility guarantee.

`SourceBrowser` retains a separate `DataGrid` entity rather than eager cell elements. The data viewport uses fixed 180 px columns, 22 px rows, a vertically pinned 28 px header and shared horizontal header/body offset. Actual layout bounds determine visible ranges with two-cell overscan; text/header caches retain only these ranges. New page snapshots or selected targets reset scroll. Busy, saving or error state with the same page preserves the stale view and scroll; stale sorting stays disabled.

- Wheel/trackpad scroll both axes; Shift-wheel supplies horizontal scrolling.
- With the grid itself focused, Left/Right move one column and Up/Down one row. PageUp/PageDown move one body viewport vertically. Home/End reach horizontal edges; Ctrl/Cmd-Home/End reach both-axis edges.
- Both visible scrollbar tracks accept thumb dragging (minimum 20 px thumb) and track clicks, with clamped offsets through resize and content edges.
- Tab traversal includes only mounted visible/overscan headers. Click or Enter/Space retains the existing sorting cycle and guards; grid scrolling does not hijack focused header activation.

This is viewport navigation, **not** active-cell selection or cell inspection/copy. Offscreen headers are not hundreds of focus stops; no screen-reader readiness is claimed. Typed values and loaded-page CSV remain unchanged. Simulated header/cell pixel alignment and bounded materialization are recorded in [testing](testing.md#wide-grid-performance-revision); native smoothness, high-DPI, visual and accessibility checks still require a user retry of the latest macOS build.

## Database Explorer redesign validation

Color (optional) sits below Name, with manual `#RRGGBB` entry and labeled Default, Blue, Green, Amber, Red and Purple swatches. Default stores None. Color is a marker, not a production/risk classification; custom colors are not guaranteed AA. Version 1 JSON uses an optional `color` field with `serde(default)` for legacy profiles. Legacy load produces None without automatic rewrite, preserves exact mixed-case hex on round-trip, and does not touch Keychain. The source-field whitelist accepts color while password rejection and failed-load overwrite protection remain unchanged.

Verified migration/color tests preserve legacy None and mixed-case hex and reject malformed values without overwriting settings. Historical redesign totals: **54 headless**, **57 simulated UI** and **four Python bundle tests** passed, along with formatting, strict lint, bundle build and plist/signature/license checks. Native visual/accessibility, hosted CI, live transports and Keychain were not rerun for this UI/profile-only iteration.

## Historical source-slice validation

- Six live fixture smokes passed on MySQL 8.4.11 and MariaDB 11.4.13: direct TCP/HTTP CONNECT, all filter operators and fixture values, view rejection and untrusted default-TLS rejection.
- Historical source-slice evidence: passed 37 default headless Rust tests (4 ACP, 13 app, 5 core, 15 driver), 27 simulated GPUI tests (3 native-input, 5 source-form, 4 source-model, 3 browser, 12 shell) and four Python bundle-helper tests. One generated native Keychain round-trip passed with cleanup. Historical counts above are not evidence for this slice.
- Ten secure-transport tests passed: trusted database TLS direct/HTTP CONNECT reads and wrong-hostname/untrusted-CA/untrusted-HTTPS-proxy rejection, five cases per engine. Six actual SSH tests passed: two reads, two wrong-host-key and two wrong-identity rejections, without modifying user SSH/OS CA state. Trusted system-CA HTTPS proxy success remains unverified.
- Simulated input tests cover selection/replacement/paste and source-form behavior, not native macOS input correctness or absence of OS input-system credential leakage. Simulated UI tests do not establish native keyboard click-through, actual accessibility, scaled text, hosted CI, notarization or release/offline-Metal behavior.
- The source-slice Dalan.app was rebuilt, plist-linted, ad-hoc signed and signature-verified. The older development instance was quit and the bundle relaunched; macOS confirmed Dalan and its bundle executable. The app was left open. This is launch evidence, not manual source-flow or visual verification.
- The desktop source implements the experimental source UI; the final bundle rebuild is a separate gate. ACP remains explicitly Not connected, with no transport, process, prompt or provider configuration.

Previous icon/sorting/export results: 48 headless and 42 simulated GPUI tests, four bundle-helper tests, one generated Keychain round-trip and 22 actual direct/CONNECT/TLS/SSH cases passed. Formatting, strict lint, debug bundle build/signature and license-resource checks passed. No new manual visual/accessibility validation is claimed.

Connection-UX revision: 23 unique live cases passed (7 direct/CONNECT/authentication, 10 TLS, 6 SSH), including first login by a fresh uncached MySQL SHA2 account with TLS disabled and RSA authentication. The standalone repetition is counted once. 52 headless, 46 simulated UI and four Python tests passed; formatting, strict lint and debug bundle plist/signature checks passed; no Keychain rerun is claimed. The reported remote account remains unconfirmed: a credential-free probe reached TCP but reset before the greeting. The next evidence is a user retry with the sanitized diagnostic, not an assumption that the password or account was fixed. No new dependency or license change is introduced.

Current gate: verify all five hosted jobs on the next main push, verify system-trusted HTTPS proxy success, then exercise real macOS source-dialog/browse keyboard and error/cancel flows. Scaled text and assistive-technology validation remain release gates; any future light/system proposal needs separate validation. Preserve the historical records and [initial validation](initial-validation.md) as evidence of their own iterations, not current compatibility.

[Overview](../README.md) · [Design direction](../DESIGN.md) · [Testing](testing.md) · [Development](development.md)

## Carbonfox revision verification boundary

Theme mapping/compositing and contrast tests, compact geometry, source-form bounds and shared input colors are unit/simulated checks. Current results: 67 headless, 88 simulated UI and four Python bundle tests passed, plus formatting, strict lint, build, plist/signature and bundled-license checks. The preceding 67/83/four totals remain metadata history. The rebuilt app was reopened; window-only capture was blocked by macOS without permission changes. No native visual/accessibility pass, database/cache/SSH/password change or Keychain rerun is claimed. Hosted CI for this revision remains a separate gate.


Historical wide-grid verification passed 74 headless, 99 simulated UI and four bundle tests with formatting, strict lint and signed-bundle/resource checks. Display text is capped to 128 grapheme clusters plus ellipsis; underlying typed values and export remain unchanged. Overscan headers outside the viewport cannot receive Tab focus, and unpressed pointer moves cancel stale scrollbar drags. Current icon-led verification passed 74 headless, 106 simulated UI and four Python tests with strict lint and signed-bundle checks; see [testing](testing.md#icon-led-chrome-revision). Current hosted CI and native performance verification remain separate gates.

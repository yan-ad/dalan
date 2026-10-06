# Dalan design direction

Status: production GPUI Kit standard-control/default-theme foundation with an experimental read-only database workspace. This is not a finished SQL client, native visual approval or accessibility certification. **Dalan** means “ways” in Javanese.

## Current visual direction

The user now requires **all standard controls from GPUI Kit and Kit's default theme**. This supersedes the earlier Carbonfox - opaque runtime direction. DataGrip informs database workflows, not styling; compact database-only layout remains application-owned. Kit Button, Input, Checkbox, tabs, dropdown/popup menus and tooltips supply standard interaction rather than custom lookalikes. Driver/authentication menus and saved SSH popup choices are functional native dropdown alternatives, not claimed Combobox/Select usage.

`desktop/theme.rs` retains compact layout metrics and maps active Kit semantic `Hsla` values for shell and specialized grid paint. It does not install a fixed palette or Carbonfox override. Theme initialization belongs to Kit; a top-right Sun/Moon control invokes `Theme::change` between Kit default light/dark modes alongside ACP and Layout. No added persistence or exact initial appearance is promised. Old Carbonfox contrast numbers do not validate this theme. Native screenshots, high-DPI/text scaling, VoiceOver and performance remain separate gates.

## Supplied application artwork and historical provenance

The supplied Apple Icon Composer package is retained unchanged in `crates/app/assets/brand/Dalan.icon`. README uses the PNG fallback; macOS bundles use the generated multi-size `.icns` unless Icon Composer compilation is explicitly requested. Project/artwork redistribution terms remain unspecified; see [artwork provenance](crates/app/assets/brand/README.md).

Historical [Carbonfox JSON](crates/app/assets/themes/carbonfox-opaque.json) and [provenance](crates/app/assets/themes/README.md) may remain, but are not the runtime palette. Retain the complete MIT notices for Christian Angermann's Zed port and James Simpson's original Nightfox palette. Existing Lucide/Feather, SQLite, adapted-code and application bundle Resources notices remain required. Theme migration is not a claim that historical assets were deleted or that the project license was selected.

## Compact database workspace

One collapsible/resizable Database Explorer, retained table/query-console tabs, a separate source window and an optional disconnected ACP panel define the workspace. ACP's trigger is now top-right, not in the status bar. Generic Files, Git, build tools, terminal and plugin/toolbox chrome are excluded. ACP honestly says **Not connected**; no transport, agent launch or provider settings exist. Cached-only explorer search finds collapsed branches and preserves ancestors without database queries; searchable console database choices use a real Kit Combobox, while source-form choices remain dropdown alternatives.

App layout metrics remain 34 px titlebar, 28 px headers/status/controls, 22 px tree/grid rows, flush panes and a 4 px divider hit area with a 1 px visible line. Explorer prefers 320 px within 200–480 px, reserving 240 px main content. ACP starts closed and prefers 300 px; compact layouts may temporarily suppress explorer without changing retained preferences. Actual standard-control styling comes from Kit rather than independently copied palette/radius behavior.

Source setup is a normal independent window, initially 1040 × 760 with 780 × 560 minimum, not an OS modal sheet. A 34 px native drag strip protects traffic lights; a persistent Name/Color header sits above General, Options, SSH/SSL and Schemas Kit tabs. Color uses a compact preset/custom-hex dropdown. SSH uses saved reusable sessions only, without inline compatibility or source-side key discovery. Source/SSH menus, password fields, pickers, saving guards and retained drafts preserve their model contracts. Source colors remain marker metadata, not execution-risk or accessibility guarantees.

## Rich canvas table contract

The shipping result body remains specialized direct canvas paint with cached shaped text and row/grid quads, **zero per-cell Divs**, two-axis viewport virtualization and native interactive header controls. This is an intentional app-owned rendering exception, not a standard Table replacement. The pinned 44 px row gutter uses body y only and stays at x = 0; numbering is `offset + row + 1`. Display previews sanitize newline/control text and cap at 128 graphemes/4,096 bytes without splitting combining clusters or altering typed/export values. Native single-line shaping previously panicked on multiline data; this is not a font/OS-renderer diagnosis.

WHERE/ORDER BY are single-line drafts: typing never fetches; Apply/Enter validates and submits both. Invalid conditions retain visibly stale rows. Name-only headers expose type/key metadata in tooltips and separate sort targets; query result headers remain nonsortable. Refresh/owned Cancel, loaded CSV, paging and range feedback are functional, with no placeholder mutation/DDL/history menus.

Best-effort inactive-result retention remains **16 MiB/eight pages**, protecting active/busy/export/save results and excluding metadata/drafts/GPU/export temporary allocations. Eviction drops pages/caches, not retained tab identities/drafts/scroll. Table activation can refresh an evicted page; consoles never autoexecute. Kit DataTable is still a benchmark candidate: the 100 × 512 pilot's 328 initial/146 last-column delegate calls are not canvas parity or native FPS.

## Tabs and SQL editing

Up to 32 tabs retain independent model/view/request/result state. Tables deduplicate source/database/table identity; consoles have unique sessions and monotonic Console N names. Kit tab controls use prefix icons so labels remain visible, with 100–220 px bounds, ellipsis/full tooltips, meaningful draft/running indicators and small ghost close actions that stop propagation. Nonempty console drafts require confirmation even after successful Run; closing affects only that tab's owned work and closing an inactive tab does not activate it.

The custom SQL editing engine is replaced by a thin adapter around Kit's rope EditorState. Kit owns native selection, clipboard, IME, scrolling, undo/redo and SQL Tree-sitter highlighting. Dalan owns source/database context, validation and selected-or-whole Run/Cancel. The SQL policy remains 64 KiB; interactive limit rejection is a current regression gate, not an inherited atomicity guarantee. Do not carry forward the bespoke editor's exact undo-count/indent/geometry promises without current tests.

Database-aware completion, cursor-statement execution, persistent history, scripts and transaction/write workflows remain future scope. PostgreSQL/MongoDB/Redis labels or editor language support do not implement executors; MySQL/MariaDB remain experimental. The UI migration does not alter credential, cache, query-safety, worker-budget or ACP-only contracts.

## Evidence boundary

Historical theme/layout test counts and pilot results describe their own revisions only. Production adoption needs current format/headless/UI/lint/bundle checks and native font/shader/window, menu focus/dismissal, password extraction, Unicode/IME, picker and close/save checks. No screenshot, VoiceOver, native performance or live database/Keychain rerun is claimed by this documentation update.

[Overview](README.md) · [Migration](docs/gpui-kit-migration.md) · [UI foundation](docs/ui-foundation.md) · [Source management](docs/source-management.md) · [Table browser](docs/table-browser.md) · [Testing](docs/testing.md)

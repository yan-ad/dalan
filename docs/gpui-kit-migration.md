# GPUI Kit migration

Status: **production component migration implemented; validation gates remain open**. The root workspace aliases `gpui` to `gpui-kit = 0.7.1` with `tree-sitter-sql`. Its coordinated `gpui-pre = 0.3.8` core/platform family replaces the old GPUI 0.2 dependency; production controls share one UI type family. Kit ships from the main workspace; the earlier experiment and its separate CI job have been removed.

## Production ownership

- Bootstrap uses `gpui::application`, asset registration and `gpui::init`. Kit `open_window` installs Base Root for every main, source, About and SSH window, providing the component overlay lifecycle.
- All standard controls use Kit: Button, Input, Checkbox, Tab/TabBar, popup menus, dropdowns and tooltips. Driver/authentication choices use Kit dropdown menus; saved SSH choices use a Kit popup list. These are functional native dropdown alternatives, **not a claim that Kit Combobox or Select is used**.
- The default theme is Kit-owned. `desktop/theme.rs` maps active Kit semantic `Hsla` colors for app-owned layout/grid paint, rather than installing fixed app colors or a Carbonfox override. Do not promise a particular initial light/dark mode beyond Kit initialization behavior.
- Application artwork/provider assets and existing provenance remain separate. Historical Carbonfox JSON and MIT notices may remain as reference material; they are not the runtime palette.
- GPUI-independent models, credentials, cache, typed results, request cancellation, worker limits and read-only execution policy remain application-owned and unchanged.

## Input and editor adapters

`desktop/input.rs` is a thin window-less construction/value-cache adapter around Kit `InputState`, not a second editing engine. Kit owns rendering, selection, editing, IME and clipboard actions. Password fields use Kit masking/copy-cut protection plus a native `PasswordInputHandler` that suppresses OS surrounding-text extraction while delegating edits, composition, selection and geometry to Kit. Plain fields retain Kit's normal native surrounding-text behavior. Read-only/plain presentation uses Kit controls and existing callbacks.

`desktop/sql_editor.rs` replaces the roughly 1,131-line bespoke editor with a small adapter (roughly 140 lines before additional guards/tests) around Kit's rope-backed `EditorState`. Kit owns selection, undo/redo, IME, scrolling and SQL Tree-sitter highlighting. Dalan retains context-free draft/selection access, programmatic loads, validation and Run/Cancel ownership. SQL grammar is explicitly enabled; JSON also appears in the dependency graph, not an all-language bundle.

The application SQL policy remains 64 KiB. Programmatic oversized loads are rejected without replacing the draft. Oversized interactive paste/IME edits restore the previously accepted draft through Kit at the next render and report validation; this exceptional rollback clears undo history, because Kit 0.7.1 has no Editor validator hook. Transient clipboard/editor allocations are not a hard memory bound. Normal undo, selection and highlighting remain Kit-owned. Database-aware completion, cursor-statement execution, persistent history and scripts are still pending application features; grammar/editor hooks alone do not implement them.

## Specialized canvas exception

The two-axis canvas result grid is essential app-owned specialized paint, not a standard control awaiting cosmetic replacement. It retains exact typed values, viewport shaping, pinned gutter and best-effort inactive-result retention of **16 MiB/eight pages**, excluding metadata/drafts/GPU/export temporary memory. Worker/admission limits are unchanged.

Kit DataTable remains a benchmark candidate, not the shipping result renderer. Stateless Table is not the wide-result solution. The removed isolated pilot historically passed four tests with a 100 × 512 fixture and positive last-column bounds. Delegate callbacks were **328 initially and 146 at column 511** at 1040 × 760: render/measurement counts, not unique cells, elapsed time, FPS or parity with the canvas. These are archived proof-of-concept results, not a runnable local target. Compare memory, fixed columns, long values, NULL/binary/decimal fidelity, selection/copy and resizing before replacement. Validate the shipping workspace using [development commands](development.md) and [test gates](testing.md).

## Validation and remaining work

Production migration passes current regression gates and native startup, but is not a native visual, accessibility or performance certification. Run current formatting, headless/UI tests, strict Clippy and signed bundle checks against the coordinated production graph. Exercise Root/overlay lifecycle in all windows, native fonts/shaders, Tab/Shift-Tab, dropdown targeting/dismissal/focus return, Unicode/IME, password extraction/clipboard privacy, native pickers and saving/cancellation guards. Record owner-run outcomes separately; the passing pilot and historical GPUI 0.2 test counts are not current production evidence.

The blank-results defect was fixed independently: the result wrapper is a bounded column flex container. Historical actual QueryConsole hierarchy tests used 100 × 128 results at 1040 × 760 and 780 × 560 with positive body bounds, scrolling and retained busy/error states. Revalidate that hierarchy after component migration rather than treating upstream widgets as proof.

MySQL/MariaDB remain experimental. PostgreSQL, MongoDB and Redis require engine-specific executors and conformance; local enum/catalog choices must reject the wrong executor rather than silently route to MySQL. No history persistence, plugin runtime, live ACP transport, provider keys or autonomous execution is added. ACP remains the only application AI transport.

## Licensing and sources

Kit software/examples are Apache-2.0; eligible upstream documentation prose/illustrations are separately CC BY 4.0. This is an original source-audit summary, not copied documentation. Retain existing app/bundle Resources notices, Lucide/Feather attribution, adapted-code provenance and complete historical Carbonfox MIT texts; adoption does not select Dalan's project/artwork license or require deleting historical assets.

- [Getting started](https://gpui-kit.com/docs/getting-started)
- [Editor](https://gpui-kit.com/component/editor)
- [DataTable](https://gpui-kit.com/component/data-table)
- [Theme](https://gpui-kit.com/component/theme)
- [Assets](https://gpui-kit.com/docs/assets)
- [Published Kit API](https://docs.rs/gpui-kit/0.7.1/gpui_kit/)

[Roadmap](../ROADMAP.md) · [ADR 0006](adr/0006-gpui-kit-migration.md) · [Test evidence](testing.md)


Historical migration verification: 121 unit tests plus one native-wire test, 160 production UI tests, seven Python bundle tests, four tests in the now-removed isolated pilot and 29 live MySQL/MariaDB transport cases passed. Formatting, strict Clippy, production dependency-tree checks and signed-bundle checks passed at that revision. Only the coordinated GPUI pre-release 0.3.8 family appears in the shipping graph; GPUI 0.2.2 is absent. The migrated app launched; launch is not pixel/interaction certification. Current development-fix local validation passed **121 headless unit tests** (4 ACP, 54 app, 5 core, 58 drivers), **one native-wire integration test**, **174 simulated production UI tests**, and **14 Python tests** (seven bundle + seven preview). `cargo fmt`, strict Clippy for both the workspace and desktop/UI with all targets, signed debug bundle checks, plist lint and signature verification passed. Bacon 3.26.0 is installed; `bacon --list-jobs` successfully parsed the default preview job. Actual native preview lifecycle smoke is not claimed here. Hosted CI is explicitly skipped for this task: do not trigger, poll or infer a new hosted result.

## Main-workspace interaction fixes

The titlebar groups ACP, a Kit-default light/dark toggle and Layout at the top right; ACP no longer lives in the status bar and still reports **Not connected**. The theme control uses Kit's registered default Sun/Moon assets and `Theme::change`, without a custom palette or added persistence contract. Tab icons use `Tab.prefix` rather than `Tab.icon`, which suppresses the label in this Kit version: table names and Console N remain visible within 100–220 px widths, with ellipsis/full tooltips. Small ghost close buttons stop propagation; closing an inactive tab does not activate it and nonempty-draft confirmation is unchanged.

The console database picker is an actual searchable Kit `ComboboxState<SearchableVec<DbChoice>>`, not the source form's dropdown alternative. Optional-string values distinguish None from a database literally named “No default database”. Its virtual list supports the 1,000-name fixture; stable catalog state survives busy/result notifications, and guarded changes restore selection while running. Cached-only explorer search uses a thin Kit Input wrapper, preserves ancestors and finds collapsed branches; it does not fetch or change database/query state. Its lazy index covers the 50,000-item fixture without flattening the full catalog on scroll.

SQL regressions exercise the actual rendered Kit editor and native UTF-16 selection interface. Run immediately returns focus to the editor, enabling subsequent typing; completion does not refocus and steal a deliberately focused result. Canvas display previews defensively sanitize multiline/control text before native single-line shaping; see [root cause and regression boundary](testing.md#native-multiline-result-crash-and-development-fixes).

Tokio completion is handed back through a scheduler-owned polling bridge, awaiting only completed handles rather than waking deterministic/local GPUI tasks from Tokio worker threads. Abort and generation checks remain. The existing production bridge uses 10 ms executor timers; test harnesses advance scheduler-owned clocks explicitly; no production polling or database deadline was changed by this UI fix. These scheduling intervals are not database deadlines or frame-performance measurements.

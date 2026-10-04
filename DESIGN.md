# Dalan design direction

Status: proposal derived from explicit user direction, not a finished interface, implemented design system, or completed brand identity. The user selected the name `Dalan`, meaning “ways” in Javanese. The name is presented as text only; no logo, brand assets, or settled typography license is invented here.

## Source of direction

The user asked for a Rust + GPUI native tool with a Zed-like look and DataGrip-like layout/UX. Interpret this as restrained editor chrome, compact density, keyboard-first navigation, and a recognizable database IDE workspace. Do not copy Zed assets/source or imply affiliation. The user explicitly permitted an open-source main icon set. Fourteen pinned Lucide utility SVGs have independent provenance and license notices; other future third-party code or assets still require review.

Reason: references establish qualities to pursue, not permission to reproduce another product.

Source setup lives in a dedicated normal GPUI window, not an OS modal sheet: 1040 × 760 initially with a 780 × 560 minimum. The main window remains usable; Add, Manage and the centered connection action reuse one application-wide source window and retain its draft. No DataGrip advanced driver tabs or redundant form header are added. Host and Port share a horizontal row for each endpoint; the bounded, nonshrinking form body scrolls rather than compressing controls. Source setup begins directly with its Engine selector rather than repeating a Data Source title and selected engine in a separate header. The password row places a visible labeled Keychain checkbox on the right; the CA row pairs editable path text with a native Browse action. Both use the existing compact control sizes and keyboard focus treatment, not text-glyph approximations of controls.

## Adopted shell foundation

The user subsequently supplied a dark DataGrip screenshot and requested its sidebar/top-bar feel, explicitly excluding main content. The implemented [shell foundation](docs/ui-foundation.md) uses graphite chrome, inset dark panes, native macOS traffic lights, one collapsible/resizable Database Explorer sidebar without an activity rail, a main area used for database browsing and a read-only table view, with source setup in a separate dedicated dialog window, and an explicitly permitted optional database-focused ACP panel. Its bottom-right 28 px bot-message-square icon trigger, labeled AI · ACP by tooltip and focus help, opens an honest disconnected placeholder, not generic tool chrome or a working agent session. It does not copy the welcome screen, gradients, sample data sources, vendor assets, or unimplemented toolbar actions. Energy/rhythm/motion remain 1/1/1.

The shell uses a 320 px preferred database sidebar width, bounded to 200–480 px, a 6 px divider gap and 6 px outer padding on both sides, with no reserved rail width, reserving at least 240 px for content. The ACP panel is initially closed and prefers 300 px, capped by available space. Compact layout may temporarily hide Database Explorer while ACP is visible; database visibility/width preferences are retained and restored after closing it. Layout has four rows: toggle Database Explorer, narrow, widen, and reset. macOS shell shortcuts are Cmd-B, Cmd-Alt-0, and Cmd-Shift-A for ACP, not Ctrl variants.

Generic Files explorer, code viewer, Git UI, build/run integrations, generic terminal, and plugin/toolbox chrome are excluded by user direction. Database consoles, SQL scripts, and database-focused import/export remain in scope.

The initial blank-main shell is historical. Experimental MySQL/MariaDB browse controls now occupy that area; source setup uses a separate resizable Data Sources · Dalan dialog window; see [source scope](docs/mysql-sources.md). They do not copy reference advanced options, marketplace or visual tools. The input control adapts GPUI Apache-2.0 code with attribution; this does not select a project license.

The main utility set is Lucide at revision `500620a2e8123f8d1db191538886dc0c223f69a9`: database, database-zap, plus, settings-2, refresh-cw, trash-2, panel-left, chevron-down, minus, bot-message-square, arrow-up, arrow-down, download, check, chevron-right, folder, table, list-tree and chevrons-down-up. Local trash-2.svg contains upstream trash.svg from that same pin; see the [asset provenance](crates/app/assets/README.md). The bot/message glyph communicates agent conversation instead of decorative sparkle; arrow direction is explicit. It is not a Dalan app or brand icon. SVGs are embedded, with no runtime asset download or icon font. Complete ISC and retained Feather MIT notices and adapted GPUI input Apache-2.0 attribution are in [third-party notices](THIRD_PARTY_NOTICES.md) and bundled resources. Calm, dense 13 pt UI typography remains the direction; no font asset is added.

Dark shell tokens and control behavior are recorded in the foundation document. This iteration ships only the reference's dark direction, without a nonfunctional theme toggle. Light/system variants, scaled text, and actual assistive-technology validation remain first-release requirements.

Database Explorer now has only a 32 px toolbar with six 28 px icon actions: Add, Manage, Refresh, Remove, Expand Loaded and Collapse All. The Database Explorer title header is removed, as are the activity rail and header hide/minimise controls. The bottom-left 28 px panel-left toggle is unchanged; Cmd-B and native View/Layout alternatives remain. Closing the explorer retains that preference until toggled or reset. At 720 px, the explorer's maximum effective width is 462 px while main content retains 240 px; compact ACP suppression still preserves the preference.

Source rows show name and engine label with abstract Lucide database/MySQL and database-zap/MariaDB cues, not branded driver icons. Optional color marks a small indicator only, never the name or engine text. Below Name, labeled swatches and manual `#RRGGBB` entry provide Default, Blue, Green, Amber, Red and Purple choices. Custom colors are not guaranteed WCAG AA and have no assigned production or risk role. With no sources, a centered Connect to a Source action opens the real form by mouse or Enter/Space and remains available with the explorer hidden. No demo/trial welcome or inactive DDL, console or advanced-option icons are introduced.

## Compact lazy explorer

The database hierarchy is a single-focus, virtual uniform list of 22 px rows, not a stack of per-node buttons. Explicit constrained text leaves names nonblank and ellipsized with full-name tooltips. Source rows combine abstract driver icon, optional color marker, name, engine label and loaded-only count; database rows have disclosure/icon cues; Tables/Views folder counts appear only once metadata is known. Table-icon view leaves are visibly unavailable, not working queries. The old layout accompanied reported blank buttons, but no measured font/GPU root cause is established.

Use Up/Down/Home/End to move focus; Right expands or enters a branch, Left collapses or goes to its parent. Enter/Space toggles branches; Enter browses an available table leaf. Toolbar Enter/Space acts on the focused control; tree-specific keys are focus-guarded, and chevrons stop propagation. Expand Loaded expands cached branches only; Collapse All retains cache. Refresh reloads the explorer-selected source root, not a different source's table page. The original DataGrip reference informs chrome only: it does not authorize a generic code viewer or extra tool panels.

See [UI foundation](docs/ui-foundation.md#compact-lazy-explorer) and [structural verification](docs/testing.md#compact-lazy-explorer).

## Proposed visual principles

| Decision | Direction | Reason |
| --- | --- | --- |
| Workspace | One Database Explorer sidebar without an activity rail; planned central database consoles/table views, bottom results/output/sessions, optional database-focused ACP panel | Matches the requested daily IDE organization. |
| Density | Compact rows, restrained padding, resizable panes, virtualized large views, adjustable text size | Database work benefits from visible context without sacrificing legibility. |
| Hierarchy | Connection identity first; content next; chrome quiet but discoverable | Users need to know where an operation will run. |
| Theme | Planned dark default; light and system modes also planned, with both dark and light tested before first release | A dark default suits the requested developer-tool direction without excluding light-mode users. |
| Type | System sans for UI, system/candidate licensed monospace for SQL and data; tabular numeric alignment where available | Native legibility and stable data columns matter more than decorative type. |
| Color | Neutral surfaces, one restrained accent, semantic statuses with text/icons | Data and risk should be clear without ornamental color. |
| Motion | Minimal, short focus/pane transitions, respect reduced-motion preference; no decorative animation | Motion should clarify changes without distracting from active operations. |
| Branding | `Dalan` as text only | The name is confirmed; visual identity remains open. |

Theme choice is a proposal, not a user requirement. Both themes must cover editor, tree, grid, dialogs, status, focus, selection, read-only, stale, error, and destructive review states. System mode follows the OS preference using tested variants, not an untested third palette.

## Proposed design dials

- **Energy 1:** quiet tool surfaces, no promotional gradients, large hero areas, or attention-seeking ornament.
- **Rhythm 1:** intentionally dense, regular rows and predictable pane geometry rather than dramatic spacing changes.
- **Motion 1:** minimal functional transitions; operation progress is honest, not decorative.

These are qualitative direction dials, not measured user outcomes or framework capabilities. They do not override accessibility, readability, or explicit execution safety.

Reason: a daily database IDE should support sustained work rather than compete for attention.

## Candidate neutral palette

The following values are proposed exploration tokens only. No contrast or accessibility compliance is claimed. Validate every foreground/background pairing, state, and text size against WCAG AA before adopting them; adjust or replace values that fail.

| Token | Dark candidate | Light candidate | Intended role |
| --- | --- | --- | --- |
| Workspace | `#181A1D` | `#F5F6F7` | Outer workspace |
| Surface | `#202328` | `#FFFFFF` | Editor, grid, panels |
| Raised surface | `#292D33` | `#E9ECF0` | Menus and dialogs |
| Primary text | `#E6E8EB` | `#20242A` | Main labels and data |
| Secondary text | `#AEB5BF` | `#505966` | Supplementary labels, subject to contrast validation |
| Border | `#424A55` | `#B7C0CA` | Dividers, not assumed sufficient for focus |
| Accent/focus candidate | `#8AB4F8` | `#2457A7` | Focus and selected control hints |

Semantic error, warning, success, read-only, selection, and production-environment tokens must be designed and tested separately. Never use color as the only signifier. Selected text, syntax highlighting, disabled labels, and low-emphasis metadata need explicit contrast treatment, not unchecked opacity reductions.

Reason: candidate tokens make discussion concrete without presenting untested colors as accessible final design.

## Proposed component behavior

- Tree rows use engine-appropriate labels and loading/error/stale states; expansion does not run database writes.
- Tabs distinguish unsaved text, staged data edits, active transactions, and running operations with separate labels or markers.
- Results use stable column headers, readable null/binary/truncated representations, logical keyboard cell focus, and visible loaded/export scope. Empty results differ from not-yet-executed views.
- SQL and Redis share chrome but retain their own controls. Redis value viewers show type, TTL, bounds, and binary identity rather than imitation SQL table semantics.
- Destructive reviews name connection, environment, object/key, and action. Unknown outcomes stay visible until reconciled, not hidden by a toast.
- The ACP panel emphasizes review/insertion and context permission. It does not contain app provider selectors, app API-key fields, or autonomous database execution controls.

Reason: visual consistency must not erase semantic differences or safety information.

## Validation before design commitment

1. Prototype actual GPUI layout, text rendering, virtualized views, keyboard focus, resizing, and native macOS integration; no finished UI is promised by these notes.
2. Test both dark and light themes for WCAG AA contrast and non-color state recognition, including focus and selection.
3. Audit actual platform accessibility exposure and assistive-technology behavior. GPUI pre-1.0 is a risk to investigate, not a basis for fabricated screen-reader support claims.
4. Run keyboard-only workflows and scaled-text checks. Compact defaults must not obscure controls or operation scope.
5. Measure on declared hardware/data fixtures against [proposed performance targets](docs/product-plan.md#proposed-release-gates); report results only after tests exist.
6. Record adopted tokens, parser/completion choices, GPUI pin/upgrade policy, and unresolved constraints in [ADRs](docs/adr/README.md).

Reason: a useful direction becomes a design system only after real implementation and validation.

[Overview](README.md) · [Product plan](docs/product-plan.md) · [Roadmap](docs/roadmap.md) · [UX](docs/ux.md) · [Architecture](docs/architecture.md) · [Drivers](docs/drivers.md) · [ACP](docs/acp.md) · [Security](docs/security.md) · [Development](docs/development.md) · [Testing](docs/testing.md) · [ADRs](docs/adr/README.md)

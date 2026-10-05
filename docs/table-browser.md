# Read-only table browser

Status: implemented experimental MySQL/MariaDB table UX, informed by DataGrip workflows while retaining compact **Carbonfox - opaque** styling. This is not a DataGrip visual replica or a completed SQL client. See [source setup](mysql-sources.md), [architecture](architecture.md) and [verification](testing.md#rich-canvas-table-browser).

## Titles, columns and row numbers

The table title keeps the actual source/database/table context, ellipsized with a full tooltip. Workspace tab labels likewise keep essential names and ellipsize rather than expanding chrome. Sorting belongs to column headers, not a duplicate table-title control. Explorer roles, source profiles and connection behavior remain unchanged.

Headers show a type/key glyph and compact ellipsized **column name only**, not a long colored type suffix. Full name/type, nullable/key metadata and contextual sorting help are in tooltips. Primary keys use `key-round`; integer/numeric columns use `hash`, text uses `text-initial`, dates use `calendar-clock`, binary uses a binary glyph, JSON uses braces, booleans use check, and unknown types fall back to text. Each sortable header has an explicit ascending/descending/default `arrow-down-up` glyph with its own click target; propagation is stopped so one click cycles once. Header click and focused Enter/Space retain ascending → descending → default sorting. Query-console headers do not offer sorting.

A **44 px pinned row-number gutter** stays at x = 0 when columns scroll horizontally. Labels are `page.offset + row + 1`: offset 100 starts at 101. Gutter and body share the same vertical offset, so row numbers stay aligned. Rows are 22 px, fixed-width columns 180 px, pinned headers 28 px and browse text 12 px.

## WHERE and ORDER BY

The toolbar has two editable single-line inputs: **WHERE** with `list-filter`, and **ORDER BY** with `arrow-down-up`. Enter applies both conditions when focus is in a table-browser input; the native binding predicate is `TableBrowser > DalanInput`. Typing changes drafts only: it does not fetch, read credentials or execute SQL. Each tab retains its own drafts and applied conditions.

Apply/check, Clear/minus, Refresh, owned Cancel/stop, Export/download, Previous/Next and the loaded-range footer remain working guarded controls. Clear is disabled while busy; cancel the owned request first. Apply validates against the current page's column metadata **before credential access or network admission**. Malformed input produces a static sanitized error and keeps the previous page, visibly stale rather than clearing valid rows or calling them fresh. Pagination/export retain their freshness and completeness guards.

Enter fragments without the leading keywords, for example:

```sql
-- WHERE input (comments here are documentation, not accepted input):
status = 'ready' AND id BETWEEN 10 AND 100
-- ORDER BY input:
id DESC
```

The clause compiler uses pinned **sqlparser 0.62.0 / MySqlDialect**. WHERE accepts metadata-validated unqualified identifiers (including backticks), literal comparisons `=`, `!=`, `<>`, `<`, `<=`, `>`, `>=`, AND/OR/NOT, LIKE, BETWEEN, literal IN lists and IS NULL/IS NOT NULL. IN lists are limited to 256 literals. Quoted literal contents are data, including punctuation that would be rejected outside a literal. Values are native bound parameters: signed/unsigned integers, decimal bytes and text remain exact without floating-point rounding.

Rejected constructs include functions, subqueries, qualified identifiers, statements/semicolons, comments, extra SQL clauses and unsupported expressions. Each fragment is bounded to **16 KiB, 1,024 meaningful tokens, 24 parenthesis/CASE nesting levels and 128 operators/recursive constructs**. ORDER BY accepts at most **eight unique actual columns**, optional ASC/DESC; ordinals, expressions, functions and NULLS directives are rejected. Unknown columns are rejected, not passed through to the server.

The driver compiles validated fragments into a fixed generated SELECT and parameterizes values and LIMIT/OFFSET; it never executes a user-supplied full statement from these fields. Syntax/budget validation happens before connecting even through the direct backend API. Actual column membership requires server metadata for direct callers; the production UI already has the current page metadata and validates before credential resolution. Empty clauses default to primary-key ordering where available. Explicit ORDER BY appends ascending primary-key tie-breakers not already included. Header cycles update ORDER BY while preserving WHERE; the third step removes that column's explicit ordering. Stable order where possible is **not snapshot consistency**: concurrent mutation can shift offset pages.

Legacy typed filter/sort requests remain accepted by the backend for compatibility only; the production toolbar no longer exposes the old column/operator/value filter controls. Console SQL remains a separate, differently bounded SELECT policy; table fragments do not inherit console function/subquery support.

## Production canvas and bounded caches

`DataGrid` is a retained entity sharing immutable `Arc<TablePage>` with the model and export. The production body has **no per-cell Div elements**. Canvas painting uses `Window::paint_quad` / `PaintQuad` for row backgrounds and grid lines (one per visible line), then paints cached `ShapedLine` text directly for viewport cells. Native interactive header Div controls are retained rather than replacing focus/click behavior with decorative paint.

Display previews are capped at 128 Unicode grapheme clusters plus ellipsis and fitted to each cell with grapheme-safe ellipsis; typed values and loaded CSV remain unchanged. Display strings, shaped cell lines, row-number lines and headers are bounded to current row/column ranges with two-cell overscan. Snapshot replacement/eviction invalidates caches. Tiny wheel movement within unchanged ranges reuses shaping. Statistics distinguish cached/materialized cells, last-painted cells and last-shaped cells; they are not FPS or process-memory measurements. Normal text/type/Unicode regressions do not clone or format the full dataset on redraw.

Wheel/trackpad, Shift-wheel, grid-focused viewport keys and two draggable/clickable scrollbar tracks still work. The gutter shares only vertical scrolling; headers share horizontal scrolling. Visible headers retain guarded Tab/Enter/Space activation. Active-cell selection, resizing and cell inspection/copy are not implemented.

## Workspace result retention

The shared workspace uses an **estimated 16 MiB owned-result allocation budget and at most eight loaded pages**, evicting least-recently-used inactive results, **not tabs**. The 32-tab cap is separate. SQL/WHERE/ORDER BY drafts, applied clauses, tab identity and scroll state survive result eviction; retained views release display/shaping caches when the model page becomes None. Duplicate table opening still activates the same stable tab identity, including invalidated cached-source tabs.

Active, busy and export/save-protected results may exceed these thresholds: retention is explicitly **best effort**, not a hard process-memory ceiling. The estimate is computed once per Arc identity using a pointer token, without extra budget-owned Arc references or repeated row walks on cursor/small model notifications. Source/catalog metadata, drafts, GPU/shaping caches, runtime/decoder memory and temporary export Arc/page-vector allocations are outside this budget.

Activating an evicted table asynchronously refreshes its applied conditions. An evicted query console **never auto-runs SQL**: it preserves the draft and previous submitted-query provenance, labels the result unavailable and asks the user to rerun. Neither rows nor SQL/drafts/tabs are persisted in SQLite; the existing metadata cache remains names/kinds only.

Backend bounds remain 200 rows, 512 columns, 4 KiB cells, 2 MiB retained page data and an actual 8 MiB per-packet limit. Canvas text and LRU retention do not magically bound all memory or server work. Loaded CSV still exports the full fresh complete loaded page, not only visible cells and not the whole table.

## Evidence and remaining gates

The narrow simulated **730 × 258**, **512-column × 200-row** fixture reports **40 painted cells**, **40 cached shaped lines**, **six header controls**, zero production cell elements and **zero newly shaped cells** after a subsequent tiny wheel movement. The integrated **1,000-database / 100 × 512** fixture at **1280 × 720** has a **906 × 570 body**, **300/51,200 materialized cells**, **32 sidebar rows** and **zero explorer projection rebuilds**. Materialized/cached counts are not interchangeable with painted-cell counts; no integrated native FPS is measured. The old 310-cell virtual-element fixture is historical, not the current canvas count.

Current gates passed **114 headless unit tests plus one separate native-wire test**, **154 simulated UI tests**, **four Python bundle tests**, and **29 unique live cases** (11 direct/URL/socket/CONNECT/authentication, 12 TLS, six SSH), formatting, strict lint and signed debug bundle checks. No repeated fixture assertions or wire tests are double-counted. Historical `4e389d0` passed all five jobs in [run 37266563119](https://github.com/yan-ad/dalan/actions/runs/37266563119); current hosted CI remains a separate gate.

Native capture permissions previously blocked screenshots. Native visual/IME/VoiceOver/scaled-text, real high-DPI/GPU performance and user retry on the latest rebuilt macOS app remain required; simulated bounds/counts are not those claims. Use owned disposable loopback fixtures, never private DataGrip credentials, IPs, usernames or row data. No permissions are changed for capture. No mutation tools, placeholder history/DDL/transaction menus or app brand icon are introduced. The 36 Lucide assets (28 existing plus eight) keep the same pin and full ISC/Feather notices; see [asset provenance](../crates/app/assets/README.md).

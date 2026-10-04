# Dalan UX proposal

Status: product UX proposal with an implemented [UI shell foundation](ui-foundation.md). Top bar, native traffic lights, layout controls, one database rail, one collapsible/resizable Database Explorer sidebar, status strip with AI · ACP toggle, disconnected optional ACP panel, and About Dalan window exist; main content and database/ACP interactions remain unimplemented. The compact keyboard-first IDE, Zed-like visual restraint, DataGrip-like layout, and optional ACP panel are explicit user direction. Product workflows below remain proposals to test.

## Current shell

Main content is blank, with an initially closed optional database-focused ACP right panel and no Files rail. Database Explorer defaults to 320 px with 200–480 px bounds. The rail is 36 px, the divider gap is 6 px, and right padding is 6 px; content retains at least 240 px. ACP prefers 300 px capped by available space. Compact layout retains at least 240 px for main content and may temporarily hide Database Explorer while ACP is visible, preserving visibility/width preferences for restoration after close. Layout contains exactly four rows: toggle Database Explorer, narrow, widen, and reset.

AI · ACP in the 32 px status strip toggles the panel, as does Cmd-Shift-A. The panel says **Not connected**: no ACP transport or agent launch exists, and there is no text prompt input or BYOK/provider settings. Its close button returns focus to the trigger; Escape does so only when focus is inside the panel. The native About Dalan menu opens a separate 420 × 280 nonresizable GPUI window with Cargo version, “ways” in Javanese, and database-workspace scope; no new libraries or brand/icon artwork.

Generic Files browsing, code viewing, Git, build/run integrations, generic terminals, and plugin/toolbox chrome are explicitly excluded. Query consoles, database SQL scripts, and database-focused import/export are valid database workflows.

## Proposed database workspace anatomy

- **Left:** data-source tree with connection, environment label, engine, databases/schemas, and engine-appropriate objects. Redis shows logical databases/keys, not SQL tables.
- **Center:** tabbed SQL/Redis consoles and table or value views. Each tab retains visible connection and session identity. Dirty drafts and pinned transactions have distinct markers and text alternatives.
- **Bottom:** results, output, and sessions. Results include operation identity, elapsed state, loaded/export scope, and truncation; output records errors and uncertain outcomes; sessions expose transaction state without requiring users to infer it from a console tab.
- **Right, optional:** ACP transcript, agent state, context permissions, and suggestion review. Closing the panel does not imply terminating the external agent; lifecycle controls make that choice explicit.
- **Status area:** active connection/environment, transaction/read-only status, running jobs, and agent state. Never communicate production or write risk through color alone.

Reason: familiar spatial organization reduces navigation cost while retaining the identity of every database action.

## Proposed primary journeys

### Connect and browse

The first workspace offers Add connection, then New query console after selecting a connection, without demonstration databases presented as real connections. Connection setup names the engine, host, authentication, TLS verification, and optional credential persistence. Test connection gives actionable errors without logging secrets. Keychain failure offers retry or explicit session-only use, never plaintext fallback. Tree refresh preserves selection where possible and marks stale objects after reconnect.

Reason: connection setup is a security decision as well as navigation.

### Execute and inspect SQL

A console shows connection and execution scope before running. Run selection takes precedence only when text is actually selected; otherwise Run current statement uses validated dialect parsing. Run script is a distinct action. Results remain tied to their submitted text/session even if the editor changes. Paging displays loaded rows and ordering; it does not invent total counts or snapshot consistency. Export review distinguishes loaded rows from a separately requested full-query export.

Explicit transactions use pinned sessions. Begin/Commit/Rollback controls show their effect and the relevant session; DDL and storage-engine limitations appear where applicable. A tab close with an active transaction requires a resolution choice and cannot silently commit. Disconnect uncertainty remains visible after reconnect.

Reason: execution scope and transaction ownership must never be inferred from visual proximity alone.

### Review table edits

Eligible primary-key rows support local staged edits, with clear original/new values and per-row change markers. Review identifies connection, table, keys, affected columns, and the apply strategy. No eligible primary key means read-only, with an explanation. Apply is separate from Run query; unexpected affected-row counts or concurrent modifications enter a conflict state. Failed/uncertain writes retain the draft and operation record without implying it is safe to reapply.

Reason: editing a cell must not silently mutate the server.

### Browse and operate on Redis

Incremental SCAN shows cursor/loading state, deduplicates visible keys where practical, and explains that concurrent changes can affect results. Readable escaped key labels are not the stored key identity; binary bytes round-trip through inspection and operations. Type-specific viewers bound bytes/items for strings, hashes, lists, sets, sorted sets, and streams. TTL shows persistent, expires, or missing separately, plus observation time.

Write/delete/TTL review names the selected connection/database and exact key with a safe byte representation. Missing or changed type triggers refresh/conflict handling. The command console documents its restricted command policy; blocked commands fail before dispatch. SQL transaction controls do not appear for Redis.

Reason: familiar browsing should not conceal binary identity, expiry, or Redis command risk.

### Use ACP suggestions

Agent setup explains external auth/billing and independent host privileges. Metadata sharing starts off and is scoped by explicit opt-in. Rows never enter context automatically. A suggestion offers Review, Copy, and Insert; Insert modifies a draft only. The normal user execution action remains necessary. Agent unavailability does not block manual database workflows. No direct provider or application API-key settings exist.

Reason: the panel is assistance, not autonomous control or a sandbox.

## Proposed state matrix

| State | Visible treatment | Available actions / rules |
| --- | --- | --- |
| Empty workspace | Clear setup action and no fictitious data | Add connection; after selecting a connection, New query console; command palette |
| Empty result/key range | Explain zero returned items versus nothing fetched | Refresh or modify query/filter; retain scope |
| Loading/connecting | Named operation, progress where real, otherwise indeterminate indicator | Cancel if supported; do not fabricate percentage |
| Running | Connection/session, submitted operation, bounded output | Cancel request is separate from editor Escape |
| Error | Engine/context, safe diagnostic, failed stage | Retry only when safe; show prior successful script statements |
| Cancel requested | Pending cancellation and original operation remain visible | Do not imply immediate termination or rollback |
| Cancelled, confirmed | State exactly what is known, preserve partial results where valid | Explicit rerun; no automatic replay |
| Unknown outcome | Persistent warning with operation/session identity | Inspect/reconcile; do not claim rollback or auto-retry |
| Stale | Timestamp/reason and stale marker | Refresh explicitly; preserve drafts for review |
| Disconnected | Lost connection and transaction/session consequences | Reconnect creates a new session, not recovered transaction state |
| Read-only | Text/icon state with reason | Read/export; editing controls disabled with explanation |
| Unsaved script | Dirty marker distinct from running/transaction state | Save, discard, or cancel close |
| Staged data edits | Count and changed cells; not yet applied | Review/apply/discard; explicit close choice |
| Edit conflict | Original/draft/current context when available | Refresh and reconcile; never overwrite silently |
| Truncated/bounded | Actual fetched limits and representation | Explicit bounded fetch-more; no false complete-data label |
| Active transaction | Pinned session and transaction state | Explicit commit/rollback; guard disconnect/close |
| AI unavailable/denied | Agent status and sharing state | Reconnect/change permission; manual work unaffected |

Reason: an accurate failure state is more important than a generic success/error toast.

## Proposed macOS shortcuts

Except for the implemented shell bindings noted below, these bindings are candidates, subject to conflict testing with native menus, text editing, and accessibility. Shell shortcuts use macOS Command, not Control. Menus show discoverable shortcuts. Linux/Windows mapping follows platform conventions later.

| Shortcut | Proposed action |
| --- | --- |
| Cmd+Shift+P | Command palette |
| Cmd+P | Quick-open saved script or database object |
| Cmd+N | New console for explicitly selected connection |
| Cmd+O / Cmd+S | Open / save local script |
| Cmd+Return | Run selected SQL, otherwise current validated statement; submit reviewed Redis console input |
| Cmd+Shift+Return | Review and run SQL script, not apply grid edits |
| Cmd+. | Request cancellation of the explicitly identified focused running operation |
| Cmd+W | Close current tab with draft/transaction guards |
| Cmd+B | Toggle Database Explorer, implemented shell binding |
| Cmd+Option+0 | Reset database layout, implemented shell binding |
| Cmd+J | Toggle bottom pane |
| Cmd+Shift+A | Toggle AI panel (ACP only) |
| Ctrl+Tab / Ctrl+Shift+Tab | Next / previous workspace tab, pending platform audit |
| Escape | Close transient menu, leave cell edit by discarding its local uncommitted edit, or dismiss search; never execute, discard all staged edits, or cancel a server query |

Global shortcuts do not override text-field editing unexpectedly. Apply edits, commit, rollback, destructive actions, and export remain named commands and buttons; any eventual key bindings must be separately reviewed. Focus transitions are deterministic: opening a pane focuses its primary control, closing it restores the originating control, dialogs return focus to their invoker, and virtualization preserves logical focus.

Reason: common IDE bindings help expert use, but Escape and destructive actions need unambiguous behavior.

## Proposed accessibility and usability gates

Use system sans UI and monospaced data with selectable text, scalable sizing, visible focus, labeled controls, predictable Tab navigation, and non-color indicators. Test light and dark palettes against WCAG AA contrast, including selection/focus/error states. Audit and test GPUI/macOS accessibility exposure with actual assistive technology before claiming screen-reader support. Record limitations and release decisions explicitly. Dense defaults must still permit readable scaling and keyboard access to offscreen/virtualized content.

Measure key-to-visible response, frame timing, and shell startup against the proposed targets in the [product plan](product-plan.md); measurements are not yet available. Acceptance testing includes keyboard-only connection, query, result inspection, export, edit review, and AI opt-in/insert flows, plus cancellation and unknown outcomes.

Reason: a dense tool is usable only if navigation, readability, and state remain accessible.

[Overview](../README.md) · [Product plan](product-plan.md) · [Roadmap](roadmap.md) · [Design direction](../DESIGN.md) · [Architecture](architecture.md) · [Drivers](drivers.md) · [ACP](acp.md) · [Security](security.md) · [Development](development.md) · [Testing](testing.md) · [ADRs](adr/README.md)

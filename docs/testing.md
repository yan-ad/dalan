# Test strategy and release evidence

## Background macOS JDK discovery with manual fallback

Drivers and JDBC source forms share a lazy runtime field that discovers local Java only when the JDBC controls are shown and no explicit path is configured. The worker runs the fixed macOS system locator `/usr/libexec/java_home -F -v "17+"` without a shell, validates an absolute executable and checks Java 17+ with the existing bounded version probe. Both child processes have five-second deadlines, cleared environments, capped output and kill-on-drop. No Java download, driver JAR execution or database/network access is part of detection. Successful detection replaces manual entry with a runtime/path label and an explicit override; failures expose manual entry. Saved paths and changes made during discovery reject stale results. Source persistence remains explicit Apply/OK.

Owned temporary locator/runtime fixtures test fixed arguments, spaces in paths, Java 21 success, old-Java/nonzero/malformed/missing/oversized-output rejection. UI fixtures inject synthetic success/failure rather than inspecting installed user runtimes, and exercise fallback/override and non-overwrite guards. Local verification passed **245 headless unit tests + one native-wire integration test, 255 production UI tests and 14 Python tests**, formatting, strict workspace/desktop Clippy and macOS debug bundle/plist/ad-hoc signature checks. Installed user JDK detection and native visual behavior were not exercised by these synthetic tests; no hosted CI, live servers or private credentials were used. CI remains skipped.

## Source context menus, schema counters and full-row environment colors

Removed source configuration gears and tiny environment color bars. Kit context menus capture the right-clicked source for Manage / Copy / Remove, retain destructive AlertDialog confirmation and reject busy/saving/open-settings operations. A compact database counter follows each source name, displaying total loaded databases or visible-of-total. Its Kit popover uses searchable virtualized cached checkboxes, All databases and a default-database visibility shortcut. Enter/Apply/outside click persists display-only visibility; Escape cancels. It does not open source settings, discover metadata, read credentials, change the default connection database or select an execution target. Persistence failures preserve previous settings; changed source/catalog snapshots invalidate popup commits.

Full-row configured color tints are blended into opaque theme backgrounds with stronger selected/hovered states; uncolored rows retain normal selection and provider icons preserve original fills. Regressions cover counter semantics, light/dark opaque blending, actual right-click menu actions, popup cancellation/Enter/outside behavior and stale snapshots, plus synthetic visibility storage success/failure and unchanged settings/credentials/catalogs. Existing large-tree virtualization/search and workspace tests remain passing.

Local verification: **244 headless unit tests + one native-wire integration test, 253 production UI tests, 14 Python tests**, formatting and strict workspace/desktop Clippy. The macOS debug bundle passes plist and ad-hoc signature checks. UI event/geometry tests are not native visual or screen-reader certification; live/private servers and credentials were not used. CI remains skipped.

## MongoDB Compass-style connection string URI and Direct Connection controls

Added support for full MongoDB connection string URIs including options (`retryWrites`, `loadBalanced`, `serverSelectionTimeoutMS`, `connectTimeoutMS`, `authSource`, `authMechanism`, `directConnection`, `tls`/`ssl`, and standard driver parameters). Extended `parse_mongo_uri`, `MongoOptions`, and `set_mongo_uri_direct_connection` in `dalan-drivers`, and propagated parsed options (`auth_mechanism`, `retry_writes`, `load_balanced`, timeouts) into native `mongodb` `ClientOptions` and `Credential`.

Switched MongoDB connection form UX in `SourceForm` to Compass style: hides manual host/port/user/password/database individual fields in favor of direct connection string URI input with two-way synchronized `Direct Connection` checkbox (`source-mongo-direct`) and session password preservation with scrubbed URL display. Unit tests cover full Compass URI parsing, credential extraction without URL retention, bidirectional Direct Connection toggling, and exclusion of manual fields from the DOM in MongoDB mode.

Local verification passed **244 headless unit tests + one native-wire integration test, 248 production UI tests and 14 Python tests**, formatting, strict workspace/desktop Clippy and signed debug bundle/plist/signature checks. No private credentials or live servers were contacted. CI remains skipped.

## PostgreSQL User & Privileges driver and settings Cmd-W window close routing

Ported DBX's PostgreSQL role administration query and DCL generation logic into native `dalan-drivers` (`postgres_users.rs` and `postgres.rs`). Added `list_roles`, `show_role_grants`, and `execute_role_admin` with safe SQL identifier quoting (`quote_identifier`) and literal escaping (`quote_literal`). The query architecture uses PostgreSQL's `pg_catalog.pg_roles` system catalog, `pg_auth_members`, and a unified CTE evaluating role attributes, parent/child role memberships, database-level privileges (`has_database_privilege`), schema-level privileges (`has_schema_privilege`), and table-level privileges (`information_schema.role_table_grants`). Six unit tests cover quoting, DDL generation (`CREATE ROLE`, `ALTER ROLE PASSWORD`, `ALTER ROLE LOGIN`, `DROP ROLE`), multi-scope grants (`DATABASE`, `SCHEMA`, `TABLE`, `ALL TABLES IN SCHEMA`, `ROLE`), and grant line categorization.

In addition, fixed source settings dialog close routing on `Cmd+W` across focused input and sub-page contexts (`SourceForm`, `SshManager`, `SourceDialog`) by registering `"w"` in the platform key listener and extending `CloseWindow` keybindings.

Local verification passed **243 headless unit tests + one native-wire integration test, 247 production UI tests and 14 Python tests**, formatting, strict workspace/desktop Clippy and signed debug bundle/plist/signature checks. No private credentials or live servers were contacted. CI remains skipped.

## Accessible Kit AlertDialog modals for destructive confirmations

Replaced all text-only discard, abort, remove, and close confirmations across Dalan with accessible GPUI Kit `AlertDialog` modal overlays (`super::confirm::open` and `WindowExt::open_alert_dialog`) without breaking safety guards or DataGrip/DBX-inspired workflows. The shared helper sets default initial focus on the safe cancel button, routes Escape to cancellation, disables backdrop dismissal, prevents stacking dialogs on repeated shortcuts, and verifies that stale targets, in-flight operations (`write_busy`, `saving`), or changed revisions reject destructive callbacks.

Flows migrated to Kit AlertDialog:
- Staged table write discard and apply confirmations in the data grid (`grid-confirm-apply`, `grid-confirm-discard`, `grid-keep-staging`, `grid-keep-edits`).
- Unsaved draft close and active source deletion in source settings dialog (`settings-discard-close`, `settings-keep-editing`, `settings-confirm-remove`, `settings-keep-source`).
- Saved source removal in source browser explorer (`confirm-delete`, `cancel-delete`).
- Unsaved query console and staged table tab closure in the workspace (`discard-query-draft`, `keep-query-draft`).
- SSH manager unsaved session discard on close / in-flight test cancellation, and session draft deletion (`ssh-discard-close`, `ssh-keep-editing`, `ssh-delete-confirm`, `ssh-delete-cancel`).

Local verification passed **237 headless unit tests + one native-wire integration test, 247 production UI tests and 14 Python tests**, formatting, strict workspace/desktop Clippy and signed debug bundle/plist/signature checks. No private credentials or live servers were contacted. CI remains skipped.


## Statement gutter, Run chooser and sequential batches

Added source-preserving dialect statement spans/numbered ellipsized labels, visible Kit Run/Stop gutter buttons before line numbers, and a real Kit toolbar chooser with All Query / numbered statements. Cmd-Enter executes selection/current statement. All Query validates all bounded statements before credentials/network and executes sequentially, with first-error stop, generation-guarded progress/cancellation and individual retained result selectors. It is not a shared transaction or a write-console expansion.

Ten pure splitter regressions cover Unicode/comments/quotes/semicolons/dollar quoting and statement/input bounds. Model tests cover whole-batch prevalidation before auth access, bounded result retention, progress and cancellation preservation. UI tests exercise actual chooser/gutter actions, no-network unsafe statement rejection, running Stop/stale edited-draft cancellation and native gutter scroll geometry. Existing caret/undo/performance work-count checks remain intact. Local verification passed **237 headless unit tests + one native-wire integration test, 243 production UI tests and 14 Python tests**, formatting, strict workspace/desktop Clippy and signed debug bundle/plist/signature checks. No private data/server/auth file access was needed. Synthetic/model checks do not certify all-script success on live server matrices. CI remains skipped.

## Compact persistent database action row and composed search

Consolidated Sidebar/Add/Refresh/Tree expansion/New Query into a gapless 110 px icon row beside traffic lights, with 22 px controls, actual sidebar visibility glyphs and retained tooltips/accessibility names. The group stays available in the main titlebar when the sidebar is hidden. Composed Kit Input prefix/suffix slots place Search and Regex inside one retained field instead of an external button; cached filtering, regex errors/Escape and privacy adapters remain unchanged.

Added event/geometry coverage checks ordered adjacency/full-height hit targets in visible/hidden/restored layouts, in-field addon bounds, Add dropdown access and New Query with hidden explorer. Updated native Tab traversal and preserved menu/shortcut controls; standalone explorer harnesses retain their explicit action row while the production shell removes the duplicate row. Local verification passed **227 headless unit tests + one native-wire integration test, 238 production UI tests and 14 Python tests**, formatting, strict workspace/desktop Clippy and signed debug bundle/plist/signature checks. Hosted CI remains skipped. Geometry is simulation evidence, not native visual certification.

## MongoDB URI autofill and manual TLS defaults

Reproduced pre-network rejection of credential-bearing MongoDB URLs and all URL query parameters. Explicit form paste now extracts credentials into masked fields, scrubs URL/history, selects User & Password and autofills endpoint fields. Restricted directConnection/authSource/tls options are validated and materialized into native driver options; URI auth source defaults to database/admin. Persisted-profile validation still refuses URL credentials. New manual sources default TLS Disabled, without rewriting saved verified profiles or weakening conservative foreign imports/secure-scheme requirements.

Synthetic tests cover percent-decoded credentials, session-only save choice, password clearing, scrubbed JSON/undo, no native work during paste, sanitized rejected URI errors, duplicate/unsupported/malformed options, driver-option handoff, explicit tls=true and saved TLS round trips. No supplied server/credential was contacted or used in fixtures. Local verification passed **227 headless unit tests + one native-wire integration test, 237 production UI tests and 14 Python tests**, formatting, strict workspace/desktop Clippy and signed debug bundle/plist/signature checks. Hosted CI remains skipped. Actual server connectivity/authentication/TLS remains a user-triggered test, not a verified outcome of parsing fixes.

## First JDBC bridge and installer

Current implementation: isolated Java 17+ source-file-launcher child per operation, bounded credential-bearing stdin JSON and checked 2 MiB responses, cleared environment/output isolation, per-operation verified JAR snapshots and manifest Class-Path rejection; conservative SELECT/read-only/rollback policy; curated discovery/version feeds, mandatory-checksum installer and installed-driver UI/source routing. See [the JDBC contract](jdbc-drivers.md) for exact limits and unsupported features.

An actual temporary JDK successfully compiled the owned fake JDBC driver/helper and exercised source launch, class loading, success and failure secret/log redaction through `./scripts/test-jdbc-bridge --jdk /absolute/path/to/jdk`. Synthetic temporary-file tests and public metadata/sidecar probes are scoped evidence, **not** real vendor/server qualification or a complete downloaded-JAR install/execution test. Public sidecar/version probes encountered 404 responses (including SQLite's pinned SLF4J auxiliary sidecars and Oracle/MariaDB/MySQL version probes); mandatory sidecars remain fail-closed, and listed versions are not guaranteed installable. Any Maven entry failure fails the entire version refresh; JetBrains discovery can fall back to curated coordinates.

**Local verification passed:** **224 headless unit tests + one native-wire integration test, 235 production UI tests and 14 Python tests**, formatting, strict workspace/desktop Clippy and signed debug bundle/plist/signature checks. The explicit temporary-JDK fixture compiled and launched the Java bridge, loaded a synthetic JDBC Driver, and verified authentication failure/output redaction. Public catalog/version/sidecar probes established endpoint availability and fail-closed checksum limitations; no complete vendor-JAR installation or real-server qualification is claimed. Prior section totals below are historical evidence for their own changes, not JDBC totals. Real vendor/server/authentication/TLS matrices, full remote artifact installs, native UI interaction, OS permissions/process-tree lifecycle, and memory/performance qualification remain unrun. Child termination does not kill descendants and driver wire allocations are not hard capped. This docs update does not run CI, access private files/databases or claim release certification.

## Bundled driver inventory, preferences and brand artwork

Driver-page Kit tables display exact compiled versions/status and protocol-specific capabilities/TLS/server-qualification limits. Latest bundled and exact-pin choices persist in `dalan.config`; only one backend version is currently included per engine, so alternate-version downloads or execution are not claimed. Catalog/config validation rejects unsupported choices, exact dependency pins are regression-checked, old config files default empty preferences, and appearance/driver saves preserve one another. UI tests exercise the actual version picker/Apply, cancel isolation, centered icon/name geometry and minimum window layout. Provider images decode the original multicolor SVG rather than a monochrome theme mask; artwork files are unchanged and cached handles avoid per-render decoding identities.

Local verification passed **207 headless unit tests + one native-wire integration test, 230 production UI tests and 14 Python tests**, formatting, strict workspace/desktop Clippy and signed debug bundle/plist/signature checks. No private configuration/credentials/servers were accessed; selector persistence uses temporary fixtures. Native visual/color certification and actual-server version/TLS matrix remain unrun. Hosted CI remains skipped.

## Editor notification isolation and complete toggle icons

SQL adapter state observation previously propagated caret blink/focus/selection notifications after materializing the whole rope as a string. It now subscribes only to actual Kit `InputEvent::Change`; selection is read directly from retained state only when an operation requests it. The console does not notify its outer toolbar on unchanged editor content. Tab strips/result-retention bookkeeping react only to presentation/page/target changes, and result grids preserve visible caches on unrelated query-draft notifications. Explicit table-edit revisions keep staged writes repainting correctly.

Regression counters cover four seconds of simulated caret blink and selection on a roughly 56 KiB draft producing zero outer editor notifications, while real input still propagates; 30 query-draft updates cause zero result-grid notifications/cache invalidations; 38 continued nonempty draft updates cause zero workspace/tab-strip notifications or budget recomputations. These are deterministic work-count bounds, not measured native typing latency/FPS. Kit highlighter/native font/IME profiling remains a separate gate.

Named control icons now resolve from the complete bundled Kit catalog when missing from its small default asset subset, without inventory enumeration on each load. Tests exercise exact formatter/wrap/case/save/open/clipboard/overflow/theme icon paths as well as the full inventory; wrap/case buttons have selected states and overflow items have icons. Local verification passed **203 headless unit tests + one native-wire integration test, 227 production UI tests and 14 Python tests**, formatting, strict workspace/desktop Clippy and signed debug bundle/plist/signature checks. Hosted CI remains skipped.

## Black and pink app artwork

Replaced the old source layer with the user-supplied `Dalan New Transparent.png` from the updated Icon Composer package. Source configuration uses the pink layer only, solid black fill and no shadow/translucency. A dependency-free compositor preserves its alpha silhouette with exact sRGB `#FF6AEB` foreground on opaque black, producing the README/portable PNG and conventional multi-size ICNS through `scripts/prepare-icons`. The user's Downloads package is unchanged.

All **14 Python bundle/preview tests passed**, including reproducible composition, exact color/opaque pixels, malformed PNG rejection and source-layer configuration. ICNS extraction produced all ten expected sizes and the 1024px decoded pixels matched the composed PNG; bundled PNG/ICNS bytes matched the repository assets. The debug app rebuilt and plist/signature checks passed. No Rust behavior changes or current-task Rust-suite rerun was needed. Apple `actool`/real compiled Icon Composer rendering remains unavailable/unverified; native Dock/Finder cache refresh is not certified. CI was skipped.

## Flat SQL editor chrome

Removed rounded/card styling and inter-button gaps from the SQL/JSON console workspace only, using Kit ButtonRounded::None and its borderless editor/Combobox appearance APIs. Added a geometry regression requiring adjacent Run/Cancel/tool and source/database/clear controls, full-height hit targets, and editor-to-pane flush edges at 1200 px and compact 560 px widths. Retained input text/gutter padding and global theme/dialog styling are unchanged. Caret, Cmd-Enter, undo/selection and cached database picker regressions remain intact. Local verification passed **203 headless unit tests + one native-wire integration test, 223 production UI tests and 14 Python tests**, formatting, strict workspace/desktop Clippy and signed debug bundle/plist/signature checks. Hosted CI remains skipped. Layout simulation does not claim native visual pixel certification.

## Full-height explorer chrome and regex search

Shell layout now separates the full-height left sidebar from the right workspace/titlebar, keeping the DB toggle beside traffic lights and search immediately below. Tests cover top/bottom bounds, full-height divider, compact resize, hide/show fallback, adjusted keyboard traversal and preserved layout controls. Regex regressions cover case-insensitive anchored Unicode/engine matches, invalid/oversized/unsupported syntax, visible toggle/error, Escape restoration and unchanged cached expansions/results with no I/O. Literal search and the 50,000-object lazy index regressions remain intact. Local verification passed **203 headless unit tests + one native-wire integration test, 222 production UI tests and 14 Python tests**, formatting, strict workspace/desktop Clippy and signed debug bundle/plist/signature checks. Hosted CI remains skipped. This is simulated layout/behavior evidence, not native traffic-light/visual certification.

## Staged SQL table mutations

DBX-inspired first write stage supports local cell changes/NULL, row add/clone/delete/restore and explicit Apply/Discard on eligible SQL base tables. Preview originals are retained, with whole-table type/key/truncation guards, metadata fingerprints, typed bound values and 100-operation/64 KiB-cell/2 MiB-batch caps. Backends lock/reread metadata, compare complete original rows, require exactly one affected row per operation, and commit the batch once. MySQL requires InnoDB/strict mode; PostgreSQL uses a dedicated writable transaction. Schema/target changes and uncertain commits cannot be silently retried.

Pure staging/compiler regressions cover DEFAULT/NULL/empty text, original/key retention, no-op edits, request/schema/type/bounds validation and conflict counts. UI tests verify a cell editor stages its captured target rather than a later selection, Apply first presents confirmation, discard retains the original page, added/cloned/deleted rows remain bounded canvas previews, and in-flight writes block Cancel while source changes defer invalidation. Three owned-loopback PostgreSQL transaction tests exercise actual native extended-protocol binding/metadata/mutation sequences: successful COMMIT, zero affected rows with ROLLBACK/no COMMIT, and dropped commit acknowledgement yielding `CommitUncertain` without retries.

Local validation passed **203 headless unit tests + one native-wire integration test, 219 production UI tests and 14 Python tests**, formatting, strict workspace/desktop Clippy and signed debug bundle/plist/signature checks. No private auth/server/user-table access was performed; actual-server MySQL/PostgreSQL transaction/permissions/TLS validation remains unrun. This is experimental, partial table-editing parity—not all DBX layouts, bulk tools or MongoDB/Redis writes. See [table editing boundaries](table-editing.md). Hosted CI remains skipped.

## Settings footer notification cleanup

Removed connector-feedback labels appended below the fixed Cancel/Apply/OK row. Import/export picker cancellation is silent and clears transfer feedback. Newly opened settings discard stale completed notices except warnings accompanying incoming imported drafts. Import/export errors and remaining settings notices use a dismissible upper-right overlay without changing footer/editor bounds; important failures are not suppressed. Tests cover silent native-picker cancellation, stale-message removal and error overlay/dismissal geometry. Local verification passed **186 headless unit tests + one native-wire integration test, 216 production UI tests and 14 Python tests**, formatting, strict workspace/desktop Clippy and signed debug bundle/plist/signature checks. Hosted CI remains skipped.

## Native PostgreSQL, MongoDB and Redis adapters

Native engine routing now exposes experimental PostgreSQL, MongoDB and Redis alongside MySQL/MariaDB; this is not JDBC support. New tests cover PostgreSQL quoted identifiers, typed cells/NUMERIC, startup framing and cancellation; MongoDB JSON read-command allowlist/BSON previews and actual official-driver handshake framing; Redis RESP2 frame bounds, authentication/SELECT/SCAN/read-value fixtures and cancellation. PostgreSQL syntax/function/write rejection and normalized `jdbc:rediss://` TLS enforcement have additional regressions. UI checks exercise driver/port selection, direct-only guards, collection/key routing, JSON console preservation and rejecting SQL before credentials/network access.

Local validation passed **186 headless unit tests + one native-wire integration test, 215 production UI tests and 14 Python tests**, formatting, strict workspace/desktop Clippy and signed debug bundle/plist/signature checks. Test traffic uses owned loopback endpoints and synthetic values; no existing databases, local auth files or private keys were accessed. An actual PostgreSQL/MongoDB/Redis server authentication/query/TLS matrix remains unrun, and these modules are experimental rather than release-certified. PostgreSQL and MongoDB previews do not impose hard wire/batch allocation caps; Redis’s RESP2 decoder does. See [native driver support boundaries](native-drivers.md). Hosted CI remains skipped.

## Contextual tabs and Cmd-T

Tabs display provider icons, Source@Database · console number / Table@Database labels, retained dirty/running indicators and full contextual tooltips. Saved source colors override stable source-UUID fallback colors; subtle inactive/active washes and a colored active underline preserve Kit text contrast. Native small tabs remain 24 px high, with 100–280 px width bounds/ellipsis, scrolling and independent close controls.

Added geometry checks require source washes to cover full active/inactive tabs and driver prefixes to remain separate from close buttons; metadata name/color changes update presentation without recreating tabs. Cmd-T is tested from both empty shell and native editor, inheriting active target and preserving the previous draft; Cmd-Shift-N remains an alias. Local validation passed **165 headless unit tests + one native-wire integration test, 212 production UI tests and 14 Python tests**, formatting, strict workspace/desktop Clippy and signed debug bundle/plist/signature checks. Hosted CI remains skipped. This is simulated layout/behavior evidence, not native pixel certification.

## DBX toolbar Stage 1 validation

[Stage 1 toolbar behavior](dbx-toolbar-parity.md) is implemented. **Local validation passed: 165 headless unit tests + one native-wire integration test, 211 production UI tests and 14 Python tests**, formatting, strict workspace/desktop Clippy, and signed debug bundle/plist/signature checks. Hosted CI remains deliberately skipped; no private profiles, auth files or live databases were required. Historical totals below remain evidence for their recorded revisions, not this change.

Regression gates cover conservative whole-document token-gap formatting and keyword-case scope; compression preserving adjacency/comments/newline semantics; escaped newline/tab clipboard cells; UTF-8/BOM/64 KiB file limits, literal-CR refusal, private new-file publication, no overwrite/symlink paths and changed-draft picker guards; Kit undo/active caret retention; wrap/Unfold without fake Fold All; Run/Stop plus retained Cancel; source/tab descriptor retargeting; cached searchable database choices/Clear Database; root profile default persistence without auth and URL-only refusal; and full/compact 800 px More routing. Export loaded CSV keeps its existing eligibility and snapshot checks.

Only synthetic, credential-free fixtures are appropriate. No private credentials/profiles/row contents, commit/push or current CI result are claimed. Hosted CI is explicitly skipped. Native picker interaction, visual/IME/VoiceOver and measured responsive performance remain separate acceptance gates; simulated geometry does not establish exact DBX overflow/hysteresis or full 1:1 parity. Stages 2–5 need their own execution-policy, session-ownership and evidence gates before shipping.

## Flush workspace bottom edge

Removed the empty 28 px shell status strip left behind by the ACP trigger relocation. Regression geometry requires explorer, main workspace and visible ACP to reach the 800 px window bottom; main content also reaches the edge with explorer hidden and at compact 720 × 480 size. Connector/appearance feedback is an explicit dismissible absolute overlay whose presence/dismissal never changes workspace bounds. Existing result footers are not removed. Local validation passed: **151 headless unit tests + one native-wire integration test, 203 production UI tests and 14 Python tests**, formatting, strict workspace/desktop Clippy and signed debug bundle/plist/signature checks. Hosted CI remains skipped. Geometry is simulated, not a native visual certification.

## SQL caret initialization and Cmd-Enter regression

A failing-before/passing-after regression reproduced an editor focused before lazy Kit state creation accepting input without starting its caret blink notifications. The adapter now transfers and restores focus through `EditorState::focus`, not only its `FocusHandle`. It preserves the retained text/selection/history; Run, cached database selection and console activation use the same state-aware focus path. Routine result notifications still do not refocus the editor.

A second failing-before/passing-after regression sends actual `cmd-enter` through the native editor: Kit’s deeper `Input` binding selected its `Enter { secondary: true }` instead of the outer `RunQuery`. The console captures that native action only for its focused SQL editor and unchanged modifier mode, then uses normal guarded Run. Plain Enter remains editing, selected SQL remains selected, and unsafe SQL rejects before auth/network access. This is not a global editor-keybinding override.

Four regressions cover actual shortcut dispatch/plain newline/retained selection, lazy focus blink scheduling, horizontal/vertical short-query caret geometry across blur/refocus/repaints, and many-line vertical scroll without cursor reset. Simulated geometry/notifications are not pixel-rendered or native OS certification: the test context has no HeadlessRenderer. A separate long-unwrapped-line stress case revealed an unresolved upstream horizontal-scroll limitation (caret outside the viewport); the current initialization fix does not claim to solve that distinct case. Local validation passed: **151 headless unit tests + one native-wire integration test, 202 production UI tests and 14 Python tests**, formatting, strict workspace/desktop Clippy and signed debug bundle/plist/signature checks. Before-fix logs `/tmp/dalan-cmd-enter-before.log` and `/tmp/dalan-caret-lazy-before.log` recorded the two failing targeted reproductions on this development machine; temporary logs are not committed. Hosted CI remains skipped.

## Current appearance and local auth validation

Current behavior supersedes historical Keychain and light/dark-only evidence below. The appearance control cycles **System → Light → Dark → System**. System is the default and follows the current OS appearance automatically; it is not an alias for Light. A global window-appearance callback updates Kit only while System is selected; explicit Light/Dark ignores OS changes. The preference is persisted as JSON version 1 `appearance` (`system`, `light` or `dark`) in `dalan.config` beside `sources.json`. All modes use Kit defaults, with no custom palette.

Passwords remain session-only by default, with no credential write. The former **Save in Keychain** option is removed: checking **SaveForever** explicitly opts into local **unencrypted plaintext** password storage, not an OS vault. `dalan.auth` is JSON version 1 with a `credentials` map from source UUID to password, separate from password-free `sources.json`, in the same platform support directory. The backend no longer depends on `keyring` and does not access, migrate or delete old Keychain items. An old remembered-password profile without a local saved credential requires re-entry. No secure memory-erasure guarantee is made.

Coverage must verify auth JSON version/UUID/password round trips; malformed/duplicate UUID and bounds rejection (1 MiB/100 credentials/64 KiB passwords); private Unix creation modes and rejection of permissive auth directories/files, symlinks and Unix hard-linked auth files; no credential writes for session-only saves; local saved credential resolution and old remembered profiles without local auth requiring re-entry; no implicit Keychain access/migration/deletion; compensating rollback failure reporting rather than transaction claims; No Auth cleanup and credential-free export. Windows inherited user-directory ACLs are not enforced or validated. Portable Rust persistence is not native app/platform certification.

UI coverage must verify default System tracks current OS appearance (including dark startup), persisted System/Light/Dark cycle and global window-appearance callbacks affecting System only, across main/source/About windows; Kit defaults without custom palette; and three-second Show/Hide presentation with copy/cut and surrounding-text extraction still blocked, retained undo/caret/selection, and no model/password mutation. Memory secure erasure is not promised.

**Local verification passed:** **151 headless unit tests + one native-wire integration test, 198 simulated UI tests and 14 Python tests**, formatting, strict Clippy for workspace and desktop/UI all-targets, signed debug bundle/plist/signature checks. Synthetic tests cover config defaults/validation/private atomic replacement; local auth updates/deletions/failure retention/permissions/symlink/hardlink/concurrent stores; appearance System mapping and persisted explicit choices; three-second reveal and continued clipboard/native extraction privacy. The mapping test drives the callback’s appearance logic, not native OS event delivery. No user auth files, Keychain credentials or live database access was needed; hosted CI was deliberately skipped. Historical suite counts/generated Keychain round trips remain evidence for their own revisions only. Native OS appearance switching, Windows ACL behavior, platform packaging, password/IME/accessibility privacy and visual review remain uncertified.

## Production GPUI Kit validation gates

Production now aliases `gpui` to Kit 0.7.1 with SQL grammar, on coordinated `gpui-pre 0.3.8`, with old GPUI 0.2 removed. Kit application/init/open_window Base Root covers main/source/About (with SSH now embedded in SourceDialog); all standard controls/default theme are Kit-owned. Thin InputState/password privacy and rope EditorState adapters replace bespoke editing engines. Source adoption is not native certification.

Historical migration verification passed **121 headless unit tests** (4 ACP, 54 app, 5 core, 58 drivers), **one native-wire test**, **160 production UI regressions** in parallel, **seven Python bundle tests**, **four tests in the now-removed isolated Kit pilot**, and **29 unique live cases** (11 direct/URL/socket/CONNECT/auth, 12 TLS, 6 SSH). Formatting, both strict Clippy paths, signed debug bundle and dependency graph checks passed at that revision. Password OS extraction and masked clipboard, default-theme invalidation, populated-result bounds, focus/menu/dialog/save guards, shared pages/canvas painting, credential retrieval, schema cache and tab isolation were covered. Kit SQL syntax/selection/undo and 64 KiB programmatic/interactive rollback tests passed. Exceptional oversized interactive rollback resets editor undo history; this is documented rather than hidden. Current development-fix results are recorded in the next section. Hosted CI is explicitly skipped for this task; no new run is triggered, checked or claimed.

Historical counts below apply only to their recorded revisions; they do not establish current Kit production parity. Earlier Carbonfox palette/manual instructions and bespoke-editor undo/geometry guarantees are superseded by Kit default-theme/editor behavior. Keep historical assets and notices. Native fonts/shaders/windows, visual/high-DPI/VoiceOver, measured performance and real database and native privacy checks remain separate gates (old Keychain reruns are historical, not a current backend gate). See [migration](gpui-kit-migration.md).

## Connector import and export

Coverage targets the Kit **New Connection** dropdown (Create Manually and the DBX/Navicat NCX/DataGrip Import section), native **File > New / Import > Connectors List / foreign import choices / Export**, and action routing in both the main window and unified SourceDialog. Import regressions cover single-file picker/cancel behavior, retained existing drafts, fresh UUID/password-free unsaved review drafts, capacity guards and warnings without automatic persistence, credentials or network work.

Parser/export coverage targets the 1 MiB/100-entry limits; plaintext DBX `connections`/bare arrays and encrypted-export rejection; NCX attributes and DataGrip `dataSources.xml`; UTF-8/UTF-16 XML and forbidden DTDs; unsupported drivers, URL credentials/options, foreign SSH/proxy/HTTP tunnels and read-only policy fail-closed skipping; VerifyIdentity defaults and review warnings. Native `dalan-connectors` version 1 `profiles` round trips retain supported proxy settings and only valid saved SSH UUID references/materialized transport, exclude SSH definitions and reject inline SSH. Missing local SSH references require session selection before Apply/Test. Export coverage excludes drafts/passwords/key contents, retains local paths, and targets atomic new-file publication, no overwrite and symlink-ancestor rejection. Fixtures must remain synthetic and credential-free.

**Local verification passed:** **135 headless unit tests + one native-wire integration test, 193 simulated UI tests and 14 Python tests**, formatting, strict Clippy on workspace and desktop/UI all-targets paths, and signed debug bundle/plist/signature checks. The 15 new connector-transfer unit regressions replace none; UI regressions include native File action routing in the settings window as well as the main dropdown/import review flow. Hosted CI is deliberately skipped. Previous results below remain historical. No full vendor-version compatibility, current hosted CI, live database/Keychain credentials or native picker/visual/accessibility pass is inferred. See [user contract](source-management.md#connector-import-and-export) and [security](security.md#connector-interchange-boundaries).

## Unified Data Sources and Drivers

Current coverage targets the unified retained SourceDialog: 48 px Sources/SSH/Drivers rail, 228 px provider-icon saved/new list, Add/Cmd-N, Duplicate/Cmd-D and confirmed removal; persistent right identity/tabs/test strip and global Cancel/Apply/OK footer; 1160 × 760 initial / 1040 × 560 minimum layout. Regression coverage includes retained per-source invalid fields, selection, session passwords and tabs, source navigation independent of main-browser selection, and form-route invalidation of stale test/Keychain callbacks.

Embedded SSH coverage includes retained manager drafts across rail switches, persisted Apply/Saved catalog publication to all retained forms, Back discarding pending SSH drafts, and Use selecting the route/returning to Sources without saving the source. Driver information is built-in MySQL/MariaDB only, not a JDBC/plugin executor. Save/dismissal coverage targets active-page Apply rather than all-source/batch-atomic save, source Apply staying open, OK applying the active source before an explicit discard/keep prompt for other dirty drafts, dirty-close confirmation, and busy-save switching/close guards. Kit default-control geometry is simulation evidence, not screenshot pixel matching; no unimplemented Add comment, templates or Advanced controls are advertised.

**Local verification passed:** **120 headless unit tests + one native-wire integration test, 189 simulated UI tests and 14 Python tests**, formatting, strict Clippy on both workspace and desktop/UI all-targets paths, and signed debug bundle/plist/signature checks. New regressions additionally cover preset/schema dirty tracking, password-load notifications never acknowledging a save, and inactive SSH catalog refresh not canceling another source’s active form worker. Earlier counts below remain historical to their revisions. Live database/native Keychain reruns, native visual/IME/VoiceOver/scaled-text/performance review are separate gates. Hosted CI is skipped; no private profiles, credentials, source-identifying data or raw crash reports are committed as documentation/test evidence. README's six-section structure remains unchanged.

## Native multiline result crash and development fixes

A local macOS IPS crash report identified a **Rust panic** in native `WindowTextSystem::shape_line`: newline-bearing result text reached its single-line debug assertion and aborted the app. A synthetic multiline Chinese fixture reproduced the panic before the fix. This is a concrete application text-shaping boundary failure, not evidence of bad OS rendering, fonts or GPU drivers.

Display-only previews now map CRLF to one `↵`, other newline/CR/U+2028/U+2029 separators to `↵`, tabs to `⇥`, and other controls (including NUL/ESC) to visible escapes. They are bounded to 128 graphemes and 4,096 bytes; an oversized combining cluster is dropped atomically rather than split. `shape_cell` also sanitizes defensively, including shared shaping paths such as row-number lines. Header aliases use Kit's own text-line handling. Typed values and loaded CSV are unchanged. Regression fixtures contain only generic synthetic text, never private usernames, endpoints, SQL or row contents.

Current regression scope includes actual Kit editor rendering/native UTF-16 selection and typing immediately after Run, without completion-time focus theft; readable prefixed Kit tab labels and positive bounds; inactive close propagation/draft confirmation; top-right ACP/theme controls; searchable value-safe 1,000-name database choices retained through notifications; and cached search through collapsed branches with a lazily built 50,000-item index and no scroll-time full-metadata flattening. Completed local validation passed **121 headless unit tests** (4 ACP, 54 app, 5 core, 58 drivers), **one native-wire integration test**, **174 simulated production UI tests**, and **14 Python tests** (seven bundle + seven preview). `cargo fmt`, strict Clippy for both the workspace and desktop/UI with all targets, signed debug bundle checks, plist lint and signature verification passed. The historical 160 UI count is not this revision's total.

Bacon 3.26.0 is installed, and `bacon --list-jobs` successfully parsed the default preview job. A native Bacon lifecycle smoke also passed: the default preview launched the signed bundle, a watched-file change stopped the old owned process and launched a fresh one, and Bacon quit cleaned up the preview while an unrelated process stayed alive. The normal debug bundle was then reopened. Automated lifecycle tests verify owned process-group stop/restart, including Cargo and the foreground native app, without killing unrelated instances. Native app launch is the first runtime gate, not a screenshot, manual click-through, pixel comparison or performance pass. Native capture remains unavailable; VoiceOver, IME and measured frame latency remain separate. A crash report may be inspected locally under `~/Library/Logs/DiagnosticReports/`; do not upload or commit the raw IPS dump or user-profile data. No crash artifact is required in the repository. See [Bacon lifecycle and database caveats](development.md#bacon-live-preview).

Hosted CI is explicitly skipped for this task. The removed experiment has no runnable commands or replacement CI job; historical hosted results below apply only to their recorded commits.

## Historical GPUI Kit pilot and populated-console results

The removed isolated Kit pilot historically compiled and built on macOS arm64 with Kit 0.7.1 and coordinated `gpui-pre 0.3.8`; its four tests and strict Clippy passed. SQL grammar highlighting/native editor events, button/checkbox callbacks, icon assets and nonzero wide-table result bounds were covered. The 100 × 512 fixture observed 328 initial and 146 last-column delegate callbacks; those are historical render/measurement calls, not FPS or parity with the canvas. There is no remaining pilot manifest, local command or separate CI job; use the [main-workspace migration guide](gpui-kit-migration.md).

The console wrapper was independently fixed to a column flex container, giving its DataGrid child real remaining height. Historical hierarchy regressions with 100 × 128 results passed at 1040 × 760 and 780 × 560. Initial measured pane/body heights were 357.5/319.5 px and 241.5/203.5 px; retained busy/error states remained positive and scrollable. These are simulated geometry from the pre-migration revision, not current native evidence. Production now uses Kit; rerun the hierarchy against that graph. See [the migration guide](gpui-kit-migration.md).

## README and supplied application icon

The README now has only Introduction, Installation, Development, Contribution, Reference and License sections, with the supplied PNG logo and truthful CI/experimental/platform/build badges. The original Icon Composer package is preserved byte-for-byte. `scripts/prepare-icons` generated the 1024 px PNG and conventional macOS multi-size `.icns`; bundle resources and `CFBundleIconFile` identify that fallback.

Historical icon-packaging verification passed seven Python bundle/icon tests, including resource bytes, PNG/icns headers, package-layer references, fallback versus compiled metadata, and simulated compiler missing/invalid/success outcomes. The app bundle built, passed plist lint, and verified its ad-hoc signature. This does not verify real Apple Icon Composer output: the local host has no `actool`; `--icon-composer` is an explicit Xcode 26+ path and failure must preserve the existing bundle. PNG is available for future platforms; Linux/Windows desktop integration is not claimed. Artwork redistribution terms remain unspecified. Rust UI/database behavior was unchanged and those suites were not rerun for this packaging/docs-only iteration; hosted CI is explicitly skipped for the current task.

The rebuilt app was reopened. `NSWorkspace.icon(forFile:)` resolved the supplied database/path artwork for Dalan.app, and that icon image was inspected without screen capture. This verifies macOS bundle-icon recognition, not the native Icon Composer material/appearance variants or a full app UI screenshot. Missing `actool` was tested directly and left the existing bundle metadata unchanged. The earlier roadmap commit `17c64f9` passed all five jobs in run 37280905819; that historical result does not establish current hosted status.

## DBX reference and ecosystem roadmap

The root [ROADMAP.md](../ROADMAP.md) is the canonical forward plan. DBX was cloned for read-only investigation at `38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad`; upstream builds/tests were not run. The initial Dalan catalog adapts descriptor/capability lookup and validation patterns only, with source attribution and the complete upstream Apache-2.0 license retained in source and bundle Resources. No driver executor, plugin host or AI transport was copied.

That historical revision passed **120 headless unit tests** (4 ACP, 54 app, 5 core, 57 driver), **one native-wire integration test**, **159 simulated UI tests**, and **four Python bundle-helper tests**, formatting, both strict Clippy paths, the catalog-validating diagnostic, debug bundle build, plist lint and ad-hoc signature verification. Bundle tests verify the DBX license bytes and attribution. Five new catalog tests prevent invalid/duplicate identities and planned-driver capability claims. Existing live DB/Keychain evidence remains historical; no native UI/compatibility/performance pass follows from source research. Hosted CI is explicitly skipped for the current task.

## Relocated source controls

Historical local verification passed **115 headless unit tests + one native-wire integration test, 159 simulated UI tests and four Python bundle-helper tests**, plus formatting, both strict Clippy paths, debug build, plist lint, ad-hoc signature and bundled-license checks. Copy/Manage/Remove exact-source targeting, credential isolation, capacity/save/draft guards, merged expansion and keyboard titlebar creation have regression coverage. Database/Keychain live suites were unchanged and not rerun for this UI/model change; hosted CI is skipped for this task; native visual/accessibility remains a separate gate.

Regression scope includes:

- Database collapse toggle at x = 84 px and adjacent same-height 28 px New Connection plus/text control, no duplicate titlebar Dalan label or explorer Add button; pointer/Enter/Space, tooltip, loading/saving guards and sidebar-hidden creation through the retained root subscription.
- Exactly three explorer toolbar icons: explicit-selected-source Refresh, stateful `toggle-tree-expansion`, existing New Query Console. Any visibly expanded saved source means Collapse All; hidden descendant preferences do not change the button direction. Expand Loaded otherwise uses cache only with no network fan-out.
- 18 px `source-actions-{id}` row gears; real-bounds popovers, exact captured UUID for Manage / Copy / Remove, independence from global selection, Tab/Shift-Tab/Up/Down, Escape/outside dismissal and focus return; saving disabled.
- Same tabbed SourceDialog Manage; Copy fresh UUID and UTF-8-safe 256-byte ` copy` name, endpoint/authentication/schema/Options/TLS/color/SSH reference cloning, empty password and `save_password = false`. No credential retrieval; original JSON/results/schema caches unchanged, form schema choices memory-only, no SQLite metadata copy before Save and normal fresh discovery afterward. Existing draft/static notice and 100-profile guards remain.
- Stable-ID removal confirmation despite selection changes, no server-object deletion, unrelated results preserved. Old selected-edit helper remains test-only; native SSH manager unaffected. Cached virtual tree/grid behavior remains, including the existing 300-cell integrated viewport fixture, without a new performance claim.

Copy/Save regressions use a fake credential store and never read real Keychain entries. Live database/transport and native Keychain suites were not rerun for this UI/model change; the historical 29 live cases below are not new evidence. Native screenshot, performance and VoiceOver/accessibility remain unverified. Current local checks are recorded above; hosted CI is explicitly skipped for this task, and no commit/push or CI result is claimed here.

No new dependencies, utility assets or branded logo are introduced. Existing provider-icon commit `5a8452b` and its attribution are preserved separately; a normal main push includes that pre-existing ancestry, not rewritten history or a source-icon change attributed to this task. See [interaction contract](source-management.md#row-actions-and-copy-boundaries) and [UI controls](ui-foundation.md#controls).

## Rich canvas table browser

Historical rich-canvas verification passed **114 headless unit tests** (4 ACP, 53 app, 5 core, 52 driver), **one separate native-wire integration test**, **154 simulated UI tests**, **four Python bundle tests**, and **29 unique live cases** (11 direct/URL/socket/CONNECT/authentication, 12 TLS, six SSH). Formatting, both strict Clippy paths, signed debug build, plist lint and bundled-license checks passed. Repeated fixture assertions and standalone reruns are not double-counted. Current hosted CI and native visual/performance/accessibility remain separate gates.

Historical **`4e389d0` passed all five hosted jobs** in [run 37266563119](https://github.com/yan-ad/dalan/actions/runs/37266563119). This closes that commit's gate only. Current local checks are recorded above; hosted CI is skipped for this task and native review remains separate. No post-push result is inferred.

### Canvas structural evidence

- Production body: no per-cell Divs; row backgrounds/grid lines are `Window::paint_quad`/`PaintQuad` (one per visible line), cached `ShapedLine`s paint text directly. Native headers retain interaction.
- Narrow **730 × 258 / 512 × 200** fixture: **40 exact painted cells**, **40 cached shaped lines**, **six header controls**; a subsequent tiny wheel has **zero newly shaped cells**. Verify exact viewport paint separately from overscan/materialization.
- Integrated **1,000 databases / 100 × 512** at **1280 × 720**: **906 × 570 body**, **300/51,200 materialized cells**, **32 sidebar rows**, **zero projection rebuilds** and unchanged shared page identity. The historical 310-cell virtual-element count below is not the current result; no integrated paint count is claimed without its counter.
- Range-bounded display/shaped/header/row-number caches, overscan two, 128-grapheme cap and cell-fit ellipsis preserve grapheme boundaries/typed values. Regressions cover Unicode/type icons, same-Arc reuse, snapshot eviction and no full-data clone/walk on small redraws.
- Pinned 44 px gutter at x = 0, shared body y, page offset 100 → first label 101; headers have compact name-only labels/full metadata tooltips, explicit glyph click propagation, header Enter/Space and nonsortable console results.
- WHERE/ORDER BY typing is draft-only and per-tab; input-focused Enter uses `TableBrowser > DalanInput`. Invalid clauses fail before UI credential/network work, retain stale old pages, and preserve bound exact literal values. Compiler coverage checks syntax/token/nesting/operator/IN/order limits, metadata membership/quoting and rejection of functions/subqueries/qualified names/comments/statements/extra clauses. Live existing fixtures add clause assertions without inflating case counts.
- Retention tests cover estimated 16 MiB/eight loaded pages, inactive LRU, active/busy/export/save protection, retained tabs/drafts/applied clauses/scroll, page-None cache release, table async refresh and console explicit rerun/provenance. Budget estimates run once per Arc pointer identity without budget-owned references or notification row walks; protection can exceed limits. These are not process-memory metrics.
- Busy Clear remains disabled; the fixture cancels its owned loopback request first rather than weakening production guards. No private DataGrip credentials/hosts/accounts or row data are fixtures.

### Commands and unverified gates

```sh
cargo test --workspace --locked
cargo test -p dalan-drivers --lib --locked
cargo test -p dalan-app --lib --locked grid_viewport::tests
cargo test -p dalan-app --bin dalan --features runtime-shaders,ui-tests --locked data_grid::tests
cargo test -p dalan-app --bin dalan --features runtime-shaders,ui-tests --locked canvas_paints_only_visible_cells_and_reuses_shaped_text -- --nocapture
cargo test -p dalan-app --bin dalan --features runtime-shaders,ui-tests --locked integrated_scrolling_keeps_shared_snapshot_and_projection_stable -- --nocapture
cargo test -p dalan-app --bin dalan --features runtime-shaders,ui-tests --locked
python3 -m unittest discover -s scripts/tests -v
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy -p dalan-app --bin dalan --features runtime-shaders,ui-tests --all-targets --locked -- -D warnings
# Opt-in owned disposable fixtures only:
./scripts/test-databases
./scripts/test-secure-transports
./scripts/test-ssh-transport
```

The primary workflow ran the complete checks recorded above. Canvas counts are simulated operation counts, **not native FPS/latency**. Cargo elapsed time is not a frame benchmark. Native screenshots remain capture-permission blocked; component pins/test bounds do not prove native appearance. User retry of the latest macOS build, native visual/IME/VoiceOver/scaled-text, high-DPI/GPU profiling, release/offline-Metal and future hosted CI remain open. Do not change permissions or reproduce private source values for screenshots. The 36-icon subset keeps the existing revision and full ISC/Feather notices; [table browser](table-browser.md) documents exact current contracts. Older sections below retain historical evidence, not current pass claims.

## Titlebar database shortcut

The explorer toggle replaces the duplicate in-window Dalan label beside native macOS controls, without a bottom-left duplicate. A regression checks x = 84 px, 28 px width, titlebar placement, repeated toggling and absence of the old brand/rail/header controls. Existing Tab, Cmd-B, Layout popover and compact-pane tests pass. Local verification: 106 UI tests, strict UI Clippy, formatting, four Python bundle tests and signed debug bundle checks passed. Headless/database/Keychain suites were unchanged and not rerun for this placement-only iteration. Native visual verification and hosted CI require separate evidence.


## Icon-led chrome revision

Historical icon-led local validation passed **74 headless Rust tests**, **106 simulated GPUI tests**, and **four Python bundle-helper tests**. Formatting, both strict Clippy paths, debug build, plist lint and signature/resource checks passed. New coverage verifies compact icon controls, passive read-only status, cached empty states and Layout popover keyboard/click behavior. Hosted CI is skipped for the current task; native visual/accessibility verification is not claimed.

Coverage targets Dalan-only titlebar and 28 px Layout popover trigger with preserved keyboard/focus behavior; tooltip-only Carbonfox status spacer; twenty-four embedded icons; fixed 18 px cache-state markers with meaning/timestamp/error tooltips; engine/count tooltips without duplicate row badges/numbers; honest saved-source Select a table versus empty-profile connection action; conditional qualified table header/passive read-only lock; and guarded icon filter/export/pagination actions with visible column/operator/value controls. Errors, stale rows and nonfatal cache warnings must remain visible text. Existing virtual tree/two-axis grid, `Arc<TablePage>`, cached-row and connection/selection regressions remain unchanged.

Historical credential fix **`9b3d3d8` passed all five jobs** in [run 37212641100](https://github.com/yan-ad/dalan/actions/runs/37212641100). That closes the previous pending hosted gate for that commit only, not current hosted status. Live database and native Keychain reruns are not required for this UI-only change; historical fixture results remain separate.

No new native screenshot/manual, VoiceOver or performance pass is claimed. Prior capture was blocked; real native source/cache state is not a synthetic fixture, and source-identifying private values must be excluded from published evidence. Native Open dialogs/tooltips require actual interaction, not test-title text assertions. GPUI focus/control semantics are tested, not an accessibility-foundation guarantee. Asset inventory adds hard-drive, triangle-alert, loader-circle, lock-keyhole and chevron-left at the existing Lucide pin; SQLite/Carbonfox notices and bundle-resource contracts remain unchanged.

## Saved credentials on cached-table opening

Fixed a cached-catalog path that previously sent an empty password when no in-memory password had been loaded. Table browsing now uses lazy credential resolution before connecting and retains the retrieved password for the session. Three regressions cover session-first resolution, cached table opening without settings and one credential read across retries, and denied/missing credentials producing no database connection. Tests use an isolated credential-store substitute and loopback endpoints, not user credentials.

That historical revision passed 74 headless Rust tests, 102 simulated UI tests and four Python bundle tests, formatting, both strict Clippy paths, debug bundle build, plist lint and signature checks. Native Keychain authorization dialogs and live database fixtures were not rerun. macOS may require authorization, particularly for rebuilt ad-hoc signed binaries; the fix removes the settings detour, not OS security prompts. Commit `9b3d3d8` passed all five hosted jobs in [run 37212641100](https://github.com/yan-ad/dalan/actions/runs/37212641100); this is historical evidence, not current icon-led CI.

Status: headless and simulated UI tests exist, plus verified disposable MySQL/MariaDB live fixtures. Current recorded suites and generated native Keychain validation passed as detailed below. Agent transport, full release and actual accessibility suites remain future work.

## Wide-grid performance revision

The reported 100-row/100+-column lag identified three structural costs: eager elements for every row/column, display-string allocation for all cells and a deep `TablePage` clone on browser redraw. That is 10,000 cells at 100 columns and exactly 51,200 at 512. The retained `DataGrid` now virtualizes both axes, shares immutable `Arc<TablePage>` with the model/counters/export and caches only visible/overscan `SharedString` cells/headers. Backend typed values, limits, SQL, SQLite catalog metadata, profiles/passwords and CSV scope are unchanged. `serde`'s `rc` feature supports shared snapshot serialization tests, not row persistence; there are no new crates or license changes.

### Structural evidence

The combined simulated workspace regression passed with **1,000 databases and 100 × 512 cells** at **1280 × 720**. Its emitted counts were a **950 × 574.5 body viewport**, **310/51,200 materialized cells (~0.61%)**, **32 sidebar rows** and **projection rebuild delta 0**. Scrolling to row 50/column 200 kept headers aligned with cells. A subsequent 20,000 px sidebar wheel reached distant database rows without changing the grid viewport or shared model page pointer, rebuilding the explorer projection, starting catalog tasks or making the model busy. The page remained the same `Arc`.

The narrower isolated 730 × 258 fixture has a 720 × 220 body viewport and materializes **112 cells** after scrolling to row 50/column 200 in a 200-row snapshot. Seven pure geometry tests cover range bounds/overscan, partial and empty viewports, resizing/shrinking/reset and minimum-thumb edge/monotonic mapping. Grid tests cover both axes, header alignment, focus/keyboard sorting guards, page-versus-status reset behavior, track/drag/resize, mounted-header Tab traversal and empty pages. The text-cache regression checks zero newly formatted cells for small wheel movement within unchanged ranges, bounded eviction and fresh text after snapshot replacement. Model tests check shared-reader/export snapshots and serialization.

These are structural operation counts per simulated frame, **not native FPS, latency or a measured millisecond improvement**. Cargo wall-clock time includes compilation and is not a grid benchmark. Layout measurement runs in canvas prepaint with deferred updates only on changed bounds; stable wheel events schedule no bounds update. Initial layout does not depend on a future native frame callback that GPUI's test platform does not provide.

### Commands and remaining gates

```sh
cargo test -p dalan-app --lib --locked grid_viewport::tests
cargo test -p dalan-app --bin dalan --features runtime-shaders,ui-tests --locked data_grid::tests
# Prints the integrated operation counts:
cargo test -p dalan-app --bin dalan --features runtime-shaders,ui-tests --locked integrated_scrolling_keeps_shared_snapshot_and_projection_stable -- --nocapture
# Final owner-run totals and regression gates:
cargo test --workspace --locked
cargo test -p dalan-app --bin dalan --features runtime-shaders,ui-tests --locked
python3 -m unittest discover -s scripts/tests -v
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy -p dalan-app --bin dalan --features runtime-shaders,ui-tests --all-targets --locked -- -D warnings
```

**Historical verification passed: 74 headless tests, 99 simulated UI tests and four Python bundle-helper tests**, plus formatting, both strict Clippy paths, debug bundle build, plist lint, ad-hoc signature and bundled-license checks. New regressions cover viewport geometry, both-axis scrolling, shared snapshot identity, text reuse, pinned headers/sorting, visible-only header focus, missed mouse-up cancellation, and the combined explorer/grid fixture. Native Keychain/database transport suites were not rerun for this rendering-only change. No current hosted result is claimed; this task skips hosted CI.

Display-only cell previews are capped at 128 Unicode grapheme clusters plus an ellipsis, bounding text shaping for long strings/blobs. The immutable typed values and CSV export are unchanged. Fully clipped overscan headers are not tab stops; scrollbar dragging is canceled when pointer moves without a pressed button. GPUI retains some previous debug-bound entries, so virtualization assertions use current ranges and materialization counters rather than absence of stale entries.

Disposable database/transport fixtures and native Keychain were not rerun for this UI/shared-snapshot-only change; their historical results remain unchanged. Native window screenshot capture remains unavailable. User retry of the latest rebuilt macOS app, real wide-table scroll/resize/keyboard behavior, high-DPI/GPU frame profiling, native visual review, VoiceOver and release/offline-Metal validation remain open. Do not change OS permissions or profile/secrets, or retain private fixture content, for capture. The synthetic `database_0` fixture is not production data. Cell inspection/copy, column resizing and active-cell selection are not implemented; mounted-header focus tests do not establish screen-reader readiness.

## Carbonfox opaque revision

The user selects compact Zed-like UI and explicitly rejects DataGrip visual styling; DataGrip informs database UX/workflows only. Tests reference the complete [vendored variant](../crates/app/assets/themes/carbonfox-opaque.json) and [compiled tokens](../crates/app/src/desktop/theme.rs) from Nightfox's Zed port commit `3511a6f1f665455c70a24d14fd5d2de0eaab58fa`. Both full MIT notices are retained; see [provenance](../crates/app/assets/themes/README.md). Runtime does not load the JSON; `include_str!` is test-only.

New coverage checks exact upstream mappings and opaque compositing of toolbar/selection/border over PANEL, RGB-only tokens, readable text/control boundaries, compact shell geometry and shared input colors. Pure contrast calculations give minimum normal-state primary **13.04:1**, secondary **7.22:1**, focus **6.10:1** and input boundary **3.44:1**. Semantic labels are tested on panel/input surfaces only; pink error labels are not claimed readable on filled hover controls. User-defined source markers have no AA guarantee and remain separate from name/engine labels. Disabled labels use MUTED without opacity.

Compact checks cover a 34 px titlebar, 28 px toolbar/headers/status/controls, 22 px tree/grid rows, 3 px control radius, square flush panes, 0 px outer padding, 4 px divider hit area/1 px line, and 476 px explorer maximum at 720 px with 240 px main content. Source-form regressions retain native 1040 × 760 and minimum 780 × 560, 28 px inputs, 30 px endpoint parents/candidates, 18 px Keychain indicator and existing APIs/focus guards, with 8 px gaps, 16 px scroll padding and footer padding 8 px/16 px. Input selection is opaque and cursor/placeholder use shared tokens. Main/source/About backgrounds are explicitly opaque, with no blur/transparency/toggle; normal theme-name status retains focus-help override behavior.

**Historical theme local verification passed: 67 headless Rust tests, 88 simulated UI tests and four Python bundle-helper tests.** Formatting, both strict Clippy paths, debug bundle build, plist lint and ad-hoc signature verification passed. Bundle tests verify both complete theme MIT notices in Resources; raw reference JSON and standalone theme-license files are not required at runtime. Database/transport and native Keychain suites were not rerun for this styling-only revision.

The updated Dalan.app was reopened after quitting the old instance; macOS confirmed its bundle executable. A window-only screenshot was attempted without changing permissions and failed with `could not create image from window`. Native visual comparison, manual click-through, VoiceOver and scaled-text verification remain unclaimed. The app was left open. Theme commit `0ca0221` passed all five hosted jobs in [run 37204370849](https://github.com/yan-ad/dalan/actions/runs/37204370849). This is historical evidence for that commit; it does not establish current hosted status.

No new native screenshot capture, visual pass, measured pixel matching, VoiceOver, scaled-text or performance result is claimed. Unit/simulated geometry and contrast are not native evidence. Database/cache/SSH/password persistence and Keychain semantics are unchanged; no native Keychain rerun is claimed. Historical metadata commit `4c8af09` passed all five hosted jobs in [run 37201696519](https://github.com/yan-ad/dalan/actions/runs/37201696519); hosted CI is skipped for the current task.

## Current tests and evidence

- Core and ACP retain headless policy/schema/path tests; ACP has no live transport.
- Drivers test experimental MySQL/MariaDB identity, source validation, generated read/filter SQL, value conversion, bounds and relay behavior.
- App headless tests cover layout and versioned source-store/credential failure contracts. One opt-in generated native Keychain round-trip passed and cleaned up its item. Session-only Save tests use no Keychain call; failure tests cover save errors and the failed-load overwrite guard.
- GPUI tests cover shell controls plus source-form/table interactions and stale states. Simulated tests do not establish actual macOS click-through, VoiceOver or text-scaling behavior.
- Bundle helper has four standard-library Python tests, which do not imply app launch.

Historical source-slice results: **37 default headless Rust tests passed** (4 ACP, 13 app, 5 core, 15 driver); **27 simulated GPUI tests passed** (3 native-input, 5 source-form, 4 source-model, 3 browser, 12 shell); **four Python bundle-helper tests passed**. These predate the icon/sorting/export revision and are retained as historical evidence. Default workspace tests remain headless and do not compile the feature-gated GPUI binary.

### Persistent metadata revision

The simulated GPUI suite passed **83 tests**, and **four Python bundle-helper tests** passed. All three disposable live scripts were rerun successfully: **7 direct/CONNECT/authentication**, **10 TLS**, **6 SSH** (23 unique cases, including uncached SHA2/RSA authentication). Full-catalog fixture assertions now cover direct/CONNECT, TLS success/rejection and SSH routes. More metadata requests/assertions do not add unique test cases. The SSH fixture uses serialized test-only configuration for determinism; production configuration is unchanged. No production VPN/edge endpoint success or private logs are claimed.

Final local verification passed **67 headless tests** (4 ACP, 34 app, 5 core, 24 driver), **83 simulated UI tests** and **four Python bundle-helper tests**, plus formatting, both strict Clippy paths, debug build, plist lint, ad-hoc signature and bundled-license checks. App coverage includes eight SQLite cache tests and a real loopback Save-to-discovery failure regression that preserves the last snapshot. One native Keychain test remains opt-in and was not rerun. Metadata commit `4c8af09` passed all five hosted jobs in [run 37201696519](https://github.com/yan-ad/dalan/actions/runs/37201696519), including bundled SQLite headless checks. This is historical evidence for that commit, not a current wide-grid-revision pass.

Cache tests cover atomic replacement/rollback, version and corrupt-cache rejection without destructive reset, private filesystem handling, stale ticket/identity checks, deleted/re-added UUIDs and edit-away/back guards, byte/count limits, and the separate database versus table/view boundary (50 × 1,000 objects plus 1,000 empty databases in the relevant fixtures). Simulated model/browser tests cover startup restore without credentials/network, cache warnings while source JSON still loads, refresh failure retaining rows/tree, explicit-selection-only Refresh independent of table paging, cached status/timestamp labels and nonfatal metadata notices. These tests do not prove native credential save-to-network end-to-end behavior or production UI autosave timing; automatic metadata refresh is a scoped implementation contract, not native E2E evidence.

Cancellation may permit an already-blocking valid cache write to complete; tests and identity/global-generation guards prevent stale publication or profile resurrection, while UI generations ignore canceled completions. No throughput, native FPS, accessibility, plaintext-encryption or offline server-row-access claim is inferred. See [architecture](architecture.md#persistent-metadata-cache-implemented) and [privacy](security.md#persistent-metadata-privacy).

### Historical icon, sorting and loaded-export revision

Verified results for that revision: **48 default headless tests** (4 ACP, 20 app, 5 core, 19 driver), **42 simulated GPUI tests**, four Python bundle-helper tests and one generated native Keychain round-trip passed. Formatting, both strict Clippy paths, debug bundle build, plist lint, ad-hoc signature verification and bundled license-resource checks passed. The 22 live database/transport tests also passed with sorting coverage. These are separate from manual visual/accessibility, hosted CI and release/offline-Metal verification.

New coverage includes nine embedded Lucide assets and labels; header click/Enter/Space sort cycles, column switches, retained filters and offset reset; metadata-validated quoted SQL and primary-key ties; UTF-8 CSV escaping, NULL syntax, formulas and exact decimal representation; bounds/truncation, private publication, no overwrite/symlink overwrite; picker cancellation, duplicate busy requests, generation changes and unavailable-page rejection. The 22 live cases below were rerun with sorting on actual direct, CONNECT, TLS and SSH routes. No new native manual source/save-picker, visual or accessibility evidence is claimed.

### Connection UX and uncached authentication revision

The live rerun passed **23 unique tests**: **seven direct/HTTP CONNECT/authentication**, **ten TLS** and **six SSH** cases. A standalone fresh-account test passed as well, then passed again within the seven-test smoke suite; count it once, not twice. MySQL 8.4.11 and MariaDB 11.4.13 remain the recorded fixture versions.

The MySQL test uses a newly provisioned `caching_sha2_password` account for its first login with TLS disabled, exercising mysql_async RSA authentication. The original readiness login used the reader account and warmed its cache; readiness now uses fixture root and creates the fresh account afterward. This is disposable first-login evidence, not universal authentication compatibility or proof of a fix for the reported remote account. A credential-free remote probe connected at TCP level but reset before the MySQL greeting; no credentials were sent. A user retry with the new sanitized diagnostic remains necessary.

Historical verification passed **52 headless Rust**, **46 simulated GPUI** and **four Python** tests, plus formatting, both strict Clippy paths, debug bundle build, plist lint and ad-hoc signature verification. Earlier generated Keychain and bundle validation above remain historical; Keychain was not rerun for this revision. New coverage targets typed secret-free diagnostics, metadata-only bounded SSH identity discovery, stored Tab-stop state and visual-order traversal, Unicode/password double-click selection without drag shrink, secret clipboard suppression and native surrounding-text privacy. Simulation is not native manual/IME/accessibility verification. No dependencies or license changes were added.

The rebuilt Dalan.app was reopened after quitting the older instance; macOS confirmed its bundle executable and the app was left open. This verifies launch, not the reported remote login or manual visual behavior.

## Historical CA picker and inline credential controls

The source form now has a native single-file CA picker beside its editable path, a visible square/check and clickable Keychain label beside Password, and no redundant form-title/engine strip. Regression tests cover checkbox position and click/Space/Enter behavior, saving guards, updated Tab/Shift-Tab order, file-only picker options, picked-path editing, cancellation, stale manual edits and dialog errors. GPUI 0.2.2's test platform does not implement native Open dialogs, so tests exercise the shared picker completion handler; native macOS dialog interaction remains a manual check.

That historical revision passed 52 headless tests, 49 simulated UI tests and four Python bundle-helper tests, formatting, both strict Clippy paths, debug bundle build, plist lint and ad-hoc signature verification. No database transport, TLS policy or Keychain backend changes were made; live database and generated-Keychain results above are historical rather than rerun evidence for this UI-only change. The user identified WireGuard/VPN routing as the reason for the earlier endpoint issue; no private endpoint details are recorded here.

## Database Explorer redesign: verified

Verified results: **54 headless Rust tests** (22 driver, 23 app, 5 core, 4 ACP), **57 simulated GPUI tests** and **four Python bundle-helper tests** passed. Formatting, both strict Clippy paths, debug bundle build, plist lint, signature verification and bundled-license checks passed. The previous CA-picker record of 52 headless and 49 UI remains historical. New UI coverage includes the center action with the explorer hidden.

Scoped migration/color tests passed: legacy version 1 profiles without color load exactly as None, mixed-case `#AB12cd` survives version 1 serialization unchanged, malformed colors fail load/save without overwriting stored bytes, and password fields remain rejected. No automatic legacy rewrite or Keychain migration is introduced.

New regression scope covers fourteen embedded Lucide assets and abstract engine cues, the four compact toolbar actions and disabled states, labeled color presets/manual validation and form traversal, and the centered no-source action with mouse/Enter/Space. Shell coverage removes rail/header-hide selectors, checks the bottom-left toggle and retained closed preference, 6 px padding on both sides, the 462 px clamp at 720 px, and unchanged compact ACP suppression. Formatting, strict lint, suites and bundle/signature/resource checks passed. Earlier live transport and generated Keychain results are historical, not reruns for this redesign. Native keyboard/visual/VoiceOver/scaled-text checks remain unverified.

## Compact lazy explorer

Historical compact-explorer evidence: the scoped simulated GPUI suite passed **71 tests**. Coverage includes the removed explorer title header, six 28 px toolbar actions, 22 px rows, nonblank constrained/ellipsized names, Unicode/quoted collision-safe identities, single-list navigation, tree-focus guards and toolbar Enter/Space, disclosure propagation, lazy caching, branch-local cancellation/errors, request generations/concurrency and explorer selection independent of table-page identity. Views stay unavailable and do not issue browse queries; Refresh reloads only the explorer-selected catalog root. Expand Loaded has no network fan-out and Collapse All preserves cache.

The large-catalog regression loads **1,000 databases**, verifies **at most 40 painted rows**, scrolls to **row 900** and exercises **End**. The flattened tree is rebuilt on model notification rather than wheel events, and the uniform list renders the visible range only. These are tested structural/render-bound contracts, not measured FPS, latency or a manual native smoothness pass. The reported blank-button cause was not measured; no font/GPU diagnosis is verified.

Historical local verification passed **57 headless tests** (26 app, 22 driver, 5 core, 4 ACP), **71 simulated UI tests** and **four Python bundle-helper tests**, plus formatting, both strict Clippy paths, debug app build, plist lint, ad-hoc signature and bundled-license checks. Live database and generated Keychain results remain historical; no native smoothness/FPS/latency, visual or accessibility claim is implied. Hosted CI is skipped for the current task.

That historical icon set had **nineteen** pinned Lucide assets: ChevronRight, Folder, Table, ExpandTree (`list-tree`) and CollapseTree (`chevrons-down-up`) added five to the existing fourteen at the same fixed revision, with complete existing ISC/Feather MIT license notices. The historical icon-led set had twenty-four; the console slice adds four at the same pin. No dependencies or project-license choice are added.

## Source dialog window and Windows fixture fix

Historical source-dialog local suites passed **54 headless Rust**, **63 simulated GPUI** and **four Python bundle-helper** tests. These supersede the explorer redesign's historical 57 UI tests; they do not constitute native visual/accessibility evidence. Live database/transport and generated native Keychain evidence above is retained as historical: those suites were not rerun for this window/layout and test-fixture change. Source storage, credentials and backend transport contracts are unchanged; no new dependencies, icons or license changes are introduced.

Simulated regression coverage includes:

- A single reused application-wide Data Sources · Dalan window, initial 1040 × 760 (minimum 780 × 560), preserving draft edits across Add/Manage/center triggers and focusing Name on creation.
- Generation-aware replacement of a loaded draft inside the same window, without resetting forms on ordinary model notifications; successful-save notification closes/releases the dialog and allows reopening, without auto-connect. Save-completion tests simulate UI state, not a native Keychain transaction.
- Cancel, Escape, Cmd-W and native close discard/cancel the draft/test; saving guards reject action/native close during credential/JSON save. Backend cancellation continues to own its relay lifecycle.
- Inline database/SSH/HTTP/HTTPS Host–Port geometry and real input Tab/Shift-Tab sequence. Defaults are localhost with ports 3306/22/8080/443; Port remains 96 px wide (minimum 80 px).
- Draw-bound regressions at 1040 × 760 and 850 × 600 with long/short values, long key labels, password and color changes. The max-720 px natural-height body scrolls, input/candidate rows retain 28/30 px heights, the 150 px key list ellipsizes labels, and the footer stays intact. This addresses a real flex-shrink defect even if the supplied screenshot showed an older sidebar form; it is not dismissed as screenshot staleness.

This is a separate normal dialog window, not an OS modal sheet or a main-window focus trap. Native manual dialog visual/keyboard/VoiceOver/scaled-text verification remains open. Existing screenshot capture attempts were blocked by Screen Recording permissions; no permission changes or native visual pass are claimed. Simulated drawing bounds are layout evidence only.

<a id="hosted-ci-historical-failure-and-pending-main-verification"></a>

### Hosted CI: historical failure and subsequent verification

Hosted GitHub Actions run **37192402473** confirmed a failure only in the Windows headless SSH-key unit fixture: creating `id_bad\nname` fails with Windows OS error **123**, because control characters are invalid filenames. The macOS and Ubuntu headless jobs, Ubuntu database fixtures and macOS desktop job passed in that historical run. These are observed historical logs, not new-branch or updated-main pass claims.

The fixture now creates newline, carriage-return and tab filenames only under `cfg(unix)`, where they can be represented and discovery rejection can be tested. Windows still runs the complete metadata-discovery assertions for valid filenames and exclusions. No test checks are disabled, no workflow is changed, and no production filename policy is weakened.

Historical source-dialog fix **3c4fdae** passed all five jobs in [run 37194669634](https://github.com/yan-ad/dalan/actions/runs/37194669634), including Windows and the native offline-Metal macOS bundle. Do not infer current hosted status from that prior commit; this task explicitly skips hosted CI.

### Verified live database slice

Six smoke tests passed against MySQL **8.4.11** and MariaDB **11.4.13** on the same fixture. Coverage includes direct TCP and anonymous HTTP CONNECT reads, all seven filter operators and fixture value representations, BASE TABLE browsing/view rejection, and default verified TLS rejecting untrusted servers. A separate secure-transport suite passed **ten tests**, five per engine: trusted custom-CA database TLS reads over direct TCP and HTTP CONNECT, wrong database hostname rejection, untrusted database CA rejection and untrusted HTTPS proxy rejection. A separate actual SSH suite passed **six tests**: two positive transport reads, two wrong-host-key rejections and two wrong-identity rejections. The SSH fixture exercises a selected known-host path containing spaces. Trusted system/native-CA HTTPS proxy success remains unverified; database CA selection is not proxy CA configuration. This is not a general auth/plugin/topology or managed-service compatibility claim. See the [scope matrix](mysql-sources.md#scope-and-evidence-matrix).

```sh
# Explicitly opt-in, disposable localhost fixtures only:
./scripts/test-databases
./scripts/test-secure-transports
./scripts/test-ssh-transport
# Generated native Keychain item, requires unlocked macOS Keychain, may prompt:
cargo test -p dalan-app --locked generated_keychain_item_round_trip -- --ignored --nocapture
```

The database helper requires a running Docker-compatible runtime, uses `mysql:8.4` and `mariadb:11.4`, binds random localhost ports and grants the fixture user SELECT. It cleans up only its own containers/volumes, including failure paths. Tags may resolve to new patches, so record exact server versions per run. Never substitute production credentials or an arbitrary developer server. The direct/CONNECT/authentication smoke fixture disables database TLS for success cases; trusted database-TLS evidence comes from the separate secure suite. Secure and SSH helpers use disposable generated certificates/keys and temporary directories, cleaning their owned containers/volumes on exit; SSH additionally cleans its dedicated network and generated SSH image. Pulled database images are retained. They require local OpenSSL/SSH key tooling as applicable and do not modify user known-host files or OS CA trust. This does not imply that ordinary SSH default mode avoids reading user/system trust.

## Commands

```sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p dalan-app --bin dalan-doctor --locked
cargo doc --workspace --no-deps --locked
python3 -m unittest discover -s scripts/tests -v

# macOS with full Xcode/Metal:
cargo check -p dalan-app --bin dalan --features desktop --locked
cargo build -p dalan-app --bin dalan --features desktop --locked
cargo clippy -p dalan-app --bin dalan --features desktop --locked -- -D warnings
```

Use `runtime-shaders` instead of `desktop` for a development check without the offline Metal compiler, with the limitations in [development](development.md). Standard shader/release validation remains required.

```sh
# macOS only; GPUI test support stays off in default headless builds:
cargo test -p dalan-app --bin dalan --features runtime-shaders,ui-tests --locked
cargo clippy -p dalan-app --bin dalan --features runtime-shaders,ui-tests --all-targets --locked -- -D warnings
```

GPUI's test platform does not request real display frames; resize tests explicitly refresh. Its 0.2.2 debug-bound map retains removed-element entries, so compact-layout checks use the current layout decision and newly painted content geometry rather than stale absence checks.

The current source-slice Dalan.app was rebuilt, plist-linted, ad-hoc signed and signature-verified, then reopened after quitting the older instance. macOS confirmed the Dalan display name and bundle executable; the app was left open. This does not establish manual source-flow, visual or accessibility verification. Hosted evidence is recorded above; next-main CI verification, local full-Xcode/offline-Metal and release checks remain open. CI defines an Ubuntu `database-fixtures` job for direct/HTTP CONNECT and secure database TLS/proxy-rejection fixtures, not SSH fixtures.

## macOS bundle smoke test

```sh
./scripts/macos
plutil -lint target/debug/bundles/Dalan.app/Contents/Info.plist
codesign --verify --strict --verbose=2 target/debug/bundles/Dalan.app
open target/debug/bundles/Dalan.app
```

Use the actual printed output path if Cargo target configuration changes it. Confirm the launched process is inside `Dalan.app/Contents/MacOS/Dalan`, the macOS app name is Dalan, and an older loose-binary instance is not being activated. `./scripts/macos --run` captures foreground diagnostics; LLDB can run the bundle executable. Release/offline-shader smoke testing still requires full Xcode. Ad-hoc signature verification does not establish Gatekeeper/notarization or release readiness.

## Planned coverage

| Layer | Required cases |
| --- | --- |
| Domain | Session generations, approvals bound to immutable target/operation, cancellation/unknown outcomes, draft conflicts, bounded events |
| SQL parsing | Strings/comments/quoted identifiers, PostgreSQL dollar quoting, MySQL/MariaDB delimiter/stored-program boundaries, unsupported syntax, no partial guessing |
| PostgreSQL | Real server auth/TLS, schema refresh, lossless types, multiple results, session affinity, errors, cancellation, write uncertainty |
| MySQL and MariaDB | Independent fixtures for both, modes/auth/plugins, DDL implicit commit, nontransactional tables, metadata/type differences |
| Redis | Raw byte keys/arguments, all six value types, TTL -1/-2/races, collection and byte limits, SCAN duplicates/mutation, denied command classes |
| Credentials | Save/read/update/delete, locked/denied/missing keychain, no plaintext fallback or export/log leakage |
| ACP fake peer | Initialization negotiation, unsupported capabilities/auth, session/prompt streaming, permission supplied IDs, cancelled permission races, stderr/oversize/malformed output, process exit/shutdown/reconnect |
| ACP real agents | Explicit version/auth matrix, context consent, no automatic execution, no direct provider fallback |
| UI | Focus/navigation, empty/loading/error/stale/cancelled/uncertain states, grid virtualization, layout resizing, scaled text, adopted Carbonfox contrast, actual accessibility API behavior; future themes require separate validation |
| Distribution | macOS debug/release Apple Silicon, Intel decision, signing/notarization, install/uninstall; Linux then Windows separately |

Containerized MySQL/MariaDB fixtures are implemented in `scripts/test-databases`, not a production requirement. Other engine fixtures remain planned. Bind locally, use throwaway credentials, pin images/versions once the matrix is settled, isolate tests, and ensure cleanup after failure. Avoid production credentials and destructive tests against arbitrary developer URLs. Integration test commands must opt in explicitly.

## Manual shell smoke test

Verify Kit's default theme and standard controls consistently in main/source/About/SSH windows, including Base Root overlays, dropdown/menu dismissal and focus return. Confirm no Carbonfox override or hardcoded application palette, no persistent theme/focus-help text or nonfunctional Theme control. Check action tooltips/focus, input selection/cursor/placeholder and readable semantic/disabled states against the active Kit theme rather than historical Carbonfox RGB/contrast assertions. Preserve compact layout and canvas viewport alignment. Exercise password OS surrounding-text suppression, masked clipboard protection, plain Unicode clipboard, native IME/selection and current SQL limit handling. If capturing screenshots, record whether capture succeeds without altering permissions; exclude private source-identifying values and do not infer a visual pass from test-platform drawing.

On a logged-in macOS session, launch Dalan.app, verify the top bar/native controls/Database Explorer render and the browser/table workspace renders and titlebar New Connection opens the dedicated source dialog, including with the sidebar hidden. Open About Dalan from the native menu and check its Cargo version, meaning, scope, and fixed window size. Inspect the 28 px bot-message-square button and its AI · ACP tooltip; exercise it, Cmd-Shift-A, its close button, focused Escape/focus return, and Escape outside the panel; confirm honest Not connected copy and no prompt/provider fields. Verify the activity rail and header hide/minimise control are absent, exercise the titlebar Database toggle at x = 84 px, adjacent 28 px New Connection plus/text and retained closed preference, and check that the explorer title header is absent and its 28 px toolbar has exactly three icons/tooltips/guards: Refresh selected source, combined Expand Loaded / Collapse All and New Query Console. Exercise each 18 px source-row gear and exact-UUID Manage / Copy / Remove menu with pointer/Enter/Space, Tab/Shift-Tab/Up/Down, Escape/outside dismissal and focus return; verify saving guards, draft preservation, nonsecret unsaved Copy and stable-ID removal after selection changes. Inspect readable source names, existing engine cues, engine/full-name/count tooltips and fixed 18 px cache-state icons with timestamp/error tooltips; optional color presets/manual hex remain marker-only. With saved sources but no selected/loaded table, check the small-icon Select a table state and absent browser/read-only header. For loaded tables check qualified titles, passive read-only lock tooltip, icon filter/export/pagination controls, visible column/operator/value and minimal range with loaded-count/has_more tooltip. Errors/stale feedback/cache warnings must stay text. With no sources, activate the centered Connect to a Source by mouse/Enter/Space with explorer shown and hidden. Exercise every control in [UI foundation](ui-foundation.md), resize to 720 × 480 and back with ACP both closed and open, verify retained database preferences after close, inspect stderr, and record OS/GPU/GPUI/features. Simulated GPUI event tests are not manual macOS click-through or VoiceOver evidence. Verify the 1040 × 760 source dialog and its 780 × 560 minimum separately from the main window: repeated New Connection/Manage/center triggers preserve one draft/window with static close-current-draft feedback, Host/Port rows remain readable at 850 × 600 with long values and expanded keys, and main-window interaction remains possible. Exercise Name focus on form open, Cancel/Escape/Cmd-W/native-close draft cancellation and refusal to close while saving, Test without save, Save followed by metadata-only refresh without automatic row browsing, explicit Connect, optional database discovery, filtering/paging, clickable and Enter/Space ascending/descending/none sort cycles with retained filter and offset reset, loaded CSV native save/cancel/error and no-overwrite behavior, Edit, confirmed Delete, password re-entry after restart, TLS warnings and stale/error/cancel states. Delete must never remove server objects. Distinguish this manual evidence from verified driver fixtures; agent I/O remains unimplemented. General file browsing is excluded by product scope, not deferred.

## Performance and accessibility evidence

Treat budgets in [product plan](product-plan.md#proposed-release-gates) as proposed targets, not measured results. Record hardware, OS, build/features, fixture sizes, methodology, sample sizes, cold/warm runs, and p50/p95 results. Measure input latency/frame time separately from database/network latency. Assert memory bounds under slow consumers and oversize payloads.

Check every adopted text/background pair, focus/control boundary, state, and theme. Keyboard-only execution and scaled text must be exercised in the real GPUI app. Do not claim screen-reader support based on a layout or library choice; audit actual VoiceOver and later platform tooling.

## Completion definition

A milestone closes only with executable workflow evidence and failure-path tests on its supported targets. Record commands and outcomes, unresolved constraints, and any feature exclusions. Hosted CI, cross-platform UI, real servers, and real agents must not be reported passing until those tests actually run.

## Workspace tabs and query consoles

Historical local verification passed **93 headless Rust tests** (4 ACP, 44 app, 5 core, 40 drivers), **132 simulated GPUI tests** and **four Python bundle-helper tests**, formatting, both strict Clippy paths, debug build, plist lint, signature verification and bundled-license checks. The **23 unique live cases** also passed with query coverage: 7 direct/CONNECT/authentication, 10 TLS and 6 SSH. Repeated standalone authentication tests are not double-counted. Historical `cf24e48` passed all five jobs in [run 37215973701](https://github.com/yan-ad/dalan/actions/runs/37215973701); this is not current hosted evidence.

Headless coverage includes source/database/table deduplication, capacity with duplicate activation, monotonic consoles, close neighbors and parser allow/deny/budget cases. Driver rejection tests cover DDL/DML/CALL/SHOW/EXPLAIN/session SET, SELECT INTO, variables/locks, executable comments/hints, write CTEs and unknown/qualified functions, including quoted/commented semicolons and pathological nesting/operators. Validation occurs before opening a connection; rejected writes are never executed to prove rejection.

Simulated desktop coverage includes independent table/filter/grid-scroll/result state, tab close and request isolation, meaningful running/draft state, console context/database controls, selected-or-whole execution, prior-result SQL/elapsed/warning retention on failure, default Keep Open confirmation and editor selection/clipboard/undo/indent/visible-line behavior. A 1,000-line visible-shaping fixture is structural evidence only, not native frame-rate or IME/accessibility validation. Query grid headers remain unsortable and do not inherit table filters.

Use the existing locked headless and macOS desktop test commands above, Python bundle-helper suite, formatting, strict lint and bundle checks. The live inventory remains **23 unique cases**: 7 direct/CONNECT/authentication, 10 TLS and 6 SSH. Updated positive checks execute actual accepted console SQL through the native protocol on the scripts' own disposable MySQL/MariaDB containers and existing transports, not production accounts. Repeated standalone cases do not add to the unique count. Historical local and live results for that revision are recorded above; native Keychain was not rerun. Native UI remains a separate gate; hosted CI is skipped for this task.

Native manual visual/keyboard/IME/VoiceOver review, high-DPI/GPU performance, release/offline-Metal and positive system-trusted HTTPS proxy success remain open. Do not publish private profile, query or row values as fixtures/screenshots. Cmd-W must close an active tab and retain the shell window-close fallback with no tabs; the native OS close control is unchanged. See [query guide](query-consoles.md), [ADR 0004](adr/0004-workspace-tabs-and-read-only-consoles.md) and [security](security.md#query-console-defense-in-depth).


The final workspace regressions cover healthy duplicate-table reuse, replacement of a connection-invalidated table model under the same tab ID, isolation of neighboring tabs, and Cmd-W closing the window when the workspace is empty. Failed/canceled queries preserve old successful result SQL/elapsed/warning metadata rather than relabeling old rows. A shell-level Cmd-Shift-N test verifies exactly one console opens under the explicitly selected database, and Keep open supports Space/Enter with safe default focus.

## Source-manager redesign

**Historical source-manager verification passed.** These recorded results apply to that revision, not the current development-fix run:

| Suite | Historical total | Boundary |
| --- | --- | --- |
| Headless Rust | 105 unit tests: 4 ACP, 48 app, 5 core, 48 drivers; plus one native-wire test | Passed; No Auth handshake sends empty user/auth response |
| Simulated GPUI | 141 | Passed; not native visual/accessibility evidence |
| Python bundle helper | 4 | Passed; historical bundle-helper coverage |
| Expanded live fixtures | 29 unique: 11 direct/URL/socket/CONNECT/authentication, 12 TLS, 6 SSH | Passed; repeated standalone cases are not extra unique tests |

The older console revision's **23 unique cases** remains historical; the historical expanded 29-case run is separate. Run the existing headless/UI/Python commands in this guide and `./scripts/test-databases`, `./scripts/test-secure-transports`, `./scripts/test-ssh-transport` against owned disposable fixtures. Formatting, both strict Clippy paths, debug build, plist lint, signature and bundle-helper checks passed.

Repository tests use complete temporary files and cover reusable SSH version 1 persistence, bounds/private permissions, references, missing-reference failures, in-use removal and metadata-only paths. Source tests cover backward defaults, credential-free URL modes/rejection/IPv6, local-socket restrictions, No Auth Keychain bypass/remembered-credential removal and compensation, applied Options, exact schema names/empty selection/full cache generation behavior, and driver TLS identity validation. UI tests cover titlebar geometry/traffic-light reservation and blank native title, actual combos, tab traversal, draft/save guards, named SSH profile selection, manager lifecycle, simulated file selection, TLS warnings and loaded-page footer bounds.

Expanded direct fixtures include Unix socket and URL/option behavior; TLS fixtures exercise Required and VerifyCA behavior alongside trusted/rejected VerifyIdentity paths. Live SSH forwarding remains strict driver testing. **Positive mTLS is not live-tested**: compilation/path validation/driver wiring alone are insufficient. Positive Parse config handshake and manager successful remote `true` are not established: manager tests cover prelaunch/cancellation and simulated GPUI picker paths, not a successful live manager test. A forwarding-only bastion may correctly reject remote `true`. No encrypted-key built-in passphrase support is claimed.

Historical **`9ecae4c` passed all five hosted jobs** in [run 37255792022](https://github.com/yan-ad/dalan/actions/runs/37255792022). This is confirmed historical evidence, not the new revision's CI. Hosted CI is explicitly skipped for the current task. Native screenshot/window capture was previously blocked; no exact visual/titlebar match, manual source/manager test, accessibility/IME pass or performance measurement follows from simulation. No new native Keychain round-trip is claimed. Keep endpoint/account/private screenshot values out of documentation. Positive system-trusted HTTPS CONNECT, release signing/notarization and full native review remain open.

See [source-management contracts](source-management.md), [security](security.md) and [driver boundaries](drivers.md). The revision adds no new icons/packages or license selection; a direct `url` declaration uses the existing dependency graph and legitimate root lockfile changes are not evidence of a new package.


The MySQL No Auth live rejection can be a server 1045 or an unsupported decoy auth-plugin category: MySQL selects a built-in decoy plugin for unknown accounts and mysql_async does not implement sha256_password. The native mock-wire regression verifies empty username, zero-length authentication response, no supplied-password bytes and no fallback proof. Transport/TLS/timeouts are not accepted as authentication-rejection success. Page summaries report all configured 200 rows rather than cap their count at 100.

Hosted run 37266101943 exposed a Windows-only certificate-path fixture failure: Unix `/nonexistent/...` paths are not absolute Windows paths. The fixture now derives absent certificate/key paths from the platform temporary directory, without creating or reading files. Absolute-path validation and every CI check remain unchanged; the follow-up revision requires its own hosted run.

The Kit-backed Dalan.app historically launched from its app bundle. Native visual/VoiceOver/high-DPI and measured frame latency remain unverified. Production uses Kit runtime shaders for both debug and release; the legacy offline-shader option rejects explicitly, and optional Icon Composer still requires full Xcode. The separate pilot job has been removed; production/backend validation remains in the main workspace. Hosted CI is explicitly skipped for this task, not replaced with a claimed hosted pass.

## Historical separate SSH session workflow

The SSH/SSL source tab now uses a Kit **Enable SSH** checkbox, a saved-session picker, and an explicit **Manage SSH Sessions** button instead of default inline SSH fields. The manager owns reusable host/user/port/authentication/path settings; Apply updates the parent’s saved-session list without enabling SSH or choosing another session. Use Session saves then fills the parent draft. Existing inline profiles remain supported through a labeled legacy option.

New regressions cover required selection before Test/Save, retained selection across disable/re-enable, current metadata materialization and missing references, Space keyboard activation and minimum-window bounds, Apply publishing complete added/edited/deleted lists, Cancel isolation and Use publishing more than the chosen session. Fixtures use owned temporary files, synthetic host names and canceled-before-launch SSH tests; no private SSH keys, credentials or live servers are accessed. Local verification passed: **121 unit tests + 1 native-wire integration test**, **180 production UI tests**, **14 Python tests**, formatting, strict Clippy on both workspace and desktop/UI paths, and signed debug bundle/plist/signature checks. Hosted CI remains deliberately skipped. Live SSH authentication and visual certification were not performed.

## Session-only SSH and persistent identity header

Inline-only SSH transport is now rejected both in the source form and in metadata resolution; saved sessions are the only application connection path. Removed the inline SSH fields/actions/key picker and its unused filename-discovery module. No local settings or credentials are rewritten; experimental sources without a session require explicit selection. Shared transport remains the driver representation of resolved sessions, not an application fallback.

Name/Color live outside the scrolling tab body in a compact persistent header below the native traffic-light strip and above tabs. Color uses Kit’s default dropdown, preset swatches and custom hex editor. Regressions assert retained input values and identical header bounds across tabs/scroll, usable presets/custom editing, titlebar/header/tab ordering at normal and minimum window sizes, keyboard SSH activation, unavailable sessions and inline-only rejection. Prior SSH manager Apply/Use/Cancel/source-reference protection regressions remain intact. Local verification passed: **120 unit tests + 1 native-wire integration test**, **181 production UI tests**, **14 Python tests**, formatting, strict workspace/desktop Clippy and signed debug bundle/plist/signature checks. The count reduction removes the unused SSH key-discovery module’s two tests and adds an inline-rejection unit regression; remaining protections were not weakened. Hosted CI is skipped. No live SSH authentication or full native visual certification is claimed.

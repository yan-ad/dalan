# UI foundation

Implemented scope: macOS dark desktop shell, following the user's supplied DataGrip screenshot for chrome and sidebar organization, with restrained editor styling. Main content hosts the experimental MySQL/MariaDB browser/read-only table workspace; source setup uses a separate dedicated dialog window. This is not a completed SQL client or accessibility/theme system.

## Composition and purpose

```text
[ native window controls | Dalan   Database workspace          Layout ]
[ 6 px ][ Database Explorer ][ database browser / table view ][ optional ACP ][ 6 px ]
[ panel-left | UI foundation / focused-control help         | bot-message-square ]
```

- Native macOS traffic lights remain real OS controls. A transparent titlebar allows the compact custom top bar; its workspace-label region drags the window, and double-click requests the native zoom behavior.
- A 38 px top bar, 32 px pane headers, a separate 32 px explorer toolbar, and 32 px status strip retain the dense proportions of the reference. Main content has database browsing and bounded table results, but no SQL editor, welcome cards or gradients.
- The single Database Explorer sidebar defaults to 320 px, bounded to 200–480 px. Dark inset surfaces use 6 px structural corner radii, a 6 px divider gap, and 6 px outer padding on both sides, not floating cards or reserved rail space. The optional scoped right ACP panel is initially closed; there is no Files rail.
- System UI typography is a calm, dense 13 pt, with 12 px secondary copy and an 11 px status strip. No font assets or logo are introduced.
- Fourteen pinned Lucide SVGs provide utility glyphs: database, database-zap, plus, settings-2, refresh-cw, trash-2, panel-left, chevron-down, minus, bot-message-square, arrow-up, arrow-down, download and check. Local trash-2.svg is the unchanged upstream trash.svg alias from the same pin; see the [asset README](../crates/app/assets/README.md). No vendor-brand artwork, decorative AI sparkle or icon-font dependency.
- Database Explorer shows real saved sources and discovered databases/tables. No fabricated sources or results.
- The status strip identifies the development scope without implying a connected database or agent. Its bottom-right 28 px bot-message-square icon button, with AI · ACP tooltip/focus-help label, toggles the right panel. The panel says **Not connected**, with no ACP transport or agent launch implemented, no text prompt input yet, and no BYOK/provider settings.

The activity rail and header hide/minimise button are removed. Database Explorer has one 32 px title header without add text or a repeated title row, followed by a 32 px toolbar with four 28 px icon buttons. Add, Manage, Refresh and Remove have action-specific tooltips; no inactive DDL, console or advanced-option icons are shown. Source selection rows show name, readable engine label and abstract Lucide database/MySQL or database-zap/MariaDB cue, not vendor logos or branded driver icons. Optional color affects only the small marker; selection/focus and names stay independently readable.

With no sources, the main area centers a working Connect to a Source action, not a demo/trial welcome screen. It opens the same dedicated source dialog window by mouse, Enter or Space and remains available when Database Explorer is hidden. Loading and sanitized errors remain visible in the appropriate source/browse state.

## Controls

| Control | Behavior |
| --- | --- |
| Bottom-left 28 px panel-left | Toggle Database Explorer; retain the closed preference until toggled or reset |
| Explorer Add / plus | Open/reuse the dedicated source dialog window; disabled during saving |
| Explorer Manage / settings-2 | Edit selected source; disabled without selection or while busy/saving |
| Explorer Refresh / refresh-cw | Refresh selected source; disabled without selection or while busy/saving |
| Explorer Remove / trash-2 | Request confirmed local removal; disabled without selection or while busy/saving |
| Main Connect to a Source | With no sources, open/reuse the dedicated source dialog window by mouse or Enter/Space, even with explorer hidden |
| Layout | Open/close the layout menu |
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

Controls have hover feedback/tooltips, keyboard focus-help text in the status strip, and a high-contrast focus border/resize indicator. The menu focuses its first row on open and returns to the trigger on dismissal. Source add/edit/delete and connection controls are implemented; no generic search or version-control controls are added. AI · ACP exposes only panel visibility, not a working agent feature.

## Resize behavior

The window minimum is 720 × 480. Compact layout clamps Database Explorer toward its 200 px minimum while reserving at least 240 px for main content. At 720 px with ACP closed, the maximum effective sidebar width is 462 px: 720 minus 12 px outer padding, 6 px gap and 240 px content. The ACP panel prefers 300 px, capped by available space; its initial visibility is false. While ACP is visible, a compact window may temporarily hide Database Explorer. Database visibility and width preferences survive this temporary suppression and are restored after closing ACP or when space allows. Layout preferences are in-memory only; source profiles have separate versioned persistence.

A pane toggle changes the user's visibility preference, independently of width clamping. The bottom-left toggle's selected state reflects actual visibility; the menu's Shown/Hidden text reflects the retained preference.

## About Dalan

The native macOS application menu opens **About Dalan** in a separate 420 × 280 nonresizable GPUI window. It displays the Cargo package version, the name’s Javanese meaning “ways,” and database-workspace scope. It uses existing GPUI/native integration, not external libraries. The titlebar brand is capitalized **Dalan**; no brand/app icon artwork is introduced. Utility icons are distinct from app identity.

## Adopted dark tokens

| Role | Value | Reason |
| --- | --- | --- |
| Chrome | `#262729` | Match the reference's quiet outer frame |
| Inset surface | `#191A1C` | Separate work areas from surrounding chrome |
| Pane header | `#1F2022` | Compact hierarchy without a floating toolbar |
| Hover | `#35373B` | Pointer feedback, not a persistent decoration |
| Active selection | `#344C72` | Navigation state, paired with active border |
| Primary text | `#E6E8EB` | Readable small UI labels |
| Secondary text | `#B8BEC8` | Lower hierarchy without failing selected-state contrast |
| Focus | `#8AB4F8` | Single accent for actual focus/selection |

Text/focus pairings across chrome, surface, header, hover, and selected backgrounds are computationally checked. This does not establish screen-reader support, scaled-text resilience, or future syntax/status colors.

Minimum measured ratios across these backgrounds: primary text 7.06:1, secondary text 4.64:1, focus indicator 4.11:1. Focus uses the 3:1 non-text threshold and is not used as label text.

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

The historical blank-main state above is superseded by the source slice below. SQL editor work requires its own scoped milestone. Light/system theme, text scaling, and native accessibility remain required before release.

## Dedicated source dialog window

**Data Sources · Dalan** is one application-wide, resizable normal GPUI window: initial 1040 × 760, minimum 780 × 560. It is not an OS modal sheet and does not trap or block the main window. Add, Manage and the centered connection action activate/reuse it, preserving draft edits. A new form or model `form_generation` refresh (including loaded passwords) replaces the form inside the existing window and focuses Name; ordinary notifications do not reset it. Cancel/Escape/Cmd-W/native close discard the draft and cancel testing. Native close and Cmd-W refuse dismissal while credential/JSON saving is active. Model-observed successful Save closes the window, without auto-connect.

The form starts at Engine, without a redundant internal title strip. Database, SSH, HTTP and HTTPS Host/Port fields share horizontal rows, defaulting to localhost and 3306/22/8080/443 respectively. Port is 96 px wide, minimum 80 px; native-input Tab still moves Host → Port. The explicit body maximum is 720 px; zero minimum widths allow text truncation, while nonshrinking rows retain 28 px input and 30 px candidate heights. The viewport scrolls a natural-height body instead of vertically compressing it. The key list is capped at 150 px and long labels ellipsize.

The supplied screenshot may predate the full-width form, but it exposed a real flex-shrink defect, not merely an old sidebar layout. Draw-bound regressions at 1040 × 760 and 850 × 600 exercise long values, key labels and password/color changes, proving fields no longer collapse to single-character widths in simulation. Current local suites passed 54 headless, 63 simulated UI and four Python tests. Native visual/accessibility review remains unverified; existing capture attempts were blocked by Screen Recording permissions, which were not changed. See [testing](testing.md#source-dialog-window-and-windows-fixture-fix) for CI evidence and the pending next-main gate.

## Current source and read-view slice

Source setup has no separate Data Source/repeated-engine header. Password and its labeled Keychain checkbox share a row; the checkbox has an 18 px visible indicator and Lucide check rather than a Unicode glyph. CA path accepts both manual edits and a native single-file Browse action. Cancel preserves the path, stale selections do not replace newer manual edits, and focus returns to the path after the dialog. Native Open-dialog behavior remains a manual macOS check; shared completion and form control behavior have regression tests.

The main area remains the browser/table workspace. The dedicated Add/Edit Source dialog has engine, name, endpoint, user, optional database and password fields, plus transport/TLS choices. Database defaults to None, host/port to localhost:3306 and user to root; recommend a least-privilege account. Opening Add/Edit focuses Name; Escape cancels. SSH exposes an optional Known hosts file field requiring an absolute existing file. Test does not save. Save persists a profile without connecting; Connect on the selected source triggers real discovery. Delete is confirmed and removes local settings/Keychain only, not server objects.

Passwords are session-only unless saved explicitly to native macOS Keychain. After restart, Edit/re-enter a session-only password. Profiles use stable UUIDs and version 1 JSON without passwords. JSON/Keychain changes are not atomic across resources and compensation failures remain visible. Session-only Save is tested without a Keychain call, save failures are visible and a failed profile load blocks overwriting settings. The file limit is 1 MiB with at most 100 profiles.

Source-form traversal uses a persistent root whose stored Tab-stop state is false, a single tracked focus handle per input, and a form Tab group in visual order. All visible controls participate; keyboard focus can leave inputs instead of being trapped by duplicated nested handles. Database followed by Transport is intentional. Double-click (two or more clicks) selects all Unicode or password text; the ensuing drag does not shrink that selection. Password copy/cut is suppressed and native surrounding-text requests do not expose the secret. These protections do not establish native accessibility/IME validation.

SSH has an explicit background metadata-only identity picker for likely `$HOME/.ssh` files, preserving manual paths with no automatic selection and an explicit Use SSH agent choice. Candidate filenames are not proof of key format. There is no new passphrase prompt; unlock encrypted keys with `ssh-add` outside Dalan. Strict known-host checking is unchanged. See [MySQL sources](mysql-sources.md) for discovery limits and troubleshooting.

The table view offers 100-row pages and a column-cycle filter with Contains, Equals, NotEquals, greater than, less than, is null and is not null. Views are listed but unavailable for browsing. Stale rows remain visibly labeled on errors or filter changes, retaining the labeled previous page with pagination disabled until refresh succeeds. Source changes clear old rows. Primary-key order where available is not a cross-page snapshot. Column headers now cycle ascending/descending/none by click or Enter/Space, retaining filters and resetting offset. Loaded CSV uses a native save picker (default `Dalan-loaded-page.csv`) and fresh nontruncated rows only; visible feedback reports success, cancellation or errors. No arbitrary SQL, writes, full-query export or whole-table export are exposed. Transport, TLS, representation and backend bounds are documented in [MySQL sources](mysql-sources.md).

The input control adapts GPUI Apache-2.0 code with attribution. Lucide SVGs are explicitly permitted open-source utility assets pinned to `500620a2e8123f8d1db191538886dc0c223f69a9`. Complete ISC and retained Feather MIT notices are in [third-party notices](../THIRD_PARTY_NOTICES.md), alongside GPUI input Apache-2.0 attribution. The bundle includes these notices and the full Lucide license in Resources. The bot/message glyph represents chat-agent communication, not app branding. No reference advanced-options panel, marketplace, visual tools or vendor-brand artwork are copied. Dependency attribution does not choose a project license; that remains undecided.

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

Current gate: verify all five hosted jobs on the next main push, verify system-trusted HTTPS proxy success, then exercise real macOS source-dialog/browse keyboard and error/cancel flows. Light/system theme, scaled text and assistive-technology validation remain release requirements. Preserve the historical records and [initial validation](initial-validation.md) as evidence of their own iterations, not current compatibility.

[Overview](../README.md) · [Design direction](../DESIGN.md) · [Testing](testing.md) · [Development](development.md)

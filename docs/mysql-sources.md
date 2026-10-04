# Experimental MySQL and MariaDB sources

Dalan has a dedicated macOS source dialog window and a read-only table workspace. This is an experimental slice, not a general SQL client or a release compatibility promise. MySQL/MariaDB are first by explicit user direction, replacing the earlier PostgreSQL-first sequence. PostgreSQL and Redis remain future work; MongoDB remains after the first release.

## Create, test, save, connect

1. Build/open the experimental native app with `./scripts/macos --open` on macOS. Use the **Add** icon in Database Explorer or, with no sources, the main **Connect to a Source** action. The main action remains available with the explorer hidden and opens the same dedicated **Data Sources · Dalan** window by mouse or Enter/Space. Manage edits the selected profile. The resizable window starts at 1040 × 760, with a 780 × 560 minimum; the main browser/table workspace remains separate and usable. Repeated triggers activate the existing window without replacing its unsaved draft. New or refreshed forms focus Name.
2. Choose MySQL or MariaDB and a source name. Optionally set **Color (optional)** below Name: enter `#RRGGBB` or choose labeled Default, Blue, Green, Amber, Red or Purple swatches. Default means no custom color. The database endpoint defaults to `localhost:3306` and user `root`. Host and Port share one horizontal row for database, SSH (`localhost:22`), HTTP CONNECT (`localhost:8080`) and HTTPS CONNECT (`localhost:443`); Port is 96 px wide with an 80 px minimum. Host-to-Port Tab/Shift-Tab order is unchanged. Prefer a dedicated least-privilege SELECT account rather than root. Database is optional: None discovers available databases with `SHOW DATABASES`, then lets you choose a database and table.
3. Enter a password. It is session-only unless you explicitly choose to save it in macOS Keychain. After restart, a session-only source requires Edit and password re-entry.
4. Choose transport and TLS settings below. **Test** checks connectivity without saving the source. **Save** persists the profile and closes the dialog without automatically connecting. Cancel, Escape, Cmd-W and native close discard the draft and cancel its active test. Close/cancel is guarded during credential/JSON saving. This is a normal separate dialog window, not a true OS modal sheet.
5. Connect or expand a saved source to discover databases if not cached; open a database to fetch its table metadata if missing. The Tables group opens automatically and Views starts collapsed. Only BASE TABLE leaves can be browsed; view leaves are unavailable and do not issue a browse query.
6. Use **Manage** to edit the explorer-selected source. Explorer **Refresh** invalidates that source's catalog cache and reloads its root only; it does not reload the current table page. **Remove** confirms a captured stable source UUID and removes only local source settings and its saved Keychain password, never a server database or table. Managing or removing another source preserves the current table page.

Database Explorer now has only a 32 px toolbar with six 28 px icon actions: Add, Manage, Refresh, Remove, Expand Loaded and Collapse All. The Database Explorer title header is removed, as are the activity rail and header hide/minimise controls. The bottom-left 28 px panel-left toggle is unchanged; Cmd-B and native View/Layout alternatives remain. Closing the explorer retains that preference until toggled or reset. Actions have tooltips and selection/busy/save guards. No inactive DDL/console/advanced toolbar is added.

Source rows show the name, readable engine label and abstract Lucide database (MySQL) or database-zap (MariaDB) icon, not vendor logos. Custom color changes only a small marker and has no automatic production or risk meaning. Names and engines remain readable without color; arbitrary custom colors are not guaranteed WCAG AA. Loading and sanitized connection errors remain explicit; no sample/demo/trial source state is added.

Profiles have stable UUIDs and live in version 1 JSON at `~/Library/Application Support/Dalan/sources.json`. Passwords never belong in that file. JSON and Keychain updates are separate operations, not an atomic cross-resource transaction. Compensation failures are visible; a failed operation can require reconciliation rather than implying both resources changed together. Do not hand-edit credentials into JSON. The source file is limited to 1 MiB and 100 profiles; a failed load blocks saving rather than overwriting unreadable settings. Session-only Save is tested without calling Keychain.

### Optional color and legacy profiles

Color is stored as optional `color` in the existing version 1 JSON. `serde(default)` loads an absent field as None; opening legacy profiles does not automatically rewrite them. Valid `#RRGGBB` hex preserves exact letter case on round-trip. The source-field whitelist accepts color, but passwords remain rejected. Invalid color save/load does not overwrite stored bytes, and failed loads continue blocking saves. This metadata change does not affect Keychain or credentials. Scoped migration tests passed for exact legacy None, mixed-case hex and malformed-color no-overwrite behavior; the historical explorer redesign passed 54 headless, 57 simulated UI and four bundle tests; see [testing](testing.md#database-explorer-redesign-verified).

## Lazy catalog navigation

The compact tree uses 22 px source, database, Tables/Views folder and Table-icon leaf rows. Source counts appear only once databases load; folder counts appear only after metadata loads. Names are constrained and ellipsized with full-name tooltips, preserving Unicode and quoted identifiers via collision-safe row identities. Database discovery is lazy: connect/expand a source fetches `SHOW DATABASES` only when missing; opening a database fetches its tables only when missing. Tables automatically opens, Views starts collapsed, and views remain unreadable with no browse query.

Cache entries are keyed by source UUID and database. Other database expansions and cached metadata survive pagination, filters and source changes. Expand Loaded opens only cached branches and never issues a network fan-out; Collapse All preserves metadata. Collapsing a branch cancels only its catalog requests. Catalog jobs are independent of table-page and form jobs, with per-key generations/abort checks and at most two active requests; the newest request supersedes the oldest when capacity is reached. Source profiles are capped at 100, databases at 1,000 per source and tables at 1,000 per database; virtualization is not an unlimited catalog or process-memory promise.

Explorer-selected source identity is separate from the current table-page source. Refresh invalidates that explorer source's cache and reloads the root only, leaving the table page untouched. Manage/Remove on another source preserve the page; removal confirmation captures a stable UUID. Loading/errors use per-branch status dots with full sanitized error tooltips, while the default global error remains available. Errors retain nonsecret endpoint context, not credentials or raw server payloads. See [keyboard controls](ui-foundation.md#compact-lazy-explorer) and [structural tests](testing.md#compact-lazy-explorer).

## Transport and TLS

Transport carries the MySQL wire protocol. TLS for the database is a separate choice and retains the original database hostname even through a relay.

| Transport | Configuration and boundary |
| --- | --- |
| Direct TCP | Connect to the database host/port directly. |
| SSH | System `/usr/bin/ssh -W`, strict known-host checking, key/agent authentication and BatchMode. SSH endpoint defaults to `localhost:22`, user to OS `USER`; key file is optional. A selected identity enables `IdentitiesOnly=yes`. No SSH password prompt. The optional Known hosts file must be an absolute existing file. Establish host trust outside Dalan; do not bypass known-host verification. |
| HTTP CONNECT | Anonymous proxy, default `localhost:8080`, tunneling the database wire protocol with CONNECT. |
| HTTPS CONNECT | Anonymous proxy, default `localhost:443`, CONNECT over TLS to the proxy. Database TLS remains independent. |

HTTP/HTTPS here are not a database-query HTTP gateway. There is no universal SQL-over-HTTP endpoint. A gateway integration requires a named service and its protocol/authentication contract; it is not implemented by these proxy options. Proxy authentication is not implemented.

**VerifyIdentity** is the default database TLS mode. A custom CA file is optional. Verification failures do not trigger a plaintext fallback. Local MySQL installations may not have a certificate trusted by the host: configure an appropriate CA and hostname, or explicitly choose Disabled only for a disposable/local localhost setup and heed the warning. Disabled is an explicit insecure override, not an automatic compatibility fix. Trusted custom-CA database TLS succeeds over direct TCP and HTTP CONNECT in disposable fixtures. Wrong database hostname, untrusted database CA and untrusted HTTPS proxy certificates are rejected. Actual SSH transport reads and host-key/identity rejection passed. Positive HTTPS CONNECT using a proxy trusted by system/native CA roots remains unverified; the database CA field does not configure proxy trust.

The CA path remains manually editable and has a **Browse…** button that opens a native, single-file macOS dialog. Cancel leaves the path unchanged; selecting a file fills the same field and returns keyboard focus to it. A late selection does not replace a path edited while the dialog was open. Selecting a file configures its path only; certificate parsing/trust validation still happens during Test Connection.

**Save in Keychain** is a labeled checkbox directly beside Password, not a separate Credentials row or a Unicode checkbox glyph. Its visible square/check state, label click, Space/Enter, and focus border share the same persisted `save_password` setting. The source form starts at the Engine row; redundant Data Source and repeated engine title text are removed. Compact layouts can wrap the inline checkbox below the password field instead of overflowing.

In SSH mode, the explicit identity-picker button lists likely files from `$HOME/.ssh` on a background task. It reads filenames and metadata only, never private-key contents. Candidates match `id_*`, `.pem`, `.key`, or a companion `.pub`; public keys, configuration/trust files, hidden files, directories and symlinks are excluded. Discovery is bounded to 512 entries and 128 candidates. Names are not proof of valid key format. Nothing is selected automatically, manual paths are preserved, and **Use SSH agent** is an explicit choice. There is no new key-format validation or passphrase prompt: unlock encrypted identities outside Dalan with `ssh-add`. Strict host trust remains unchanged.

The optional SSH `known_hosts_file` is backward-compatible: absent/None retains OpenSSH default user/system known-host files. `StrictHostKeyChecking=yes` always remains enabled. Selecting a file sets `UserKnownHostsFile` to that path and `GlobalKnownHostsFile=/dev/null`, making the selected file authoritative. A live fixture verifies paths containing spaces. Dalan does not modify user SSH trust files; default mode can still read system/user trust.

Relay transports use an unauthenticated loopback listener that accepts the first connection. Another process under the local host account can race it. This is not a sandbox or protection against a compromised host. Cancellation closes owned tunnels/relays; stopping client work is not proof of a server-side abort.

## Connection troubleshooting

A connection failure is not necessarily a bad password. The reported remote route remains unconfirmed: an unauthenticated public-greeting probe established TCP, then received a reset before the MySQL greeting; it sent no credentials. That evidence does not establish account validity or identify the cause. No private endpoint or account details are recorded here.

1. Compare the working DataGrip configuration exactly: route (direct/SSH/proxy), database host and port, TLS mode and CA/hostname, selected database and user. A screenshot alone does not establish every setting. An optional database may be blank; required fields cannot be blank. After Database, Tab legitimately reaches Transport.
2. Use **Test** with those settings, without auto-selecting another transport, authentication method or insecure TLS fallback. Disabled TLS is explicitly represented by `.ssl_opts(None)`; it was already the effective previous default for that mode, not an identified root cause. VerifyIdentity remains the normal default.
3. Retry in the updated app and report the **exact sanitized diagnostic**, including a server code such as 1045 if shown. Do not paste passwords, profile dumps, SQL, raw server messages or private host/account details. A 1045 result is an authentication/grant denial, not proof that changing the password is the right fix.
4. A reset before the greeting calls for checking the endpoint/route and server-side access policy with the administrator. Verified-TLS errors require correct trust and hostname settings; unsupported authentication needs server/account compatibility review, not a legacy or plaintext fallback.

Diagnostics use typed I/O kinds and fixed descriptions for server codes including 1045 (authentication), 1044 (database access), 1049 (unknown database), 1130 (client-host grant), 1251 (authentication protocol), 3159 (secure transport required) and 1820 (expired password). Unsupported plugins and unavailable SSL are explicit failures. Raw server messages, payloads, SQL and credential names are not included. A successful disposable fixture does not confirm the reported remote account works.

## Browse, filter and sort

The UI provides 100-row pages. The backend caps requests at 200 rows, cells at 4 KiB, retained page data at 2 MiB, columns at 512, and database/table discovery at 1,000 entries each. The driver has an actual 8 MiB **per-packet** limit. This is not an absolute process-memory bound: protocol assembly, decoding, metadata and runtime allocations exist outside retained-page accounting.

Columns cycle through the filter control. The seven operators are Contains, Equals, NotEquals, greater than, less than, is null and is not null. Values are bound parameters, not SQL interpolation. Contains escapes LIKE metacharacters with `!`. Comparisons retain server type/collation semantics; null operators do not need a value.

Click a column header, or focus it and press Enter/Space, to cycle ascending, descending and no explicit sort. Changing columns begins ascending. Sort changes retain the filter and reset the offset to zero; the previous page is labeled stale until refresh succeeds. The driver validates the sort column against discovered metadata, quotes its identifier and whitelists direction. There is no raw SQL sort input; filter values remain bound parameters. Explicit sorting adds available primary-key columns as ascending tie-breakers. With no explicit sort, generated reads use primary-key order where available. Without a primary key ordering/pagination can be unstable; even ordered offset pages are not a snapshot and can shift under concurrent changes. `next_offset` advances by rows actually retained, including when byte limits truncate a page. No total-row or snapshot guarantee is implied.

NULL is distinct from empty text; binary data displays as hex, MySQL JSON as text, and decimals/large integers as strings rather than lossy floating point. Errors or superseded results do not silently become fresh data: retained rows are labeled stale and pagination is disabled until a successful refresh. Changing a filter retains the labeled previous page while refreshing; changing the source clears it. Loading, cancellation and failure are visible states.

There is no arbitrary SQL console, write action, staged grid editing, or transaction-control UI. Generated browsing uses read-only transactions where server capabilities permit. This does not secure a broadly privileged account; use least-privilege server roles.

## Export loaded CSV

Use **Export loaded CSV** (Lucide download) on a fresh, complete loaded page. It opens the native save picker with `Dalan-loaded-page.csv` as the default name and writes only the currently loaded UI page of up to 100 rows. It does not fetch more rows, export a whole table or rerun a query. Missing, stale, truncated or busy pages cannot be exported. Success, cancellation and errors are shown in the table view. The chosen filename is accepted as-is: output is CSV, but the native picker currently does not enforce a `.csv` extension.

CSV is UTF-8 without a BOM, with a header, commas, doubled embedded quotes and CRLF record separators. Non-NULL values are quoted; NULL is unquoted `\N`, and literal text `\N` is quoted. Ordinary CSV parsers often discard quote information, so they can lose this NULL/text distinction. Binary and temporal values use loaded display strings, not raw database bytes. Empty results export headers only. Truncated values are rejected rather than advertised as complete.

Spreadsheet-safe mode is on by default. It prefixes formula-risk textual values and headers with an apostrophe, intentionally changing their content; quoting alone does not stop formulas. Decimal numeric strings are validated lexically without numeric conversion, retaining exact large-integer/decimal representation in the file. Spreadsheet import can nevertheless round numbers or alter formatting. This is not a promise of lossless spreadsheet round-trip.

Existing files and symlinks are never overwritten: choose a new name. A private staging file in the destination directory is synced and atomically published using a same-filesystem hard link; Unix creation mode is `0600`. Unsupported hard links fail explicitly. These portable path operations do not defend against hostile directory replacement; a cleanup error after publication can leave a complete destination while reporting an error. Encoding is bounded to 8 MiB, 512 columns and 200 backend rows, separate from the 100-row UI scope.

A source/table/page generation change before the picker returns cancels without writing. Once writing begins, it uses the captured loaded-page snapshot, not a newer selection; a later selection change does not retroactively change or cancel that file. This is a local captured-page export, not a database snapshot or full-query export.

## Scope and evidence matrix

| Capability | Current scope | Evidence / remaining gate |
| --- | --- | --- |
| Profile persistence | Version 1 JSON, stable UUID, no password; 1 MiB / 100 profiles | Headless persistence/failure tests and session-only Save without Keychain verified |
| Credentials | Opt-in native macOS Keychain, session-only alternative | One generated native Keychain round-trip passed, with item cleanup |
| MySQL/MariaDB | Test, discovery, columns, BASE TABLE browsing, seven filters | MySQL 8.4.11 and MariaDB 11.4.13 verified on the same disposable fixture |
| Column sorting | Ascending/descending/none, metadata validation, retained filter, offset reset, primary-key tie-breakers | Headless/generated-SQL and simulated UI coverage; live sorting rerun passed across direct, CONNECT, TLS and SSH routes |
| Loaded CSV | Native save picker, fresh complete loaded page only, no overwrite | CSV/file and simulated picker tests; native manual save flow pending |
| Direct TCP / HTTP CONNECT | Implemented | Live fixture success verified |
| VerifyIdentity default | Implemented, no insecure retry | Trusted custom-CA direct/HTTP CONNECT success, wrong-hostname and untrusted-CA rejection verified |
| SSH | Strict selected/default known-host trust, keys/agent | Two actual transport reads, two wrong-host-key and two wrong-identity rejections passed |
| HTTPS CONNECT | Proxy TLS independent of database TLS | Two untrusted proxy rejections passed; trusted system-CA proxy success unverified |
| Views | Listed, not browsable | Live rejection verified |
| PostgreSQL / Redis | Planned | No working adapter |
| MongoDB | Later, after first release | No working adapter |
| ACP | Disconnected panel and schema boundary only | No agent process, transport or context sharing |

Six live smoke tests passed: the two servers cover direct/HTTP CONNECT reads, all filter operators and fixture value representations, view rejection, and default-TLS rejection. These do not validate every authentication plugin, topology, managed service, TLS configuration or transport. The historical source-slice suites passed 37 default headless Rust tests, 27 simulated GPUI tests, four Python bundle-helper tests and one generated native Keychain round-trip. Ten secure-transport tests passed (five per engine): trusted database TLS direct and HTTP CONNECT reads, wrong-hostname and untrusted-CA database rejection, and untrusted HTTPS proxy rejection. Six SSH tests passed (three per engine): an actual transport read, wrong-host-key rejection and wrong-identity rejection. Fixtures did not modify user SSH or OS CA state. Positive system-trusted HTTPS proxy success remains unverified.

The previous icon/sorting/export suites passed 48 headless and 42 simulated UI tests, four bundle-helper tests, one generated Keychain round-trip and 22 live transport cases with sorting coverage. Bundle build/signature/license checks passed. No additional native manual interaction is claimed; see [testing](testing.md).

The connection-UX revision reran **23 unique live cases**: seven direct/CONNECT/authentication cases, ten TLS cases and six SSH cases. The fresh MySQL `caching_sha2_password` first-login test also passed standalone; that repeat is not an additional unique case. Earlier readiness checks authenticated the reader and warmed its authentication cache. Readiness now uses the fixture root account and provisions a new account afterward, so the first tested login exercises uncached RSA authentication with TLS disabled. This does not cover every plugin or prove the reported remote account is fixed. Final results: 52 headless, 46 simulated UI and four Python tests passed. Formatting, strict lint and debug bundle plist/signature checks passed. The earlier Keychain pass remains historical; no new Keychain run is claimed.

Run disposable fixtures with `./scripts/test-databases` after starting a Docker-compatible runtime. It uses `mysql:8.4` and `mariadb:11.4`, random localhost ports, a SELECT-granted fixture user, and cleans up only its own containers/volumes. Image tags can resolve to newer patches; record exact versions on every run. Never point these tests at production. `./scripts/test-secure-transports` and `./scripts/test-ssh-transport` separately opt into disposable TLS and actual SSH fixtures. They require Docker plus the local certificate/key tooling, use temporary directories and generated keys, and clean up their owned containers/volumes; the SSH helper also removes its dedicated network and generated SSH image. Pulled database images are not removed. No OS CA installation or user known-host edits are required.

[Overview](../README.md) · [Drivers](drivers.md) · [Architecture](architecture.md) · [Security](security.md) · [Development](development.md) · [Testing](testing.md)

Source-dialog regression tests verify nonshrinking fields, footer and SSH candidate rows at 1040 × 760 and 850 × 600, with long values and password/color changes. The body is at most 720 px wide and scrolls at its natural height; key lists are capped at 150 px with ellipsized labels. Historical source-dialog local totals are 54 headless, 63 simulated UI and four Python tests; native visual review remains unverified. See [source-dialog evidence and pending main CI](testing.md#source-dialog-window-and-windows-fixture-fix). No storage, transport, dependency, icon or license change is introduced.

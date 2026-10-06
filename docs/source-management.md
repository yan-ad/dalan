# Source management

Main-workspace development fixes add **cached-only explorer search**, using a thin Kit Input wrapper. It finds source/engine/database/table/view names in collapsed branches while preserving ancestors and retained expansions, without discovery, credentials, database selection changes or SQL execution. A lazy index covers the 50,000-item fixture; scrolling does not flatten all metadata. Escape clears focused search, rather than closing an unrelated source menu; the 256-character matching cap is not an Input byte-limit guarantee. Unsupported routines are not fabricated. The console's separate database selector is an actual searchable Kit Combobox with value-safe optional-string choices; this does not change the source form's dropdown or saved SSH popup alternatives.

Optional Bacon preview rebuilds/restarts the owned native app, not hot reload. Unsaved forms and session-only passwords are lost; SaveForever credentials are local plaintext in `dalan.auth`. Other app instances are not killed by name. Source/profile/cache/credential persistence contracts remain unchanged. See [development lifecycle](development.md#bacon-live-preview) and [current local evidence](testing.md#native-multiline-result-crash-and-development-fixes). The previous development-fix counts are recorded in the linked evidence; hosted CI is explicitly skipped for this task.

Status: implemented experimental MySQL/MariaDB/PostgreSQL/MongoDB/Redis native configuration subsets (see the [support matrix](native-drivers.md)), not an exact replica of DataGrip or its native/JDBC properties. Dalan uses Kit standard controls and Kit's default theme with compact app-owned layout. Driver/authentication choices are Kit dropdown menus; saved SSH choices are Kit popup lists, not claimed Combobox/Select usage. Kit InputState owns editing/IME/selection; a thin value adapter and native password surrounding-text privacy handler preserve model/safety contracts alongside Kit masked clipboard protections. Current native/UI validation remains gated. Source-control relocation does not change engine glyphs, provider assets or attribution. See [setup and browsing](mysql-sources.md), [security](security.md) and [migration](gpui-kit-migration.md).

## Source window and actions

Titlebar **New Connection** opens a Kit dropdown with **Create Manually** and an **Import** section offering **Import from DBX**, **Import from Navicat NCX** and **Import from DataGrip**. Manual creation, row **Manage / Copy** and **Connect to a Source** use one retained independent, resizable source window without replacing an unsaved draft. Creation works with the sidebar hidden because the root retains the source-model subscription and dialog lifecycle. New Connection combines the existing plus glyph and explicit label at 28 px high immediately right of the Database collapse toggle at x = 84 px. Pointer, Enter and Space activation, tooltip and loading/saving guards apply; no explorer plus button remains.

The unified **Data Sources and Drivers** SourceDialog starts at **1160 × 760**, minimum **1040 × 560**, reserving width for its new sidebar. It is not a modal sheet or a main-window focus trap. A 34 px native drag strip protects traffic lights. A **48 px icon rail** switches between **Sources**, **SSH** and **Drivers**; Sources has a **228 px source list** and a right-hand editor. The right-hand Name/Color identity header and General / Options / SSH/SSL / Schemas tabs remain persistent above the scrolling body, with a persistent Test Connection/status strip and a global **Cancel / Apply / OK** footer. Kit open_window/Base Root owns overlays; Kit default styling is not a pixel-identical reproduction of a supplied screenshot. No unimplemented Add comment, templates or Advanced buttons are offered.

### Row actions and copy boundaries

- The explorer toolbar has exactly three 28 px icons: Refresh selected source, combined expansion toggle (`toggle-tree-expansion`) and the existing New Query Console action. Refresh uses the explicit explorer-selected source and retains old metadata on failure. If any source, database or group is expanded, the toggle offers Collapse All; otherwise it offers Expand Loaded. Expansion uses cached branches only, never network fan-out; collapse preserves metadata.
- Each source has an 18 px Manage gear trigger, `source-actions-{id}`. Pointer or Enter/Space opens a popover near that row using real model/layout bounds, not a fixed global anchor. Its exact **Manage / Copy / Remove** actions capture the row UUID, not global explorer selection. Tab/Shift-Tab and Up/Down traverse the menu; Escape and outside dismissal restore focus. Saving disables actions; existing busy/loading guards remain.
- **Manage** opens that profile in the same SourceDialog with General, Options, SSH/SSL and Schemas. Existing credential/edit rules are unchanged. Source-window navigation is independent of main-browser selection.
- **Copy** creates an unsaved editable draft with a fresh UUID. Its name appends ` copy`, truncating the original at valid UTF-8 boundaries to fit the 256-byte name limit. It clones endpoint, authentication mode/account, schema visibility, Options, TLS/client paths, color and SSH reference/settings, but no password, session secret or saved credential. `save_password` is false; Copy performs no credential retrieval.
- Copy leaves the original saved JSON, results and schema caches unchanged. Cached database choices are copied into form memory only so Schemas selection remains useful. No copied-source SQLite metadata is registered or persisted until normal source Apply/OK; applying commits the new profile and starts fresh metadata discovery. There is no new copied-data-source service.
- The source list contains saved and new rows with provider icons. **Add / Cmd-N**, **Duplicate / Cmd-D** and confirmed **Remove** operate on source drafts; the existing 100-profile limit remains. Each source retains its own form entity, including invalid field text, input selection, session passwords and active tab. Switching rows does not discard drafts or select the main browser. Canceling an unapplied duplicate creates no saved profile. Form-route guards invalidate old Test Connection and credential callbacks when navigation changes; stale completions cannot overwrite the newly active form.
- **Remove** confirms the captured stable UUID even if selection changes after opening the menu or confirmation. It removes local profile settings, associated credentials and cached metadata only, never database objects. Unrelated source results remain unaffected.

No new dependencies, branded logo or utility assets are introduced. The 22 px virtual tree, cached projection and viewport-only rendering, including the 300-cell integrated grid fixture, retain their existing contracts; this is not a new native performance measurement. See [UI controls](ui-foundation.md#controls) and [pending verification](testing.md#relocated-source-controls).

The shared header owns Name and optional marker Color; General owns the five built-in engine and authentication dropdowns. Tab/Shift-Tab traverses the active form. **Test Connection** tests without storing it. **Apply** commits only the active source, stays open and starts metadata-only discovery, never automatic row browsing. On the SSH page, Apply saves the manager catalog instead; neither operation is a batch-atomic save of all source drafts. **OK** applies the active source, then closes only if no other dirty drafts remain; otherwise an explicit discard/keep prompt protects them. Cancel, Escape, Cmd-W and native close require explicit confirmation before discarding dirty drafts and cancel owned tests on dismissal. Busy saves block switching and close.

The **Drivers** page provides built-in MySQL/MariaDB/PostgreSQL/MongoDB/Redis information only. It does not install JDBC drivers or host plugins. **JDBC is not implemented**; the native three-engine additions precede optional worker/JDBC support. Imports still accept only MySQL/MariaDB; driver menu support does not widen import support.

Passwords remain session-only by default, with no credential write. The former **Save in Keychain** option is removed: checking **SaveForever** explicitly opts into local **unencrypted plaintext** password storage, not an OS vault. `dalan.auth` is JSON version 1 with a `credentials` map from source UUID to password, separate from password-free `sources.json`, in the same platform support directory. The backend no longer depends on `keyring` and does not access, migrate or delete old Keychain items. An old remembered-password profile without a local saved credential requires re-entry. No secure memory-erasure guarantee is made.

New auth directories/files use Unix `0700`/`0600`; permissive auth parent directories/files are rejected rather than silently repaired. Auth paths reject symlinks and nonregular files; Unix auth files also reject hard links. Limits are **1 MiB per auth file, 100 credentials and 64 KiB per password**. On Windows, privacy relies on inherited user-directory ACLs: those ACLs are **not enforced or validated** by this implementation. These checks are neither encryption nor same-account isolation and cannot rule out hostile concurrent path replacement.

**Show / Hide** temporarily reveals the password for **three seconds** as presentation only. Copy/cut remain blocked and native surrounding-text extraction remains hidden even while revealed; text, undo history and caret/selection are retained. This is not native OS/IME/accessibility privacy certification. Profile/auth changes use compensating rollback, not a transaction; metadata commits separately. Connector export excludes credentials. See [storage locations and security](security.md#credentials-and-persistence).

## Connector import and export

The native **File** menu offers **New**, **Import > Connectors List**, the foreign **Import from DBX / Import from Navicat NCX / Import from DataGrip** choices, and **Export**. These File actions work from both the main window and the unified SourceDialog. New creates a manual draft; imports use a native picker for **one explicitly selected file**, not directory scanning or automatic vendor-profile discovery.

Imports are bounded to **1 MiB and 100 entries**, support only MySQL/MariaDB, and create **unsaved review drafts with fresh UUIDs**. Existing drafts are retained and the source-list capacity still applies. Passwords are never imported, `save_password` is false, and import does not recover credentials from local Keychain or connect to a database. Review warnings and local paths, enter credentials yourself, then explicitly Test or Apply the selected source; import itself is not persistence or a successful connection test.

| Import choice | Supported file subset |
| --- | --- |
| DBX | Plaintext JSON with a `connections` array, or a bare array of connections. Encrypted DBX exports are rejected. |
| Navicat NCX | XML `Connections` / `Connection` attributes, decoded as UTF-8 or UTF-16. DTDs are forbidden. |
| DataGrip | One selected `dataSources.xml` file containing supported data-source settings. No companion local credential files or Keychain recovery. XML DTDs are forbidden. |
| Connectors List | Native JSON envelope with `format: "dalan-connectors"`, `version: 1` and a `profiles` array. This is an interchange file, not a replacement `sources.json` repository. |

Foreign SSH, proxy/HTTP-tunnel settings and read-only policies that cannot be represented are **skipped fail closed**, never silently converted to Direct or treated as an enforceable read-only guarantee. Unsupported drivers and unsafe entries are skipped with warnings; a file with no safe supported entries fails. Security settings not represented by the importer default to **VerifyIdentity** with review warnings, not an inferred vendor-equivalent policy. Credential-bearing URLs and unsupported URL options/properties are rejected. These parsers implement bounded format subsets, not full compatibility with every vendor/version.

Native lists retain supported proxy settings on round trip. Native SSH references are retained only when they contain an already-valid saved-session UUID reference and materialized transport; SSH session definitions are **not** included in the export. Import rejects inline SSH. A missing local saved reference fails closed and must be repaired by selecting a saved SSH session before Apply/Test; an exported snapshot is not a substitute for that session.

**Export** includes saved profiles only and ignores all unsaved drafts. It includes no credentials or local private-key contents and does not read `dalan.auth` or Keychain; certificate/key **paths remain**, so the file still contains sensitive endpoint/account/path metadata. Output uses the native version 1 envelope above. Choose a **new file**: publication is atomic without overwriting an existing destination, and symlink destinations or ancestors are rejected. These checks do not guarantee protection against hostile concurrent directory replacement. See [security boundaries](security.md#connector-interchange-boundaries) and [verification status](testing.md#connector-import-and-export).

## Persistent source identity

**Name** and **Color** share a compact, non-scrolling header above General / Options / SSH/SSL / Schemas. They remain at the same position as tabs change or the tab body scrolls. Color is a Kit dropdown with a current-color swatch, Default/five presets and custom `#RRGGBB` input; changes belong to the same unsaved source draft. Native traffic lights have their own 34 px drag strip above this header. Identity settings are no longer General-tab rows.

## General: endpoints and authentication

| Connection mode | Behavior |
| --- | --- |
| Default | Customizable inline Host/Port, initially `localhost:3306`. Optional Database restricts discovery to that database. |
| Unix Socket | Absolute local socket path, entered manually or with a native file picker. Unix only; Direct transport and TLS Disabled are required. Validation rejects incompatible settings rather than silently downgrading them. |
| URL-only | Credential-free `mysql://` or `mariadb://` URL, optionally prefixed by `jdbc:`. Parsed host, port and optional database override the separate endpoint fields. IPv6 and percent-encoded database names are supported. |

Default mode shows a generated credential-free URL synchronized with Host, Port and Database. Editing this URL switches to URL-only. URLs reject **all** userinfo (including username-only), query parameters and fragments; JDBC connection properties and arbitrary driver properties are not supported. Credentials belong in the authentication controls, not the URL. Rejection diagnostics are static and do not echo supplied URI credentials. Do not embed credentials or arbitrary properties in JSON either.

**User & Password** uses the explicit username and password. **No Auth** hides Password and supplies neither username nor password to the driver. It is an intentional mode, not a fallback after login failure; server policy can still reject it. No Auth never reads saved credentials and ignores old remembered credentials. Saving it forces `save_password = false` and removes a previously remembered password through the existing compensating-save/rollback path. An old version 1 profile with No Auth and `save_password = true` can load, but that obsolete credential policy is ignored, not used for login.

Database remains nullable. With no default database, discovery covers the databases visible to the configured driver account. This does not grant access to databases the account cannot use.

## Options: applied limits

| Option | Range | Default | Applied to |
| --- | --- | --- | --- |
| Connect timeout | 1–60 seconds | 10 seconds | Connection establishment, including the configured route |
| Query timeout | 1–120 seconds | 20 seconds | Metadata query steps, table work and restricted console runs |
| Page size | 1–200 rows | 100 rows | Browse pages and bounded console results |

Console runs use the configured client deadline and server setting: MySQL `MAX_EXECUTION_TIME` in milliseconds or MariaDB `max_statement_time` in seconds. Cancellation closes owned client work, not an acknowledged server KILL. Complete catalog discovery still has an independent **120-second overall cap**, one serial connection/tunnel and existing object bounds; connection establishment uses Connect timeout and each database-name/table-name query uses Query timeout. There is no extra fixed 20-second step cap when a larger Query timeout is selected.

The loaded-page footer reports actual retained rows and the configured page size; the backend maximum remains 200. Neither that footer nor `has_more` invents a total row count. Display-byte, packet, column and retention bounds still apply.

## SSH/SSL: transport and reusable SSH profiles

This tab contains actual **Transport** settings, not simulated SOCKS controls. Existing Direct, SSH, anonymous HTTP CONNECT and HTTPS CONNECT routes remain available. CONNECT tunnels the database wire protocol; it is not a SQL-over-HTTP API. SOCKS and proxy authentication are not implemented.

For SSH, check **Enable SSH**, then select a saved session by readable name and endpoint, not raw UUID. **Manage SSH Sessions** opens the embedded SSH page in the same window and is available even before enabling SSH. New SSH drafts show **Select SSH session…**, not editable inline host/key settings or an implicit localhost tunnel. Test and source Apply/OK reject enabled SSH without a selected session. Disabling SSH uses Direct and omits the saved reference from the submitted source; re-enabling it retains the draft’s previous session choice. SSH and CONNECT are mutually exclusive routes, not nested tunnels.

The local forwarding port is dynamically allocated and read-only; there is no fixed local-port setting. **All SSH connections require a saved session.** Inline SSH host/key fields, Custom fallback and local key-discovery picker have been removed. Host/port/user, authentication, identity and known-hosts paths and Parse config belong exclusively in Manage SSH Sessions. An old experimental inline-only source remains visible but requires session selection before Apply/OK/Test/connecting; no automatic migration or private local-file changes are performed. Unix socket connections remain local and cannot enable SSH.

### Manage SSH Sessions

The embedded manager owns retained drafts, keyboard traversal and saving guards. Rail switching preserves pending manager edits. It supports:

- **Add** a session; **Duplicate** with a new identity; confirmed **Remove**; **Apply** to save without closing or enabling SSH. Apply/Saved publication refreshes the persisted SSH catalog in all retained source forms, preserving their selected sessions and routes. Unapplied manager drafts are never published.
- **Use Session** saves the sessions, enables/selects the chosen SSH route in the active source draft and returns to Sources. It does **not** automatically save the database source.
- **Back** discards pending SSH manager drafts and returns to Sources. Global Cancel/Escape/Cmd-W/close protects dirty drafts with explicit confirmation and cancels the owned test on dismissal; saving blocks switching and close.
- Agent or KeyPair authentication; host, port and user; an identity-file manual path and native picker; optional known-hosts file; explicit **Parse config** checkbox.

Removal checks references in saved source JSON and refuses to remove an in-use configuration. Unsaved parent drafts are not persisted references: a missing reference must be repaired before connecting or saving.

Reusable metadata is stored in version 1 `~/Library/Application Support/Dalan/ssh-configurations.json`: at most 100 profiles and 1 MiB, with Unix file `0600` and directory `0700`. It contains paths and settings, never passwords, passphrases or private-key contents. These permissions are not encryption or protection against another process under the same OS account.

A source stores `ssh_configuration_id` as a UUID reference **plus** a materialized transport snapshot for the driver. Startup, Test Connection and source Apply/OK resolve the latest reusable configuration. Connection paths resolve current referenced settings, so editing a reusable profile affects sources that reference it. Missing references and inline-only SSH transports fail closed; they never silently use the old snapshot. Offline cached trees remain available with a metadata warning. Refresh can update the resolved SSH identity in memory; it does not rewrite source JSON until source Apply/OK. Cache registration belongs to startup/source Apply/OK, not a late worker completion that could resurrect a deleted source.

### SSH security and Test

OpenSSH `/usr/bin/ssh`, strict `StrictHostKeyChecking=yes`, BatchMode and agent/key authentication remain required. A selected key enables `IdentitiesOnly=yes`. With a selected known-hosts file, that file is authoritative and global trust is disabled; otherwise OpenSSH's user/system known-host files remain in use. Establish trust outside Dalan.

**Parse config is off by default** for sessions: commands use `-F /dev/null`. Opting in permits local OpenSSH configuration, whose `ProxyCommand` and `Match exec` can execute local commands. Only enable trusted local configuration. This opt-in does not relax strict host verification.

The manager's **Test** invokes strict batch SSH with remote `true`, not a database test. It is bounded to 20 seconds. Cancel, close and input changes kill the owned child; executable arguments are passed separately, with no shell interpolation, including paths containing spaces. No cleartext SSH password is supplied. A bastion restricted to port forwarding can reject remote `true` even when database forwarding works.

Encrypted keys must be unlocked externally using `ssh-add` and an external agent; there is no built-in passphrase dialog or passphrase persistence. SSH password authentication and PuTTY private-key format are unsupported; convert to an OpenSSH-compatible key outside Dalan. Manager prelaunch/cancellation and simulated picker tests are not evidence of a live successful manager `true` handshake; live driver forwarding tests are a separate boundary.

## SSL: database TLS, not Java truststores

The [native support matrix](native-drivers.md#source-setup-and-tls) is authoritative per engine: PostgreSQL/Redis use native-tls and all four modes; MongoDB uses official-driver rustls and rejects VerifyCa, and its paired identity paths must reference the same combined PEM. MongoDB/Redis are direct-only, without Unix/SSH/CONNECT. The MySQL-specific backend details below are not universal driver properties.

| TLS mode | Guarantee and warning |
| --- | --- |
| VerifyIdentity (default) | Encrypts and validates certificate chain and database hostname. |
| VerifyCA | Encrypts and validates the certificate chain **without hostname verification**; the server identity is not fully checked. |
| Required | Encrypts **without certificate-chain or hostname validation**; an explicit insecure override vulnerable to impersonation. |
| Disabled | No database TLS encryption or identity validation; explicit insecure override. |

No verification failure triggers plaintext or weaker-mode retry. The original database hostname remains the identity through a relay. MySQL/MariaDB Unix sockets require Disabled because that backend does not upgrade Unix-domain streams to database TLS; this is validated explicitly, not silently selected.

An optional database CA path is manually editable and has a native file picker. PEM client certificate and private-key paths must be supplied as a pair; their metadata is validated and both are wired to the database driver. There is no encrypted TLS-key passphrase control; encrypted TLS private keys are unsupported. Positive mutual-TLS authentication is **not yet live-tested**: compilation and validation are not a handshake claim.

MySQL/MariaDB database TLS uses the mysql_async Rustls backend's built-in webpki roots, not the macOS system Keychain trust store. The HTTPS CONNECT connector separately uses native/system roots for the proxy. The database CA field does not configure proxy trust. No Java/IDE truststore controls or “use system truststore” checkbox are offered. Positive system-trusted HTTPS CONNECT remains unverified.

## Schemas: explorer visibility only

Schemas shows real database names from the cached catalog or Test Connection's database results in a searchable uniform checkbox list. The candidate list unions discovered names, the default database and previously selected names, preserving exact names including commas. **All** displays all discovered branches; **Selection** displays only the checked names. An optional legacy comma-separated field adds manual names, but cannot represent a comma-containing name as precisely as the checkbox list.

An empty Selection hides all database branches while retaining the source and reporting real visible counts of zero. This is a **display filter**, not an account grant, access restriction or SQL security boundary. The SQL console's database choices still come from the full allowed `root.tree.databases` list; only the explorer projection applies `visible_schema`. SQLite retains the complete allowed catalog, not the filtered projection. Generation checks still guard old catalog completions.

## Persistence and cache compatibility

Source JSON stays version 1; new fields use backward-compatible serde defaults, with no metadata-cache schema migration. Legacy custom SSH settings still work. Passwords remain session-only or opt-in plaintext in the separate version 1 `dalan.auth` JSON repository, never in source or SSH-profile JSON. Profile/auth writes use compensating rollback, not one atomic transaction; metadata commits separately and failures remain visible.

The SQLite connection identity uses resolved endpoint, authentication, TLS/client-identity paths, transport and Connect/Query timeouts, never passwords. Name, color and schema visibility do not change that identity; Page size is excluded from the persisted identity. Workspace synchronization compares the whole Options value, so a Page size change can still invalidate affected child workspace state. Saving changed connection/TLS settings invalidates matching metadata appropriately; changing a referenced global SSH profile can make an old cached identity stale until source registration/Apply/OK. Offline metadata never proves current grants or enables offline row access.

## Verification boundary

Historical relocated-control verification recorded 115 unit tests plus one native-wire integration test, 159 simulated UI tests and four Python tests, with formatting, strict lint and signed bundle checks passed. Copy never accesses the credential store. Live database and native Keychain suites were not rerun; native visual/performance/accessibility and current hosted CI remain separate gates. See [historical relocated-control boundary](testing.md#relocated-source-controls). Current unified-settings results are [pending primary confirmation](testing.md#unified-data-sources-and-drivers).

Historical source-manager verification passed 105 headless unit tests plus one native-wire No Auth test, 141 simulated UI tests, four Python bundle tests and 29 unique live cases (11 direct/URL/socket/CONNECT/authentication, 12 TLS, 6 SSH). Formatting, strict lint, debug build, plist/signature and bundle-resource checks passed; see [evidence](testing.md#source-manager-redesign). MySQL No Auth can reject through its unknown-account decoy plugin instead of server 1045; the wire test proves no supplied credentials were transmitted. The older query-console 23-case result remains historical. Positive mTLS and live manager remote `true`, native visual/window/accessibility review, positive trusted HTTPS proxy and current hosted CI remain separate unverified gates.

Connector picker cancellation is silent. Transfer errors/review warnings appear in a dismissible settings overlay, never as labels beneath Cancel/Apply/OK; stale completed notices are not carried into a newly opened settings window.

## Full-height explorer and regex search

The explorer spans the window from top to bottom, with the database toggle beside the native traffic lights. Search sits directly beneath this 34 px chrome row, ahead of the explorer toolbar. If the explorer is hidden or temporarily suppressed in a compact layout, the toggle moves into the full-width titlebar so it remains accessible. The divider/resizer also spans the full window height.

Normal search is case-insensitive substring matching against cached metadata. The `.*` toggle enables case-insensitive regular expressions; use `^name$` for an exact object-name match. Rust regex syntax does not support look-around/backreferences. Patterns are limited to 256 Unicode characters and bounded compilation memory. Invalid/oversized patterns display a sanitized error and restore the ordinary cached tree without altering expansions, selected database/results or performing I/O. Escape clears the text/error while preserving the regex-mode choice.

## Built-in driver versions

Drivers now show a [bundled-version table and capability matrix](native-drivers.md#bundled-library-versions-and-selection) instead of generic information. Latest bundled and exact pin choices are validated against executable backend inventory and persist in `dalan.config` on Apply/OK, independently of source drafts. One version is currently shipped per engine; alternate native library packages cannot yet be installed/switched. No certified database-server version ranges are invented. Driver/provider artwork keeps its original SVG fills and rows center-align icons and correctly spelled names.

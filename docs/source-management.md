# Source management

Status: implemented experimental MySQL/MariaDB configuration subset, not an exact replica of DataGrip or its native/JDBC properties. Dalan uses Kit standard controls and Kit's default theme with compact app-owned layout. Driver/authentication choices are Kit dropdown menus; saved SSH choices are Kit popup lists, not claimed Combobox/Select usage. Kit InputState owns editing/IME/selection; a thin value adapter and native password surrounding-text privacy handler preserve model/safety contracts alongside Kit masked clipboard protections. Current native/UI validation remains gated. Source-control relocation does not change engine glyphs, provider assets or attribution. See [setup and browsing](mysql-sources.md), [security](security.md) and [migration](gpui-kit-migration.md).

## Source window and actions

Titlebar **New Connection**, row **Manage / Copy** and **Connect to a Source** use one retained independent, resizable source window without replacing an unsaved draft. Creation works with the sidebar hidden because the root retains the source-model subscription and dialog lifecycle. New Connection combines the existing plus glyph and explicit label at 28 px high immediately right of the Database collapse toggle at x = 84 px. Pointer, Enter and Space activation, tooltip and loading/saving guards apply; no explorer plus button remains.

The source window starts at 1040 × 760, minimum 780 × 560; it is not a modal sheet or a main-window focus trap. Kit tabs occupy the 34 px native-titlebar layout, reserve 84 px for traffic lights, and contain **General**, **Options**, **SSH/SSL** and **Schemas**. `TitlebarOptions` has an empty title to suppress duplicate native “Data Sources · Dalan” text. Kit open_window/Base Root owns overlays in source and SSH windows; transparent titlebar integration is not a blur feature or a Carbonfox runtime override.

### Row actions and copy boundaries

- The explorer toolbar has exactly three 28 px icons: Refresh selected source, combined expansion toggle (`toggle-tree-expansion`) and the existing New Query Console action. Refresh uses the explicit explorer-selected source and retains old metadata on failure. If any source, database or group is expanded, the toggle offers Collapse All; otherwise it offers Expand Loaded. Expansion uses cached branches only, never network fan-out; collapse preserves metadata.
- Each source has an 18 px Manage gear trigger, `source-actions-{id}`. Pointer or Enter/Space opens a popover near that row using real model/layout bounds, not a fixed global anchor. Its exact **Manage / Copy / Remove** actions capture the row UUID, not global explorer selection. Tab/Shift-Tab and Up/Down traverse the menu; Escape and outside dismissal restore focus. Saving disables actions; existing busy/loading guards remain.
- **Manage** opens that profile in the same SourceDialog with General, Options, SSH/SSL and Schemas. Existing credential/edit rules are unchanged. The old selected-source edit helper remains test-only and does not change the native SSH manager.
- **Copy** creates an unsaved editable draft with a fresh UUID. Its name appends ` copy`, truncating the original at valid UTF-8 boundaries to fit the 256-byte name limit. It clones endpoint, authentication mode/account, schema visibility, Options, TLS/client paths, color and SSH reference/settings, but no password, session secret or Keychain item. `save_password` is false; Copy performs no credential retrieval.
- Copy leaves the original saved JSON, results and schema caches unchanged. Cached database choices are copied into form memory only so Schemas selection remains useful. No copied-source SQLite metadata is registered or persisted until normal Save; Save commits the new profile and starts fresh metadata discovery. There is no new copied-data-source service.
- An existing draft is not discarded. A static `metadata_notice` instructs the user to close the current draft before opening/managing/copying another source. The existing 100-profile guard also rejects creation/copy at the limit. Canceling Copy creates no saved profile.
- **Remove** confirms the captured stable UUID even if selection changes after opening the menu or confirmation. It removes local profile settings, associated credentials and cached metadata only, never database objects. Unrelated source results remain unaffected.

No new dependencies, branded logo or utility assets are introduced. The 22 px virtual tree, cached projection and viewport-only rendering, including the 300-cell integrated grid fixture, retain their existing contracts; this is not a new native performance measurement. See [UI controls](ui-foundation.md#controls) and [pending verification](testing.md#relocated-source-controls).

General owns Name, optional marker Color, a real MySQL/MariaDB driver combo and a real authentication combo. Tab/Shift-Tab traverses the active form; Escape, Cmd-W and native close discard the draft and cancel Test, except while Save is guarded. **Test Connection** tests the draft without storing it. **Save** commits the profile/credential policy, closes the window and starts metadata-only discovery, never automatic row browsing. **Cancel** discards unsaved changes.

The Password row retains the labeled **Save in Keychain** checkbox on its right; compact layouts can wrap it rather than overflow. Passwords are never profile JSON fields.

## General: endpoints and authentication

| Connection mode | Behavior |
| --- | --- |
| Default | Customizable inline Host/Port, initially `localhost:3306`. Optional Database restricts discovery to that database. |
| Unix Socket | Absolute local socket path, entered manually or with a native file picker. Unix only; Direct transport and TLS Disabled are required. Validation rejects incompatible settings rather than silently downgrading them. |
| URL-only | Credential-free `mysql://` or `mariadb://` URL, optionally prefixed by `jdbc:`. Parsed host, port and optional database override the separate endpoint fields. IPv6 and percent-encoded database names are supported. |

Default mode shows a generated credential-free URL synchronized with Host, Port and Database. Editing this URL switches to URL-only. URLs reject **all** userinfo (including username-only), query parameters and fragments; JDBC connection properties and arbitrary driver properties are not supported. Credentials belong in the authentication controls, not the URL. Rejection diagnostics are static and do not echo supplied URI credentials. Do not embed credentials or arbitrary properties in JSON either.

**User & Password** uses the explicit username and password. **No Auth** hides Password and supplies neither username nor password to the driver. It is an intentional mode, not a fallback after login failure; server policy can still reject it. No Auth never reads Keychain and ignores old remembered credentials. Saving it forces `save_password = false` and removes a previously remembered password through the existing compensating-save/rollback path. An old version 1 profile with No Auth and `save_password = true` can load, but that obsolete credential policy is ignored, not used for login.

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

For SSH, select **Custom SSH connection** or a saved configuration by readable name, not raw UUID. The ellipsis control opens an independent native SSH configuration manager. The local forwarding port is dynamically allocated and read-only; there is no fixed local-port setting. Custom retains backward-compatible inline host/port/user, identity selection, optional known-hosts path and Parse config settings.

### SSH configuration manager

The manager owns its draft, keyboard traversal and saving guard independently from the source window. It supports:

- **Add** a profile; **Duplicate** with a new identity; confirmed **Remove**; **Apply** to save without closing.
- **Use** to commit the selected configuration and fill the parent source draft. It does **not** automatically save the database source.
- **Cancel**, Escape, Cmd-W or close to discard unapplied edits and cancel the owned test, guarded while saving.
- Agent or KeyPair authentication; host, port and user; an identity-file manual path and native picker; optional known-hosts file; explicit **Parse config** checkbox.

Removal checks references in saved source JSON and refuses to remove an in-use configuration. Unsaved parent drafts are not persisted references: a missing reference must be repaired before connecting or saving.

Reusable metadata is stored in version 1 `~/Library/Application Support/Dalan/ssh-configurations.json`: at most 100 profiles and 1 MiB, with Unix file `0600` and directory `0700`. It contains paths and settings, never passwords, passphrases or private-key contents. These permissions are not encryption or protection against another process under the same OS account.

A source stores `ssh_configuration_id` as a UUID reference **plus** a materialized transport snapshot for compatibility. Startup, Test Connection and Save resolve the latest reusable configuration. Connection paths resolve current referenced settings, so editing a reusable profile affects sources that reference it. Missing references fail closed; they never silently use the old inline snapshot. Offline cached trees remain available with a metadata warning. Refresh can update the resolved SSH identity in memory; it does not rewrite source JSON until source Save. Cache registration belongs to startup/Save, not a late worker completion that could resurrect a deleted source.

### SSH security and Test

OpenSSH `/usr/bin/ssh`, strict `StrictHostKeyChecking=yes`, BatchMode and agent/key authentication remain required. A selected key enables `IdentitiesOnly=yes`. With a selected known-hosts file, that file is authoritative and global trust is disabled; otherwise OpenSSH's user/system known-host files remain in use. Establish trust outside Dalan.

**Parse config is off by default**, including legacy profiles: commands use `-F /dev/null`. Opting in permits local OpenSSH configuration, whose `ProxyCommand` and `Match exec` can execute local commands. Only enable trusted local configuration. This opt-in does not relax strict host verification.

The manager's **Test** invokes strict batch SSH with remote `true`, not a database test. It is bounded to 20 seconds. Cancel, close and input changes kill the owned child; executable arguments are passed separately, with no shell interpolation, including paths containing spaces. No cleartext SSH password is supplied. A bastion restricted to port forwarding can reject remote `true` even when database forwarding works.

Encrypted keys must be unlocked externally using `ssh-add` and an external agent; there is no built-in passphrase dialog or passphrase persistence. SSH password authentication and PuTTY private-key format are unsupported; convert to an OpenSSH-compatible key outside Dalan. Manager prelaunch/cancellation and simulated picker tests are not evidence of a live successful manager `true` handshake; live driver forwarding tests are a separate boundary.

## SSL: database TLS, not Java truststores

| TLS mode | Guarantee and warning |
| --- | --- |
| VerifyIdentity (default) | Encrypts and validates certificate chain and database hostname. |
| VerifyCA | Encrypts and validates the certificate chain **without hostname verification**; the server identity is not fully checked. |
| Required | Encrypts **without certificate-chain or hostname validation**; an explicit insecure override vulnerable to impersonation. |
| Disabled | No database TLS encryption or identity validation; explicit insecure override. |

No verification failure triggers plaintext or weaker-mode retry. The original database hostname remains the identity through a relay. Unix sockets require Disabled because this backend does not upgrade Unix-domain streams to database TLS; this is validated explicitly, not silently selected.

An optional database CA path is manually editable and has a native file picker. PEM client certificate and private-key paths must be supplied as a pair; their metadata is validated and both are wired to the database driver. There is no encrypted TLS-key passphrase control; encrypted TLS private keys are unsupported. Positive mutual-TLS authentication is **not yet live-tested**: compilation and validation are not a handshake claim.

Database TLS uses the mysql_async Rustls backend's built-in webpki roots, not the macOS system Keychain trust store. The HTTPS CONNECT connector separately uses native/system roots for the proxy. The database CA field does not configure proxy trust. No Java/IDE truststore controls or “use system truststore” checkbox are offered. Positive system-trusted HTTPS CONNECT remains unverified.

## Schemas: explorer visibility only

Schemas shows real database names from the cached catalog or Test Connection's database results in a searchable uniform checkbox list. The candidate list unions discovered names, the default database and previously selected names, preserving exact names including commas. **All** displays all discovered branches; **Selection** displays only the checked names. An optional legacy comma-separated field adds manual names, but cannot represent a comma-containing name as precisely as the checkbox list.

An empty Selection hides all database branches while retaining the source and reporting real visible counts of zero. This is a **display filter**, not an account grant, access restriction or SQL security boundary. The SQL console's database choices still come from the full allowed `root.tree.databases` list; only the explorer projection applies `visible_schema`. SQLite retains the complete allowed catalog, not the filtered projection. Generation checks still guard old catalog completions.

## Persistence and cache compatibility

Source JSON stays version 1; new fields use backward-compatible serde defaults, with no metadata-cache schema migration. Legacy custom SSH settings still work. Passwords remain session-only or opt-in Keychain, never in either JSON repository. JSON and Keychain operations are compensating operations, not one atomic transaction; failures remain visible.

The SQLite connection identity uses resolved endpoint, authentication, TLS/client-identity paths, transport and Connect/Query timeouts, never passwords. Name, color and schema visibility do not change that identity; Page size is excluded from the persisted identity. Workspace synchronization compares the whole Options value, so a Page size change can still invalidate affected child workspace state. Saving changed connection/TLS settings invalidates matching metadata appropriately; changing a referenced global SSH profile can make an old cached identity stale until source registration/Save. Offline metadata never proves current grants or enables offline row access.

## Verification boundary

For relocated controls, verified local totals are 115 unit tests plus one native-wire integration test, 159 simulated UI tests and four Python tests, with formatting, strict lint and signed bundle checks passed. Copy never accesses the credential store. Live database and native Keychain suites were not rerun; native visual/performance/accessibility and current hosted CI remain separate gates. See [current boundary](testing.md#relocated-source-controls).

Historical source-manager verification passed 105 headless unit tests plus one native-wire No Auth test, 141 simulated UI tests, four Python bundle tests and 29 unique live cases (11 direct/URL/socket/CONNECT/authentication, 12 TLS, 6 SSH). Formatting, strict lint, debug build, plist/signature and bundle-resource checks passed; see [evidence](testing.md#source-manager-redesign). MySQL No Auth can reject through its unknown-account decoy plugin instead of server 1045; the wire test proves no supplied credentials were transmitted. The older query-console 23-case result remains historical. Positive mTLS and live manager remote `true`, native visual/window/accessibility review, positive trusted HTTPS proxy and current hosted CI remain separate unverified gates.

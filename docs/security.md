# Security and privacy design

Status: experimental MySQL/MariaDB reads, versioned source settings and native macOS Keychain integration exist. Broader execution and agent controls remain planned. No telemetry, agent process or ACP transport is implemented. See [MySQL sources](mysql-sources.md) for setup and the scope/evidence matrix.

## Trust boundaries

Credentials, metadata, row data and local settings are sensitive. Server values and external-agent output are untrusted. The app is not a sandbox against other programs running under the host account. Least-privilege server roles are required; read-only UI labels and client limits are not authorization or server-resource limits.

## Credentials and persistence

Source profiles have stable UUIDs in version 1 JSON at `~/Library/Application Support/Dalan/sources.json`, without passwords. Native macOS Keychain password saving is opt-in. Otherwise credentials are session-only; after restart, Edit and re-enter them. Keychain failure is visible, never a plaintext fallback. One generated native Keychain round-trip passed with item cleanup; this is not blanket locked/denied Keychain or OS input privacy validation. Session-only Save is tested without Keychain calls. The source file is limited to 1 MiB and 100 profiles; a failed load blocks saving over unreadable settings.

JSON and Keychain changes are not an atomic cross-resource transaction. Compensation failures are reported and may require reconciliation. Confirmed Delete removes only profile settings and its Keychain entry, not a database. Test does not save; Save does not automatically connect. Avoid real credentials in fixtures, logs, crash reports, CLI arguments, exported files or agent context. Memory erasure cannot be guaranteed.

## Connectivity

Database TLS defaults to VerifyIdentity with an optional custom CA. Peer/hostname failures never trigger an insecure retry. The database hostname remains the TLS identity through relays. A local server without a trusted certificate requires CA setup or an explicit Disabled override, recommended only for disposable localhost use with a visible warning. Trusted custom-CA direct and HTTP CONNECT database TLS reads passed on both engines, alongside wrong-hostname and untrusted-CA rejection.

SSH uses `/usr/bin/ssh -W`, strict known-host checking and BatchMode with keys/agent, not interactive SSH passwords. A selected key enables `IdentitiesOnly=yes`. Do not disable host checking to make a test pass. Anonymous HTTP/HTTPS CONNECT carries MySQL wire traffic, not database-query HTTP requests; a named gateway integration is separate work. SSH actual transport reads, wrong-host-key and wrong-identity rejection passed on both engines. The optional absolute existing Known hosts file sets `UserKnownHostsFile` and `GlobalKnownHostsFile=/dev/null`, making selected trust authoritative; absent/None preserves OpenSSH default user/system known-host reads. `StrictHostKeyChecking=yes` remains enabled, and paths with spaces are tested. Disposable SSH fixtures do not edit user SSH state or install OS trust. Untrusted HTTPS proxy rejection passed; positive HTTPS CONNECT with system/native-trusted proxy roots remains unverified. The database custom CA field does not supply proxy trust. HTTPS proxy TLS does not replace database TLS.

Relay transports have an unauthenticated loopback listener accepting the first connection. A local process can race it; do not claim local account isolation or sandboxing. Cancellation closes owned tunnels/relays but is not proof that the server aborted work.

## Read-only database slice

Only app-generated reads are exposed; no arbitrary SQL, writes or stored-program execution UI. Browsing rejects views and accepts BASE TABLE objects. Filters bind values and escape LIKE with `!`; identifier validation/quoting is separate from parameter binding. Sort columns are metadata-validated and quoted; direction is a typed whitelist, not raw SQL. Available primary-key tie-breakers improve ordering but do not establish a snapshot. Read-only transactions are used where capabilities permit. These controls do not secure a privileged account or guarantee transactional behavior for every storage engine.

UI pages contain 100 rows, backend requests at most 200. Display cells are bounded at 4 KiB, retained pages at 2 MiB, columns at 512 and database/table discovery at 1,000. An actual 8 MiB per-packet limit is not an absolute process-memory cap; decoding, protocol assembly and runtime allocations need independent consideration. Client retention limits do not bound server work. Simulated password-input selection/replacement/paste tests are not proof against credential leakage through native OS input systems. Offset pages are not snapshots. Errors preserve clearly labeled stale rows with pagination disabled rather than passing them off as fresh.

## Loaded CSV safety

Export is explicit and limited to the fresh, complete loaded page. Missing/stale/truncated/busy states reject export. No full query or whole table is fetched. Source/table/page changes before save-picker return cancel; after writing starts, the captured page remains the exported data even if current selection changes. Native save success/cancel/error feedback does not imply database snapshot consistency.

CSV quoting is not spreadsheet formula protection. Default spreadsheet-safe mode prefixes formula-risk text and headers with an apostrophe, intentionally altering content. Lexically validated numeric strings are not converted or rounded in the file, but spreadsheets can still lose large-number precision. NULL is unquoted `\N` and literal text is quoted; ordinary CSV parsers may discard that distinction. See [export contract](mysql-sources.md#export-loaded-csv).

Existing files/symlinks are not overwritten. A same-directory staging file is created privately (`0600` on Unix), synced and published atomically by same-filesystem hard link; unsupported filesystems fail explicitly. Cleanup failure can be reported after a complete file has been published. This does not protect against hostile directory replacement, guarantee secure deletion or hide exported sensitive rows from the user-selected location. The native picker accepts chosen names without extension enforcement; output remains CSV.

## Future execution safety

Core's declared-risk decision table is a policy helper, not a SQL/Redis classifier or authorization system. SELECT can invoke side-effecting functions; EXPLAIN ANALYZE runs SQL; dialect constructs can change state. Do not use prefix regexes as a safety boundary. Future approvals must bind immutable target, operation and session generation. Staged writes need primary-key identity, conflict/affected-row checks and visible unknown outcomes after network loss. Never blindly retry writes or conflate cancellation with rollback.

Redis requires binary identity, command-class controls for scripting/blocking/admin operations, bounded payloads and explicit delete/TTL review. No browser KEYS scans. SQL and Redis must be tested independently.

## External agents

No credentials or row data are sent to an agent by this slice. Future ACP integration is suggestion-only with explicit insertion, no autonomous execution, no direct provider SDKs and no app BYOK. Metadata context requires consent; row sharing is never automatic.

Launch only an explicitly trusted executable, not a shell-expanded string. Absolute-path shape validation is not trust verification. Empty ACP filesystem/terminal capabilities remove client-mediated methods only: an external agent can still read files, run tools and access networks with its own OS privileges. Working directory and permission prompts are not a sandbox. Explain independent privileges and privacy before launch.

## Diagnostics and release gates

No telemetry is implemented and no network telemetry is planned by default. Future crash reporting requires explicit opt-in and reviewed payloads. Do not persist rows by default. History/conversation/cache retention and deletion are future design work; source persistence does not imply history storage.

Test certificate/hostname failures and trusted success, credential denial/update/delete, secret-free diagnostics, stale/cancel behavior, oversize inputs, relay races/lifecycle and native interactions. Future execution adds classification-bypass, immutable approval and uncertain-outcome tests; future ACP adds trust, shutdown and consent tests. Dependency licenses/vulnerabilities, signing/notarization and updater design are separate release work. No updater exists.

[Testing](testing.md) · [ACP](acp.md) · [Product plan](product-plan.md)

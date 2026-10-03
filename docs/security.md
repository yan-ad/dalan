# Security and privacy design

Status: planned controls, not a claim that this scaffold secures database or agent operations. No database access, agent process, persistent settings, credential store, or telemetry is implemented.

## Trust boundaries

Assets include database credentials, query text, result data, local settings/history, agent conversations, and live transaction state. Inputs from servers, result cells, connection imports, SQL, Redis commands, and external agents are untrusted. The user's host account and chosen external agent are separate trust decisions.

| Boundary | Proposed controls | Important limit |
| --- | --- | --- |
| App to database | Verified TLS, bounded operations, explicit target/session, server privileges | App read-only labels alone do not restrict server roles |
| App to credential store | OS keychain references, explicit failure, no plaintext fallback | An unlocked host/account has its own access risks |
| App to agent | Explicit trust/launch, filtered environment, consented context, bounded protocol | ACP and cwd are not an OS sandbox |
| App to disk/export | Versioned permissions-aware writes, scope preview, retention controls | Exported data leaves credential protection and may be sensitive |
| Server/agent to UI | Text rendering, payload bounds, schema validation, safe links | Strings must not become commands or trusted markup |

## Credentials and connectivity

macOS Keychain first; use OS credential services on later platforms. Settings store references, never passwords. A keychain failure must explain the error and offer explicit session-only credentials; never silently write plaintext. Clear sensitive buffers where practical without promising perfect memory erasure. Avoid deriving Debug for credential-bearing structs or logging launch arguments that may contain secrets.

Peer and hostname verification on by default. Custom CA/client certificate behavior needs tested configuration. Any insecure mode must be a deliberate per-profile override with persistent warning, not automatic retry after verification failure. Do not put secrets in command-line arguments, URLs printed in logs, crash reports, agent environment, or imported/exported connection files.

## Database execution

The implemented decision table is a **declared-risk policy helper**, not a SQL/Redis classifier or an authorization system. It allows declared reads, denies nonreads in read-only mode, and requires confirmation for nonreads in read-write mode. Until a real adapter classifies and enforces risk, it does not protect a database.

Use least-privilege server roles and supported server-side read-only controls. SELECT can invoke functions with side effects; EXPLAIN ANALYZE runs SQL; stored programs and dialect-specific constructs can change state. Unknown classification is denied in read-only mode and requires explicit review otherwise. Prefix regexes are not sufficient. App-generated mutations use parameterized values and verified identifier quoting.

Approvals must cover exact immutable operation, connection, environment, and session generation. Staged edits require unambiguous primary keys and conflict/affected-row checks. Cap result bytes/rows and duration; a result cap does not cap server work. Preserve uncertain write outcomes after network loss/cancel and prohibit blind retries. Distinguish client cancellation, server cancellation, transaction rollback, and unknown outcome.

Redis needs command-class controls for scripting, blocking, flush, shutdown, configuration, and other administrative commands, binary-safe key identity, bounded payloads, and explicit delete/TTL review. No browser-wide KEYS scans. SQL and Redis safety contracts must be tested independently.

## External agents

Run only an explicitly trusted configured executable, never a shell-expanded string. Shape validation is not trust verification. Show the executable path and invocation safely; review environment inheritance and working directory. No automatic install/download or arbitrary auto-start.

Withholding ACP filesystem/terminal capabilities removes client-mediated RPC methods only. An agent can still read files, run tools, or access networks/databases with its own OS privileges and credentials. Permission UI cannot enforce actions an agent never reports. Real sandboxing would require an independently designed OS/process/container boundary; it is not in this initial scope. Do not claim safety based on cwd, prompting, or ACP permissions.

First release exposes no app database execution tools to agents. Prompt injection from schemas, query results, or tool output must not gain authority or silently change sharing/execution rules. Explain data sharing and remote-agent privacy before sending any context. Terminal authentication, if added, must not accidentally enable general agent terminal execution capabilities.

## Local data and diagnostics

History/conversation persistence should be opt-in until retention/redaction behavior is settled. Do not persist row data by default. Give users local deletion and per-profile history controls. Any schema cache contains potentially sensitive metadata and requires the same retention review.

No telemetry is implemented. The proposal is no network telemetry by default; future crash reporting requires explicit opt-in and a reviewed payload. Logs should record operation IDs/outcomes and safe metadata, not raw results, passwords, complete SQL, or agent prompts by default. Bounded agent stderr capture is still potentially sensitive.

## Release review

Test certificate/hostname failures, credential-store denial, logs/exports for secret leakage, SQL classification bypasses, immutable approvals, uncertain outcomes, oversize result/protocol messages, agent trust/launch shutdown, and context-consent changes. Audit dependency licenses and vulnerabilities before distributing builds. Code signing/notarization and updater signature design are separate macOS distribution work; no updater exists yet.

[Testing](testing.md) · [ACP](acp.md) · [Product plan](product-plan.md)

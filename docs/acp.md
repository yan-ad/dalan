# ACP-only AI integration

Status: proposed integration; the scaffold links the official Rust SDK and provides schema/configuration helpers only. No agent is launched and no session is opened today.

## Product contract

Dalan is an Agent Client Protocol client. AI features connect to user-configured ACP-compatible agents. The app does not collect model-provider API keys, implement provider SDKs, offer direct-provider HTTP fallbacks, or manage model billing. The external agent handles login, subscriptions, providers, and its own credentials. An agent may itself use API keys: ACP-only does not mean credentials cannot exist outside Dalan.

Normal database features must work with AI disabled, no agent installed, or an agent unavailable. The first AI milestone supports query explanation/drafting and review/insertion. No application-provided database execution tools, autonomous queries, or MCP database server are proposed for first release. ACP is not a database-tool protocol; any future execution tools require a separately reviewed MCP interface or extension and app-owned enforcement.

## Version baseline

Workspace pin: `agent-client-protocol = 2.2.0`, default features disabled. SDK package v2.2.0 is distinct from **stable ACP protocol v1**. Do not enable `unstable_protocol_v2`, `unstable_llm_providers`, or the umbrella unstable feature for this baseline. The scaffold reexports `schema::v1` and declares supported protocol version 1.

The SDK's current role/builders/handler API differs from older `Agent`/`Client` trait tutorials. Use versioned rustdoc and source for implementation. The SDK is runtime-neutral and uses `futures::io`; validate subprocess helpers against required launch, environment, shutdown, and logging controls.

## Proposed transport and lifecycle

Initial transport: a directly launched executable over stdio, UTF-8 newline-delimited JSON-RPC 2.0. Agent stdout is protocol only; stderr is diagnostic and may be sensitive. No shell command string interpolation. Configure an absolute executable, argument vector, and absolute working directory. The existing `validate()` checks path shape only; it does not check executable existence, trust, permissions, or sandboxing.

```text
Disabled -> Configured -> Trust review -> Starting -> Initializing
  -> Authentication required / Ready -> Session -> Prompt streaming
  -> Awaiting permission / Complete / Cancelled / Failed
```

1. User explicitly selects/trusts agent invocation; do not auto-download or start configured programs on workspace restore.
2. Start with a reviewed environment allowlist and filtered diagnostics. Do not pass database secrets or model keys from the app.
3. Initialize with client identity, stable protocol version, and only implemented capabilities. Reject unsupported negotiated versions.
4. Handle advertised authentication methods, then create a session. Working directory is context, not isolation.
5. Send bounded opt-in context and prompt; handle streaming updates, tool-call descriptions, and permission requests.
6. Cancellation sends `session/cancel`; resolve pending permission requests as `cancelled`. Stop accepting stale updates; handle process exit and timeout distinctly.
7. On disconnect, retain a clear interrupted state. Do not silently replay prompts that may have triggered agent tools. Shut down the owned process and descendants according to a tested policy.

Test malformed/oversize messages, unsupported versions, unexpected exits, stderr saturation, shutdown, and reconnect. Draft HTTP transport is not part of the initial milestone.

## Capabilities and authentication

The scaffold returns `ClientCapabilities::default()`. It advertises no filesystem read/write or tool terminal methods, and no terminal-auth support. This is a future initialization value, not a live negotiation today.

For initial integration, keep filesystem and tool-terminal capabilities unadvertised. Agent-managed authentication can use the advertised `agent` method and `authenticate(methodId)`. If an agent requires terminal authentication, show an actionable unsupported-method message until interactive login is implemented. Never advertise `auth.terminal` before supporting the same configured executable/invocation with the agent's appended login arguments/environment and tested restart/reinitialization. Terminal auth is separate from tool terminal access. Logout is capability-gated.

## Permission decisions

`session/request_permission` supplies option IDs with allow/reject once/always kinds. UI decisions must return a supplied option ID, never an invented option. Default is no automatic approval; avoid persistent allow-always decisions initially. Show agent/tool/action details without rendering agent text as trusted UI.

Permission requests are optional in the protocol. Tool-call updates describe agent work; they do not enforce universal authorization. An agent may execute tools using its own OS privileges even when Dalan does not advertise filesystem/terminal methods. Dalan cannot guarantee every external action is requested or reported. See [security](security.md).

## Context and privacy

- No automatic schema, query, or result sharing on connect.
- Users opt in to selected connection/schema metadata for a scoped session. Preview what will be sent, reset consent when the target changes.
- Query text may contain credentials or sensitive literals. Warn and allow removal; do not claim automatic redaction detects all secrets.
- Row/result data is excluded from automatic context. Any future explicit sharing needs a separate bounded preview and confirmation.
- Never include connection URLs with secrets, passwords, tokens, private keys, credential-store content, or app environment dumps.
- The external agent may send context to remote models and retain logs; disclose agent-owned privacy behavior, which the app cannot guarantee.
- Suggestions are text. Inserting one only modifies the console draft. Normal execution rules and fresh user action still apply.

## Acceptance evidence needed

Use a deterministic fake ACP peer for initialize, capability gating, auth-required/unsupported auth, session/new, prompt/update/cancel, permission ID fidelity, cancellation races, errors, and crash/oversize output. Then test chosen real agents and record exact agent versions and supported auth paths. Do not advertise compatibility with every ACP agent solely because the SDK compiles.

[Architecture](architecture.md) · [Security](security.md) · [Testing](testing.md) · [Sources](references.md)

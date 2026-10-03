# 0002: ACP-only AI

Status: ACP-only is a user requirement; suggestion-only integration and stdio details are proposals, SDK boundary adopted for scaffold.

## Context

The user wants AI available via ACP, not application BYOK. ACP standardizes agent/client communication but does not isolate external programs.

## Decision

Use the official `agent-client-protocol` Rust SDK, currently pinned at 2.2.0, stable schema/protocol v1, no unstable features. Propose directly launched stdio agents that own auth/provider/billing. No app provider SDKs, model-key settings, direct-provider fallback, or first-release database execution tools. Start with opt-in context and suggestions that users review/insert and explicitly run. Advertise only implemented capabilities; scaffold defaults are empty.

## Consequences

Users need a compatible external agent for AI, while database features remain independent. Agents may manage API keys outside the app. Unsupported auth methods/capabilities need clear errors. ACP permission UI and working directories are not sandboxing; independent agent tools can bypass app-mediated surfaces. Future database tools require a separate security/interface decision. Transport and authentication are not implemented by linking the SDK.

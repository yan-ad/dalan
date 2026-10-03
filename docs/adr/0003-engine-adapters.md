# 0003: Engine adapters

Status: user-required first-release scope; adapter/library details proposed.

## Context

First release must target PostgreSQL, MySQL, MariaDB, and Redis. The user confirmed MongoDB follows first release. These engines do not share query, result, transaction, or metadata semantics.

## Decision

Keep explicit engine identities and capability-aware adapters. Propose SQLx for PostgreSQL and MySQL/MariaDB, `redis` (redis-rs) for Redis, on a managed Tokio runtime separate from GPUI. MySQL and MariaDB share the wire library but keep dialect/metadata checks. No SQLx/Redis dependency is installed until its vertical slice starts. No public plugin ABI, dynamic driver loading, or universal SQL facade now.

## Consequences

Shared chrome is possible without false feature parity. Tests must run against MySQL and MariaDB separately. SQL transaction affinity, Redis cursors/binary values, lossless decoding, bounded result events, cancellation, and uncertain outcomes remain required contracts. MongoDB later needs document-native browsing and aggregation UX rather than a SQL shim. Exact versions/auth/topologies stay unclaimed until tested.

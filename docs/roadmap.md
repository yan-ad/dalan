# Dalan roadmap index

The canonical forward plan is now the root **[ROADMAP.md](../ROADMAP.md)**. It covers now/next/future priorities, source-backed DBX reuse, driver ecosystem, database-focused plugin ecosystem, ACP-only AI, release/platform gates and owner decisions. This document remains for existing links; do not maintain a competing milestone sequence here.

- [Current baseline](../ROADMAP.md#where-we-are-now)
- [Now: foundation](../ROADMAP.md#now-consolidate-the-foundation-before-adding-breadth)
- [Next: three ecosystems](../ROADMAP.md#next-grow-three-ecosystems-in-parallel-behind-that-foundation)
- [Future](../ROADMAP.md#future-mature-the-ecosystem-without-bloating-the-workspace)
- [Concrete next implementation](../ROADMAP.md#dependency-order-and-concrete-next-task)
- [DBX assessment and initial adaptation](dbx-reuse.md)
- [Implemented feature checklist](feature-checklist.md)
- [Test evidence](testing.md)

## Completed multi-tab / read-only console slice

The existing multi-table workspace and restricted MySQL/MariaDB query consoles are experimental implementations, not full DataGrip parity. See the [query guide](query-consoles.md), [ADR 0004](adr/0004-workspace-tabs-and-read-only-consoles.md) and [current baseline](../ROADMAP.md#where-we-are-now). This heading preserves earlier decision-record links. Syntax completion/history, scripts, pinned transactions and guarded writes have separate roadmap gates.

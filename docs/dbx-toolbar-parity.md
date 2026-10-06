# Staged DBX query-toolbar parity

Status: **full DBX parity approved as a staged goal**, not delivered 1:1 visual or logical parity. Stage 1 below is implemented; later stages are planned and gated. The restricted MySQL/MariaDB execution policy is unchanged. [ROADMAP](../ROADMAP.md) remains the canonical forward plan; [query consoles](query-consoles.md) defines shipping behavior and [testing](testing.md#dbx-toolbar-stage-1-validation) separates evidence from plans.

## Stage 1 — safe toolbar and editor tools (implemented)

| Control / boundary | Current Dalan behavior |
| --- | --- |
| Run / Stop | One stateful primary control runs when idle and cancels when running. The separate Cancel control and Cmd-Period remain available. Cmd-Enter still uses guarded Run; selected text or the whole draft runs, never implicit script execution. |
| Format SQL | Simple whole-document token-gap formatting, not a full dialect-aware formatter. Changes recognized keyword case and existing whitespace gaps; preserves identifiers, literal/comment text and original token adjacency. Unsafe/unlexable input stays unchanged. The per-console uppercase/lowercase preference affects Format only, not typing or execution. |
| Compress SQL | Whole-document whitespace-gap compression, preserving token adjacency, comments, literal text and newline-bearing gaps. It does not blindly remove all whitespace or merge tokens; keyword case is unchanged. |
| Soft wrap / Unfold | Kit soft wrap is retained in memory per console. Unfold All uses Kit's available API. Kit has no public Fold All API for this integration: no Fold All control or inert placeholder is shipped. |
| Open SQL file | Single-file, bounded **64 KiB UTF-8**, optional UTF-8 BOM removal and CRLF/CR normalization outside literals. Literal carriage returns that Kit cannot preserve are refused rather than silently changing string data. A changed draft while the picker/read is pending is not overwritten. |
| Save SQL to new file | Explicit native picker and private, no-overwrite publication to a new SQL file; existing files and symlink paths are refused. This is not overwrite-in-place, a SQL library, persistent console recovery or a file watcher. Unix private creation modes do not establish a Windows ACL guarantee. |
| Paste as IN condition | Clipboard newline/tab cells become a parenthesized list of quoted text, not expressions. Quotes/backslashes are escaped; spaces and empty cells remain data, one final row terminator is ignored, and invalid/oversized input is refused. Insertion does not execute SQL. |
| Undo / caret | Transformations, file open and insertion use Kit editor history. Undo can restore the previous draft; active-console caret/focus behavior remains intact. No independent editor engine or arbitrary history-depth guarantee is introduced. |
| Source / database | Source picker is on the right. Database choices are cached-only, searchable and value-safe; Clear Database removes this console's default. Changes are guarded while running. Retargeting preserves the tab/draft instead of replacing its identity; the source/database descriptor is refreshed on each render. |
| Set Default | Updates the saved source's default through root profile persistence without credential access or resetting the tab target. Selecting the existing default toggles it off. For URL-only sources, the effective database can be picked, but Set Default is refused: edit the connection URL in settings instead. |
| Compact toolbar | At **800 px available toolbar width**, controls switch tiers; safe tools move into More and source selection uses a compact dropdown. This is a Dalan tiered layout, not DBX's measured overflow with exact hysteresis. |
| Export loaded CSV | Retained as an extra Dalan action with its existing fresh/nonbusy/nonstale/nontruncated loaded-page contract. It is not full-query export and does not imply DBX export parity. |

Tools never broaden the SQL allowlist or automatically connect/execute. Drafts, wrap/case preferences and results remain session-memory state; saving a SQL file is an explicit exception for its captured text, not automatic persistence. Source-default changes persist password-free profiles, not auth. Existing close/discard semantics remain: a nonempty draft still prompts even after saving a copy.

Implementation ownership: `crates/app/src/sql_tools.rs`, `crates/app/src/desktop/sql_editor.rs`, `crates/app/src/desktop/query_console.rs`, `crates/app/src/desktop/source_model.rs`, `crates/app/src/desktop/source_workspace.rs` and `crates/app/src/workspace_tabs.rs`.

## Stages 2–5 — remaining parity

Each stage needs scoped capability/policy decisions, regression tests and native review. Toolbar presence alone is never an acceptance gate.

| Stage | Planned scope | Acceptance / safety dependencies |
| --- | --- | --- |
| **2: results and explain** | Execute into a new result tab; explain plan and explicitly gated analyze/actual execution; richer result handling. | Immutable SQL/source/database provenance, isolated result ownership and retention budgets; distinguish estimated plans from analyze that executes work. Current EXPLAIN rejection remains until an explicit policy change and engine-specific tests. |
| **3: archives, script library and editor intelligence** | Result archives, script library/history retention and deletion; diagnostics/hints; folding and database-aware completion/LSP integration. | Opt-in persistence with retention/delete and credential/row-data boundaries; bounded files and results; real supported Kit folding APIs or an explicit upstream integration; engine-aware diagnostics and bounded language-service lifecycle. No fake Fold All, completion or hint placeholders. |
| **4: multiple databases and cancellation** | Multi-database workflows and stronger per-operation cancellation. | Typed driver capabilities, exact source/database routing, isolated operation ownership, confirmed cancellation versus uncertain outcomes, and real-server conformance tests. Existing socket disposal is not server KILL acknowledgement or rollback proof. |
| **5: transactions and permissions** | Dedicated sessions, transaction mode, commit/rollback, execution permissions and explicit rollback/confirmation flows. | Pinned physical-session ownership, fresh exact-target/operation confirmation, failure/unknown-outcome semantics, permission denial tests and no automatic write retry. No silent expansion of the read-only console into arbitrary SQL. |

**Not implemented in Stage 1:** new result tabs, explain/analyze, result archives, script library/history, transactions, multi-database execution, LSP completion, diagnostic hints or Fold All. Each Run still owns a fresh bounded read-only connection, not a persistent transaction session. Stage 1 completion does **not** mean full 1:1 DBX appearance, responsive geometry or logic.

## Upstream conceptual references and provenance

Reference revision: [`t8y2/dbx` at `38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad`](https://github.com/t8y2/dbx/tree/38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad), inspected locally under ignored `target/research/dbx`. Relevant upstream paths are behavioral specifications, not imported modules:

- `apps/desktop/src/components/layout/EditorToolbar.vue` and `editorToolbarActions.ts`: execution/editor tools, target selectors and transaction controls.
- `apps/desktop/src/composables/useToolbarOverflow.ts` and `components/ui/ToolbarOverflowMenu.vue`: measured overflow reference; Dalan's 800 px tiers deliberately do not claim exact reproduction.
- `apps/desktop/src/components/layout/SqlEditorWorkspace.vue` and `composables/useSqlExecution.ts`: editor/execution and result ownership reference.
- `apps/desktop/src/components/layout/SqlLibraryPanel.vue` and `lib/query/queryResultArchive.ts` / `queryResultArchiveFile.ts`: future script-library and archive reference.
- `apps/desktop/src/composables/useDatabaseOptions.ts`: target-selection reference; Dalan uses its own cached metadata and profile persistence boundaries.

DBX's root/package code is Apache-2.0; vendor components require separate review. **These toolbar tools are independently implemented: no DBX assets or code were copied, so this slice requires no license transfer or new adapted-code notice.** This does not remove the existing catalog adaptation's attribution/license obligations or grant a license to Dalan's own project/artwork. See [DBX reuse assessment](dbx-reuse.md) and [third-party notices](../THIRD_PARTY_NOTICES.md).

## Evidence boundary

Stage 1 local validation passed **165 headless unit tests + one native-wire integration test, 211 production UI tests and 14 Python tests**, formatting, strict workspace/desktop Clippy and signed debug bundle/plist/signature checks. Historical totals elsewhere apply to their own revisions. Synthetic tests must cover token/literal/comment safety, file bounds/private no-overwrite behavior, undo/caret retention, target persistence without auth, URL-only refusal and compact toolbar routing. Native picker, visual/IME/accessibility and measured responsiveness remain separate gates. Hosted CI is explicitly skipped; no private credentials, user profiles or row data are needed or committed, and this documentation update makes no commit/push claim.

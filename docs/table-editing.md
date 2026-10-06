# Staged SQL table editing

Status: experimental, bounded SQL write stage, not release-certified database editing or full DBX/DataGrip parity. MySQL/MariaDB and PostgreSQL base-table writes have an explicit staging/Apply path; consoles remain read-only. Current full-suite results await primary confirmation in [testing](testing.md). Actual-server write/commit fixtures and native interaction checks remain unrun; implementation and simulated tests are not evidence that a real server committed or rolled back successfully.

## Eligible tables and values

Editing requires a SQL **BASE TABLE**, a fresh complete, untruncated loaded preview with every column and complete original row values, and the complete primary key (including every component of a composite key). Key metadata must be NOT NULL and loaded key values must be non-NULL. A complete preview means a complete bounded loaded page, not that the whole table is loaded. Do not reconstruct originals from clipped display strings.

The backend independently proves the table kind and schema when Apply runs:

| Engine | Eligible server objects | Supported write column types |
| --- | --- | --- |
| MySQL / MariaDB | InnoDB BASE TABLE only | Scalar text/char/varchar families and integers |
| PostgreSQL | Ordinary (`relkind = r`) or partitioned (`relkind = p`) base tables | Scalar text/character/character varying, smallint/integer/bigint and boolean |

All columns must be supported: this is a **whole-table gate**, not permission to edit a supported cell while ignoring unsupported original fields. Decimal/numeric, floating point, temporal, binary, JSON, enum, arrays, domains and other unsupported/lossy types disable editing, even when the proposed edit touches only text. Decimal parsing helpers do not make decimal table writes supported. PostgreSQL booleans use `true` or `false`; integers use whole decimal input with type/range validation, not floating-point conversion. NUL-containing write values are rejected.

Query results, views, keyless tables, incomplete/truncated previews, non-InnoDB MySQL/MariaDB tables, MongoDB and Redis are not writable. No SQL-console command enables writes, arbitrary SQL execution, DDL or stored-program execution. Server permissions remain authoritative; a client eligibility check does not grant access or sandbox triggers/functions.

## Local changes, then explicit Apply

Cell and row actions modify an in-memory overlay over the shared immutable `Arc<TablePage>`. They do not fetch, save or execute SQL automatically.

- **Edit cell** stages a typed value. Empty text is an empty string, not SQL NULL; the literal text `NULL` in a text column is still text.
- **Set NULL** is a separate operation and is rejected for NOT NULL columns.
- **Add row** creates an inserted row whose initially omitted fields mean **DEFAULT**. Omission is not NULL. The staging model supports returning an inserted field to DEFAULT; this is not an arbitrary SQL DEFAULT expression editor or a promise of a dedicated menu action.
- **Clone row** copies displayed values but omits every primary-key field so the server can generate defaults. A table without suitable key defaults can reject the insert; cloning does not manufacture a unique key.
- **Delete row** marks a loaded row for deletion; **Restore row** removes that mark while retaining prior cell edits. Deleting a staged insert removes it locally rather than sending a DELETE.
- **Discard staged changes** removes local updates/deletes/inserts, not changes already committed to the database. Returning a cell to its original value removes that cell's pending update.
- **Apply changes** requires explicit confirmation (**Apply to database** or **Keep editing**). Only this boundary resolves credentials and submits the bounded generated write request. Successful Apply clears staging and reloads the applied page; that reload is a separate read, not the transaction's snapshot.

One batch permits at most **100 row operations**, **64 KiB per value**, and **2 MiB of value data**, including complete original rows and supplied changes. An updated or deleted row counts as one operation, not one operation per edited cell. Schema metadata is bounded to **512 expected columns**. These bounds are not a process-memory ceiling or a guarantee of bounded server work; normal preview bounds are unchanged.

## Backend transaction and optimistic conflict contract

The request carries the target, ordered `expected_columns`, complete originals for updates/deletes and explicitly supplied insert/update values. It accepts no SQL text. Values are parameters and identifiers are separately validated/quoted.

Each Apply owns a fresh physical write session and one transaction for the whole batch. MySQL/MariaDB verifies InnoDB/base-table metadata before and after acquiring a transaction-held metadata lock. PostgreSQL checks `r`/`p` before and after `LOCK TABLE … IN ROW EXCLUSIVE MODE`; ordinary-table updates/deletes use `ONLY` so inherited child rows cannot borrow the parent's uniqueness. Partitioned parents retain their own key requirements.

Under that lock the backend rereads columns and compares the **exact ordered schema fingerprint**: column name, type, nullability and primary-key membership. A difference fails closed and requires reload; UI metadata is not the final authority. The entire batch is compiled/validated before its first mutation.

Updates/deletes use the primary key **and every complete original column value** in NULL-aware predicates for optimistic conflict detection. A changed or missing row is not silently overwritten. Every individual insert/update/delete must have **exactly one affected row**; zero or multiple rows abort the batch. MySQL/MariaDB strict conversion and warning checks reject reported lossy writes. Constraints/defaults/triggers can still cause failure; this contract counts direct statement effects, not arbitrary trigger side effects.

All operations commit together or the failure path attempts rollback. There is no automatic retry, partial-success Apply, reusable transaction session, autocommit toggle or user transaction console. Do not infer rollback proof from a closed connection, local cancellation or a timeout.

## Unknown outcomes and target changes

A failed or timed-out **COMMIT acknowledgement** is explicitly **outcome unknown**, not “rolled back.” The server may already have committed. Apply/edit retry is disabled: **do not resend the batch**. Discard local staging, reload the same target and verify the database before preparing any new edits. A successful read clears the local uncertainty guard, but reading one bounded page is not proof of all effects; verify affected keys/rows as necessary. Ordinary schema/conflict/constraint failures retain staging for review; reload requires discarding it first, and no automatic retry occurs.

While Apply is in flight, tab close, Cancel, discard, paging, refresh and sorting/condition changes are blocked. Pending changes protect their loaded page from result eviction and require Keep Open/Discard before tab close; paging, refresh and sorting are blocked until staging is applied or discarded. These are application guards, not guarantees against a native OS quit, process kill, crash or development restart. Such termination may leave an unknown write outcome; the app cannot prove rollback or promise persisted staging/recovery.

Source changes during Apply do not retarget the request: it retains the captured original target. Invalidation is deferred until the completion boundary, then the tab is invalidated and requires verification of the **original server**. There is no automatic reload against the newly configured source and no retry through that new endpoint. Verify the old target independently and reopen the tab only after reconciliation.

## Canvas and scope boundary

The production grid remains two-axis canvas paint with viewport/overscan-bounded display and shaping allocations, not a per-cell DOM/Div editor. Staged-cell and deleted-row highlights are painted; native toolbar/context-menu actions operate on the selected cell/row, with **one editor overlay**, not one input per cell. The immutable page and bounded staging data are separate from viewport caches; viewport-only painting does not mean only viewport rows participate in Apply.

DBX supplies a **conceptual behavior reference**, not copied 1:1 UI or full feature parity. Current scope is bounded local set-cell/NULL, insert/clone, delete/restore/discard and explicit transactional Apply. Bulk editing tools, details/filter drawers, hide/freeze columns, grouping, archives, general transaction controls and permanent/full DBX parity remain TODO, not hidden implemented features. Broader formatting/console tools do not change write eligibility.

See [table browser](table-browser.md), [read-only consoles](query-consoles.md), [security](security.md), [feature checklist](feature-checklist.md) and [strategic roadmap](../ROADMAP.md). No hosted CI, private endpoint testing, native visual approval or release readiness is claimed by this documentation stage.

Multiline/tab-containing values remain visible and copyable but cannot be opened in this stage’s single-line cell editor; the future full-value editor must preserve those bytes. Blank/default-only inserts require at least one explicit supplied value before Apply; `DEFAULT` omissions are not silently converted to NULL.

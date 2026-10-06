# 0006: GPUI Kit migration boundary

Status: accepted; production framework and standard-control adoption implemented. Native validation remains gated.

## Context

Custom controls and editing engines repeatedly consumed maintenance work. The initial isolated Kit pilot established compatibility and bounded wide-table render evidence. The user's subsequent decision is to use **Kit for all standard controls and its default theme**, superseding the earlier proposal to preserve Carbonfox runtime tokens.

## Decision

- The root `gpui` alias resolves to `gpui-kit = 0.7.1`, with `tree-sitter-sql`, on the coordinated `gpui-pre = 0.3.8` family. Remove the old GPUI 0.2 production family rather than mix incompatible UI types.
- Use `gpui::application`, `gpui::init` and Kit `open_window`/Base Root for main, source, About and SSH windows.
- Use Kit Button, Input, Checkbox, tabs, dropdown/popup menus and tooltips for standard interaction. Driver/authentication and saved SSH choices are native dropdown/popup alternatives; do not label them Combobox/Select implementations without actual compiled use.
- Leave theme initialization to Kit. App layout and custom grid paint consume active semantic `Hsla` colors; no Carbonfox override or fixed application palette is installed. Preserve historical assets/licenses and existing application/provider artwork provenance.
- Replace custom editing engines with thin Kit InputState and rope EditorState adapters. Kit owns edits, selection, IME and undo; Dalan owns field/model synchronization, SQL validation/context and Run/Cancel. Password native extraction is suppressed by a delegating privacy adapter in addition to Kit masking/clipboard protections.
- Keep the specialized two-axis canvas result renderer until DataTable demonstrates representative native performance, memory and interaction parity. A passing 100 × 512 pilot is not parity. Preserve typed models, 16 MiB/eight-page best-effort inactive retention and worker limits.
- Preserve driver, credential, storage, cancellation, tabs and read-only query contracts. SQL highlighting is implemented; database-aware completion, history, scripts, new engine executors and live ACP remain separate work.

## Consequences

The production migration is real, but source adoption does not establish native font/shader/window, accessibility or performance certification. Current format/test/lint/bundle and native input/overlay/privacy regressions passed; the production app launched. Current hosted CI and native visual/accessibility/performance remain separate gates. The 64 KiB SQL policy is tested for programmatic loads and interactive paste/native commit. Interactive rejection restores accepted SQL through Kit and clears undo history for that exceptional rollback, rather than maintaining another editing engine.

Keep existing distribution notices, including full historical Carbonfox MIT texts and icon/adapted-code notices. Kit software/examples and documentation have different licenses. No upstream feature/performance guarantee becomes a Dalan guarantee without testing; no Shell/JavaScript extension runtime is adopted merely for components.

[Migration guide](../gpui-kit-migration.md) · [Pilot](../../experiments/gpui-kit-pilot/README.md) · [Roadmap](../../ROADMAP.md)

//! SQL-specific adapter around Kit's editor. Editing, selection, IME, scrolling,
//! syntax highlighting and undo/redo are owned by Kit, not by this view.

use gpui::component::input::{Editor, EditorState};
use gpui::{App, Context, Entity, FocusHandle, Focusable, Subscription, Window, div, prelude::*};

const MAX_BYTES: usize = 64 * 1024;

pub(super) struct SqlEditor {
    // The public constructor has no Window. Create the Kit state on first render
    // and keep a snapshot for the console's context-free accessor methods.
    state: Option<Entity<EditorState>>,
    subscription: Option<Subscription>,
    focus_handle: FocusHandle,
    content: String,
    selection: Option<String>,
    pending_value: bool,
    pub(super) validation_error: Option<String>,
}

impl SqlEditor {
    pub(super) fn new(value: &str, cx: &mut Context<Self>) -> Self {
        let value = normalize(value);
        let oversized = value.len() > MAX_BYTES;
        Self {
            state: None,
            subscription: None,
            focus_handle: cx.focus_handle().tab_stop(true).tab_index(20),
            content: if oversized { String::new() } else { value },
            selection: None,
            pending_value: false,
            validation_error: oversized.then(limit_message),
        }
    }

    pub(super) fn value(&self) -> String {
        self.content.clone()
    }

    pub(super) fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    /// Focus through Kit's state API so a lazy/mounted editor starts its caret
    /// lifecycle even when the native focus event was queued before mounting.
    pub(super) fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(state) = &self.state {
            state.update(cx, |state, cx| state.focus(window, cx));
        } else {
            self.focus_handle.focus(window, cx);
        }
    }

    pub(super) fn selected_text(&self) -> Option<String> {
        self.selection.clone()
    }

    /// Programmatic loads start a new history; oversized loads leave SQL intact.
    /// Kit's setter requires a Window, so apply accepted loads at the next render.
    pub(super) fn set_value(&mut self, value: String, cx: &mut Context<Self>) {
        let value = normalize(&value);
        if value.len() > MAX_BYTES {
            self.validation_error = Some(limit_message());
        } else {
            self.content = value;
            self.selection = None;
            self.pending_value = true;
            self.validation_error = None;
        }
        cx.notify();
    }
}

impl Focusable for SqlEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SqlEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.state.is_none() {
            let was_focused = self.focus_handle.is_focused(window);
            let state = cx.new(|cx| {
                EditorState::new(window, cx)
                    .default_value(self.content.clone())
                    .language("sql")
                    .soft_wrap(false)
            });
            self.focus_handle = state.read(cx).focus_handle(cx);
            if was_focused {
                state.update(cx, |state, cx| state.focus(window, cx));
            }
            self.subscription = Some(cx.observe(&state, |this, state, cx| {
                // A pending console load takes precedence over stale editor state.
                if !this.pending_value {
                    let state = state.read(cx);
                    let content = state.value().to_string();
                    if content.len() > MAX_BYTES {
                        // Kit 0.7.1 only exposes validators on single-line
                        // InputState, not EditorState. Restore the last accepted
                        // value at render instead of persisting an oversized
                        // draft. Kit's setter starts a new undo history on this
                        // exceptional path; ordinary edits remain entirely Kit's.
                        this.pending_value = true;
                        this.validation_error = Some(limit_message());
                    } else {
                        if content != this.content {
                            this.validation_error = None;
                        }
                        this.content = content;
                        let selected = state.selected_text().to_string();
                        this.selection = (!selected.is_empty()).then_some(selected);
                    }
                }
                cx.notify();
            }));
            self.state = Some(state);
            self.pending_value = false;
        }
        let state = self.state.as_ref().expect("editor initialized");
        if self.pending_value {
            state.update(cx, |state, cx| {
                state.set_value(self.content.clone(), window, cx);
            });
            self.pending_value = false;
        }

        div()
            .id("dalan-sql-editor")
            .flex_1()
            .min_h_0()
            .w_full()
            .h_full()
            .overflow_hidden()
            .child(
                Editor::new(state)
                    .h_full()
                    .tab_index(20)
                    .aria_label("SQL editor"),
            )
    }
}

/// Kit initialization installs the editor bindings. Query execution belongs to
/// the console, so no local editing or Cmd-Enter bindings are installed here.
pub(super) fn bind_keys(_: &mut App) {}

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn limit_message() -> String {
    "SQL is limited to 64 KiB; load a smaller query before running it.".into()
}

#[cfg(all(test, feature = "ui-tests"))]
mod tests {
    use super::*;
    use gpui::component::highlighter::{HighlightTheme, SyntaxHighlighter};
    use gpui::test::TestWindowExt;
    use gpui::{ClipboardItem, EntityInputHandler, TestAppContext, VisualTestContext};

    fn fixture<'a>(
        cx: &'a mut TestAppContext,
        value: &str,
    ) -> (Entity<SqlEditor>, &'a mut VisualTestContext) {
        cx.update(gpui::init);
        let editor = cx.new(|cx| SqlEditor::new(value, cx));
        assert!(editor.read_with(cx, |editor, _| editor.state.is_none()));
        let (_, visual) =
            cx.add_window_view(|window, cx| gpui::base::Root::new(editor.clone(), window, cx));
        visual.refresh().unwrap();
        visual.run_until_parked();
        visual.update(|window, cx| {
            editor.read(cx).focus_handle().focus(window, cx);
        });
        (editor, visual)
    }

    #[gpui::test]
    fn windowless_loads_normalize_and_reject_oversized_utf8_atomically(cx: &mut TestAppContext) {
        let editor = cx.new(|cx| SqlEditor::new("SELECT '文';\r\nSELECT 2;\r", cx));
        editor.update(cx, |editor, cx| {
            assert_eq!(editor.value(), "SELECT '文';\nSELECT 2;\n");
            let original = editor.value();
            editor.set_value("é".repeat(MAX_BYTES / 2 + 1), cx);
            assert_eq!(editor.value(), original);
            assert!(editor.validation_error.is_some());
            editor.set_value("x".repeat(MAX_BYTES), cx);
            assert_eq!(editor.value().len(), MAX_BYTES);
            assert!(editor.validation_error.is_none());
            assert!(editor.state.is_none());
        });
        let oversized = cx.new(|cx| SqlEditor::new(&"x".repeat(MAX_BYTES + 1), cx));
        oversized.read_with(cx, |editor, _| {
            assert!(editor.value().is_empty());
            assert!(editor.validation_error.is_some());
        });
    }

    #[gpui::test]
    fn native_typing_newline_indent_and_undo_are_owned_by_kit(cx: &mut TestAppContext) {
        let (editor, visual) = fixture(cx, "");
        visual.update(|window, cx| {
            window.input("SELECT 1;", cx);
            window.press("enter", cx);
            window.input("SELECT 2;", cx);
            let state = editor.read(cx).state.as_ref().unwrap().clone();
            assert_eq!(state.read(cx).value(), "SELECT 1;\nSELECT 2;");
            window.press("secondary-a", cx);
            window.press("tab", cx);
            assert_eq!(state.read(cx).value(), "  SELECT 1;\n  SELECT 2;");
            window.press("secondary-z", cx);
            assert_eq!(state.read(cx).value(), "SELECT 1;\nSELECT 2;");
        });
        visual.run_until_parked();
        editor.read_with(visual, |editor, _| {
            assert_eq!(editor.value(), "SELECT 1;\nSELECT 2;");
        });
    }

    #[gpui::test]
    fn native_selection_copy_and_sql_grammar_use_production_state(cx: &mut TestAppContext) {
        let sql = "SELECT name FROM people WHERE id > 0;";
        let (editor, visual) = fixture(cx, sql);
        visual.update(|window, cx| {
            let state = editor.read(cx).state.as_ref().unwrap().clone();
            assert_eq!(state.read(cx).language_name(), "sql");
            // Kit keeps its highlighter private. Parse the production state's
            // actual rope with its configured language, not a pilot editor or
            // hard-coded substitute language.
            let mut syntax = SyntaxHighlighter::new(&state.read(cx).language_name());
            assert!(syntax.update(None, state.read(cx).text(), None));
            let tree = syntax.tree().expect("SQL grammar must produce an AST");
            assert!(
                !tree.root_node().has_error(),
                "{}",
                tree.root_node().to_sexp()
            );
            let styles = syntax.styles(&(0..sql.len()), &*HighlightTheme::default_dark());
            assert!(styles.iter().any(|(range, _)| *range == (0..6)));
            window.press("secondary-a", cx);
            window.press("secondary-c", cx);
            assert_eq!(
                cx.read_from_clipboard().unwrap().text().as_deref(),
                Some(sql)
            );
        });
        visual.run_until_parked();
        editor.read_with(visual, |editor, _| {
            assert_eq!(editor.selected_text().as_deref(), Some(sql));
        });
    }

    #[gpui::test]
    fn programmatic_load_flushes_and_oversized_native_paste_preserves_sql(cx: &mut TestAppContext) {
        let (editor, visual) = fixture(cx, "SELECT 1;");
        editor.update(visual, |editor, cx| {
            editor.set_value("SELECT '文';\r\n".into(), cx)
        });
        visual.refresh().unwrap();
        visual.run_until_parked();
        visual.update(|window, cx| {
            let state = editor.read(cx).state.as_ref().unwrap().clone();
            assert_eq!(state.read(cx).value(), "SELECT '文';\n");
            window.press("secondary-a", cx);
            cx.write_to_clipboard(ClipboardItem::new_string("é".repeat(MAX_BYTES / 2 + 1)));
            window.press("secondary-v", cx);
        });
        visual.run_until_parked();
        visual.refresh().unwrap();
        visual.run_until_parked();
        editor.read_with(visual, |editor, cx| {
            assert_eq!(
                editor.state.as_ref().unwrap().read(cx).value(),
                "SELECT '文';\n"
            );
        });
        editor.read_with(visual, |editor, _| {
            assert_eq!(editor.value(), "SELECT '文';\n");
            assert!(editor.validation_error.is_some());
        });
        visual.update(|window, cx| {
            window.press("secondary-a", cx);
            window.input("SELECT 3;", cx);
        });
        visual.run_until_parked();
        editor.read_with(visual, |editor, _| {
            assert_eq!(editor.value(), "SELECT 3;");
            assert!(editor.validation_error.is_none());
        });
        // Native text/IME commits must obey the byte cap too, not just paste.
        visual.update(|window, cx| {
            window.press("secondary-a", cx);
            let state = editor.read(cx).state.as_ref().unwrap().clone();
            state.update(cx, |state, cx| {
                state.replace_text_in_range(None, &"文".repeat(MAX_BYTES / 3 + 1), window, cx);
            });
        });
        visual.run_until_parked();
        visual.refresh().unwrap();
        visual.run_until_parked();
        editor.read_with(visual, |editor, cx| {
            assert_eq!(editor.value(), "SELECT 3;");
            assert_eq!(editor.state.as_ref().unwrap().read(cx).value(), "SELECT 3;");
            assert!(editor.validation_error.is_some());
        });
    }

    #[gpui::test]
    fn repaint_notifications_preserve_native_focus_selection_and_caret(cx: &mut TestAppContext) {
        let (editor, visual) = fixture(cx, "SELECT 1;");
        let state = editor.read_with(visual, |editor, _| editor.state.as_ref().unwrap().clone());
        visual.update(|window, cx| {
            window.press("secondary-a", cx);
        });
        visual.run_until_parked();
        // Query/result updates must not recreate Kit state or reset its selection.
        for _ in 0..3 {
            editor.update(visual, |_, cx| cx.notify());
            visual.refresh().unwrap();
            visual.run_until_parked();
            visual.update(|window, cx| {
                let editor = editor.read(cx);
                assert_eq!(editor.state.as_ref().unwrap(), &state);
                assert!(editor.focus_handle().is_focused(window));
                assert_eq!(state.read(cx).selected_text().to_string(), "SELECT 1;");
            });
        }
        visual.update(|window, cx| {
            window.press("right", cx);
            window.input(" -- caret", cx);
            assert_eq!(state.read(cx).value(), "SELECT 1; -- caret");
        });
    }

    #[gpui::test]
    fn caret_geometry_stays_inside_editor_after_lazy_focus_notifications_and_refocus(
        cx: &mut TestAppContext,
    ) {
        let (editor, visual) = fixture(cx, "SELECT '文';");
        visual.simulate_resize(gpui::size(gpui::px(800.), gpui::px(250.)));
        visual.refresh().unwrap();
        visual.run_until_parked();
        let state = editor.read_with(visual, |e, _| e.state.as_ref().unwrap().clone());
        visual.simulate_keystrokes("cmd-end");
        visual.run_until_parked();
        for _ in 0..3 {
            visual.update(|window, app| {
                window.blur(app);
            });
            visual.run_until_parked();
            visual.update(|window, app| editor.update(app, |editor, cx| editor.focus(window, cx)));
            visual.run_until_parked();
            editor.update(visual, |_, cx| cx.notify());
            visual.refresh().unwrap();
            visual.run_until_parked();
            let (caret, viewport) = state.read_with(visual, |s, _| {
                let mut caret = s.cursor_layout().expect("caret geometry").0;
                caret.origin.y += s.scroll_offset().y;
                (caret, s.input_bounds())
            });
            assert!(
                caret.size.width > gpui::px(0.)
                    && caret.left() >= viewport.left()
                    && caret.right() <= viewport.right(),
                "horizontal caret {caret:?} viewport {viewport:?}"
            );
            assert!(
                caret.size.height > gpui::px(0.)
                    && caret.top() >= viewport.top()
                    && caret.bottom() <= viewport.bottom(),
                "caret {caret:?} viewport {viewport:?}"
            );
        }
    }

    #[gpui::test]
    fn caret_follows_many_sql_lines_and_vertical_scroll_without_resetting_selection(
        cx: &mut TestAppContext,
    ) {
        let sql = (0..80)
            .map(|i| format!("-- {i} {}", "SQL column ".repeat(3)))
            .collect::<Vec<_>>()
            .join("\n");
        let (editor, visual) = fixture(cx, &sql);
        visual.simulate_resize(gpui::size(gpui::px(700.), gpui::px(180.)));
        visual.refresh().unwrap();
        visual.run_until_parked();
        let state = editor.read_with(visual, |e, _| e.state.as_ref().unwrap().clone());
        visual.update(|_, app| {
            state.update(app, |s, cx| s.set_selected_range(sql.len()..sql.len(), cx))
        });
        visual.run_until_parked();
        visual.update(|window, app| editor.update(app, |e, cx| e.focus(window, cx)));
        visual.refresh().unwrap();
        visual.run_until_parked();
        let (mut caret, viewport, offset) = state.read_with(visual, |s, _| {
            (
                s.cursor_layout().unwrap().0,
                s.input_bounds(),
                s.scroll_offset(),
            )
        });
        caret.origin.y += offset.y;
        assert!(
            caret.top() >= viewport.top() - gpui::px(1.)
                && caret.bottom() <= viewport.bottom() + gpui::px(1.),
            "caret {caret:?} viewport {viewport:?}"
        );
        assert!(
            caret.left() >= viewport.left() - gpui::px(1.)
                && caret.right() <= viewport.right() + gpui::px(1.),
            "caret {caret:?} viewport {viewport:?}"
        );
        assert_eq!(
            state.read_with(visual, |s, _| s.selected_range()),
            sql.len()..sql.len()
        );
    }

    #[gpui::test]
    fn lazy_focused_editor_starts_caret_blink_lifecycle(cx: &mut TestAppContext) {
        cx.update(gpui::init);
        let editor = cx.new(|cx| SqlEditor::new("SELECT 1;", cx));
        let (_, visual) = cx.add_window_view(|window, cx| {
            editor.read(cx).focus_handle().focus(window, cx);
            gpui::base::Root::new(editor.clone(), window, cx)
        });
        visual.refresh().unwrap();
        visual.run_until_parked();
        let state = editor.read_with(visual, |e, _| e.state.as_ref().unwrap().clone());
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let _observe = visual.update(|_, app| {
            let count = count.clone();
            app.observe(&state, move |_, _| {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            })
        });
        visual
            .executor()
            .advance_clock(std::time::Duration::from_millis(500));
        visual.run_until_parked();
        assert!(
            count.load(std::sync::atomic::Ordering::SeqCst) > 0,
            "focused editor never started caret blink lifecycle"
        );
    }

    #[gpui::test]
    fn focus_requested_before_lazy_initialization_transfers_to_kit(cx: &mut TestAppContext) {
        cx.update(gpui::init);
        let editor = cx.new(|cx| SqlEditor::new("", cx));
        let (_, visual) = cx.add_window_view(|window, cx| {
            editor.read(cx).focus_handle().focus(window, cx);
            gpui::base::Root::new(editor.clone(), window, cx)
        });
        visual.refresh().unwrap();
        visual.run_until_parked();
        visual.update(|window, cx| {
            assert!(editor.read(cx).focus_handle().is_focused(window));
            window.input("SELECT 4;", cx);
        });
        visual.run_until_parked();
        editor.read_with(visual, |editor, _| assert_eq!(editor.value(), "SELECT 4;"));
    }
}

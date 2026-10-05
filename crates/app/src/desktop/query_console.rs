//! Independent read-only SQL console. The draft belongs to the editor; result
//! notifications never reload it or disturb its selection/undo history.
use gpui::{
    App, Context, Div, Entity, FocusHandle, Focusable, KeyBinding, Stateful, Subscription, Window,
    actions, div, prelude::*, px, relative, rgb, uniform_list,
};

use super::{
    data_grid::DataGrid,
    icons::{Icon, icon},
    source_model::SourceModel,
    sql_editor::SqlEditor,
    theme::*,
};

actions!(query_console, [RunQuery, CancelQuery]);

pub(super) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-enter", RunQuery, Some("QueryConsole")),
        KeyBinding::new("cmd-.", CancelQuery, Some("QueryConsole")),
    ]);
}

pub(super) struct QueryConsole {
    model: Entity<SourceModel>,
    catalog: Entity<SourceModel>,
    editor: Entity<SqlEditor>,
    grid: Entity<DataGrid>,
    last_text: String,
    databases: Vec<Option<String>>,
    database_menu: bool,
    controls: [FocusHandle; 4],
    _subscriptions: Vec<Subscription>,
}

impl QueryConsole {
    pub(super) fn new(
        model: Entity<SourceModel>,
        catalog: Entity<SourceModel>,
        cx: &mut Context<Self>,
    ) -> Self {
        let initial_sql = model.read(cx).query_sql.clone();
        let editor = cx.new(|cx| {
            let mut editor = SqlEditor::new("", cx);
            editor.set_value(initial_sql, cx);
            editor
        });
        let last_text = editor.read(cx).value();
        let grid = cx.new(|cx| DataGrid::new(model.clone(), cx));
        let subscriptions = vec![
            cx.observe(&model, |_, _, cx| cx.notify()),
            cx.observe(&catalog, |this, _, cx| {
                this.refresh_databases(cx);
                cx.notify();
            }),
            cx.observe(&editor, |this, editor, cx| {
                let text = editor.read(cx).value();
                // Cursor/selection changes also notify. Do not mark those dirty,
                // invalidate work, or write identical SQL back to the model.
                if text != this.last_text {
                    this.last_text = text.clone();
                    this.model
                        .update(cx, |model, cx| model.set_query_sql(text, cx));
                }
                cx.notify();
            }),
        ];
        let mut this = Self {
            model,
            catalog,
            editor,
            grid,
            last_text,
            databases: Vec::new(),
            database_menu: false,
            controls: std::array::from_fn(|_| cx.focus_handle().tab_stop(true).tab_index(20)),
            _subscriptions: subscriptions,
        };
        this.refresh_databases(cx);
        this
    }

    pub(super) fn focus(&self, window: &mut Window, cx: &mut App) {
        self.editor.read(cx).focus_handle().focus(window);
    }

    fn refresh_databases(&mut self, cx: &mut Context<Self>) {
        let model = self.model.read(cx);
        let catalog = self.catalog.read(cx);
        let mut names = model
            .selected_source
            .as_ref()
            .and_then(|source| catalog.tree.databases.get(source))
            .cloned()
            .unwrap_or_default();
        if let Some(profile) = catalog
            .profiles
            .iter()
            .find(|profile| Some(&profile.id) == model.selected_source.as_ref())
            && let Some(database) = &profile.database
        {
            names.push(database.clone());
        }
        // The current context remains visible even if metadata was refreshed
        // while a query is running. No connection/discovery is initiated here.
        if let Some(database) = &model.selected_database {
            names.push(database.clone());
        }
        names.sort();
        names.dedup();
        self.databases = std::iter::once(None)
            .chain(names.into_iter().map(Some))
            .collect();
    }

    fn run(&mut self, cx: &mut Context<Self>) {
        if self.model.read(cx).busy || self.model.read(cx).saving {
            return;
        }
        let editor = self.editor.read(cx);
        if let Some(error) = editor.validation_error.clone() {
            self.model.update(cx, |model, cx| {
                model.error = Some(error);
                cx.notify();
            });
            return;
        }
        let sql = editor.selected_text().unwrap_or_else(|| editor.value());
        // Keep validation in the model: rejected SQL must never access Keychain
        // or the network, including runs issued through keyboard actions.
        self.model.update(cx, |model, cx| model.run_query(sql, cx));
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        if self.model.read(cx).busy {
            self.model.update(cx, |model, cx| model.cancel_query(cx));
        }
    }

    fn choose_database(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.model.read(cx).busy || self.model.read(cx).saving {
            return;
        }
        let Some(database) = self.databases.get(index).cloned() else {
            return;
        };
        let names = self.databases.iter().flatten().cloned().collect();
        self.model.update(cx, |model, cx| {
            // Metadata can arrive after this independent tab was forked.
            // Supply only the cached choices before the model validates them.
            model.databases = names;
            model.select_console_database(database, cx);
        });
        self.database_menu = false;
        self.focus(window, cx);
        cx.notify();
    }

    fn control(
        &self,
        index: usize,
        id: &'static str,
        disabled: bool,
        tooltip: &'static str,
        cx: &mut Context<Self>,
        activate: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + Clone + 'static,
    ) -> Stateful<Div> {
        let click = activate.clone();
        let focus = self.controls[index].clone();
        div()
            .id(id)
            .debug_selector(move || id.into())
            .track_focus(&focus)
            .tab_stop(!disabled)
            .flex()
            .items_center()
            .justify_center()
            .h(px(CONTROL_HEIGHT))
            .min_w(px(CONTROL_HEIGHT))
            .flex_shrink_0()
            .rounded(px(CONTROL_RADIUS))
            .border_1()
            .border_color(rgb(PANEL))
            .bg(rgb(PANEL))
            .text_color(rgb(if disabled { MUTED } else { TEXT }))
            .focus(|style| style.border_color(rgb(FOCUS)))
            .when(!disabled, |el| {
                el.cursor_pointer().hover(|style| style.bg(rgb(HOVER)))
            })
            .tooltip(move |_, cx| cx.new(|_| super::ControlTooltip(tooltip)).into())
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(move |this, _, window, _| {
                    if !disabled {
                        this.controls[index].focus(window);
                    }
                }),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                if !disabled {
                    click(this, window, cx);
                }
            }))
            .on_key_down(
                cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                    if !disabled && matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        cx.stop_propagation();
                        activate(this, window, cx);
                    }
                }),
            )
    }
}

impl Focusable for QueryConsole {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.editor.read(cx).focus_handle()
    }
}

impl Render for QueryConsole {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.model.read(cx);
        let busy = model.busy;
        let disabled = busy || model.saving;
        let database = model
            .selected_database
            .clone()
            .unwrap_or_else(|| "No default database".into());
        let export_disabled = disabled
            || model.export_busy
            || model.error.is_some()
            || model.page.as_ref().is_none_or(|page| page.truncated);
        let error = model.error.clone();
        let editor_error = self.editor.read(cx).validation_error.clone();
        let warnings = model.query_warnings.clone();
        let feedback = model.export_feedback.clone();
        let dirty_results =
            model.page.is_some() && (model.query_dirty || model.busy || model.error.is_some());
        let elapsed = model.query_elapsed_ms;
        let status = if busy {
            "Running…".to_owned()
        } else if let Some(page) = &model.page {
            format!(
                "{} loaded rows{}{}",
                page.rows.len(),
                if page.truncated {
                    " · truncated"
                } else if page.has_more {
                    " · row limit reached"
                } else {
                    ""
                },
                elapsed.map(|ms| format!(" · {ms} ms")).unwrap_or_default()
            )
        } else {
            "Run a single read-only query".to_owned()
        };

        let toolbar = div().flex().items_center().h(px(TOOLBAR_HEIGHT)).flex_shrink_0()
            .bg(rgb(PANEL)).border_b_1().border_color(rgb(BORDER)).gap(px(4.))
            .child(self.control(0, "query-run", disabled,
                "Run selection or entire query (Cmd-Enter)", cx, |this, _, cx| this.run(cx))
                .w(px(28.)).child(icon(Icon::Play, if disabled { MUTED } else { TEXT })))
            .child(self.control(1, "query-cancel", !busy,
                "Cancel query (Cmd-.)", cx, |this, _, cx| this.cancel(cx))
                .w(px(28.)).child(icon(Icon::Stop, if busy { TEXT } else { MUTED })))
            .child(self.control(2, "query-database", disabled,
                "Default database for unqualified table names; no default permits fully qualified queries",
                cx, |this, _, cx| {
                    if !this.model.read(cx).busy && !this.model.read(cx).saving {
                        this.database_menu = !this.database_menu;
                        cx.notify();
                    }
                }).max_w(px(280.)).px(px(6.)).gap(px(5.))
                .child(icon(Icon::Database, MUTED))
                .child(div().min_w(px(0.)).overflow_hidden().text_ellipsis().child(database))
                .child(icon(Icon::Chevron, MUTED)))
            .child(div().id("query-read-only").w(px(28.)).flex().justify_center()
                .child(icon(Icon::ReadOnly, MUTED))
                .tooltip(|_, cx| cx.new(|_| super::ControlTooltip(
                    "Read-only: one SELECT, CTE, or UNION query. No writes or multiple statements."
                )).into()))
            .child(div().flex_1())
            .child(self.control(3, "query-export", export_disabled,
                "Export only the loaded result rows to a new CSV file", cx, |this, _, cx| {
                    this.model.update(cx, |model, cx| model.request_export(cx));
                }).w(px(28.)).child(icon(Icon::Download, if export_disabled { MUTED } else { TEXT })));

        div()
            .id("query-console")
            .debug_selector(|| "query-console".into())
            .key_context("QueryConsole")
            .relative()
            .size_full()
            .min_h(px(0.))
            .min_w(px(0.))
            .flex()
            .flex_col()
            .bg(rgb(PANEL))
            .text_color(rgb(TEXT))
            .text_size(px(12.))
            .on_action(cx.listener(|this, _: &RunQuery, _, cx| this.run(cx)))
            .on_action(cx.listener(|this, _: &CancelQuery, _, cx| this.cancel(cx)))
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                // Escape dismisses only this popup; never cancel an idle console
                // or steal Escape from editor composition/workspace handlers.
                if event.keystroke.key == "escape" && this.database_menu {
                    this.database_menu = false;
                    this.focus(window, cx);
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .child(toolbar)
            .child(
                div()
                    .id("query-editor-pane")
                    .h(relative(0.42))
                    .min_h(px(120.))
                    .max_h(px(400.))
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(self.editor.clone()),
            )
            .when_some(editor_error, |el, error| el.child(notice(error, ERROR)))
            .when_some(error, |el, error| el.child(notice(error, ERROR)))
            .children(warnings.into_iter().map(|warning| notice(warning, WARNING)))
            .when(self.model.read(cx).result_evicted, |el| el.child(notice("Previous result released to limit memory. Run the query again to load results; the SQL draft is retained.".into(), MUTED)))
            .when(dirty_results, |el| {
                el.child(notice(
                    "Results belong to the submitted query; the draft has changed.".into(),
                    WARNING,
                ))
            })
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .min_w(px(0.))
                    .overflow_hidden()
                    .child(self.grid.clone()),
            )
            .when_some(feedback, |el, message| el.child(notice(message, MUTED)))
            .child(
                div()
                    .id("query-status")
                    .h(px(STATUS_HEIGHT))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .px(px(8.))
                    .text_color(rgb(MUTED))
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .child(status),
            )
            .when(self.database_menu && !disabled, |el| {
                el.child(
                    div()
                        .id("query-database-menu")
                        .debug_selector(|| "query-database-menu".into())
                        .absolute()
                        .top(px(TOOLBAR_HEIGHT))
                        .left(px(64.))
                        .w(px(280.))
                        .h(px((self.databases.len() as f32 * 28.).min(224.)))
                        .bg(rgb(CHROME))
                        .border_1()
                        .border_color(rgb(INPUT_BORDER))
                        .occlude()
                        .child(
                            uniform_list(
                                "query-database-list",
                                self.databases.len(),
                                cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                                    range
                                        .map(|index| {
                                            let database = this.databases[index].clone();
                                            let selected =
                                                this.model.read(cx).selected_database == database;
                                            let id: gpui::SharedString =
                                                format!("query-database-{index}").into();
                                            let debug_id = id.clone();
                                            div()
                                                .id(id)
                                                .debug_selector(move || debug_id.clone().into())
                                                .tab_index(20)
                                                .tab_stop(true)
                                                .border_1()
                                                .border_color(rgb(CHROME))
                                                .focus(|style| style.border_color(rgb(FOCUS)))
                                                .on_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                                                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                                        cx.stop_propagation();
                                                        this.choose_database(index, window, cx);
                                                    }
                                                }))
                                                .h(px(28.))
                                                .px(px(8.))
                                                .flex()
                                                .items_center()
                                                .bg(rgb(if selected { SELECTION } else { CHROME }))
                                                .text_color(rgb(if selected {
                                                    FOCUS
                                                } else {
                                                    TEXT
                                                }))
                                                .cursor_pointer()
                                                .hover(|style| style.bg(rgb(HOVER)))
                                                .child(database.unwrap_or_else(|| {
                                                    "No default database".into()
                                                }))
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        this.choose_database(index, window, cx);
                                                    },
                                                ))
                                        })
                                        .collect()
                                }),
                            )
                            .size_full(),
                        ),
                )
            })
    }
}

fn notice(message: String, color: u32) -> Div {
    // Wrap real validation/server errors rather than hiding them behind a tooltip.
    div()
        .flex_shrink_0()
        .px(px(8.))
        .py(px(4.))
        .text_color(rgb(color))
        .child(message)
}

#[cfg(all(test, feature = "ui-tests"))]
mod tests {
    use super::*;
    use dalan_app::workspace_tabs::WorkspaceOpen;
    use dalan_drivers::SourceProfile;
    use gpui::TestAppContext;

    fn fixture(cx: &mut TestAppContext) -> (Entity<QueryConsole>, Entity<SourceModel>) {
        let profile = SourceProfile {
            database: Some("quoted ` database".into()),
            ..Default::default()
        };
        let source = profile.id.clone();
        let root = cx.new(|_| SourceModel::for_tests(vec![profile]));
        let model = root.read_with(cx, |root, _| {
            root.fork_for_workspace(&WorkspaceOpen::Console {
                source,
                database: Some("quoted ` database".into()),
            })
            .unwrap()
        });
        let model = cx.new(|_| model);
        let view = cx.new(|cx| QueryConsole::new(model.clone(), root, cx));
        (view, model)
    }

    #[gpui::test]
    fn editor_draft_notifications_do_not_run_or_reload_sql(cx: &mut TestAppContext) {
        let (view, model) = fixture(cx);
        let editor = view.read_with(cx, |view, _| view.editor.clone());
        editor.update(cx, |editor, cx| {
            editor.set_value("SELECT 1\n-- draft".into(), cx)
        });
        cx.run_until_parked();
        model.read_with(cx, |model, _| {
            assert_eq!(model.query_sql, "SELECT 1\n-- draft");
            assert!(model.query_dirty);
            assert!(!model.busy);
            assert!(model.page.is_none());
        });
        model.update(cx, |model, cx| {
            model.error = Some("Result notification".into());
            model.query_sql = "SELECT 2".into();
            cx.notify();
        });
        cx.run_until_parked();
        assert_eq!(
            editor.read_with(cx, |editor, _| editor.value()),
            "SELECT 1\n-- draft"
        );
        // Identical editor notifications must not overwrite externally updated state.
        editor.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        assert_eq!(
            model.read_with(cx, |model, _| model.query_sql.clone()),
            "SELECT 2"
        );
    }

    #[gpui::test]
    fn unsafe_sql_and_busy_runs_are_rejected_without_starting_work(cx: &mut TestAppContext) {
        let (view, model) = fixture(cx);
        let editor = view.read_with(cx, |view, _| view.editor.clone());
        editor.update(cx, |editor, cx| {
            editor.set_value("UPDATE items SET x = 1".into(), cx)
        });
        cx.run_until_parked();
        view.update(cx, |view, cx| view.run(cx));
        model.read_with(cx, |model, _| {
            assert!(model.error.is_some());
            assert!(!model.busy);
            assert!(model.query_submitted_sql.is_none());
        });
        model.update(cx, |model, _| {
            model.error = None;
            model.busy = true;
        });
        view.update(cx, |view, cx| view.run(cx));
        assert!(model.read_with(cx, |model, _| model.error.is_none()));
        view.update(cx, |view, cx| view.cancel(cx));
        assert!(!model.read_with(cx, |model, _| model.busy));
        let message = model.read_with(cx, |model, _| model.error.clone());
        view.update(cx, |view, cx| view.cancel(cx));
        assert_eq!(model.read_with(cx, |model, _| model.error.clone()), message);
        view.read_with(cx, |view, _| {
            assert_eq!(view.databases[0], None);
            assert_eq!(view.databases[1].as_deref(), Some("quoted ` database"));
        });
    }
    #[gpui::test]
    fn database_popup_uses_cached_names_and_guards_busy_changes(cx: &mut TestAppContext) {
        let (view, model) = fixture(cx);
        let (_, cx) = cx.add_window_view(|_, _| ConsoleTestRoot(view.clone()));
        cx.simulate_resize(gpui::size(px(900.), px(600.)));
        cx.refresh().unwrap();
        cx.run_until_parked();
        let bounds = cx.debug_bounds("query-database").unwrap();
        cx.simulate_click(bounds.center(), gpui::Modifiers::default());
        cx.run_until_parked();
        assert!(cx.debug_bounds("query-database-menu").is_some());
        let bounds = cx.debug_bounds("query-database-0").unwrap();
        cx.simulate_click(bounds.center(), gpui::Modifiers::default());
        cx.run_until_parked();
        assert!(model.read_with(cx, |model, _| model.selected_database.is_none()));
        assert!(!view.read_with(cx, |view, _| view.database_menu));
        assert!(cx.update(|window, app| view.read(app).focus_handle(app).is_focused(window)));
        model.update(cx, |model, cx| {
            model.busy = true;
            cx.notify();
        });
        cx.run_until_parked();
        let bounds = cx.debug_bounds("query-database").unwrap();
        cx.simulate_click(bounds.center(), gpui::Modifiers::default());
        cx.run_until_parked();
        assert!(!view.read_with(cx, |view, _| view.database_menu));
        assert!(model.read_with(cx, |model, _| model.selected_database.is_none()));
    }

    struct ConsoleTestRoot(Entity<QueryConsole>);
    impl Render for ConsoleTestRoot {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(self.0.clone())
        }
    }
}

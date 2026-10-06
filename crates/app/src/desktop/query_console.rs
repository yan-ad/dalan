//! Independent read-only SQL console. The draft belongs to the editor; result
//! notifications never reload it or disturb its selection/undo history.
use gpui::{
    App, Context, Div, Entity, FocusHandle, Focusable, KeyBinding, SharedString, Subscription,
    Window, actions, div, prelude::*, px, relative,
};

use super::{
    data_grid::DataGrid,
    source_model::SourceModel,
    sql_editor::SqlEditor,
    theme::{STATUS_HEIGHT, TOOLBAR_HEIGHT},
};

use gpui::component::{
    ActiveTheme, Disableable, Icon, IndexPath, Sizable,
    button::{Button as KitButton, ButtonVariants},
    combobox::{Combobox, ComboboxEvent, ComboboxState},
    menu::{DropdownMenu, PopupMenuItem},
    searchable_list::{SearchableListItem, SearchableVec},
};

/// The absent default is a real value, not a string sentinel: a database may
/// itself be named "No default database".
#[derive(Clone, Debug)]
struct DbChoice {
    label: SharedString,
    value: Option<String>,
}

impl DbChoice {
    fn new(value: Option<String>) -> Self {
        Self {
            label: value
                .clone()
                .unwrap_or_else(|| "No default database".into())
                .into(),
            value,
        }
    }
}

impl SearchableListItem for DbChoice {
    type Value = Option<String>;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

type DatabaseComboState = ComboboxState<SearchableVec<DbChoice>>;

actions!(
    query_console,
    [
        RunQuery,
        CancelQuery,
        FormatSql,
        SaveSql,
        OpenSql,
        PasteInCondition
    ]
);

pub(super) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-enter", RunQuery, Some("QueryConsole")),
        KeyBinding::new("cmd-.", CancelQuery, Some("QueryConsole")),
        KeyBinding::new("cmd-alt-l", FormatSql, Some("QueryConsole")),
        KeyBinding::new("cmd-s", SaveSql, Some("QueryConsole")),
        KeyBinding::new("cmd-o", OpenSql, Some("QueryConsole")),
        KeyBinding::new("cmd-shift-v", PasteInCondition, Some("QueryConsole")),
    ]);
}

pub(super) struct QueryConsole {
    model: Entity<SourceModel>,
    catalog: Entity<SourceModel>,
    editor: Entity<SqlEditor>,
    grid: Entity<DataGrid>,
    last_text: String,
    databases: Vec<Option<String>>,
    database_combo: Option<Entity<DatabaseComboState>>,
    database_items_changed: bool,
    uppercase_keywords: bool,
    file_busy: bool,
    tool_feedback: Option<String>,
    _subscriptions: Vec<Subscription>,
}

enum EitherFilePicker {
    Save(futures::channel::oneshot::Receiver<anyhow::Result<Option<std::path::PathBuf>>>),
    Open(futures::channel::oneshot::Receiver<anyhow::Result<Option<Vec<std::path::PathBuf>>>>),
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
            cx.observe(&model, |this, _, cx| {
                // External context changes can introduce a database not in the catalog.
                if !this
                    .databases
                    .contains(&this.model.read(cx).selected_database)
                {
                    this.refresh_databases(cx);
                }
                cx.notify();
            }),
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
            database_combo: None,
            database_items_changed: false,
            uppercase_keywords: true,
            file_busy: false,
            tool_feedback: None,
            _subscriptions: subscriptions,
        };
        this.refresh_databases(cx);
        this
    }

    pub(super) fn focus(&self, window: &mut Window, cx: &mut App) {
        self.editor
            .update(cx, |editor, cx| editor.focus(window, cx));
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
            && let Some(database) = profile.resolved().ok().and_then(|p| p.database)
        {
            names.push(database);
        }
        // The current context remains visible even if metadata was refreshed
        // while a query is running. No connection/discovery is initiated here.
        if let Some(database) = &model.selected_database {
            names.push(database.clone());
        }
        names.sort();
        names.dedup();
        let databases = std::iter::once(None)
            .chain(names.into_iter().map(Some))
            .collect::<Vec<_>>();
        if self.databases != databases {
            self.databases = databases;
            self.database_items_changed = true;
        }
    }

    /// Construction remains windowless; Kit input/list state is created once when
    /// the console first renders, and retained across result/scroll notifications.
    fn database_combo(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<DatabaseComboState> {
        let selected = self.model.read(cx).selected_database.clone();
        if self.database_combo.is_none() {
            let items = SearchableVec::new(
                self.databases
                    .iter()
                    .cloned()
                    .map(DbChoice::new)
                    .collect::<Vec<_>>(),
            );
            let index = self
                .databases
                .iter()
                .position(|value| *value == selected)
                .unwrap_or(0);
            let state = cx.new(|cx| {
                ComboboxState::new(items, vec![IndexPath::new(index)], window, cx).searchable(true)
            });
            self._subscriptions.push(cx.subscribe_in(
                &state,
                window,
                |this, state, event, window, cx| {
                    // Kit emits both Change and Confirm; only Change mutates context.
                    let ComboboxEvent::Change(values) = event else {
                        return;
                    };
                    let Some(database) = values.first() else {
                        return;
                    };
                    let model = this.model.read(cx);
                    let current = model.selected_database.clone();
                    if model.busy || model.saving || !this.databases.contains(database) {
                        state.update(cx, |state, cx| {
                            state.set_selected_values(&[current], window, cx);
                        });
                        return;
                    }
                    if *database != current {
                        this.choose_database(database.clone(), window, cx);
                    }
                },
            ));
            self.database_combo = Some(state);
            self.database_items_changed = false;
        }
        let state = self.database_combo.as_ref().unwrap().clone();
        if self.database_items_changed {
            let items = SearchableVec::new(
                self.databases
                    .iter()
                    .cloned()
                    .map(DbChoice::new)
                    .collect::<Vec<_>>(),
            );
            state.update(cx, |state, cx| {
                state.set_items(items, window, cx);
                state.set_selected_values(std::slice::from_ref(&selected), window, cx);
            });
            self.database_items_changed = false;
        } else if state.read(cx).selected_value() != Some(selected.clone()) {
            state.update(cx, |state, cx| {
                state.set_selected_values(&[selected], window, cx);
            });
        }
        state
    }

    fn tool(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.model.read(cx).busy
            || self.model.read(cx).saving
            || self.catalog.read(cx).saving
            || self.file_busy
        {
            return;
        }
        self.tool_feedback = None;
        if matches!(
            id,
            "query-format" | "query-compress" | "query-keyword-case" | "query-paste-in"
        ) && !matches!(
            self.model.read(cx).selected_engine(),
            dalan_drivers::DbEngine::MySql | dalan_drivers::DbEngine::MariaDb
        ) {
            self.tool_feedback=Some("MySQL-only formatting/IN tools are unavailable for this engine; input is unchanged.".into());
            cx.notify();
            return;
        }
        match id {
            "query-format" => {
                let uppercase = self.uppercase_keywords;
                self.editor.update(cx, |e, cx| {
                    e.apply_tool(
                        dalan_app::sql_tools::SqlTransform::Format { uppercase },
                        window,
                        cx,
                    )
                });
            }
            "query-compress" => self.editor.update(cx, |e, cx| {
                e.apply_tool(dalan_app::sql_tools::SqlTransform::Compress, window, cx)
            }),
            "query-keyword-case" => self.uppercase_keywords = !self.uppercase_keywords,
            "query-wrap" => {
                let wrap = !self.editor.read(cx).soft_wrap;
                self.editor
                    .update(cx, |e, cx| e.set_soft_wrap(wrap, window, cx));
            }
            "query-unfold" => self.editor.update(cx, |e, cx| e.unfold_all(window, cx)),
            "query-paste-in" => {
                if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
                    match dalan_app::sql_tools::in_condition(&text) {
                        Ok(text) => self
                            .editor
                            .update(cx, |e, cx| e.insert_text(text, window, cx)),
                        Err(_) => {
                            self.tool_feedback = Some(
                                "Clipboard cannot be converted to a bounded SQL IN list.".into(),
                            )
                        }
                    }
                }
            }
            "query-open-sql" => self.sql_file(false, window, cx),
            "query-save-sql" => self.sql_file(true, window, cx),
            "query-clear-database" => self.choose_database(None, window, cx),
            "query-set-default" => {
                let model = self.model.read(cx);
                let id = model.selected_source.clone();
                let database = model.selected_database.clone();
                if let (Some(id), Some(database)) = (id, database) {
                    let is_default = self
                        .catalog
                        .read(cx)
                        .profiles
                        .iter()
                        .find(|p| p.id == id)
                        .is_some_and(|p| p.database.as_ref() == Some(&database));
                    self.catalog.update(cx, |m, cx| {
                        m.set_source_default_database(
                            id,
                            if is_default { None } else { Some(database) },
                            cx,
                        )
                    });
                }
            }
            _ => {}
        }
        if !matches!(
            id,
            "query-open-sql" | "query-save-sql" | "query-set-default"
        ) {
            self.focus(window, cx);
        }
        cx.notify();
    }
    fn sql_file(&mut self, save: bool, window: &mut Window, cx: &mut Context<Self>) {
        let sql = self.editor.read(cx).value();
        let before = sql.clone();
        let revision = self.editor.read(cx).edit_revision();
        self.file_busy = true;
        let picker = if save {
            let home = std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| ".".into());
            EitherFilePicker::Save(cx.prompt_for_new_path(&home, Some("query.sql")))
        } else {
            EitherFilePicker::Open(cx.prompt_for_paths(gpui::PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: Some("Open SQL file (UTF-8, at most 64 KiB)".into()),
            }))
        };
        cx.spawn_in(window,async move|this,cx|{
            let path=match picker {EitherFilePicker::Save(picker)=>match picker.await{Ok(Ok(path))=>path,_=>None},EitherFilePicker::Open(picker)=>match picker.await{Ok(Ok(Some(mut paths)))if paths.len()==1=>paths.pop(),_=>None}};
            let Some(path)=path else {let _=this.update_in(cx,|this,_,cx|{this.file_busy=false;this.tool_feedback=Some("SQL file action cancelled.".into());cx.notify();});return;};
            let task=cx.background_executor().spawn(async move {if save{dalan_app::sql_tools::write_sql(&path,&sql).map(|()|None)}else{dalan_app::sql_tools::read_sql(&path).map(Some)}});
            let result=task.await;
            let _=this.update_in(cx,|this,window,cx|{this.file_busy=false;match result {
                Ok(Some(text))=>{if this.editor.read(cx).value()!=before||this.editor.read(cx).edit_revision()!=revision{this.tool_feedback=Some("SQL file was not loaded because the draft changed; open it again.".into());}else{this.editor.update(cx,|e,cx|e.replace_all_undoable(text,window,cx));this.tool_feedback=Some("SQL file loaded. Undo restores the prior draft.".into());}},
                Ok(None)=>this.tool_feedback=Some("SQL saved to a new file. Existing files are never overwritten.".into()),
                Err(_)=>this.tool_feedback=Some("SQL file action failed: check UTF-8/64 KiB bounds, access permissions and a new non-symlink destination.".into()),
            }cx.notify();});
        }).detach();
        cx.notify();
    }
    fn source_control(&self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.model.read(cx).selected_source.clone();
        let profile = self
            .catalog
            .read(cx)
            .profiles
            .iter()
            .find(|p| Some(&p.id) == current.as_ref());
        let label = profile
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "Select source".into());
        let profiles = self.catalog.read(cx).profiles.clone();
        let entity = cx.entity().downgrade();
        let disabled = self.model.read(cx).busy || self.catalog.read(cx).saving || self.file_busy;
        KitButton::new("query-source")
            .debug_selector(|| "query-source".into())
            .label(label.clone())
            .tooltip(label)
            .small()
            .ghost()
            .dropdown_caret(true)
            .disabled(disabled)
            .max_w(px(width))
            .dropdown_menu(move |mut menu, _, _| {
                for (index, p) in profiles.iter().enumerate() {
                    let p = p.clone();
                    let entity = entity.clone();
                    let supported = true;
                    let selected = Some(&p.id) == current.as_ref();
                    let id = p.id.clone();
                    menu = menu.item(
                        PopupMenuItem::element(move |_, _| {
                            div()
                                .debug_selector(move || format!("query-source-choice-{index}"))
                                .flex()
                                .gap_2()
                                .child(super::icons::provider_icon(p.engine))
                                .child(p.name.clone())
                        })
                        .checked(selected)
                        .disabled(disabled || !supported)
                        .on_click(move |_, window, cx| {
                            let _ = entity.update(cx, |this, cx| {
                                if this.model.read(cx).busy
                                    || this.model.read(cx).saving
                                    || this.catalog.read(cx).saving
                                    || this.file_busy
                                {
                                    return;
                                }
                                let names = this
                                    .catalog
                                    .read(cx)
                                    .tree
                                    .databases
                                    .get(&id)
                                    .cloned()
                                    .unwrap_or_default();
                                this.model.update(cx, |m, cx| {
                                    m.select_console_source(id.clone(), names, cx)
                                });
                                this.refresh_databases(cx);
                                this.focus(window, cx);
                                cx.notify();
                            });
                        }),
                    );
                }
                menu
            })
    }
    fn tool_button(
        &self,
        id: &'static str,
        label: &'static str,
        icon: gpui::assets::IconName,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> KitButton {
        KitButton::new(id)
            .debug_selector(move || id.into())
            .icon(icon)
            .small()
            .ghost()
            .tooltip(label)
            .disabled(disabled)
            .on_click(cx.listener(move |this, _, window, cx| this.tool(id, window, cx)))
    }
    fn more_tools(&self, disabled: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity().downgrade();
        let upper = self.uppercase_keywords;
        let wrap = self.editor.read(cx).soft_wrap;
        let default = self
            .model
            .read(cx)
            .selected_database
            .clone()
            .is_some_and(|db| {
                self.catalog
                    .read(cx)
                    .profiles
                    .iter()
                    .find(|p| Some(&p.id) == self.model.read(cx).selected_source.as_ref())
                    .is_some_and(|p| p.database.as_ref() == Some(&db))
            });
        let no_database = self.model.read(cx).selected_database.is_none();
        let mysql = matches!(
            self.model.read(cx).selected_engine(),
            dalan_drivers::DbEngine::MySql | dalan_drivers::DbEngine::MariaDb
        );
        self.tool_button(
            "query-more",
            "More actions",
            gpui::assets::IconName::Ellipsis,
            disabled,
            cx,
        )
        .dropdown_menu(move |mut menu, _, _| {
            for (id, label, checked, extra_disabled) in [
                ("query-compress", "Compress SQL", false, false),
                ("query-unfold", "Unfold all", false, false),
                (
                    "query-keyword-case",
                    if upper {
                        "Use lower-case SQL keywords"
                    } else {
                        "Use upper-case SQL keywords"
                    },
                    false,
                    false,
                ),
                ("query-wrap", "Word wrap", wrap, false),
                ("query-format", "Format SQL (Cmd-Alt-L)", false, false),
                ("query-open-sql", "Open SQL file (Cmd-O)", false, false),
                (
                    "query-save-sql",
                    "Save SQL to new file (Cmd-S)",
                    false,
                    false,
                ),
                (
                    "query-paste-in",
                    "Paste as IN condition (Cmd-Shift-V)",
                    false,
                    false,
                ),
                ("query-clear-database", "Clear database", false, no_database),
                (
                    "query-set-default",
                    if default {
                        "Clear default database"
                    } else {
                        "Set Default"
                    },
                    default,
                    no_database,
                ),
            ] {
                let entity = entity.clone();
                menu = menu.item(
                    PopupMenuItem::element(move |_, _| {
                        div()
                            .debug_selector(move || format!("{id}-menu"))
                            .child(label)
                    })
                    .checked(checked)
                    .disabled(
                        disabled
                            || extra_disabled
                            || (!mysql
                                && matches!(
                                    id,
                                    "query-format"
                                        | "query-compress"
                                        | "query-keyword-case"
                                        | "query-paste-in"
                                )),
                    )
                    .on_click(move |_, window, cx| {
                        let _ = entity.update(cx, |this, cx| this.tool(id, window, cx));
                    }),
                );
            }
            menu
        })
    }

    fn run(&mut self, cx: &mut Context<Self>) {
        if self.model.read(cx).busy
            || self.model.read(cx).saving
            || self.catalog.read(cx).saving
            || self.file_busy
        {
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
        // Keep validation in the model: rejected SQL must never access saved auth
        // or the network, including runs issued through keyboard actions.
        self.model.update(cx, |model, cx| model.run_query(sql, cx));
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        if self.model.read(cx).busy {
            self.model.update(cx, |model, cx| model.cancel_query(cx));
        }
    }

    fn choose_database(
        &mut self,
        database: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.model.read(cx).busy
            || self.model.read(cx).saving
            || self.catalog.read(cx).saving
            || self.file_busy
        {
            return;
        }
        // A metadata notification can reorder choices while the popup is open.
        // Resolve the captured value, not an index into the refreshed catalog.
        if !self.databases.contains(&database) {
            return;
        }
        let names = self.databases.iter().flatten().cloned().collect();
        self.model.update(cx, |model, cx| {
            // Metadata can arrive after this independent tab was forked.
            // Supply only the cached choices before the model validates them.
            model.databases = names;
            model.select_console_database(database, cx);
        });
        self.focus(window, cx);
        cx.notify();
    }
}

impl Focusable for QueryConsole {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.editor.read(cx).focus_handle()
    }
}

impl Render for QueryConsole {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let database_combo = self.database_combo(window, cx);
        let engine = self.model.read(cx).selected_engine();
        self.editor
            .update(cx, |e, cx| e.set_engine(engine, window, cx));
        let is_mysql = matches!(
            engine,
            dalan_drivers::DbEngine::MySql | dalan_drivers::DbEngine::MariaDb
        );
        let is_sql = !matches!(
            engine,
            dalan_drivers::DbEngine::MongoDb | dalan_drivers::DbEngine::Redis
        );
        let model = self.model.read(cx);
        let busy = model.busy;
        let saving = model.saving;
        let selected_database = model.selected_database.clone();
        let selected_source = model.selected_source.clone();
        let disabled = busy || saving || self.catalog.read(cx).saving || self.file_busy;
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
            match engine {
                dalan_drivers::DbEngine::MongoDb => {
                    "Read-only JSON find: {\"find\":\"collection\",\"filter\":{}}".into()
                }
                dalan_drivers::DbEngine::Redis => {
                    "Read-only JSON command: [\"GET\",\"key\"]".into()
                }
                _ => "Run a single read-only SQL query".into(),
            }
        };

        let theme = cx.theme().clone();
        let available: f32 = self
            .editor
            .read(cx)
            .viewport_width(cx)
            .unwrap_or_else(|| f32::from(window.bounds().size.width) - 300.);
        let full = available >= 800.;
        let database_width = if available < 400. {
            100.
        } else if full {
            200.
        } else if available < 500. {
            130.
        } else {
            170.
        };
        let source_width = if full {
            160.
        } else if available < 400. {
            72.
        } else {
            100.
        };
        let mut toolbar = div()
            .id("query-toolbar")
            .debug_selector(|| "query-toolbar".into())
            .flex()
            .items_center()
            .h(px(TOOLBAR_HEIGHT))
            .flex_shrink_0()
            .min_w_0()
            .border_b_1()
            .border_color(theme.border)
            .gap(px(2.));
        toolbar = toolbar
            .child(
                KitButton::new("query-run")
                    .small()
                    .ghost()
                    .debug_selector(|| "query-run".into())
                    .icon(Icon::empty().path(if busy {
                        "icons/circle-stop.svg"
                    } else {
                        "icons/play.svg"
                    }))
                    .disabled(
                        model.saving
                            || self.catalog.read(cx).saving
                            || self.file_busy
                            || (!busy && self.editor.read(cx).value().trim().is_empty()),
                    )
                    .tooltip(if busy {
                        "Stop query (Cmd-.)"
                    } else {
                        "Run selection or entire query (Cmd-Enter)"
                    })
                    .on_click(cx.listener(|this, _, window, cx| {
                        if this.model.read(cx).busy {
                            this.cancel(cx);
                        } else {
                            this.run(cx);
                            this.focus(window, cx);
                        }
                    })),
            )
            .child(
                KitButton::new("query-cancel")
                    .small()
                    .ghost()
                    .debug_selector(|| "query-cancel".into())
                    .icon(Icon::empty().path("icons/circle-stop.svg"))
                    .disabled(!busy)
                    .tooltip("Cancel query (Cmd-.)")
                    .on_click(cx.listener(|this, _, _, cx| this.cancel(cx))),
            );
        if full {
            for (id, label, icon) in [
                (
                    "query-format",
                    "Format SQL (Cmd-Alt-L)",
                    gpui::assets::IconName::TextAlignStart,
                ),
                (
                    "query-compress",
                    "Compress SQL",
                    gpui::assets::IconName::Minimize,
                ),
                (
                    "query-keyword-case",
                    "Toggle keyword case preference (used by Format)",
                    gpui::assets::IconName::Type,
                ),
                ("query-wrap", "Word wrap", gpui::assets::IconName::TextWrap),
                (
                    "query-save-sql",
                    "Save SQL to new file (Cmd-S)",
                    gpui::assets::IconName::Save,
                ),
                (
                    "query-open-sql",
                    "Open SQL file (Cmd-O)",
                    gpui::assets::IconName::FolderOpen,
                ),
                (
                    "query-paste-in",
                    "Paste as IN condition (Cmd-Shift-V)",
                    gpui::assets::IconName::Clipboard,
                ),
            ] {
                let sql_tool = matches!(
                    id,
                    "query-format" | "query-compress" | "query-keyword-case" | "query-paste-in"
                );
                toolbar = toolbar.child(self.tool_button(
                    id,
                    label,
                    icon,
                    disabled || (sql_tool && !is_mysql),
                    cx,
                ));
            }
        }
        toolbar = toolbar
            .child(self.more_tools(disabled, cx))
            .child(div().flex_1().min_w_0())
            .child(self.source_control(source_width, cx))
            .child(
                div()
                    .id("query-database")
                    .debug_selector(|| "query-database".into())
                    .child(
                        Combobox::new(&database_combo)
                            .small()
                            .w(px(database_width))
                            .menu_max_h(px(224.))
                            .search_placeholder("Search databases...")
                            .placeholder("No default database")
                            .disabled(disabled)
                            .render_trigger(|trigger, _, _| {
                                let label = trigger
                                    .selection()
                                    .first()
                                    .map(|(_, item)| item.title())
                                    .unwrap_or_else(|| "No default database".into());
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .child(Icon::empty().path("icons/database.svg").size(px(14.)))
                                    .child(
                                        div()
                                            .flex_1()
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .child(label),
                                    )
                                    .child(
                                        Icon::empty().path("icons/chevron-down.svg").size(px(12.)),
                                    )
                            }),
                    ),
            )
            .child(self.tool_button(
                "query-clear-database",
                "Clear database",
                gpui::assets::IconName::X,
                disabled || selected_database.is_none(),
                cx,
            ));
        if full && is_sql {
            let is_default = selected_database.as_ref().is_some_and(|db| {
                self.catalog
                    .read(cx)
                    .profiles
                    .iter()
                    .find(|p| Some(&p.id) == selected_source.as_ref())
                    .is_some_and(|p| p.database.as_ref() == Some(db))
            });
            toolbar = toolbar.child(
                KitButton::new("query-set-default")
                    .debug_selector(|| "query-set-default".into())
                    .label(if is_default { "Default" } else { "Set Default" })
                    .small()
                    .ghost()
                    .disabled(
                        disabled || selected_database.is_none() || self.catalog.read(cx).form_open,
                    )
                    .tooltip("Persist or clear this source’s default database")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.tool("query-set-default", window, cx)
                    })),
            );
        }
        toolbar = toolbar.child(
            KitButton::new("query-export")
                .small()
                .ghost()
                .debug_selector(|| "query-export".into())
                .icon(Icon::empty().path("icons/download.svg"))
                .disabled(export_disabled)
                .tooltip("Export loaded result rows to a new CSV file")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.model.update(cx, |m, cx| m.request_export(cx))
                })),
        );

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
            .bg(theme.background)
            .text_color(theme.foreground)
            .text_size(px(12.))
            .capture_action(cx.listener(|this,action:&gpui::component::input::Enter,window,cx|{
                if action.secondary && !action.shift && this.editor.read(cx).focus_handle().is_focused(window) {
                    cx.stop_propagation();
                    this.run(cx);
                    this.focus(window,cx);
                }
            }))
            .on_action(cx.listener(|this, _: &RunQuery, window, cx| { this.run(cx); this.focus(window, cx); }))
            .on_action(cx.listener(|this, _: &CancelQuery, _, cx| this.cancel(cx)))
            .on_action(cx.listener(|this,_:&FormatSql,window,cx|this.tool("query-format",window,cx)))
            .on_action(cx.listener(|this,_:&SaveSql,window,cx|this.tool("query-save-sql",window,cx)))
            .on_action(cx.listener(|this,_:&OpenSql,window,cx|this.tool("query-open-sql",window,cx)))
            .on_action(cx.listener(|this,_:&PasteInCondition,window,cx|this.tool("query-paste-in",window,cx)))
            .child(toolbar)
            .child(
                div()
                    .id("query-editor-pane")
                    .h(relative(0.42))
                    .min_h(px(120.))
                    .max_h(px(400.))
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(self.editor.clone()),
            )
            .when_some(self.tool_feedback.clone().or_else(||self.catalog.read(cx).metadata_notice.clone()),|el,feedback|el.child(notice(feedback,theme.muted_foreground)))
            .when_some(editor_error, |el, error| el.child(notice(error, theme.danger)))
            .when_some(error, |el, error| el.child(notice(error, theme.danger)))
            .children(warnings.into_iter().map(|warning| notice(warning, theme.warning)))
            .when(self.model.read(cx).result_evicted, |el| el.child(notice("Previous result released to limit memory. Run the query again to load results; the SQL draft is retained.".into(), theme.muted_foreground)))
            .when(dirty_results, |el| {
                el.child(notice(
                    "Results belong to the submitted query; the draft has changed.".into(),
                    theme.warning,
                ))
            })
            .child(
                div()
                    .id("query-result-pane")
                    .debug_selector(|| "query-result-pane".into())
                    // DataGrid is a flex child: a plain block wrapper leaves its
                    // auto height at zero despite this pane having spare space.
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.))
                    .min_w(px(0.))
                    .overflow_hidden()
                    .child(self.grid.clone()),
            )
            .when_some(feedback, |el, message| el.child(notice(message, theme.muted_foreground)))
            .child(
                div()
                    .id("query-status")
                    .h(px(STATUS_HEIGHT))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .px(px(8.))
                    .text_color(theme.muted_foreground)
                    .border_t_1()
                    .border_color(theme.border)
                    .child(status),
            )
    }
}

fn notice(message: String, color: gpui::Hsla) -> Div {
    // Wrap real validation/server errors rather than hiding them behind a tooltip.
    div()
        .flex_shrink_0()
        .px(px(8.))
        .py(px(4.))
        .text_color(color)
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
        fixture_with_profile(cx, profile)
    }

    fn fixture_with_profile(
        cx: &mut TestAppContext,
        profile: SourceProfile,
    ) -> (Entity<QueryConsole>, Entity<SourceModel>) {
        cx.update(gpui::init);
        cx.update(bind_keys);
        let source = profile.id.clone();
        let database = profile.database.clone();
        let root = cx.new(|_| SourceModel::for_tests(vec![profile]));
        let model = root.read_with(cx, |root, _| {
            root.fork_for_workspace(&WorkspaceOpen::Console { source, database })
                .unwrap()
        });
        let model = cx.new(|_| model);
        let view = cx.new(|cx| QueryConsole::new(model.clone(), root, cx));
        (view, model)
    }

    fn click_tool(cx: &mut gpui::VisualTestContext, id: &'static str) {
        cx.run_until_parked();
        let bounds = cx
            .debug_bounds(id)
            .unwrap_or_else(|| panic!("missing {id}"));
        cx.simulate_click(bounds.center(), gpui::Modifiers::default());
        cx.run_until_parked();
    }
    #[gpui::test]
    fn toolbar_local_tools_preserve_sql_undo_and_preference_only_changes_format(
        cx: &mut TestAppContext,
    ) {
        let (view, _) = fixture(cx);
        let editor = view.read_with(cx, |v, _| v.editor.clone());
        editor.update(cx, |e, cx| {
            e.set_value("select 'Keep CASE' from items where id=1".into(), cx)
        });
        let (_, cx) = cx.add_window_view(|window, cx| {
            let content = cx.new(|_| ConsoleTestRoot(view.clone()));
            gpui::base::Root::new(content, window, cx)
        });
        cx.simulate_resize(gpui::size(px(1200.), px(720.)));
        cx.refresh().unwrap();
        cx.run_until_parked();
        click_tool(cx, "query-format");
        assert!(
            editor
                .read_with(cx, |e, _| e.value())
                .contains("SELECT 'Keep CASE'")
        );
        cx.simulate_keystrokes("cmd-z");
        cx.run_until_parked();
        assert_eq!(
            editor.read_with(cx, |e, _| e.value()),
            "select 'Keep CASE' from items where id=1"
        );
        click_tool(cx, "query-keyword-case");
        assert_eq!(
            editor.read_with(cx, |e, _| e.value()),
            "select 'Keep CASE' from items where id=1"
        );
        click_tool(cx, "query-format");
        assert!(editor.read_with(cx, |e, _| e.value()).starts_with("select"));
        click_tool(cx, "query-wrap");
        assert!(editor.read_with(cx, |e, _| e.soft_wrap));
        cx.cx
            .update(|app| app.write_to_clipboard(gpui::ClipboardItem::new_string("a'\nb".into())));
        cx.simulate_keystrokes("cmd-a");
        cx.simulate_keystrokes("cmd-shift-v");
        cx.run_until_parked();
        assert_eq!(editor.read_with(cx, |e, _| e.value()), "('a''', 'b')");
        cx.simulate_keystrokes("cmd-z");
        cx.run_until_parked();
        assert!(editor.read_with(cx, |e, _| e.value()).starts_with("select"));
    }
    #[gpui::test]
    fn source_picker_changes_only_this_console_context_and_clear_drops_stale_result(
        cx: &mut TestAppContext,
    ) {
        let (view, model) = fixture(cx);
        let catalog = view.read_with(cx, |v, _| v.catalog.clone());
        let other = SourceProfile {
            name: "Other source".into(),
            database: Some("other_db".into()),
            ..Default::default()
        };
        let id = other.id.clone();
        catalog.update(cx, |m, cx| {
            m.profiles.push(other);
            m.tree.databases.insert(id.clone(), vec!["other_db".into()]);
            cx.notify();
        });
        model.update(cx, |m, cx| {
            m.profiles = catalog.read(cx).profiles.clone();
            m.set_query_sql("SELECT 1".into(), cx);
        });
        let editor = view.read_with(cx, |v, _| v.editor.clone());
        editor.update(cx, |e, cx| e.set_value("SELECT 1".into(), cx));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let content = cx.new(|_| ConsoleTestRoot(view.clone()));
            gpui::base::Root::new(content, window, cx)
        });
        cx.refresh().unwrap();
        cx.run_until_parked();
        click_tool(cx, "query-source");
        click_tool(cx, "query-source-choice-1");
        model.read_with(cx, |m, _| {
            assert_eq!(m.selected_source.as_deref(), Some(id.as_str()));
            assert_eq!(m.selected_database.as_deref(), Some("other_db"));
            assert!(m.page.is_none());
            assert_eq!(m.query_sql, "SELECT 1");
        });
        assert!(catalog.read_with(cx, |m, _| m.selected_source.is_none()));
        click_tool(cx, "query-clear-database");
        assert!(model.read_with(cx, |m, _| m.selected_database.is_none()));
        assert_eq!(editor.read_with(cx, |e, _| e.value()), "SELECT 1");
        model.update(cx, |m, cx| {
            m.busy = true;
            cx.notify();
        });
        view.update_in(cx, |v, window, cx| v.tool("query-wrap", window, cx));
        assert!(!editor.read_with(cx, |e, _| e.soft_wrap));
    }
    #[gpui::test]
    fn sql_file_cancel_and_compact_overflow_do_not_modify_draft(cx: &mut TestAppContext) {
        let (view, _) = fixture(cx);
        let editor = view.read_with(cx, |v, _| v.editor.clone());
        editor.update(cx, |e, cx| e.set_value("SELECT 'fixture'".into(), cx));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let content = cx.new(|_| ConsoleTestRoot(view.clone()));
            gpui::base::Root::new(content, window, cx)
        });
        cx.simulate_resize(gpui::size(px(560.), px(480.)));
        cx.refresh().unwrap();
        cx.run_until_parked();
        for id in [
            "query-run",
            "query-more",
            "query-source",
            "query-database",
            "query-export",
        ] {
            let b = cx.debug_bounds(id).unwrap();
            assert!(b.left() >= px(0.) && b.right() <= px(560.), "{id} {b:?}");
        }
        click_tool(cx, "query-more");
        click_tool(cx, "query-open-sql-menu");
        assert!(cx.did_prompt_for_paths());
        cx.simulate_path_prompt_response(|_| None);
        cx.run_until_parked();
        assert_eq!(editor.read_with(cx, |e, _| e.value()), "SELECT 'fixture'");
        cx.update(|window, app| view.update(app, |v, cx| v.focus(window, cx)));
        cx.simulate_keystrokes("cmd-s");
        cx.run_until_parked();
        assert!(cx.did_prompt_for_new_path());
        cx.simulate_new_path_selection(|_| None);
        cx.run_until_parked();
        assert!(!view.read_with(cx, |v, _| v.file_busy));
    }

    #[gpui::test]
    fn native_non_sql_console_rejects_sql_and_preserves_json_when_tools_are_requested(
        cx: &mut TestAppContext,
    ) {
        for (engine, text) in [
            (
                dalan_drivers::DbEngine::MongoDb,
                r#"{"find":"items","filter":{}}"#,
            ),
            (dalan_drivers::DbEngine::Redis, r#"["GET","key"]"#),
        ] {
            let (view, model) = fixture_with_profile(
                cx,
                SourceProfile {
                    engine,
                    database: if engine == dalan_drivers::DbEngine::Redis {
                        Some("0".into())
                    } else {
                        Some("fixture".into())
                    },
                    ..SourceProfile::default()
                },
            );
            let editor = view.read_with(cx, |v, _| v.editor.clone());
            editor.update(cx, |e, cx| e.set_value(text.into(), cx));
            let (_, visual) = cx.add_window_view(|window, cx| {
                let content = cx.new(|_| ConsoleTestRoot(view.clone()));
                gpui::base::Root::new(content, window, cx)
            });
            visual.refresh().unwrap();
            visual.run_until_parked();
            view.update_in(visual, |v, window, cx| v.tool("query-format", window, cx));
            assert_eq!(editor.read_with(visual, |e, _| e.value()), text);
            model.update(visual, |m, cx| m.run_query("SELECT 1".into(), cx));
            assert!(model.read_with(visual, |m, _| m.error.is_some() && !m.busy));
        }
    }

    #[gpui::test]
    fn cmd_enter_in_native_editor_runs_validation_without_inserting_a_newline(
        cx: &mut TestAppContext,
    ) {
        let (view, model) = fixture(cx);
        let editor = view.read_with(cx, |v, _| v.editor.clone());
        editor.update(cx, |e, cx| e.set_value("UPDATE items SET x = 1".into(), cx));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let content = cx.new(|_| ConsoleTestRoot(view.clone()));
            gpui::base::Root::new(content, window, cx)
        });
        cx.refresh().unwrap();
        cx.run_until_parked();
        cx.update(|window, app| view.update(app, |v, cx| v.focus(window, cx)));
        let sql = editor.read_with(cx, |e, _| e.value());
        cx.simulate_keystrokes("cmd-enter");
        cx.run_until_parked();
        assert!(
            model.read_with(cx, |m, _| m.error.is_some()),
            "Cmd-Enter was consumed by the native editor instead of RunQuery"
        );
        assert_eq!(
            editor.read_with(cx, |e, _| e.value()),
            sql,
            "Run must not insert a newline"
        );
        assert!(!model.read_with(cx, |m, _| m.busy));
        cx.simulate_keystrokes("cmd-a");
        cx.run_until_parked();
        assert_eq!(
            editor.read_with(cx, |e, _| e.selected_text()),
            Some(sql.clone())
        );
        cx.simulate_keystrokes("cmd-enter");
        cx.run_until_parked();
        assert_eq!(
            editor.read_with(cx, |e, _| e.selected_text()),
            Some(sql.clone())
        );
        cx.simulate_keystrokes("right");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(editor.read_with(cx, |e, _| e.value()), format!("{sql}\n"));
    }

    #[gpui::test]
    fn run_click_restores_native_editor_focus_after_validation_rejection(cx: &mut TestAppContext) {
        let (view, model) = fixture(cx);
        let editor = view.read_with(cx, |view, _| view.editor.clone());
        let sql = "UPDATE items SET x = 1";
        editor.update(cx, |editor, cx| editor.set_value(sql.into(), cx));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let content = cx.new(|_| ConsoleTestRoot(view.clone()));
            gpui::base::Root::new(content, window, cx)
        });
        cx.simulate_resize(gpui::size(px(900.), px(600.)));
        cx.refresh().unwrap();
        cx.run_until_parked();
        cx.update(|window, app| view.update(app, |view, cx| view.focus(window, cx)));
        cx.simulate_keystrokes("secondary-a");
        cx.run_until_parked();
        assert_eq!(
            editor
                .read_with(cx, |editor, _| editor.selected_text())
                .as_deref(),
            Some(sql)
        );
        let focus = editor.read_with(cx, |editor, _| editor.focus_handle());
        let combo = view.read_with(cx, |view, _| {
            view.database_combo.as_ref().unwrap().entity_id()
        });
        let run_bounds = cx.debug_bounds("query-run").unwrap();
        cx.simulate_click(run_bounds.center(), gpui::Modifiers::default());
        cx.run_until_parked();
        assert!(cx.update(|window, _| focus.is_focused(window)));
        assert_eq!(editor.read_with(cx, |editor, _| editor.value()), sql);
        assert_eq!(
            editor
                .read_with(cx, |editor, _| editor.selected_text())
                .as_deref(),
            Some(sql)
        );
        assert_eq!(
            view.read_with(cx, |view, _| view
                .database_combo
                .as_ref()
                .unwrap()
                .entity_id()),
            combo
        );
        model.read_with(cx, |model, _| {
            assert!(model.error.is_some());
            assert!(!model.busy);
            assert!(model.query_running_sql.is_none());
            assert!(model.query_submitted_sql.is_none());
            assert!(model.page.is_none());
        });
        // Native input replaces the retained selection: focus is functional,
        // not merely a cached focus handle on a recreated editor.
        cx.simulate_input("SELECT 1");
        cx.run_until_parked();
        assert_eq!(editor.read_with(cx, |editor, _| editor.value()), "SELECT 1");
        assert_eq!(
            model.read_with(cx, |model, _| model.query_sql.clone()),
            "SELECT 1"
        );
    }

    #[gpui::test]
    fn run_completion_and_multiline_result_notification_keep_editor_focused(
        cx: &mut TestAppContext,
    ) {
        use dalan_drivers::mysql::{CellValue, ColumnInfo, TablePage};
        use std::{net::TcpListener, sync::Arc, time::Duration};

        // Only our ephemeral loopback listener is contacted. Drop the peer before
        // any MySQL greeting, so no credentials or real database are involved.
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        server.set_nonblocking(true).unwrap();
        let (view, model) = fixture_with_profile(
            cx,
            SourceProfile {
                host: "127.0.0.1".into(),
                port: server.local_addr().unwrap().port(),
                tls: dalan_drivers::TlsMode::Disabled,
                save_password: false,
                ..Default::default()
            },
        );
        let editor = view.read_with(cx, |view, _| view.editor.clone());
        editor.update(cx, |editor, cx| editor.set_value("SELECT 1".into(), cx));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let content = cx.new(|_| ConsoleTestRoot(view.clone()));
            gpui::base::Root::new(content, window, cx)
        });
        cx.simulate_resize(gpui::size(px(900.), px(600.)));
        cx.refresh().unwrap();
        cx.run_until_parked();
        let focus = editor.read_with(cx, |editor, _| editor.focus_handle());
        let run_bounds = cx.debug_bounds("query-run").unwrap();
        cx.simulate_click(run_bounds.center(), gpui::Modifiers::default());
        cx.run_until_parked();
        assert!(cx.update(|window, _| focus.is_focused(window)));
        assert!(model.read_with(cx, |model, _| model.busy));
        let mut accepted = false;
        for _ in 0..200 {
            cx.executor().advance_clock(Duration::from_millis(10));
            cx.run_until_parked();
            if let Ok((peer, _)) = server.accept() {
                accepted = true;
                drop(peer);
            }
            if !model.read_with(cx, |model, _| model.busy) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(accepted, "Run must reach the owned loopback fixture");
        model.read_with(cx, |model, _| {
            assert!(!model.busy, "worker completion was not delivered");
            assert!(model.query_running_sql.is_none());
            assert!(model.error.is_some());
        });
        assert!(cx.update(|window, _| focus.is_focused(window)));
        assert_eq!(editor.read_with(cx, |editor, _| editor.value()), "SELECT 1");

        // Successful-result UI notification is synthetic, independently of the
        // real failed worker above. Exercise the console's populated hierarchy
        // with original multiline/CRLF bytes (the grid must shape one line).
        let text = format!("{{\r\n  \"value\": \"{}\"\n}}", "long value ".repeat(80));
        let page = Arc::new(TablePage {
            columns: vec![ColumnInfo {
                name: "document".into(),
                data_type: "JSON".into(),
                nullable: false,
                is_primary_key: false,
            }],
            rows: vec![vec![CellValue::Text(text.clone())]],
            has_more: false,
            next_offset: None,
            offset: 0,
            truncated: false,
        });
        model.update(cx, |model, cx| {
            model.page = Some(page.clone());
            model.error = None;
            model.query_submitted_sql = Some("SELECT 1".into());
            model.query_dirty = false;
            model.query_elapsed_ms = Some(12);
            cx.notify();
        });
        cx.run_until_parked();
        assert_result_geometry(cx);
        assert!(cx.debug_bounds("cell-0-0").is_some());
        let grid = view.read_with(cx, |view, _| view.grid.clone());
        assert!(grid.read_with(cx, |grid, _| grid.test_materialized_cells()) > 0);
        model.read_with(cx, |model, _| {
            assert!(Arc::ptr_eq(model.page.as_ref().unwrap(), &page));
            assert_eq!(
                model.page.as_ref().unwrap().rows[0][0],
                CellValue::Text(text)
            );
        });
        assert!(cx.update(|window, _| focus.is_focused(window)));
        assert_eq!(editor.read_with(cx, |editor, _| editor.value()), "SELECT 1");
        cx.simulate_input(" ");
        cx.run_until_parked();
        let edited = editor.read_with(cx, |editor, _| editor.value());
        assert_eq!(edited.len(), "SELECT 1".len() + 1);
        assert_eq!(edited.trim(), "SELECT 1");
        assert_eq!(
            model.read_with(cx, |model, _| model.query_sql.clone()),
            edited
        );
        assert!(model.read_with(cx, |model, _| model.query_dirty));
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
        let (_, cx) = cx.add_window_view(|window, cx| {
            let content = cx.new(|_| ConsoleTestRoot(view.clone()));
            gpui::base::Root::new(content, window, cx)
        });
        cx.simulate_resize(gpui::size(px(900.), px(600.)));
        cx.refresh().unwrap();
        cx.run_until_parked();
        let bounds = cx.debug_bounds("query-database").unwrap();
        cx.simulate_click(bounds.center(), gpui::Modifiers::default());
        cx.run_until_parked();
        cx.simulate_keystrokes("down enter");
        cx.run_until_parked();
        assert!(model.read_with(cx, |model, _| model.selected_database.is_none()));
        assert!(cx.update(|window, app| view.read(app).focus_handle(app).is_focused(window)));
        model.update(cx, |model, cx| {
            model.busy = true;
            cx.notify();
        });
        cx.run_until_parked();
        let bounds = cx.debug_bounds("query-database").unwrap();
        cx.simulate_click(bounds.center(), gpui::Modifiers::default());
        cx.run_until_parked();
        assert!(model.read_with(cx, |model, _| model.selected_database.is_none()));
    }

    #[gpui::test]
    fn searchable_database_picker_retains_state_and_selects_filtered_values(
        cx: &mut TestAppContext,
    ) {
        let (view, model) = fixture(cx);
        let catalog = view.read_with(cx, |view, _| view.catalog.clone());
        let source = model.read_with(cx, |model, _| model.selected_source.clone().unwrap());
        catalog.update(cx, |catalog, cx| {
            catalog.tree.databases.insert(
                source.clone(),
                (0..1000)
                    .map(|index| format!("database_{index:04}"))
                    .collect(),
            );
            cx.notify();
        });
        cx.run_until_parked();
        // Lazy construction is important for windowless workspace/tab creation.
        assert!(view.read_with(cx, |view, _| view.database_combo.is_none()));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let content = cx.new(|_| ConsoleTestRoot(view.clone()));
            gpui::base::Root::new(content, window, cx)
        });
        cx.simulate_resize(gpui::size(px(900.), px(600.)));
        cx.refresh().unwrap();
        cx.run_until_parked();
        let state = view.read_with(cx, |view, _| view.database_combo.clone().unwrap());
        let bounds = cx.debug_bounds("query-database").unwrap();
        cx.simulate_click(bounds.center(), gpui::Modifiers::default());
        cx.run_until_parked();
        cx.simulate_input("DATABASE_0999");
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(500));
        cx.run_until_parked();
        assert_eq!(
            state.read_with(cx, |state, cx| state.query(cx).to_string()),
            "DATABASE_0999"
        );
        cx.simulate_keystrokes("down enter");
        cx.run_until_parked();
        assert_eq!(
            model.read_with(cx, |model, _| model.selected_database.clone()),
            Some("database_0999".into())
        );
        // Result/busy notifications must not rebuild the delegate or input state.
        model.update(cx, |model, cx| {
            model.busy = true;
            cx.notify();
        });
        catalog.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |view, _| view
                .database_combo
                .as_ref()
                .unwrap()
                .entity_id()),
            state.entity_id()
        );
        // A popup event already queued before work starts must restore the UI,
        // rather than leaving a value that was rejected by the backend guard.
        state.update(cx, |_, cx| cx.emit(ComboboxEvent::Change(vec![None])));
        cx.run_until_parked();
        assert_eq!(
            state.read_with(cx, |state, _| state.selected_value()),
            Some(Some("database_0999".into()))
        );
        assert_eq!(
            model.read_with(cx, |model, _| model.selected_database.clone()),
            Some("database_0999".into())
        );
    }

    #[test]
    fn absent_database_is_not_a_string_sentinel() {
        let absent = DbChoice::new(None);
        let named = DbChoice::new(Some("No default database".into()));
        assert_eq!(absent.title(), named.title());
        assert_ne!(absent.value(), named.value());
        assert!(named.matches("DEFAULT DATABASE"));
    }

    // Exercise the real console -> result pane -> DataGrid hierarchy, not a
    // standalone grid harness or an empty console. No query/connection is run.
    fn populated_fixture(cx: &mut TestAppContext) -> (Entity<QueryConsole>, Entity<SourceModel>) {
        use dalan_drivers::mysql::{CellValue, ColumnInfo, TablePage};
        let (view, model) = fixture(cx);
        model.update(cx, |model, cx| {
            assert!(model.query_console);
            assert!(model.selected_source.is_some());
            assert!(model.selected_database.is_some());
            model.page = Some(std::sync::Arc::new(TablePage {
                columns: (0..128)
                    .map(|column| ColumnInfo {
                        name: if column == 0 {
                            "id".into()
                        } else {
                            format!("document_{column}")
                        },
                        data_type: if column == 0 {
                            "BIGINT".into()
                        } else {
                            "JSON".into()
                        },
                        nullable: false,
                        is_primary_key: column == 0,
                    })
                    .collect(),
                rows: (0..100)
                    .map(|row| {
                        (0..128)
                            .map(|column| {
                                if column == 0 {
                                    CellValue::Number(row.to_string())
                                } else {
                                    CellValue::Text(
                                        serde_json::json!({"row": row, "column": column})
                                            .to_string(),
                                    )
                                }
                            })
                            .collect()
                    })
                    .collect(),
                has_more: true,
                next_offset: Some(100),
                offset: 0,
                truncated: false,
            }));
            model.query_dirty = false;
            model.busy = false;
            model.error = None;
            model.query_warnings = vec!["Result limited to 100 loaded rows.".into()];
            cx.notify();
        });
        cx.run_until_parked();
        (view, model)
    }

    fn assert_result_geometry(cx: &mut gpui::VisualTestContext) {
        let pane = cx.debug_bounds("query-result-pane").unwrap();
        let body = cx.debug_bounds("grid-body").unwrap();
        eprintln!("query result pane: {pane:?}; grid body: {body:?}");
        assert!(
            pane.size.height > px(100.),
            "result pane collapsed: {pane:?}"
        );
        assert!(body.size.width > px(0.), "grid body has no width: {body:?}");
        assert!(body.size.height > px(0.), "grid body collapsed: {body:?}");
        assert!(body.origin.y >= pane.origin.y);
        assert!(body.bottom() <= pane.bottom());
    }

    fn assert_populated_console(cx: &mut TestAppContext, width: f32, height: f32) {
        use dalan_app::grid_viewport::{COLUMN_WIDTH, ROW_HEIGHT};
        let (view, model) = populated_fixture(cx);
        let editor = view.read_with(cx, |view, _| view.editor.clone());
        let draft = editor.read_with(cx, |editor, _| editor.value());
        let grid = view.read_with(cx, |view, _| view.grid.clone());
        let (_, cx) = cx.add_window_view(|window, cx| {
            let content = cx.new(|_| ConsoleTestRoot(view.clone()));
            gpui::base::Root::new(content, window, cx)
        });
        cx.simulate_resize(gpui::size(px(width), px(height)));
        cx.refresh().unwrap();
        cx.run_until_parked();
        assert_result_geometry(cx);
        assert!(cx.debug_bounds("sort-column-0").is_some());
        assert!(cx.debug_bounds("cell-0-0").is_some());
        grid.read_with(cx, |grid, _| {
            let viewport = grid.test_viewport();
            assert!(viewport.width > 0. && viewport.height > 0.);
            assert!(grid.test_materialized_cells() > 0);
            assert!(grid.test_materialized_cells() < 100 * 128);
        });
        model.read_with(cx, |model, _| {
            let page = model.page.as_ref().unwrap();
            assert_eq!(page.rows.len(), 100);
            assert_eq!(page.columns.len(), 128);
        });
        let position = cx.debug_bounds("grid-body").unwrap().center();
        cx.simulate_event(gpui::ScrollWheelEvent {
            position,
            delta: gpui::ScrollDelta::Pixels(gpui::point(
                px(-100. * COLUMN_WIDTH),
                px(-50. * ROW_HEIGHT),
            )),
            ..Default::default()
        });
        cx.run_until_parked();
        let header = cx.debug_bounds("sort-column-100").unwrap();
        let cell = cx.debug_bounds("cell-50-100").unwrap();
        assert_eq!(header.origin.x, cell.origin.x);
        assert_eq!(header.size.width, cell.size.width);
        grid.read_with(cx, |grid, _| {
            let viewport = grid.test_viewport();
            assert_eq!(viewport.x, 100. * COLUMN_WIDTH);
            assert_eq!(viewport.y, 50. * ROW_HEIGHT);
        });

        // Retained results must remain visible during work and after failure,
        // including the stale-result notice and ordinary row-cap warning.
        for busy in [true, false] {
            model.update(cx, |model, cx| {
                model.busy = busy;
                model.query_dirty = true;
                model.error = (!busy).then(|| "Synthetic query failure".into());
                cx.notify();
            });
            cx.run_until_parked();
            assert_result_geometry(cx);
            assert!(cx.debug_bounds("cell-50-100").is_some());
            assert_eq!(editor.read_with(cx, |editor, _| editor.value()), draft);
        }
        model.update(cx, |model, cx| {
            model.page = None;
            model.error = None;
            cx.notify();
        });
        cx.run_until_parked();
        // Debug bounds can retain selectors from old frames: inspect the grid's
        // actual materialization after clearing rather than their absence.
        assert!(model.read_with(cx, |model, _| model.page.is_none()));
        assert_eq!(
            grid.read_with(cx, |grid, _| grid.test_materialized_cells()),
            0
        );
        assert_eq!(editor.read_with(cx, |editor, _| editor.value()), draft);
    }

    #[gpui::test]
    fn populated_results_have_a_viewport_and_scroll_at_desktop_size(cx: &mut TestAppContext) {
        assert_populated_console(cx, 1040., 760.);
    }

    #[gpui::test]
    fn populated_results_have_a_viewport_and_scroll_at_compact_size(cx: &mut TestAppContext) {
        assert_populated_console(cx, 780., 560.);
    }

    struct ConsoleTestRoot(Entity<QueryConsole>);
    impl Render for ConsoleTestRoot {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(self.0.clone())
        }
    }
}

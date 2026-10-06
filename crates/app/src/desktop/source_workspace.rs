use std::collections::HashMap;

use dalan_app::result_budget::{
    MAX_RETAINED_RESULT_BYTES, MAX_RETAINED_RESULT_PAGES, ResultEntry, eviction_plan,
};
use dalan_app::workspace_tabs::{WorkspaceOpen, WorkspaceTabs};
use gpui::{
    App, Context, Entity, FocusHandle, KeyBinding, SharedString, Subscription, Window, actions,
    div, prelude::*, px,
};

use super::{
    query_console::QueryConsole, source_browser::SourceBrowser, source_dialog,
    source_model::SourceModel,
};

use gpui::component::{
    ActiveTheme, Disableable, Icon, Sizable,
    button::{Button as KitButton, ButtonRounded, ButtonVariants},
    tab::{Tab, TabBar},
};

actions!(workspace, [NewConsole, CloseTab, NextTab, PreviousTab]);

pub(super) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-t", NewConsole, Some("Shell")),
        KeyBinding::new("cmd-shift-n", NewConsole, Some("Shell")),
        KeyBinding::new("cmd-w", CloseTab, Some("Workspace")),
        KeyBinding::new("cmd-alt-right", NextTab, Some("Workspace")),
        KeyBinding::new("cmd-alt-left", PreviousTab, Some("Workspace")),
    ]);
}

/// Source identity colors are explicit metadata, not a replacement Kit palette.
fn tab_color(id: &str, configured: Option<&str>) -> gpui::Hsla {
    if let Some(value) = configured
        .and_then(|v| v.strip_prefix('#'))
        .filter(|v| v.len() == 6)
        .and_then(|v| u32::from_str_radix(v, 16).ok())
    {
        return gpui::rgb(value).into();
    }
    let hash = id.bytes().fold(2166136261u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(16777619)
    });
    gpui::hsla((hash % 360) as f32 / 360., 0.60, 0.52, 1.)
}
fn contextual_tab_label(tab: &dalan_app::workspace_tabs::TabDescriptor, source: &str) -> String {
    let database = tab.database.as_deref().unwrap_or("No database");
    match &tab.kind {
        WorkspaceOpen::Table { table, .. } => format!("{table}@{database}"),
        WorkspaceOpen::Console { .. } => format!(
            "{source}@{database} · {}",
            tab.label.strip_prefix("Console ").unwrap_or(&tab.label)
        ),
    }
}

#[derive(PartialEq, Eq)]
struct TabRenderState {
    busy: bool,
    has_sql: bool,
    page: Option<usize>,
    batch_count: usize,
    source: Option<String>,
    database: Option<String>,
    error: bool,
    write_busy: bool,
    changes: bool,
}
fn tab_render_state(model: &SourceModel) -> TabRenderState {
    TabRenderState {
        busy: model.busy,
        has_sql: !model.query_sql.trim().is_empty(),
        batch_count: model
            .query_batch_results
            .iter()
            .filter(|r| r.result.is_some())
            .count(),
        page: model
            .page
            .as_ref()
            .map(|p| std::sync::Arc::as_ptr(p) as usize),
        source: model.selected_source.clone(),
        database: model.selected_database.clone(),
        error: model.error.is_some(),
        write_busy: model.write_busy,
        changes: model.has_table_changes(),
    }
}

#[derive(Default)]
struct ResultStats {
    // Only an identity token: retaining an Arc here would prevent eviction.
    page_identity: Option<usize>,
    bytes: usize,
    last_used: u64,
}

#[derive(Clone)]
enum TabView {
    Table(Entity<SourceBrowser>),
    Console(Entity<QueryConsole>),
}
struct OpenTab {
    model: Entity<SourceModel>,
    view: TabView,
    _subscription: Subscription,
}

pub(super) struct SourceWorkspace {
    model: Entity<SourceModel>,
    empty: Entity<SourceBrowser>,
    tabs: WorkspaceTabs,
    views: HashMap<String, OpenTab>,
    result_usage: HashMap<String, ResultStats>,
    usage_clock: u64,
    enforcing_budget: bool,
    #[cfg(all(test, feature = "ui-tests"))]
    budget_estimates: usize,
    open_generation: u64,
    close_confirmation: Option<String>,
    focus: FocusHandle,
    close_alert_open: bool,
    pending_focus: bool,
    tab_scroll: gpui::ScrollHandle,
    _subscription: Subscription,
}
impl SourceWorkspace {
    #[cfg(all(test, feature = "ui-tests"))]
    pub(super) fn tab_count(&self) -> usize {
        self.tabs.tabs().len()
    }

    pub(super) fn new(model: Entity<SourceModel>, cx: &mut Context<Self>) -> Self {
        model.update(cx, |model, cx| model.enable_workspace_routes(cx));
        let empty = cx.new(|cx| SourceBrowser::new(model.clone(), cx));
        let subscription = cx.observe(&model, |this, model, cx| {
            let root = model.read(cx);
            let profiles = root.profiles.clone();
            let open = if root.workspace_open_generation != this.open_generation {
                this.open_generation = root.workspace_open_generation;
                root.workspace_open.clone()
            } else {
                None
            };
            let form_open = root.form_open;
            for tab in this.views.values() {
                tab.model.update(cx, |tab, cx| {
                    tab.sync_workspace_sources(profiles.clone(), cx)
                });
            }
            if let Some(request) = open {
                this.open(request, cx);
            }
            if form_open {
                let model = this.model.clone();
                cx.defer(move |cx| source_dialog::show(model, cx));
            }
            cx.notify();
        });
        Self {
            model,
            empty,
            tabs: WorkspaceTabs::new(),
            views: HashMap::new(),
            result_usage: HashMap::new(),
            usage_clock: 0,
            enforcing_budget: false,
            #[cfg(all(test, feature = "ui-tests"))]
            budget_estimates: 0,
            open_generation: 0,
            close_confirmation: None,
            focus: cx.focus_handle().tab_stop(false),
            close_alert_open: false,
            pending_focus: false,
            tab_scroll: gpui::ScrollHandle::new(),
            _subscription: subscription,
        }
    }

    fn open(&mut self, request: WorkspaceOpen, cx: &mut Context<Self>) {
        let opened = match &request {
            WorkspaceOpen::Table { .. } => self.tabs.open_table(request.clone()),
            WorkspaceOpen::Console { source, database } => {
                self.tabs.open_console(source.clone(), database.clone())
            }
        };
        let opened = match opened {
            Ok(opened) => opened,
            Err(error) => {
                self.model.update(cx, |root, cx| {
                    root.metadata_notice = Some(error.to_string());
                    cx.notify();
                });
                return;
            }
        };
        let needs_rebuild = self
            .views
            .get(&opened.id)
            .is_some_and(|tab| tab.model.read(cx).workspace_invalidated);
        if !opened.created && !needs_rebuild {
            self.touch_active(cx);
            self.pending_focus = true;
            cx.notify();
            return;
        }
        let fork = match self.model.read(cx).fork_for_workspace(&request) {
            Ok(fork) => fork,
            Err(error) => {
                if opened.created {
                    self.tabs.close(&opened.id);
                }
                self.model.update(cx, |root, cx| {
                    root.metadata_notice = Some(error.to_string());
                    cx.notify();
                });
                return;
            }
        };
        let tab_model = cx.new(|_| fork);
        let view = match &request {
            WorkspaceOpen::Table { .. } => {
                TabView::Table(cx.new(|cx| SourceBrowser::new(tab_model.clone(), cx)))
            }
            WorkspaceOpen::Console { .. } => TabView::Console(
                cx.new(|cx| QueryConsole::new(tab_model.clone(), self.model.clone(), cx)),
            ),
        };
        let mut last = tab_render_state(tab_model.read(cx));
        let subscription = cx.observe(&tab_model, move |this, model, cx| {
            let next = tab_render_state(model.read(cx));
            if next != last {
                last = next;
                this.refresh_result_usage(cx);
                cx.notify();
            }
        });
        self.result_usage.remove(&opened.id);
        self.views.insert(
            opened.id,
            OpenTab {
                model: tab_model.clone(),
                view,
                _subscription: subscription,
            },
        );
        if let WorkspaceOpen::Table { table, .. } = request {
            tab_model.update(cx, |tab, cx| tab.select_table(table, cx));
        }
        self.touch_active(cx);
        self.close_confirmation = None;
        self.pending_focus = true;
        cx.notify();
    }

    pub(super) fn new_console(&mut self, cx: &mut Context<Self>) {
        if let Some(active) = self.tabs.active().and_then(|id| self.views.get(id)) {
            let tab = active.model.read(cx);
            if let Some(source) = tab.selected_source.clone() {
                let database = tab.selected_database.clone();
                self.model.update(cx, |root, cx| {
                    root.request_console_for(source, database, cx)
                });
                return;
            }
        }
        self.model
            .update(cx, |root, cx| root.request_query_console(cx));
    }

    fn activate(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.tabs.activate(id) {
            self.touch_active(cx);
            self.close_confirmation = None;
            self.pending_focus = true;
            cx.notify();
        }
    }
    pub(super) fn has_inflight_write(&self, cx: &gpui::App) -> bool {
        self.views.values().any(|t| t.model.read(cx).write_busy)
    }
    fn close_tab(&mut self, id: &str, force: bool, cx: &mut Context<Self>) -> bool {
        if !force && self.close_alert_open {
            return false;
        }
        let Some(tab) = self.views.get(id) else {
            return false;
        };
        let state = tab.model.read(cx);
        if state.write_busy {
            return false;
        }
        if !force
            && ((state.query_console && !state.query_sql.trim().is_empty())
                || state.has_table_changes())
        {
            self.close_confirmation = Some(id.to_owned());
            cx.notify();
            return false;
        }
        // A per-tab model owns its operation; dropping the retained view/model
        // cancels only that tab, never another tab or root metadata refresh.
        self.tabs.close(id);
        self.views.remove(id);
        self.result_usage.remove(id);
        self.touch_active(cx);
        self.close_confirmation = None;
        self.pending_focus = true;
        self.close_alert_open = false;
        cx.notify();
        true
    }
    fn cycle(&mut self, backwards: bool, cx: &mut Context<Self>) {
        let Some(active) = self.tabs.active() else {
            return;
        };
        let descriptors = self.tabs.tabs();
        let Some(index) = descriptors.iter().position(|tab| tab.id == active) else {
            return;
        };
        let next = if backwards {
            (index + descriptors.len() - 1) % descriptors.len()
        } else {
            (index + 1) % descriptors.len()
        };
        let id = descriptors[next].id.clone();
        self.activate(&id, cx);
    }

    fn touch_active(&mut self, cx: &mut Context<Self>) {
        self.usage_clock = self.usage_clock.saturating_add(1);
        if let Some(id) = self.tabs.active() {
            self.result_usage
                .entry(id.to_owned())
                .or_default()
                .last_used = self.usage_clock;
            if let Some(tab) = self.views.get(id) {
                let model = tab.model.read(cx);
                let reload = matches!(tab.view, TabView::Table(_))
                    && model.result_evicted
                    && !model.workspace_invalidated
                    && !model.busy
                    && !model.export_busy
                    && !model.saving;
                if reload {
                    tab.model.update(cx, |model, cx| model.refresh_table(cx));
                }
            }
        }
        self.refresh_result_usage(cx);
    }

    fn refresh_result_usage(&mut self, cx: &mut Context<Self>) {
        if self.enforcing_budget {
            return;
        }
        self.enforcing_budget = true;
        let mut entries = Vec::with_capacity(self.views.len());
        for (id, tab) in &self.views {
            let model = tab.model.read(cx);
            let identity = model
                .page
                .as_ref()
                .map(|page| std::sync::Arc::as_ptr(page) as usize);
            let stats = self.result_usage.entry(id.clone()).or_default();
            if stats.page_identity != identity {
                stats.page_identity = identity;
                stats.bytes = model.retained_page_bytes();
                #[cfg(all(test, feature = "ui-tests"))]
                if identity.is_some() {
                    self.budget_estimates += 1;
                }
                if identity.is_some() && self.tabs.active() == Some(id.as_str()) {
                    self.usage_clock = self.usage_clock.saturating_add(1);
                    stats.last_used = self.usage_clock;
                }
            }
            entries.push(ResultEntry {
                id: id.clone(),
                bytes: stats.bytes,
                loaded: identity.is_some(),
                protected: self.tabs.active() == Some(id.as_str())
                    || model.busy
                    || model.export_busy
                    || model.saving
                    || model.write_busy
                    || model.has_table_changes(),
                last_used: stats.last_used,
            });
        }
        // No child read borrow survives into updates, which notify observers.
        for id in eviction_plan(
            &entries,
            MAX_RETAINED_RESULT_BYTES,
            MAX_RETAINED_RESULT_PAGES,
        ) {
            if let Some(tab) = self.views.get(&id)
                && tab
                    .model
                    .update(cx, |model, cx| model.evict_result_page(cx))
                && let Some(stats) = self.result_usage.get_mut(&id)
            {
                stats.page_identity = None;
                stats.bytes = 0;
            }
        }
        self.enforcing_budget = false;
    }
}
impl Render for SourceWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for (id, view) in &self.views {
            let model = view.model.read(cx);
            if model.query_console
                && let Some(source) = &model.selected_source
            {
                self.tabs.update_console_target(
                    id,
                    source.clone(),
                    model.selected_database.clone(),
                );
            }
        }
        let active = self.tabs.active().map(str::to_owned);
        if !self.close_alert_open
            && let Some(target) = self.close_confirmation.clone()
            && let Some(tab) = self.views.get(&target)
            && !tab.model.read(cx).write_busy
        {
            let model_id = tab.model.entity_id();
            let weak = cx.entity().downgrade();
            let cancel = weak.clone();
            self.close_alert_open = super::confirm::open(
                window,
                cx,
                super::confirm::ConfirmAlert {
                    title: "Discard unsaved changes?".into(),
                    description:
                        "Closing this tab will discard its unsaved SQL draft or staged table changes."
                            .into(),
                    confirm_id: "discard-query-draft",
                    confirm_label: "Discard",
                    cancel_id: "keep-query-draft",
                    cancel_label: "Keep open",
                },
                move |_, cx| {
                    weak.update(cx, |this, cx| {
                        if this.close_confirmation.as_deref() != Some(target.as_str())
                            || !this.views.get(&target).is_some_and(|tab| {
                                tab.model.entity_id() == model_id && !tab.model.read(cx).write_busy
                            })
                        {
                            return false;
                        }
                        this.close_tab(&target, true, cx)
                    })
                    .unwrap_or(true)
                },
                move |_, cx| {
                    let _ = cancel.update(cx, |this, cx| {
                        this.close_confirmation = None;
                        this.close_alert_open = false;
                        this.pending_focus = true;
                        cx.notify();
                    });
                    true
                },
            );
        }
        if self.pending_focus && !self.close_alert_open {
            if let Some(index) = self
                .tabs
                .tabs()
                .iter()
                .position(|tab| Some(tab.id.as_str()) == self.tabs.active())
            {
                self.tab_scroll.scroll_to_item(index);
            }
            if let Some(TabView::Console(console)) = active
                .as_ref()
                .and_then(|id| self.views.get(id))
                .map(|tab| &tab.view)
            {
                console.update(cx, |console, cx| console.focus(window, cx));
            } else {
                self.focus.focus(window, cx);
            }
            self.pending_focus = false;
        }
        let selected_source = self
            .model
            .read(cx)
            .explorer_source
            .clone()
            .or_else(|| self.model.read(cx).selected_source.clone());
        let can_console = active.is_some() || selected_source.is_some();
        let mut root = div()
            .id("database-workspace")
            .debug_selector(|| "database-workspace".into())
            .track_focus(&self.focus)
            .tab_stop(false)
            .key_context("Workspace")
            .size_full()
            .min_w(px(0.0))
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .on_action(cx.listener(|this, _: &NewConsole, _, cx| {
                cx.stop_propagation();
                this.new_console(cx);
            }))
            .on_action(cx.listener(|this, _: &CloseTab, window, cx| {
                cx.stop_propagation();
                if let Some(id) = this.tabs.active().map(str::to_owned) {
                    this.close_tab(&id, false, cx);
                } else {
                    window.remove_window();
                }
            }))
            .on_action(cx.listener(|this, _: &NextTab, _, cx| {
                cx.stop_propagation();
                this.cycle(false, cx);
            }))
            .on_action(cx.listener(|this, _: &PreviousTab, _, cx| {
                cx.stop_propagation();
                this.cycle(true, cx);
            }));
        if !self.tabs.tabs().is_empty() {
            let descriptors = self.tabs.tabs().to_vec();
            let index = descriptors
                .iter()
                .position(|tab| Some(&tab.id) == active.as_ref())
                .unwrap_or(0);
            let tabs = descriptors
                .iter()
                .map(|descriptor| {
                    let id = descriptor.id.clone();
                    let state = self.views.get(&id).map(|tab| tab.model.read(cx));
                    let running = state.is_some_and(|state| state.busy);
                    let dirty = state.is_some_and(|state| {
                        state.query_console && !state.query_sql.trim().is_empty()
                    });
                    let profile = self
                        .model
                        .read(cx)
                        .profiles
                        .iter()
                        .find(|p| p.id == descriptor.source);
                    let source = profile
                        .map(|p| p.name.as_str())
                        .unwrap_or("Unavailable source");
                    let color =
                        tab_color(&descriptor.source, profile.and_then(|p| p.color.as_deref()));
                    let selected = Some(&id) == active.as_ref();
                    let label = format!(
                        "{}{}{}",
                        contextual_tab_label(descriptor, source),
                        if running { " · running" } else { "" },
                        if dirty { " •" } else { "" }
                    );
                    let detail = format!(
                        "{} / {} / {}{}{}",
                        source,
                        descriptor
                            .database
                            .as_deref()
                            .unwrap_or("No database selected"),
                        descriptor.label,
                        if running { " · running" } else { "" },
                        if dirty { " · unsaved SQL" } else { "" }
                    );
                    let engine = profile.map(|p| p.engine);
                    let tab_width = (label.chars().count() as f32 * 7.0 + 50.).clamp(100., 280.);
                    let prefix = div()
                        .id(SharedString::from(format!("workspace-tab-prefix-{id}")))
                        .flex()
                        .items_center()
                        .gap(px(3.))
                        .child(
                            div()
                                .id(SharedString::from(format!("workspace-tab-color-{id}")))
                                .debug_selector({
                                    let id = id.clone();
                                    move || format!("workspace-tab-color-{id}")
                                })
                                .absolute()
                                .top(px(-4.))
                                .left(px(-1.))
                                .w(px(tab_width))
                                .h(px(24.))
                                .bg(color.opacity(if selected { 0.24 } else { 0.09 })),
                        )
                        .when(selected, |prefix| {
                            prefix.child(
                                div()
                                    .absolute()
                                    .top(px(18.))
                                    .left(px(-1.))
                                    .w(px(tab_width))
                                    .h(px(2.))
                                    .bg(color),
                            )
                        })
                        .child(
                            div()
                                .id(SharedString::from(format!("workspace-driver-{id}")))
                                .debug_selector({
                                    let id = id.clone();
                                    move || format!("workspace-driver-{id}")
                                })
                                .when_some(engine, |icon, engine| {
                                    icon.child(super::icons::provider_icon(engine))
                                })
                                .when(engine.is_none(), |icon| {
                                    icon.child(
                                        Icon::empty().path("icons/database.svg").size(px(16.)),
                                    )
                                }),
                        );
                    let close_id = id.clone();
                    // Kit 0.7.1 Tab has no closable/on_close API. Its suffix slot
                    // hosts a Kit button, keeping close activation separate from selection.
                    let selector = format!("workspace-tab-{id}");
                    Tab::new()
                        .label(label)
                        .tooltip(move |window, cx| {
                            gpui::component::tooltip::Tooltip::new(detail.clone()).build(window, cx)
                        })
                        // Kit's `icon` selects its icon-only rendering branch,
                        // which deliberately ignores `label` and the width cap.
                        // A prefix keeps the native label/ellipsis layout active.
                        .prefix(prefix)
                        .min_w(px(100.))
                        .w(px(tab_width))
                        .debug_selector(move || selector.clone())
                        .suffix(
                            KitButton::new(SharedString::from(format!("workspace-close-{id}")))
                                .debug_selector(move || format!("workspace-close-{close_id}"))
                                .xsmall()
                                .rounded(ButtonRounded::None)
                                .ghost()
                                .icon(Icon::empty().path("icons/x.svg"))
                                .tooltip("Close tab (Cmd-W)")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.close_tab(&id, false, cx);
                                })),
                        )
                })
                .collect::<Vec<_>>();
            root = root.child(
                div()
                    .id("workspace-tab-strip")
                    .debug_selector(|| "workspace-tab-strip".into())
                    .flex()
                    .items_center()
                    .flex_shrink_0()
                    .min_w(px(0.))
                    .child(
                        TabBar::new("workspace-tabs")
                            .small()
                            .flex_1()
                            .min_w(px(0.))
                            .track_scroll(&self.tab_scroll)
                            .max_width(px(280.))
                            .selected_index(index)
                            .children(tabs)
                            .on_click(cx.listener(move |this, index: &usize, _, cx| {
                                if let Some(tab) = descriptors.get(*index) {
                                    this.activate(&tab.id, cx);
                                }
                            })),
                    )
                    .child(
                        KitButton::new("new-query-console")
                            .small()
                            .debug_selector(|| "new-query-console".into())
                            .icon(Icon::empty().path("icons/plus.svg"))
                            .disabled(!can_console)
                            .tooltip("New query console (Cmd-T)")
                            .on_click(cx.listener(|this, _, _, cx| this.new_console(cx))),
                    ),
            );
        }
        let content = match active.and_then(|id| self.views.get(&id).map(|tab| tab.view.clone())) {
            Some(TabView::Table(view)) => view.into_any_element(),
            Some(TabView::Console(view)) => view.into_any_element(),
            None => self.empty.clone().into_any_element(),
        };
        root.child(div().flex_1().min_h(px(0.0)).min_w(px(0.0)).child(content))
    }
}
#[cfg(all(test, feature = "ui-tests"))]
mod tests {
    use super::*;
    use dalan_drivers::{
        CellValue, ColumnInfo, FilterOperator, SourceProfile, TableFilter, TableInfo, TablePage,
    };
    use gpui::test::TestWindowExt;
    use gpui::{
        Modifiers, ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext, point,
    };
    use std::{net::TcpListener, sync::Arc};

    fn pump_workers(cx: &mut TestAppContext) {
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(10));
        cx.run_until_parked();
    }

    fn fixture(
        cx: &mut TestAppContext,
    ) -> (
        Entity<SourceWorkspace>,
        Entity<SourceModel>,
        TcpListener,
        &mut VisualTestContext,
    ) {
        cx.update(gpui::init);
        cx.update(crate::desktop::bind_keys);
        // Own the loopback endpoint for the entire test. It never speaks the
        // database protocol, and each table request is cancelled before seeding.
        // No user settings, credentials, or external database are involved.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let profile = SourceProfile {
            host: "127.0.0.1".into(),
            port: listener.local_addr().unwrap().port(),
            database: Some("inventory".into()),
            save_password: false,
            ..SourceProfile::default()
        };
        let source = profile.id.clone();
        let model = cx.new(|_| {
            let mut model = SourceModel::for_tests(vec![profile]);
            model
                .tree
                .databases
                .insert(source.clone(), vec!["inventory".into()]);
            model.tree.tables.insert(
                (source.clone(), "inventory".into()),
                ["items", "orders"]
                    .into_iter()
                    .map(|name| TableInfo {
                        name: name.into(),
                        kind: "BASE TABLE".into(),
                    })
                    .collect(),
            );
            model.explorer_source = Some(source);
            model
        });
        let workspace = cx.new(|cx| SourceWorkspace::new(model.clone(), cx));
        let (_, cx) =
            cx.add_window_view(|window, cx| gpui::base::Root::new(workspace.clone(), window, cx));
        cx.simulate_resize(gpui::size(px(900.), px(600.)));
        cx.refresh().unwrap();
        pump_workers(cx);
        (workspace, model, listener, cx)
    }

    fn snapshot(tag: &str) -> Arc<TablePage> {
        Arc::new(TablePage {
            columns: (0..8)
                .map(|i| ColumnInfo {
                    name: format!("column_{i}"),
                    data_type: "VARCHAR".into(),
                    nullable: false,
                    is_primary_key: i == 0,
                })
                .collect(),
            rows: (0..100)
                .map(|row| {
                    (0..8)
                        .map(|column| CellValue::Text(format!("{tag}:{row}:{column}")))
                        .collect()
                })
                .collect(),
            has_more: false,
            next_offset: None,
            offset: 0,
            truncated: false,
        })
    }

    fn active(
        workspace: &Entity<SourceWorkspace>,
        cx: &mut VisualTestContext,
    ) -> (String, Entity<SourceModel>) {
        workspace.read_with(cx, |workspace, _| {
            let id = workspace.tabs.active().unwrap().to_owned();
            let model = workspace.views[&id].model.clone();
            (id, model)
        })
    }

    fn open_table(
        workspace: &Entity<SourceWorkspace>,
        root: &Entity<SourceModel>,
        name: &str,
        cx: &mut VisualTestContext,
    ) -> (String, Entity<SourceModel>) {
        root.update(cx, |root, cx| {
            root.open_tree_table(
                root.profiles[0].id.clone(),
                "inventory".into(),
                name.into(),
                cx,
            )
        });
        pump_workers(cx);
        let (id, model) = active(workspace, cx);
        model.update(cx, |model, cx| {
            model.cancel_query(cx);
            model.error = None;
            model.page = Some(snapshot(name));
            cx.notify();
        });
        pump_workers(cx);
        (id, model)
    }

    fn console(
        workspace: &Entity<SourceWorkspace>,
        cx: &mut VisualTestContext,
    ) -> (String, Entity<SourceModel>) {
        workspace.update(cx, |workspace, cx| workspace.new_console(cx));
        pump_workers(cx);
        cx.refresh().unwrap();
        active(workspace, cx)
    }

    fn click(cx: &mut VisualTestContext, selector: &'static str) {
        let bounds = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("missing {selector}"));
        cx.simulate_click(bounds.center(), Modifiers::default());
        pump_workers(cx);
        cx.refresh().unwrap();
    }

    #[gpui::test]
    fn continued_typing_does_not_rebuild_tab_strip_or_retention_budget(cx: &mut TestAppContext) {
        let (workspace, _root, _listener, cx) = fixture(cx);
        let (_id, model) = console(&workspace, cx);
        model.update(cx, |m, cx| m.set_query_sql("SELECT 1".into(), cx));
        cx.run_until_parked();
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let subscription = cx.update(|_, app| {
            let count = count.clone();
            app.observe(&workspace, move |_, _| {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            })
        });
        cx.run_until_parked();
        count.store(0, std::sync::atomic::Ordering::SeqCst);
        let before = workspace.read_with(cx, |w, _| w.budget_estimates);
        for i in 2..40 {
            model.update(cx, |m, cx| m.set_query_sql(format!("SELECT {i}"), cx));
            cx.run_until_parked();
        }
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(workspace.read_with(cx, |w, _| w.budget_estimates), before);
        drop(subscription);
    }

    #[gpui::test]
    fn source_tints_and_driver_prefixes_span_native_tabs_without_covering_close(
        cx: &mut TestAppContext,
    ) {
        let (workspace, root, _listener, cx) = fixture(cx);
        root.update(cx, |m, cx| {
            m.profiles[0].name = "PROD FLAT".into();
            m.profiles[0].color = Some("#C76ADA".into());
            cx.notify();
        });
        let (id, _) = console(&workspace, cx);
        cx.refresh().unwrap();
        let selector = Box::leak(format!("workspace-tab-{id}").into_boxed_str());
        let tab = cx.debug_bounds(selector).unwrap();
        let tint = cx
            .debug_bounds(Box::leak(
                format!("workspace-tab-color-{id}").into_boxed_str(),
            ))
            .unwrap();
        assert_eq!(
            tint, tab,
            "source wash must cover the whole native tab, not only its icon"
        );
        let driver = cx
            .debug_bounds(Box::leak(format!("workspace-driver-{id}").into_boxed_str()))
            .unwrap();
        let close = cx
            .debug_bounds(Box::leak(format!("workspace-close-{id}").into_boxed_str()))
            .unwrap();
        assert!(driver.left() >= tab.left() && driver.right() < close.left());
        assert!(close.right() <= tab.right());
        cx.update(|window, _| {
            assert_eq!(
                window.within("workspace-tabs").find(0usize).label(),
                Some("PROD FLAT@inventory · 1")
            )
        });
        root.update(cx, |m, cx| {
            m.profiles[0].name = "STG".into();
            m.profiles[0].color = Some("#58B8A0".into());
            cx.notify();
        });
        pump_workers(cx);
        cx.refresh().unwrap();
        cx.update(|window, _| {
            assert_eq!(
                window.within("workspace-tabs").find(0usize).label(),
                Some("STG@inventory · 1")
            )
        });
        assert_eq!(tab_color("id", Some("#C76ADA")), gpui::rgb(0xC76ADA).into());
        assert_eq!(tab_color("id", None), tab_color("id", Some("invalid")));
        assert_ne!(tab_color("id", None), tab_color("other", None));
        let (second, _) = console(&workspace, cx);
        cx.refresh().unwrap();
        for tab_id in [&id, &second] {
            let bounds = cx
                .debug_bounds(Box::leak(
                    format!("workspace-tab-{tab_id}").into_boxed_str(),
                ))
                .unwrap();
            let wash = cx
                .debug_bounds(Box::leak(
                    format!("workspace-tab-color-{tab_id}").into_boxed_str(),
                ))
                .unwrap();
            assert_eq!(wash, bounds, "active/inactive full source tint");
        }
        cx.update(|window, _| {
            assert_eq!(
                window.within("workspace-tabs").find(1usize).label(),
                Some("STG@inventory · 2")
            )
        });
    }

    #[gpui::test]
    fn table_and_console_tabs_keep_labels_and_close_controls_visible(cx: &mut TestAppContext) {
        let (workspace, root, _listener, cx) = fixture(cx);
        let (table, _) = open_table(&workspace, &root, "items", cx);
        let (first, _) = console(&workspace, cx);
        let (second, _) = console(&workspace, cx);
        cx.refresh().unwrap();

        cx.update(|window, _| {
            // Kit snapshots expose accessibility labels, not rendered text.
            // Pair them with real production tab geometry below: the old
            // icon-only branch had correct accessibility labels but 28px tabs.
            let tabs = window.within("workspace-tabs");
            for (index, label) in [
                "items@inventory",
                "New source@inventory · 1",
                "New source@inventory · 2",
            ]
            .into_iter()
            .enumerate()
            {
                let tab = tabs.find(index);
                assert_eq!(tab.label(), Some(label));
                assert!(tab.visible());
            }
        });
        for id in [&table, &first, &second] {
            let tab = cx
                .debug_bounds(Box::leak(format!("workspace-tab-{id}").into_boxed_str()))
                .unwrap();
            let close = cx
                .debug_bounds(Box::leak(format!("workspace-close-{id}").into_boxed_str()))
                .unwrap();
            assert!(tab.size.width >= px(100.) && tab.size.width <= px(280.));
            assert_eq!(tab.size.height, px(24.), "keep Kit's native small height");
            assert!(close.size.width > px(0.));
            assert!(close.origin.x >= tab.origin.x);
            assert!(close.right() <= tab.right());
            // Reserve visible horizontal space for text beyond the 14px icon
            // and native close control, not merely an entity registry label.
            assert!(tab.size.width - close.size.width - px(14.) >= px(50.));
        }

        // Closing an inactive tab must not bubble into tab activation.
        let close = cx
            .debug_bounds(Box::leak(
                format!("workspace-close-{first}").into_boxed_str(),
            ))
            .unwrap();
        cx.simulate_click(close.center(), Modifiers::default());
        pump_workers(cx);
        assert_eq!(active(&workspace, cx).0, second);
        assert_eq!(
            workspace.read_with(cx, |workspace, _| workspace.tab_count()),
            2
        );
    }

    #[gpui::test]
    fn long_table_tab_labels_are_capped_without_clipping_the_close_control(
        cx: &mut TestAppContext,
    ) {
        let (workspace, root, _listener, cx) = fixture(cx);
        let name = "a_very_long_table_name_that_must_be_ellipsized_in_the_workspace_tab";
        root.update(cx, |root, _| {
            root.tree
                .tables
                .get_mut(&(root.profiles[0].id.clone(), "inventory".into()))
                .unwrap()
                .push(TableInfo {
                    name: name.into(),
                    kind: "BASE TABLE".into(),
                });
        });
        let (id, _) = open_table(&workspace, &root, name, cx);
        cx.refresh().unwrap();
        let tab = cx
            .debug_bounds(Box::leak(format!("workspace-tab-{id}").into_boxed_str()))
            .unwrap();
        let close = cx
            .debug_bounds(Box::leak(format!("workspace-close-{id}").into_boxed_str()))
            .unwrap();
        assert_eq!(tab.size.width, px(280.));
        assert!(close.right() <= tab.right());
        assert!(close.size.width > px(0.));
        cx.update(|window, _| {
            assert_eq!(
                window.within("workspace-tabs").find(0usize).label(),
                Some(format!("{name}@inventory").as_str())
            );
        });
    }

    #[gpui::test]
    fn retained_console_pages_are_bounded_without_closing_tabs_or_running_drafts(
        cx: &mut TestAppContext,
    ) {
        let (workspace, _root, _listener, cx) = fixture(cx);
        let mut tabs = Vec::new();
        for index in 0..9 {
            let (id, model) = console(&workspace, cx);
            model.update(cx, |model, cx| {
                model.query_sql = format!("SELECT {index}");
                model.page = Some(snapshot(&index.to_string()));
                cx.notify();
            });
            pump_workers(cx);
            tabs.push((id, model));
        }
        workspace.read_with(cx, |workspace, cx| {
            assert_eq!(workspace.tabs.tabs().len(), 9);
            assert_eq!(workspace.views.len(), 9);
            assert_eq!(
                workspace
                    .views
                    .values()
                    .filter(|tab| tab.model.read(cx).page.is_some())
                    .count(),
                8
            );
            assert_eq!(workspace.budget_estimates, 9);
        });
        tabs[0].1.read_with(cx, |model, _| {
            assert!(model.page.is_none());
            assert!(model.result_evicted);
            assert_eq!(model.query_sql, "SELECT 0");
        });
        assert!(tabs[8].1.read_with(cx, |model, _| model.page.is_some()));
        // Editor/cursor notifications must not walk the cells again.
        tabs[8].1.update(cx, |_, cx| cx.notify());
        pump_workers(cx);
        workspace.update(cx, |workspace, cx| workspace.activate(&tabs[0].0, cx));
        pump_workers(cx);
        tabs[0].1.read_with(cx, |model, _| {
            assert!(model.result_evicted);
            assert!(model.page.is_none());
            assert!(!model.busy, "console activation never executes SQL");
            assert_eq!(model.query_sql, "SELECT 0");
        });
        assert_eq!(
            workspace.read_with(cx, |workspace, _| workspace.budget_estimates),
            9
        );
    }

    #[gpui::test]
    fn retained_result_budget_protects_busy_exporting_and_saving_tabs(cx: &mut TestAppContext) {
        let (workspace, _root, _listener, cx) = fixture(cx);
        let mut tabs = Vec::new();
        for index in 0..9 {
            let (id, model) = console(&workspace, cx);
            model.update(cx, |model, cx| {
                model.page = Some(snapshot(&index.to_string()));
                model.busy = index == 0;
                model.export_busy = index == 1;
                model.saving = index == 2;
                cx.notify();
            });
            pump_workers(cx);
            tabs.push((id, model));
        }
        for index in [0, 1, 2, 8] {
            assert!(tabs[index].1.read_with(cx, |model, _| model.page.is_some()));
        }
        assert!(
            tabs[3]
                .1
                .read_with(cx, |model, _| model.result_evicted && model.page.is_none())
        );
        // If every remaining page is protected, exceeding the cap is permitted.
        for (_, model) in &tabs {
            model.update(cx, |model, cx| {
                model.export_busy = true;
                if model.page.is_none() {
                    model.page = Some(snapshot("protected"));
                }
                cx.notify();
            });
        }
        pump_workers(cx);
        assert!(
            tabs.iter()
                .all(|(_, model)| model.read_with(cx, |model, _| model.page.is_some()))
        );
    }

    #[gpui::test]
    fn evicted_table_reactivation_reloads_applied_clauses_without_changing_neighbors(
        cx: &mut TestAppContext,
    ) {
        let (workspace, root, _listener, cx) = fixture(cx);
        let (id, model) = open_table(&workspace, &root, "items", cx);
        let (_, neighbor) = open_table(&workspace, &root, "orders", cx);
        model.update(cx, |model, cx| {
            model.where_clause = "column_0 > 2".into();
            model.order_by = "column_0 DESC".into();
            assert!(model.evict_result_page(cx));
        });
        pump_workers(cx);
        workspace.update(cx, |workspace, cx| {
            workspace.activate(&id, cx);
            let state = model.read(cx);
            assert!(state.busy, "activation queues a table refresh");
            assert_eq!(state.where_clause, "column_0 > 2");
            assert_eq!(state.order_by, "column_0 DESC");
            assert!(state.page.is_none());
        });
        model.update(cx, |model, cx| model.cancel_query(cx));
        pump_workers(cx);
        neighbor.read_with(cx, |model, _| {
            assert!(model.page.is_some());
            assert!(!model.busy);
            assert!(model.where_clause.is_empty());
        });
        root.read_with(cx, |model, _| {
            assert!(model.page.is_none());
            assert!(!model.busy);
        });
    }

    #[gpui::test]
    fn table_routes_reuse_retained_views_with_independent_filters_pages_and_scroll(
        cx: &mut TestAppContext,
    ) {
        let (workspace, root, _listener, cx) = fixture(cx);
        let (first_id, first) = open_table(&workspace, &root, "items", cx);
        first.update(cx, |model, cx| {
            model.apply_filter(
                Some(TableFilter {
                    column: "column_0".into(),
                    operator: FilterOperator::Equals,
                    value: "first-only".into(),
                }),
                cx,
            );
            model.cancel_query(cx);
            model.error = None;
            model.page = Some(snapshot("filtered"));
            cx.notify();
        });
        pump_workers(cx);
        let first_page = first.read_with(cx, |model, _| model.page.clone().unwrap());
        let first_view =
            workspace.read_with(cx, |workspace, _| match &workspace.views[&first_id].view {
                TabView::Table(view) => view.clone(),
                _ => panic!("expected table"),
            });
        let position = cx.debug_bounds("grid-body").unwrap().center();
        cx.simulate_event(ScrollWheelEvent {
            position,
            delta: ScrollDelta::Pixels(point(px(-300.), px(-900.))),
            ..Default::default()
        });
        pump_workers(cx);
        cx.refresh().unwrap();
        let scrolled_cell = cx
            .debug_bounds("cell-40-2")
            .expect("wheel must reveal a later row and column");
        assert!(scrolled_cell.origin.y < cx.debug_bounds("grid-body").unwrap().bottom());
        let (second_id, second) = open_table(&workspace, &root, "orders", cx);
        assert_ne!(first_id, second_id);
        assert!(cx.debug_bounds("cell-0-0").is_some());
        let second_page = second.read_with(cx, |model, _| {
            assert!(model.test_filter().is_none());
            model.page.clone().unwrap()
        });
        root.update(cx, |root, cx| {
            root.open_tree_table(
                root.profiles[0].id.clone(),
                "inventory".into(),
                "items".into(),
                cx,
            )
        });
        pump_workers(cx);
        workspace.read_with(cx, |workspace, _| {
            assert_eq!(workspace.tabs.active(), Some(first_id.as_str()));
            assert_eq!(workspace.views.len(), 2);
            assert_eq!(workspace.views[&first_id].model, first);
            match &workspace.views[&first_id].view {
                TabView::Table(view) => assert_eq!(view, &first_view),
                _ => panic!("expected table"),
            }
        });
        assert_eq!(
            cx.debug_bounds("cell-40-2"),
            Some(scrolled_cell),
            "reactivation must not reset the first grid scroll"
        );
        first.read_with(cx, |model, _| {
            assert_eq!(model.test_filter().unwrap().value, "first-only");
            assert!(Arc::ptr_eq(model.page.as_ref().unwrap(), &first_page));
            assert!(!model.busy);
        });
        assert!(second.read_with(cx, |model, _| Arc::ptr_eq(
            model.page.as_ref().unwrap(),
            &second_page
        )));
        root.read_with(cx, |root, _| {
            assert!(root.page.is_none());
            assert!(!root.busy);
        });
    }

    #[gpui::test]
    fn reopening_invalidated_table_rebuilds_only_target_and_then_reuses_it(
        cx: &mut TestAppContext,
    ) {
        let (workspace, root, _listener, cx) = fixture(cx);
        let (first_id, first) = open_table(&workspace, &root, "items", cx);
        let old_model_id = first.entity_id();
        let old_model = first.downgrade();
        let old_view_id =
            workspace.read_with(cx, |workspace, _| match &workspace.views[&first_id].view {
                TabView::Table(view) => view.entity_id(),
                _ => panic!("expected table"),
            });
        drop(first);
        let (neighbor_id, neighbor) = open_table(&workspace, &root, "orders", cx);
        let neighbor_view_id = workspace.read_with(cx, |workspace, _| {
            match &workspace.views[&neighbor_id].view {
                TabView::Table(view) => view.entity_id(),
                _ => panic!("expected table"),
            }
        });
        let replacement_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let replacement_port = replacement_listener.local_addr().unwrap().port();
        root.update(cx, |root, cx| {
            // Keep the fixture's cached inventory metadata: only the connection
            // identity changes, and both endpoints are owned by this test.
            root.profiles[0].port = replacement_port;
            cx.notify();
        });
        pump_workers(cx);
        workspace.read_with(cx, |workspace, cx| {
            for id in [&first_id, &neighbor_id] {
                let model = workspace.views[id].model.read(cx);
                assert!(model.workspace_invalidated);
                assert!(model.page.is_none());
                assert_eq!(
                    model.error.as_deref(),
                    Some("Source configuration changed. Reopen this tab.")
                );
                assert!(!model.busy);
            }
        });
        root.update(cx, |root, cx| {
            root.open_tree_table(
                root.profiles[0].id.clone(),
                "inventory".into(),
                "items".into(),
                cx,
            );
        });
        pump_workers(cx);
        cx.refresh().unwrap();
        pump_workers(cx);
        let (reopened_id, rebuilt) = active(&workspace, cx);
        assert_eq!(reopened_id, first_id);
        assert_ne!(rebuilt.entity_id(), old_model_id);
        assert!(old_model.upgrade().is_none());
        workspace.read_with(cx, |workspace, _| {
            assert_eq!(workspace.tabs.tabs().len(), 2);
            assert_eq!(workspace.views.len(), 2);
            match &workspace.views[&first_id].view {
                TabView::Table(view) => assert_ne!(view.entity_id(), old_view_id),
                _ => panic!("expected table"),
            }
            assert_eq!(workspace.views[&neighbor_id].model, neighbor);
            match &workspace.views[&neighbor_id].view {
                TabView::Table(view) => assert_eq!(view.entity_id(), neighbor_view_id),
                _ => panic!("expected table"),
            }
        });
        rebuilt.update(cx, |model, cx| {
            assert_eq!(model.profiles[0].port, replacement_port);
            assert!(!model.workspace_invalidated);
            assert!(model.busy, "the rebuilt table must start a fresh read");
            assert!(model.error.is_none());
            model.cancel_query(cx);
        });
        pump_workers(cx);
        root.update(cx, |root, cx| {
            root.open_tree_table(
                root.profiles[0].id.clone(),
                "inventory".into(),
                "items".into(),
                cx,
            );
        });
        pump_workers(cx);
        assert_eq!(active(&workspace, cx), (first_id, rebuilt.clone()));
        rebuilt.read_with(cx, |model, _| {
            assert!(!model.workspace_invalidated);
            assert!(!model.busy, "a healthy duplicate must not reload");
        });
        // Both tabs share the edited source, but only the explicitly reopened
        // tab is rebuilt; the neighbor remains invalidated and retained.
        assert!(neighbor.read_with(cx, |model, _| model.workspace_invalidated));
    }

    #[gpui::test]
    fn empty_workspace_close_shortcut_removes_window(cx: &mut TestAppContext) {
        let (workspace, _root, _listener, cx) = fixture(cx);
        workspace.read_with(cx, |workspace, _| {
            assert!(workspace.tabs.active().is_none());
            assert!(workspace.views.is_empty());
        });
        cx.update(|window, app| {
            let focus = workspace.read(app).focus.clone();
            focus.focus(window, app);
        });
        cx.simulate_keystrokes("cmd-w");
        pump_workers(cx);
        // Do not read the removed window through VisualTestContext.
        assert!(cx.cx.read(|app| app.windows().is_empty()));
    }

    #[gpui::test]
    fn console_tabs_keep_native_editor_drafts_and_cycle_keyboard_focus(cx: &mut TestAppContext) {
        let (workspace, root, _listener, cx) = fixture(cx);
        let (first_id, first) = console(&workspace, cx);
        cx.simulate_input("SELECT 1");
        pump_workers(cx);
        let (second_id, second) = console(&workspace, cx);
        cx.simulate_input("SELECT 2");
        pump_workers(cx);
        assert_ne!(first_id, second_id);
        cx.simulate_keystrokes("cmd-alt-left");
        pump_workers(cx);
        assert_eq!(active(&workspace, cx).0, first_id);
        cx.simulate_input(" -- retained");
        pump_workers(cx);
        assert_eq!(
            first.read_with(cx, |model, _| model.query_sql.clone()),
            "SELECT 1 -- retained"
        );
        assert_eq!(
            second.read_with(cx, |model, _| model.query_sql.clone()),
            "SELECT 2"
        );
        cx.simulate_keystrokes("cmd-alt-right");
        pump_workers(cx);
        assert_eq!(active(&workspace, cx).0, second_id);
        root.read_with(cx, |root, _| {
            assert!(root.query_sql.is_empty());
            assert!(!root.busy);
        });
    }

    #[gpui::test]
    fn dirty_console_keyboard_close_requires_keep_or_discard(cx: &mut TestAppContext) {
        let (workspace, _root, _listener, cx) = fixture(cx);
        let (id, model) = console(&workspace, cx);
        cx.simulate_input("SELECT 1");
        pump_workers(cx);
        cx.simulate_keystrokes("cmd-w");
        pump_workers(cx);
        assert!(cx.debug_bounds("discard-query-draft").is_some());
        assert!(cx.debug_bounds("confirm-close-console").is_none());
        assert!(cx.update(|window, cx| {
            use gpui::component::WindowExt;
            window.has_active_dialog(cx)
        }));
        click(cx, "keep-query-draft");
        assert!(workspace.read_with(cx, |workspace, _| workspace.close_confirmation.is_none()));
        assert_eq!(active(&workspace, cx).0, id);
        assert_eq!(
            model.read_with(cx, |model, _| model.query_sql.clone()),
            "SELECT 1"
        );
        workspace.update(cx, |workspace, cx| workspace.close_tab(&id, false, cx));
        pump_workers(cx);
        click(cx, "discard-query-draft");
        workspace.read_with(cx, |workspace, _| {
            assert!(workspace.tabs.active().is_none());
            assert!(workspace.views.is_empty());
            assert!(workspace.close_confirmation.is_none());
        });
    }

    #[gpui::test]
    fn dirty_tab_alert_rejects_inflight_write_and_cancel_restores_editor(cx: &mut TestAppContext) {
        use gpui::component::WindowExt;
        let (workspace, _root, _listener, cx) = fixture(cx);
        let (id, model) = console(&workspace, cx);
        cx.simulate_input("SELECT 1");
        pump_workers(cx);
        model.update(cx, |m, cx| {
            m.write_busy = true;
            cx.notify();
        });
        workspace.update(cx, |w, cx| w.close_tab(&id, false, cx));
        pump_workers(cx);
        assert!(!cx.update(|w, cx| w.has_active_dialog(cx)));
        model.update(cx, |m, cx| {
            m.write_busy = false;
            cx.notify();
        });
        workspace.update(cx, |w, cx| w.close_tab(&id, false, cx));
        pump_workers(cx);
        model.update(cx, |m, cx| {
            m.write_busy = true;
            cx.notify();
        });
        pump_workers(cx);
        click(cx, "discard-query-draft");
        assert_eq!(active(&workspace, cx).0, id);
        assert!(cx.update(|w, cx| w.has_active_dialog(cx)));
        click(cx, "keep-query-draft");
        pump_workers(cx);
        assert!(!cx.update(|w, cx| w.has_active_dialog(cx)));
        model.update(cx, |m, cx| {
            m.write_busy = false;
            cx.notify();
        });
        cx.simulate_input(" -- kept");
        pump_workers(cx);
        assert_eq!(
            model.read_with(cx, |m, _| m.query_sql.clone()),
            "SELECT 1 -- kept"
        );
    }

    #[gpui::test]
    fn closing_inactive_and_active_tabs_releases_only_target_entities(cx: &mut TestAppContext) {
        let (workspace, root, _listener, cx) = fixture(cx);
        let (first_id, first) = open_table(&workspace, &root, "items", cx);
        let first_weak = first.downgrade();
        drop(first);
        let (second_id, second) = open_table(&workspace, &root, "orders", cx);
        let page = second.read_with(cx, |model, _| model.page.clone().unwrap());
        workspace.update(cx, |workspace, cx| {
            workspace.close_tab(&first_id, false, cx);
        });
        pump_workers(cx);
        cx.refresh().unwrap();
        assert!(first_weak.upgrade().is_none());
        assert_eq!(active(&workspace, cx).0, second_id);
        assert!(second.read_with(cx, |model, _| Arc::ptr_eq(
            model.page.as_ref().unwrap(),
            &page
        )));
        let (console_id, draft) = console(&workspace, cx);
        let draft_weak = draft.downgrade();
        drop(draft);
        workspace.update(cx, |workspace, cx| {
            workspace.close_tab(&console_id, false, cx);
        });
        pump_workers(cx);
        cx.refresh().unwrap();
        assert!(draft_weak.upgrade().is_none());
        assert_eq!(active(&workspace, cx).0, second_id);
        root.read_with(cx, |root, _| {
            assert!(root.page.is_none());
            assert!(!root.busy);
        });
    }

    #[gpui::test]
    fn new_console_inherits_active_tab_context_not_explorer_selection(cx: &mut TestAppContext) {
        let (workspace, root, _listener, cx) = fixture(cx);
        let (_id, table) = open_table(&workspace, &root, "items", cx);
        let source = table.read_with(cx, |model, _| model.selected_source.clone());
        let other = SourceProfile {
            database: Some("other_database".into()),
            save_password: false,
            ..SourceProfile::default()
        };
        let other_id = other.id.clone();
        root.update(cx, |root, cx| {
            root.profiles.push(other);
            root.explorer_source = Some(other_id.clone());
            root.selected_source = Some(other_id.clone());
            root.selected_database = Some("other_database".into());
            cx.notify();
        });
        pump_workers(cx);
        let (_id, child) = console(&workspace, cx);
        child.read_with(cx, |model, _| {
            assert_eq!(model.selected_source, source);
            assert_eq!(model.selected_database.as_deref(), Some("inventory"));
            assert!(model.query_console);
            assert!(!model.busy);
        });
        root.update(cx, |root, cx| root.request_query_console(cx));
        pump_workers(cx);
        let (_, explicit) = active(&workspace, cx);
        explicit.read_with(cx, |model, _| {
            assert_eq!(model.selected_source.as_deref(), Some(other_id.as_str()));
            assert_eq!(model.selected_database.as_deref(), Some("other_database"));
        });
    }

    #[gpui::test]
    fn closing_loading_table_drops_its_model_without_disturbing_neighbor(cx: &mut TestAppContext) {
        let (workspace, root, _listener, cx) = fixture(cx);
        let (neighbor_id, neighbor) = open_table(&workspace, &root, "items", cx);
        let page = neighbor.read_with(cx, |model, _| model.page.clone().unwrap());
        root.update(cx, |root, cx| {
            root.open_tree_table(
                root.profiles[0].id.clone(),
                "inventory".into(),
                "orders".into(),
                cx,
            );
        });
        pump_workers(cx);
        let (loading_id, loading) = active(&workspace, cx);
        // The owned loopback listener cannot complete a MySQL handshake. The
        // worker is genuinely pending, not a fabricated busy flag.
        loading.read_with(cx, |model, _| {
            assert!(model.busy);
            assert!(model.page.is_none());
            assert!(model.error.is_none());
        });
        let weak = loading.downgrade();
        drop(loading);
        workspace.update(cx, |workspace, cx| {
            workspace.close_tab(&loading_id, false, cx);
        });
        pump_workers(cx);
        cx.refresh().unwrap();
        assert!(
            weak.upgrade().is_none(),
            "a pending worker must not retain its closed tab"
        );
        assert_eq!(active(&workspace, cx).0, neighbor_id);
        neighbor.read_with(cx, |model, _| {
            assert!(!model.busy);
            assert!(model.error.is_none());
            assert!(Arc::ptr_eq(model.page.as_ref().unwrap(), &page));
        });
        root.read_with(cx, |model, _| {
            assert!(!model.busy);
            assert!(model.page.is_none());
        });
    }
}

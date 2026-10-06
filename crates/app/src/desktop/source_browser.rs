//! Read-only source explorer and bounded table browser.
use dalan_app::explorer_tree::{ExplorerTree, TreeKey, TreeRow};
use dalan_drivers::DbEngine;
use dalan_drivers::{SchemaSelection, SourceProfile};
use gpui::{
    Context, Div, Entity, Hsla, KeyBinding, Stateful, Subscription, Window, actions, div,
    prelude::*, px, rgb,
};

use gpui::component::{
    Disableable, Selectable, Sizable,
    button::{Button as KitButton, ButtonVariants},
    menu::{DropdownMenu, PopupMenuItem},
};

actions!(table_browser, [ApplyTableConditions]);

use super::{
    data_grid::DataGrid,
    icons::{Icon, icon, provider_icon},
    input::TextInput,
    source_model::SourceModel,
    theme::*,
};

/// Retains display rows and a lazily materialized cached-metadata search index.
/// Wheel/keyboard repaint never clones a catalog or rebuilds either projection.
pub(super) struct SourceExplorer {
    model: Entity<SourceModel>,
    rows: Vec<TreeRow>,
    search: Entity<TextInput>,
    search_query: String,
    search_index: Option<Vec<TreeRow>>,
    _search_subscription: Subscription,
    active_key: Option<TreeKey>,
    tree_focus: gpui::FocusHandle,
    scroll: gpui::UniformListScrollHandle,
    menu_source: Option<String>,
    #[cfg(test)]
    last_rendered_row_count: usize,
    #[cfg(test)]
    projection_rebuilds: usize,
    _subscription: Subscription,
}

/// Build a fully expanded, read-only projection of materialized metadata. Unlike
/// cloning/expanding the tree this never duplicates the catalog or changes its
/// expansion sets. Constructed lazily, once per model notification while searching.
fn build_search_projection(tree: &ExplorerTree, profiles: &[SourceProfile]) -> Vec<TreeRow> {
    fn label(name: &str) -> String {
        if name.is_empty() {
            "(unnamed)".into()
        } else {
            name.into()
        }
    }
    let mut rows = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut push = |key: TreeKey, name: String, depth, count| {
        let status = if tree.loading.contains(&key) {
            Some("Loading…".into())
        } else {
            tree.errors.get(&key).cloned()
        };
        rows.push(TreeRow {
            key,
            label: name,
            depth,
            count,
            status,
            expandable: depth < 3,
            expanded: depth < 3,
        });
    };
    for profile in profiles.iter().take(100) {
        if !seen.insert(&profile.id) {
            continue;
        }
        let source = &profile.id;
        let visible = |database: &&String| match &profile.schemas {
            SchemaSelection::All => true,
            SchemaSelection::Selected(names) => names.contains(database),
        };
        let databases = tree.databases.get(source);
        push(
            TreeKey::Source(source.clone()),
            label(&profile.name),
            0,
            databases.map(|dbs| dbs.iter().filter(visible).count()),
        );
        let Some(databases) = databases else {
            continue;
        };
        for database in databases.iter().filter(visible).take(1000) {
            let tables = tree.tables.get(&(source.clone(), database.clone()));
            push(
                TreeKey::Database {
                    source: source.clone(),
                    database: database.clone(),
                },
                label(database),
                1,
                tables.map(Vec::len),
            );
            for views in [false, true] {
                push(
                    TreeKey::Group {
                        source: source.clone(),
                        database: database.clone(),
                        views,
                    },
                    if views {
                        "Views"
                    } else {
                        match profile.engine {
                            dalan_drivers::DbEngine::MongoDb => "Collections",
                            dalan_drivers::DbEngine::Redis => "Keys",
                            _ => "Tables",
                        }
                    }
                    .into(),
                    2,
                    tables.map(|items| {
                        items
                            .iter()
                            .filter(|t| (!dalan_drivers::is_browsable_kind(&t.kind)) == views)
                            .count()
                    }),
                );
                if let Some(tables) = tables {
                    for table in tables
                        .iter()
                        .filter(|t| (!dalan_drivers::is_browsable_kind(&t.kind)) == views)
                    {
                        push(
                            TreeKey::Table {
                                source: source.clone(),
                                database: database.clone(),
                                table: table.name.clone(),
                                view: views,
                            },
                            label(&table.name),
                            3,
                            None,
                        );
                    }
                }
            }
        }
    }
    rows
}

/// Include matching branches and each matching descendant's ancestors. Only
/// output rows are cloned; typing neither clones metadata nor performs I/O.
fn filter_search_projection(
    index: &[TreeRow],
    query: &str,
    profiles: &[SourceProfile],
) -> Vec<TreeRow> {
    let query = query
        .chars()
        .take(256)
        .collect::<String>()
        .trim()
        .to_lowercase();
    let mut keep = vec![false; index.len()];
    let mut ancestors = Vec::<usize>::with_capacity(4);
    let mut matching_depth = None;
    for (i, row) in index.iter().enumerate() {
        while ancestors
            .last()
            .is_some_and(|&parent| index[parent].depth >= row.depth)
        {
            ancestors.pop();
        }
        if matching_depth.is_some_and(|depth| row.depth <= depth) {
            matching_depth = None;
        }
        let engine_match = matches!(row.key, TreeKey::Source(_))
            && profiles
                .iter()
                .find(|profile| profile.id == row.key.source())
                .is_some_and(|profile| {
                    profile
                        .engine
                        .display_name()
                        .to_lowercase()
                        .contains(&query)
                });
        if row.label.to_lowercase().contains(&query) || engine_match || matching_depth.is_some() {
            keep[i] = true;
            for &parent in &ancestors {
                keep[parent] = true;
            }
            if matching_depth.is_none() {
                matching_depth = Some(row.depth);
            }
        }
        ancestors.push(i);
    }
    index
        .iter()
        .zip(keep)
        .filter(|(_, keep)| *keep)
        .map(|(row, _)| row.clone())
        .collect()
}

/// Hex-encoded components avoid collisions between sources and names,
/// including names containing quotes, separators, or non-ASCII characters.
fn tree_row_id(key: &TreeKey) -> String {
    fn encode(value: &str) -> String {
        value
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
    match key {
        TreeKey::Source(source) => format!("connect-source-{source}"),
        TreeKey::Database { source, database } => {
            format!("select-database-{}-{}", encode(source), encode(database))
        }
        TreeKey::Group {
            source,
            database,
            views,
        } => format!("tree-group-{}-{}-{views}", encode(source), encode(database)),
        TreeKey::Table {
            source,
            database,
            table,
            view,
        } => format!(
            "select-table-{}-{}-{}-{view}",
            encode(source),
            encode(database),
            encode(table)
        ),
    }
}

/// Snapshot state is not a persistent connection/online indicator.
fn cached_status(loading: bool, offline: bool, failed: bool) -> Option<&'static str> {
    if loading {
        Some("Refreshing…")
    } else if failed {
        Some("Stale")
    } else if offline {
        Some("Cached")
    } else {
        None
    }
}

fn refresh_disabled(model: &SourceModel) -> bool {
    model.saving
        || model.explorer_source.as_ref().is_none_or(|id| {
            !model.profiles.iter().any(|profile| &profile.id == id) || model.refreshing_source(id)
        })
}

struct TreeTooltip(String);
impl Render for TreeTooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = colors(cx);
        div()
            .max_w(px(360.))
            .p(px(8.))
            .bg(palette.chrome)
            .text_size(px(12.))
            .text_color(palette.text)
            .child(self.0.clone())
    }
}

impl SourceExplorer {
    pub(super) fn new(model: Entity<SourceModel>, cx: &mut Context<Self>) -> Self {
        let rows = {
            let model = model.read(cx);
            model.tree.flatten(&model.profiles)
        };
        let search = cx.new(|cx| {
            let mut input = TextInput::new("", "Search sources and objects", false, cx);
            input.set_tab_order(19);
            input
        });
        let search_subscription = cx.observe(&search, |this, search, cx| {
            let query = search
                .read(cx)
                .value()
                .chars()
                .take(256)
                .collect::<String>();
            if this.search_query == query {
                return;
            }
            this.search_query = query;
            let model = this.model.read(cx);
            let rows = if this.search_query.trim().is_empty() {
                model.tree.flatten(&model.profiles)
            } else {
                let index = this
                    .search_index
                    .get_or_insert_with(|| build_search_projection(&model.tree, &model.profiles));
                filter_search_projection(index, &this.search_query, &model.profiles)
            };
            this.replace_rows(rows);
            this.scroll
                .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);
            cx.notify();
        });
        let subscription = cx.observe(&model, |this, model, cx| {
            #[cfg(test)]
            {
                this.projection_rebuilds += 1;
            }
            let model = model.read(cx);
            this.search_index = None;
            let rows = if this.search_query.trim().is_empty() {
                model.tree.flatten(&model.profiles)
            } else {
                let index = build_search_projection(&model.tree, &model.profiles);
                let rows = filter_search_projection(&index, &this.search_query, &model.profiles);
                this.search_index = Some(index);
                rows
            };
            if this
                .menu_source
                .as_ref()
                .is_some_and(|id| !model.profiles.iter().any(|profile| &profile.id == id))
            {
                this.menu_source = None;
            }
            this.replace_rows(rows);
            cx.notify();
        });
        Self {
            model,
            rows,
            search,
            search_query: String::new(),
            search_index: None,
            _search_subscription: search_subscription,
            active_key: None,
            tree_focus: cx.focus_handle().tab_stop(true).tab_index(20),
            scroll: gpui::UniformListScrollHandle::new(),
            menu_source: None,
            #[cfg(test)]
            last_rendered_row_count: 0,
            #[cfg(test)]
            projection_rebuilds: 0,
            _subscription: subscription,
        }
    }

    fn replace_rows(&mut self, rows: Vec<TreeRow>) {
        // Retain the closest visible ancestor without changing model selection.
        if let Some(key) = &self.active_key
            && !rows.iter().any(|row| &row.key == key)
            && let Some(index) = self.rows.iter().position(|row| &row.key == key)
        {
            self.active_key = self.rows[..index]
                .iter()
                .rev()
                .find(|old| {
                    old.depth < self.rows[index].depth && rows.iter().any(|row| row.key == old.key)
                })
                .map(|row| row.key.clone());
        }
        self.rows = rows;
    }

    fn select_key(&mut self, key: TreeKey, cx: &mut Context<Self>) {
        let source = key.source().to_owned();
        let database = match &key {
            TreeKey::Source(_) => None,
            TreeKey::Database { database, .. }
            | TreeKey::Group { database, .. }
            | TreeKey::Table { database, .. } => Some(database.clone()),
        };
        self.active_key = Some(key);
        self.model.update(cx, |model, cx| {
            if model.explorer_source.as_ref() != Some(&source)
                || model.explorer_database != database
            {
                model.explorer_source = Some(source);
                model.explorer_database = database;
                cx.notify();
            }
        });
        cx.notify();
    }

    fn toggle_key(&mut self, key: TreeKey, cx: &mut Context<Self>) {
        self.select_key(key.clone(), cx);
        self.model.update(cx, |model, cx| match key {
            TreeKey::Source(source) => model.toggle_source(source, cx),
            TreeKey::Database { source, database } => model.toggle_database(source, database, cx),
            TreeKey::Group {
                source,
                database,
                views,
            } => model.toggle_group(source, database, views, cx),
            TreeKey::Table {
                source,
                database,
                table,
                view: false,
            } => model.open_tree_table(source, database, table, cx),
            TreeKey::Table { view: true, .. } => {}
        });
    }

    fn keyboard(&mut self, event: &gpui::KeyDownEvent, cx: &mut Context<Self>) {
        if self.rows.is_empty() {
            return;
        }
        let index = self
            .active_key
            .as_ref()
            .and_then(|key| self.rows.iter().position(|row| &row.key == key))
            .unwrap_or(0);
        let row = self.rows[index].clone();
        let next = match event.keystroke.key.as_str() {
            "up" => Some(index.saturating_sub(1)),
            "down" => Some((index + 1).min(self.rows.len() - 1)),
            "home" => Some(0),
            "end" => Some(self.rows.len() - 1),
            "right" if row.expandable && !row.expanded => {
                self.toggle_key(row.key, cx);
                None
            }
            "right"
                if row.expanded
                    && index + 1 < self.rows.len()
                    && self.rows[index + 1].depth > row.depth =>
            {
                Some(index + 1)
            }
            "left" if row.expanded => {
                self.toggle_key(row.key, cx);
                None
            }
            "left" => self.rows[..index]
                .iter()
                .rposition(|parent| parent.depth < row.depth),
            "enter" => {
                self.toggle_key(row.key, cx);
                None
            }
            "space" if row.expandable => {
                self.toggle_key(row.key, cx);
                None
            }
            _ => return,
        };
        cx.stop_propagation();
        if let Some(index) = next {
            self.select_key(self.rows[index].key.clone(), cx);
            self.scroll
                .scroll_to_item(index, gpui::ScrollStrategy::Center);
        }
        cx.notify();
    }

    fn render_row(&mut self, row: TreeRow, cx: &mut Context<Self>) -> Stateful<Div> {
        let palette = colors(cx);
        let model = self.model.read(cx);
        let selected = self.active_key.as_ref().map_or_else(
            || match &row.key {
                TreeKey::Table {
                    source,
                    database,
                    table,
                    view: false,
                } => {
                    model.selected_source.as_ref() == Some(source)
                        && model.selected_database.as_ref() == Some(database)
                        && model.selected_table.as_ref() == Some(table)
                }
                _ => false,
            },
            |key| key == &row.key,
        );
        let view = matches!(&row.key, TreeKey::Table { view: true, .. });
        // Keep the full label even when a warning or a status is present.
        let mut tooltip = row.label.clone();
        let source_row = matches!(&row.key, TreeKey::Source(_));
        let mut failed = false;
        if source_row {
            for (key, error) in &model.tree.errors {
                if key.source() == row.key.source() {
                    failed = true;
                    tooltip.push_str(&format!("\n{error}"));
                }
            }
            if let Some(timestamp) = model.cached_at.get(row.key.source()) {
                tooltip.push_str(&format!(
                    "\nLast successful schema snapshot: {timestamp} Unix seconds"
                ));
            }
        } else if let Some(status) = &row.status {
            tooltip.push_str(&format!("\n{status}"));
        }
        if view {
            tooltip.push_str("\nRead-only views are unavailable");
        }
        let status_label = source_row
            .then(|| {
                cached_status(
                    model.refreshing_source(row.key.source()),
                    model.cached_offline.contains(row.key.source()),
                    failed,
                )
            })
            .flatten();
        let profile = model
            .profiles
            .iter()
            .find(|profile| profile.id == row.key.source());
        if source_row && let Some(profile) = profile {
            tooltip.push_str(&format!("\n{}", profile.engine.display_name()));
        }
        if let Some(count) = row.count {
            tooltip.push_str(&format!("\n{count} items"));
        }
        let glyph = match &row.key {
            TreeKey::Source(_) => match profile.map(|p| p.engine) {
                Some(DbEngine::MariaDb) => Icon::MariaDb,
                _ => Icon::Database,
            },
            TreeKey::Database { .. } => Icon::Database,
            TreeKey::Group { .. } => Icon::Folder,
            TreeKey::Table { .. } => Icon::Table,
        };
        let color = source_color(profile.and_then(|p| p.color.as_deref()), palette.muted);
        let id = tree_row_id(&row.key);
        let debug_id = id.clone();
        let label_id = format!("tree-label-{id}");
        let disclosure_id = format!("tree-disclosure-{id}");
        let key = row.key.clone();
        let disclosure_key = key.clone();
        let mut element = div()
            .id(gpui::SharedString::from(id))
            .debug_selector(move || debug_id.clone())
            .tab_stop(false)
            .h(px(22.))
            .w_full()
            .min_w(px(0.))
            .flex_shrink_0()
            .pl(px(2. + row.depth as f32 * 12.))
            .pr(px(4.))
            .flex()
            .items_center()
            .gap(px(3.))
            .overflow_hidden()
            .text_color(if view { palette.muted } else { palette.text })
            .when(selected, |el| el.bg(palette.selection))
            .hover(|style| style.bg(palette.hover))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _, window, cx| this.tree_focus.focus(window, cx)),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_key(key.clone(), cx);
                match &key {
                    TreeKey::Table { view: false, .. } => this.toggle_key(key.clone(), cx),
                    _ if row.expandable && !row.expanded => this.toggle_key(key.clone(), cx),
                    _ => {}
                }
            }))
            .child(
                div()
                    .id(gpui::SharedString::from(disclosure_id.clone()))
                    .debug_selector(move || disclosure_id.clone())
                    .w(px(18.))
                    .h(px(18.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(row.expandable, |el| {
                        el.cursor_pointer()
                            .child(icon(
                                if row.expanded {
                                    Icon::Chevron
                                } else {
                                    Icon::ChevronRight
                                },
                                if selected {
                                    palette.focus
                                } else {
                                    palette.muted
                                },
                            ))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.toggle_key(disclosure_key.clone(), cx);
                            }))
                    }),
            );
        if let TreeKey::Source(source) = &row.key {
            let marker = format!("color-indicator-{source}");
            let driver = format!("driver-glyph-{source}");
            element = element
                .child(
                    div()
                        .id(gpui::SharedString::from(marker.clone()))
                        .debug_selector(move || marker.clone())
                        .w(px(3.))
                        .h(px(14.))
                        .flex_shrink_0()
                        .bg(color),
                )
                .child(
                    div()
                        .id(gpui::SharedString::from(driver.clone()))
                        .debug_selector(move || driver.clone())
                        .flex()
                        .flex_shrink_0()
                        .child(provider_icon(
                            profile.expect("source row has profile").engine,
                        )),
                );
        } else {
            element = element.child(div().flex().flex_shrink_0().child(icon(
                glyph,
                if selected {
                    palette.focus
                } else {
                    palette.muted
                },
            )));
        }
        element = element.child(
            div()
                .id(gpui::SharedString::from(label_id.clone()))
                .debug_selector(move || label_id.clone())
                .flex_1()
                .min_w(px(if source_row { 80. } else { 0. }))
                .text_color(if view { palette.muted } else { palette.text })
                .text_ellipsis()
                .child(row.label),
        );
        if let Some(status) = status_label {
            let status_id = format!("cached-status-{}", row.key.source());
            tooltip.push_str(&format!("\n{status}"));
            let marker_tooltip = tooltip.clone();
            let glyph = match status {
                "Refreshing…" => Icon::Loading,
                "Stale" => Icon::Warning,
                _ => Icon::Cached,
            };
            element = element.child(
                div()
                    .id(gpui::SharedString::from(status_id.clone()))
                    .debug_selector(move || status_id.clone())
                    .tab_stop(false)
                    .w(px(18.))
                    .h(px(18.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(
                        glyph,
                        if status == "Stale" {
                            palette.warning
                        } else {
                            palette.muted
                        },
                    ))
                    .tooltip(move |_, cx| cx.new(|_| TreeTooltip(marker_tooltip.clone())).into()),
            );
        } else if !source_row && row.status.is_some() {
            element = element.child(
                div()
                    .w(px(18.))
                    .h(px(18.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(
                        if model.tree.loading.contains(&row.key) {
                            Icon::Loading
                        } else {
                            Icon::Warning
                        },
                        palette.warning,
                    )),
            );
        }
        if let TreeKey::Source(source) = &row.key {
            let source = source.clone();
            let action_id = gpui::SharedString::from(format!("source-actions-{source}"));
            let debug_id = action_id.clone();
            let view = cx.entity().downgrade();
            let menu_view = view.clone();
            let open_source = source.clone();
            element = element.child(
                KitButton::new(action_id)
                    .debug_selector(move || debug_id.clone().into())
                    .small()
                    .ghost()
                    .tab_index(20)
                    .w(px(18.))
                    .h(px(18.))
                    .p(px(0.))
                    .flex_shrink_0()
                    .child(icon(Icon::Manage, palette.muted))
                    .tooltip("Source actions")
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .dropdown_menu(move |mut menu, _, cx| {
                        // Capture the row's source, not the currently selected source.
                        // The Kit popover owns positioning, focus and dismissal, even
                        // while the tree's uniform list recycles its visible rows.
                        let disabled = menu_view.upgrade().is_none_or(|view| {
                            let model = view.read(cx).model.read(cx);
                            model.busy || model.saving
                        });
                        for (index, label) in ["Manage", "Copy", "Remove"].into_iter().enumerate() {
                            let view = menu_view.clone();
                            let source = source.clone();
                            menu = menu.item(
                                PopupMenuItem::element(move |_, _| {
                                    let selector = match index {
                                        0 => "source-action-manage",
                                        1 => "source-action-copy",
                                        _ => "source-action-remove",
                                    };
                                    div().debug_selector(move || selector.into()).child(label)
                                })
                                .disabled(disabled)
                                .on_click(move |_, _, cx| {
                                    let _ = view.update(cx, |this, cx| {
                                        this.model.update(cx, |model, cx| {
                                            if model.busy || model.saving {
                                                return;
                                            }
                                            match index {
                                                0 => model.manage_source(source.clone(), cx),
                                                1 => model.copy_source(source.clone(), cx),
                                                _ => {
                                                    model.request_delete_source(source.clone(), cx)
                                                }
                                            }
                                        });
                                    });
                                }),
                            );
                        }
                        menu
                    })
                    .on_open_change(move |open, _, cx| {
                        let _ = view.update(cx, |this, cx| {
                            this.menu_source = open.then(|| open_source.clone());
                            cx.notify();
                        });
                    }),
            );
        }
        element = element.tooltip(move |_, cx| cx.new(|_| TreeTooltip(tooltip.clone())).into());
        element
    }
}

impl Render for SourceExplorer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = colors(cx);
        let model = self.model.read(cx);
        let target = model
            .explorer_source
            .as_ref()
            .or(model.selected_source.as_ref());
        let console_disabled =
            target.is_none_or(|id| !model.profiles.iter().any(|p| &p.id == id)) || model.saving;
        let refresh_is_disabled = refresh_disabled(model);
        let metadata_notice = model.metadata_notice.clone();
        let busy = model.busy;
        let confirm = model.delete_confirm;
        let error = model.error.clone();
        let name = model
            .removal_source_name()
            .unwrap_or("Data source")
            .to_owned();
        let expanded = model.tree.has_visible_expansion(&model.profiles);
        let toolbar = div()
            .id("source-explorer-toolbar")
            .debug_selector(|| "source-explorer-toolbar".into())
            .flex_shrink_0()
            .h(px(TOOLBAR_HEIGHT))
            .min_w(px(0.0))
            .overflow_x_scroll()
            .px(px(6.))
            .flex()
            .items_center()
            .gap(px(2.))
            .child(toolbar_button(
                "refresh-source",
                Icon::Refresh,
                "Refresh selected source schemas (keeps cached metadata on failure)",
                refresh_is_disabled,
                cx,
                |this: &mut Self, cx| {
                    this.model.update(cx, |m, cx| {
                        if !refresh_disabled(m) {
                            m.refresh_explorer(cx);
                        }
                    })
                },
            ))
            .child(toolbar_button(
                "toggle-tree-expansion",
                if expanded {
                    Icon::CollapseTree
                } else {
                    Icon::ExpandTree
                },
                if expanded {
                    "Collapse all"
                } else {
                    "Expand loaded metadata"
                },
                false,
                cx,
                |this: &mut Self, cx| this.model.update(cx, |m, cx| m.toggle_tree_expansion(cx)),
            ))
            .child(toolbar_button(
                "explorer-new-console",
                Icon::Query,
                "New read-only query console (Cmd-T)",
                console_disabled,
                cx,
                |this: &mut Self, cx| {
                    this.model
                        .update(cx, |model, cx| model.request_query_console(cx))
                },
            ));
        let entries = gpui::uniform_list(
            "source-explorer-scroll",
            self.rows.len(),
            cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                #[cfg(test)]
                {
                    this.last_rendered_row_count = range.len();
                }
                // Clone only the requested viewport, never the entire projection.
                let rows = this.rows[range].to_vec();
                rows.into_iter()
                    .map(|row| this.render_row(row, cx))
                    .collect::<Vec<_>>()
            }),
        )
        .debug_selector(|| "source-explorer-scroll".into())
        .h_full()
        .flex_1()
        .min_h(px(0.))
        .min_w(px(0.))
        .track_scroll(&self.scroll);
        let mut root = div()
            .id("source-explorer")
            .debug_selector(|| "source-explorer".into())
            .track_focus(&self.tree_focus)
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                if this.menu_source.is_none() && this.tree_focus.is_focused(window) {
                    this.keyboard(event, cx);
                }
            }))
            .relative()
            .size_full()
            .min_w(px(0.))
            .flex()
            .flex_col()
            .overflow_hidden()
            .text_size(px(12.))
            .text_color(palette.text)
            .bg(palette.panel)
            .border_1()
            .border_color(palette.panel)
            .focus(|style| style.border_color(palette.focus))
            .child(toolbar)
            .child(
                div()
                    .id("source-explorer-search")
                    .debug_selector(|| "source-explorer-search".into())
                    .flex_shrink_0()
                    .h(px(32.))
                    .px(px(6.))
                    .flex()
                    .items_center()
                    .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                        if event.keystroke.key == "escape"
                            && this.menu_source.is_none()
                            && !this.search_query.is_empty()
                        {
                            this.search.update(cx, |input, cx| input.set_value("", cx));
                            this.tree_focus.focus(window, cx);
                            cx.stop_propagation();
                        }
                    }))
                    .child(self.search.clone()),
            );
        if self.rows.is_empty() {
            root = root.child(div().p(px(8.)).text_color(palette.muted).child(
                if self.search_query.trim().is_empty() {
                    "No data sources yet. Add a source to begin."
                } else {
                    "No matching cached sources or objects."
                },
            ));
        }
        root = root.child(entries);
        if confirm {
            root = root.child(
                div()
                    .flex_shrink_0()
                    .p(px(6.))
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(format!(
                        "Remove {name}? Saved source only; databases are not removed."
                    ))
                    .child(
                        div()
                            .flex()
                            .gap(px(4.))
                            .child(button(
                                "confirm-delete",
                                "Remove",
                                false,
                                busy,
                                cx,
                                |this: &mut Self, cx| {
                                    this.model.update(cx, |m, cx| m.confirm_delete_explorer(cx))
                                },
                            ))
                            .child(button(
                                "cancel-delete",
                                "Cancel",
                                false,
                                false,
                                cx,
                                |this: &mut Self, cx| {
                                    this.model.update(cx, |m, cx| m.request_delete_explorer(cx))
                                },
                            )),
                    ),
            );
        }
        if let Some(notice) = metadata_notice {
            root = root.child(
                div()
                    .id("source-metadata-notice")
                    .debug_selector(|| "source-metadata-notice".into())
                    .h(px(22.))
                    .flex_shrink_0()
                    .px(px(6.))
                    .text_ellipsis()
                    .text_color(palette.warning)
                    .child(notice.clone())
                    .tooltip(move |_, cx| cx.new(|_| TreeTooltip(notice.clone())).into()),
            );
        }
        if let Some(error) = error {
            root = root.child(
                div()
                    .id("source-load-error")
                    .debug_selector(|| "source-load-error".into())
                    .h(px(22.))
                    .flex_shrink_0()
                    .px(px(6.))
                    .text_ellipsis()
                    .text_color(palette.warning)
                    .child(error.clone())
                    .tooltip(move |_, cx| cx.new(|_| TreeTooltip(error.clone())).into()),
            );
        }
        root
    }
}

pub(super) struct SourceBrowser {
    model: Entity<SourceModel>,
    grid: Entity<DataGrid>,
    where_input: Entity<TextInput>,
    order_input: Entity<TextInput>,
    applied_where: String,
    applied_order: String,
    selection: (Option<String>, Option<String>, Option<String>),
    _subscriptions: Vec<Subscription>,
}

impl SourceBrowser {
    pub(super) fn new(model: Entity<SourceModel>, cx: &mut Context<Self>) -> Self {
        cx.bind_keys([KeyBinding::new(
            "enter",
            ApplyTableConditions,
            Some("TableBrowser > Input"),
        )]);
        let grid = cx.new(|cx| DataGrid::new(model.clone(), cx));
        let m = model.read(cx);
        let applied_where = m.where_clause.clone();
        let applied_order = m.order_by.clone();
        let selection = (
            m.selected_source.clone(),
            m.selected_database.clone(),
            m.selected_table.clone(),
        );
        let where_input = cx.new(|cx| {
            let mut input = TextInput::new(applied_where.clone(), "WHERE", false, cx);
            input.set_tab_order(20);
            input
        });
        let order_input = cx.new(|cx| {
            let mut input = TextInput::new(applied_order.clone(), "ORDER BY", false, cx);
            input.set_tab_order(21);
            input
        });
        let subscriptions = vec![
            cx.observe(&model, |this, model, cx| {
                let m = model.read(cx);
                let selection = (
                    m.selected_source.clone(),
                    m.selected_database.clone(),
                    m.selected_table.clone(),
                );
                let where_clause = m.where_clause.clone();
                let order_by = m.order_by.clone();
                let changed_selection = this.selection != selection;
                this.selection = selection;
                // Loading, paging, and other notifications must not clobber drafts.
                // A committed clause change (including a header sort) does sync it.
                if changed_selection || this.applied_where != where_clause {
                    this.applied_where = where_clause;
                    this.where_input.update(cx, |input, cx| {
                        input.set_value(this.applied_where.clone(), cx)
                    });
                }
                if changed_selection || this.applied_order != order_by {
                    this.applied_order = order_by;
                    this.order_input.update(cx, |input, cx| {
                        input.set_value(this.applied_order.clone(), cx)
                    });
                }
                cx.notify();
            }),
            cx.observe(&where_input, |_, _, cx| cx.notify()),
            cx.observe(&order_input, |_, _, cx| cx.notify()),
        ];
        Self {
            model,
            grid,
            where_input,
            order_input,
            applied_where,
            applied_order,
            selection,
            _subscriptions: subscriptions,
        }
    }

    fn apply(&mut self, cx: &mut Context<Self>) {
        let model = self.model.read(cx);
        if model.busy || model.selected_table.is_none() {
            return;
        }
        let where_clause = self.where_input.read(cx).value();
        let order_by = self.order_input.read(cx).value();
        self.model.update(cx, |model, cx| {
            model.apply_table_clauses(where_clause, order_by, cx)
        });
    }

    fn clear(&mut self, cx: &mut Context<Self>) {
        if self.model.read(cx).busy {
            return;
        }
        self.where_input
            .update(cx, |input, cx| input.set_value("", cx));
        self.order_input
            .update(cx, |input, cx| input.set_value("", cx));
        self.model
            .update(cx, |model, cx| model.clear_table_clauses(cx));
    }
}

impl Render for SourceBrowser {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = colors(cx);
        let m = self.model.read(cx);
        if m.profiles.is_empty() {
            let disabled = m.busy || m.saving;
            let loading = m.busy;
            let load_error = m.error.clone();
            return div()
                .id("source-browser")
                .debug_selector(|| "source-browser".into())
                .size_full()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .justify_center()
                .items_center()
                .gap(px(12.))
                .bg(palette.panel)
                .text_color(palette.text)
                .text_size(px(12.))
                .child(button(
                    "connect-empty-source",
                    "Connect to a Source",
                    false,
                    disabled,
                    cx,
                    |this, cx| this.model.update(cx, |m, cx| m.new_source(cx)),
                ))
                .child(
                    div()
                        .text_color(palette.muted)
                        .child("Configure a MySQL or MariaDB connection to begin."),
                )
                .when(loading, |el| {
                    el.child(
                        div()
                            .id("table-browser-loading")
                            .debug_selector(|| "table-browser-loading".into())
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_color(palette.muted)
                            .child(icon(Icon::Loading, palette.muted))
                            .child("Loading data sources…")
                            .tooltip(|_, cx| {
                                cx.new(|_| super::ControlTooltip("Loading...")).into()
                            }),
                    )
                })
                .when_some(load_error, |el, error| {
                    el.child(div().text_color(palette.warning).child(error))
                });
        }
        let busy = m.busy;
        let error = m.error.clone();
        let stale = busy || error.is_some();
        let page = m.page.clone();
        let export_busy = m.export_busy;
        let export_feedback = m.export_feedback.clone();
        let source = m
            .profiles
            .iter()
            .find(|p| Some(&p.id) == m.selected_source.as_ref())
            .map(|p| p.name.clone());
        let show_header = m.selected_table.is_some() || page.is_some();
        let title = match (&source, &m.selected_database, &m.selected_table) {
            (Some(source), Some(db), Some(table)) => format!("{source} / {db}.{table}"),
            (_, Some(db), Some(table)) => format!("{db}.{table}"),
            (_, _, Some(table)) => table.clone(),
            (Some(source), _, _) => source.clone(),
            _ => String::new(),
        };
        let selected_table = m.selected_table.is_some();
        let result_evicted = m.result_evicted;
        let empty = if m.selected_table.is_none() {
            "Select a table"
        } else if result_evicted {
            "Result evicted from memory. Use Refresh to reload."
        } else {
            "No table page loaded. Use Refresh to retry."
        };
        let mut body = div()
            .key_context("TableBrowser")
            .on_action(cx.listener(|this, _: &ApplyTableConditions, _, cx| this.apply(cx)))
            .id("source-browser")
            .debug_selector(|| "source-browser".into())
            .size_full()
            .min_w(px(0.))
            .flex()
            .flex_col()
            .bg(palette.panel)
            .text_color(palette.text)
            .text_size(px(12.));
        if show_header {
            body = body.child(
                div()
                    .id("table-browser-header")
                    .debug_selector(|| "table-browser-header".into())
                    .flex_shrink_0()
                    .px(px(10.))
                    .py(px(6.))
                    .min_h(px(PANEL_HEADER_HEIGHT))
                    .bg(palette.header)
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .child(
                        div()
                            .id("table-browser-title")
                            .flex_1()
                            .min_w(px(0.))
                            .text_ellipsis()
                            .child(title.clone())
                            .tooltip({
                                let title = title.clone();
                                move |_, cx| cx.new(|_| TreeTooltip(title.clone())).into()
                            }),
                    )
                    .child(
                        div()
                            .id("read-only-indicator")
                            .debug_selector(|| "read-only-indicator".into())
                            .tab_stop(false)
                            .w(px(28.))
                            .h(px(28.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon(Icon::ReadOnly, palette.muted))
                            .tooltip(|_, cx| {
                                cx.new(|_| {
                                    super::ControlTooltip(
                                        "SQL base tables with complete primary keys support staged edits. Query/view/truncated/unsupported previews remain read-only.",
                                    )
                                })
                                .into()
                            }),
                    ),
            );
        }
        if selected_table
            && matches!(
                self.model.read(cx).selected_engine(),
                dalan_drivers::DbEngine::MySql | dalan_drivers::DbEngine::MariaDb
            )
        {
            body = body.child(
                div()
                    .id("table-conditions-toolbar")
                    .debug_selector(|| "table-conditions-toolbar".into())
                    .flex_shrink_0()
                    .min_w(px(0.))
                    .px(px(6.))
                    .py(px(4.))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .child(
                        div()
                            .id("where-clause")
                            .debug_selector(|| "where-clause".into())
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(icon(Icon::Filter, palette.muted))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .child(self.where_input.clone()),
                            ),
                    )
                    .child(
                        div()
                            .id("order-by-clause")
                            .debug_selector(|| "order-by-clause".into())
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(icon(Icon::Sort, palette.muted))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .child(self.order_input.clone()),
                            ),
                    )
                    .child(toolbar_button(
                        "apply-table-conditions",
                        Icon::Check,
                        "Apply WHERE and ORDER BY (Enter)",
                        busy,
                        cx,
                        |this, cx| this.apply(cx),
                    ))
                    .child(toolbar_button(
                        "clear-table-conditions",
                        Icon::Hide,
                        "Clear WHERE and ORDER BY",
                        busy,
                        cx,
                        |this, cx| this.clear(cx),
                    ))
                    .child(toolbar_button(
                        "refresh-table",
                        Icon::Refresh,
                        "Refresh table using applied conditions",
                        busy,
                        cx,
                        |this, cx| this.model.update(cx, |model, cx| model.refresh_table(cx)),
                    ))
                    .child(toolbar_button(
                        "cancel-table",
                        Icon::Stop,
                        "Cancel table request",
                        !busy,
                        cx,
                        |this, cx| this.model.update(cx, |model, cx| model.cancel_table(cx)),
                    )),
            );
        }
        if busy {
            body = body.child(
                div()
                    .px(px(12.))
                    .py(px(6.))
                    .id("table-browser-loading")
                    .debug_selector(|| "table-browser-loading".into())
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_color(palette.muted)
                    .child(icon(Icon::Loading, palette.muted))
                    .child(if title.is_empty() {
                        "Loading…".into()
                    } else {
                        format!("Loading {title}…")
                    })
                    .tooltip(|_, cx| cx.new(|_| super::ControlTooltip("Loading...")).into()),
            );
        }
        if let Some(error) = error {
            body = body.child(
                div()
                    .px(px(12.))
                    .py(px(6.))
                    .text_color(palette.error)
                    .child(if title.is_empty() {
                        error
                    } else {
                        format!("{title}: {error}")
                    }),
            );
        }
        if let Some(page) = page {
            if let Some(feedback) = export_feedback {
                body = body.child(
                    div()
                        .px(px(12.0))
                        .py(px(6.0))
                        .text_color(palette.muted)
                        .child(feedback),
                );
            }
            if stale {
                body = body.child(div().px(px(12.0)).py(px(6.0)).text_color(palette.muted)
                    .child("Previous page shown until the current request succeeds. Pagination is disabled."));
            }
            body = body.child(self.grid.clone());
            let (summary, summary_tooltip) = page_summary(&page);
            body = body.child(
                div()
                    .px(px(10.))
                    .py(px(4.))
                    .flex_shrink_0()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap(px(6.))
                    .child(
                        div()
                            .id("table-page-summary")
                            .text_color(palette.muted)
                            .child(summary)
                            .tooltip(move |_, cx| {
                                cx.new(|_| TreeTooltip(summary_tooltip.clone())).into()
                            })
                            .when(page.truncated, |el| {
                                el.child(" · Values truncated by the reader's safety limits")
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(6.))
                            .child(toolbar_button(
                                "export-loaded-page",
                                Icon::Download,
                                "Export loaded CSV",
                                stale || page.truncated || export_busy,
                                cx,
                                |this, cx| {
                                    this.model.update(cx, |model, cx| model.request_export(cx))
                                },
                            ))
                            .child(toolbar_button(
                                "previous-page",
                                Icon::Previous,
                                "Previous page",
                                stale || page.offset == 0,
                                cx,
                                |this, cx| {
                                    if this
                                        .model
                                        .read(cx)
                                        .page
                                        .as_ref()
                                        .is_some_and(|p| p.offset > 0)
                                    {
                                        this.model.update(cx, |m, cx| m.change_page(false, cx));
                                    }
                                },
                            ))
                            .child(toolbar_button(
                                "next-page",
                                Icon::ChevronRight,
                                "Next page",
                                stale || !page.has_more,
                                cx,
                                |this, cx| {
                                    if this
                                        .model
                                        .read(cx)
                                        .page
                                        .as_ref()
                                        .is_some_and(|p| p.has_more)
                                    {
                                        this.model.update(cx, |m, cx| m.change_page(true, cx));
                                    }
                                },
                            )),
                    ),
            );
        } else if !busy {
            body = body.child(
                div()
                    .id("table-browser-empty")
                    .debug_selector(|| "table-browser-empty".into())
                    .flex_1()
                    .min_h(px(0.))
                    .p(px(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(6.))
                    .text_color(palette.muted)
                    .child(icon(Icon::Table, palette.muted))
                    .child(empty),
            );
        }
        body
    }
}

/// Treat malformed persisted/custom colors as a neutral indicator, never as CSS.
fn source_color(color: Option<&str>, fallback: Hsla) -> Hsla {
    color
        .and_then(|color| color.strip_prefix('#'))
        .filter(|hex| hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .and_then(|hex| u32::from_str_radix(hex, 16).ok())
        .map(|color| rgb(color).into())
        .unwrap_or(fallback)
}

fn toolbar_button<T: 'static>(
    id: &'static str,
    glyph: Icon,
    tooltip: &'static str,
    disabled: bool,
    cx: &mut Context<T>,
    activate: impl Fn(&mut T, &mut Context<T>) + Clone + 'static,
) -> KitButton {
    let palette = colors(cx);
    button(id, "", false, disabled, cx, activate)
        .w(px(CONTROL_HEIGHT))
        .h(px(CONTROL_HEIGHT))
        .p(px(0.))
        .flex_shrink_0()
        .child(icon(
            glyph,
            if disabled {
                palette.muted
            } else {
                palette.text
            },
        ))
        .accessibility_label(tooltip)
        .tooltip(tooltip)
}

fn page_summary(page: &dalan_drivers::TablePage) -> (String, String) {
    let count = page.rows.len();
    let range = if count == 0 {
        "0 rows".into()
    } else {
        format!("{}–{}", page.offset + 1, page.offset + count as u64)
    };
    (
        range,
        format!(
            "{count} loaded rows; {}",
            if page.has_more {
                "more rows available"
            } else {
                "no more rows"
            }
        ),
    )
}

/// Kit owns activation, focus, disabled and selected-state presentation.
fn button<T: 'static>(
    id: impl Into<String>,
    label: impl Into<String>,
    selected: bool,
    disabled: bool,
    cx: &mut Context<T>,
    activate: impl Fn(&mut T, &mut Context<T>) + Clone + 'static,
) -> KitButton {
    let id: gpui::SharedString = id.into().into();
    let primary = id.as_ref() == "connect-empty-source";
    let debug_id = id.clone();
    let label: String = label.into();
    KitButton::new(id)
        .debug_selector(move || debug_id.clone().into())
        .small()
        .when(primary, |button| button.primary())
        .when(!primary, |button| button.ghost())
        .selected(selected)
        .disabled(disabled)
        .tab_index(20)
        .tab_stop(!disabled)
        .when(!label.is_empty(), |button| button.label(label))
        .on_click(cx.listener(move |this, _, _, cx| {
            if !disabled {
                activate(this, cx);
            }
        }))
}

#[cfg(all(test, feature = "ui-tests"))]
mod tests {
    use super::*;
    use dalan_drivers::{
        SourceProfile, TablePage,
        mysql::{CellValue, ColumnInfo, SortDirection},
    };
    use gpui::{Modifiers, TestAppContext, VisualTestContext};

    fn search_fixture() -> (ExplorerTree, Vec<SourceProfile>) {
        let profiles = vec![SourceProfile {
            id: "cached".into(),
            name: "Production warehouse".into(),
            ..SourceProfile::default()
        }];
        let mut tree = ExplorerTree::default();
        tree.databases
            .insert("cached".into(), vec!["Analytics".into(), "Other".into()]);
        tree.tables.insert(
            ("cached".into(), "Analytics".into()),
            vec![
                dalan_drivers::TableInfo {
                    name: "Événements名字".into(),
                    kind: "BASE TABLE".into(),
                },
                dalan_drivers::TableInfo {
                    name: "Daily summary".into(),
                    kind: "VIEW".into(),
                },
            ],
        );
        tree.tables.insert(
            ("cached".into(), "Other".into()),
            vec![dalan_drivers::TableInfo {
                name: "unrelated".into(),
                kind: "BASE TABLE".into(),
            }],
        );
        (tree, profiles)
    }

    #[test]
    fn search_cached_collapsed_unicode_objects_preserves_ancestors_and_expansion() {
        let (tree, profiles) = search_fixture();
        let index = build_search_projection(&tree, &profiles);
        let matches = filter_search_projection(&index, "ÉVÉNEMENTS名字", &profiles);
        assert_eq!(
            matches
                .iter()
                .map(|row| row.label.as_str())
                .collect::<Vec<_>>(),
            [
                "Production warehouse",
                "Analytics",
                "Tables",
                "Événements名字"
            ]
        );
        assert_eq!(
            matches.iter().map(|row| row.depth).collect::<Vec<_>>(),
            [0, 1, 2, 3]
        );
        let views = filter_search_projection(&index, "SUMMARY", &profiles);
        assert_eq!(views[2].label, "Views");
        assert!(matches[0].expanded && matches[1].expanded && matches[2].expanded);
        assert!(tree.expanded_sources.is_empty());
        assert!(tree.expanded_databases.is_empty());
        assert!(tree.expanded_groups.is_empty());
        assert!(tree.loading.is_empty());
        assert_eq!(tree.flatten(&profiles).len(), 1);
        assert!(filter_search_projection(&index, "not cached", &profiles).is_empty());
    }

    #[test]
    fn search_parent_and_engine_matches_include_cached_branches_and_respect_schema_selection() {
        let (tree, mut profiles) = search_fixture();
        let index = build_search_projection(&tree, &profiles);
        assert_eq!(
            filter_search_projection(&index, "WAREHOUSE", &profiles).len(),
            index.len()
        );
        assert_eq!(
            filter_search_projection(&index, profiles[0].engine.display_name(), &profiles).len(),
            index.len()
        );
        let branch = filter_search_projection(&index, "ANALYTICS", &profiles);
        assert_eq!(branch.len(), 6);
        assert!(!branch.iter().any(|row| row.label == "Other"));
        profiles[0].schemas = SchemaSelection::Selected(vec!["Other".into()]);
        let restricted = build_search_projection(&tree, &profiles);
        assert!(filter_search_projection(&restricted, "Événements", &profiles).is_empty());
        assert_eq!(tree.tables.len(), 2);
        assert_eq!(
            filter_search_projection(&index, &"名".repeat(257), &profiles).len(),
            0
        );
    }

    #[gpui::test]
    fn native_sidebar_search_reuses_index_virtualizes_and_escape_restores_tree(
        cx: &mut TestAppContext,
    ) {
        let (mut tree, profiles) = search_fixture();
        tree.tables
            .get_mut(&("cached".into(), "Analytics".into()))
            .unwrap()
            .extend((0..50_000).map(|i| dalan_drivers::TableInfo {
                name: format!("object{i:05}"),
                kind: "BASE TABLE".into(),
            }));
        let model = cx.new(|_| {
            let mut model = SourceModel::for_tests(profiles);
            model.tree = tree;
            model.page = Some(std::sync::Arc::new(page()));
            model.cached_offline.insert("cached".into());
            model
        });
        let page = model.read_with(cx, |model, _| model.page.as_ref().unwrap().clone());
        let (explorer, cx) = kit_window(cx, |cx| SourceExplorer::new(model.clone(), cx));
        cx.simulate_resize(gpui::size(px(320.), px(570.)));
        cx.refresh().unwrap();
        cx.run_until_parked();
        let search = explorer.read_with(cx, |view, _| view.search.clone());
        assert!(explorer.read_with(cx, |view, _| view.search_index.is_none()));
        let focus = search.read_with(cx, |input, _| input.focus_handle());
        cx.update(|window, cx| focus.focus(window, cx));
        cx.simulate_input("object49999");
        cx.run_until_parked();
        let index_address = explorer.read_with(cx, |view, _| {
            assert_eq!(view.rows.len(), 4);
            assert_eq!(view.rows.last().unwrap().label, "object49999");
            assert!(view.last_rendered_row_count <= 40);
            assert!(view.search_index.as_ref().unwrap().len() > 50_000);
            view.search_index.as_ref().unwrap().as_ptr() as usize
        });
        // Native replacement triggers the same Kit Change observer, without a
        // model notification or rebuilding the materialized metadata index.
        cx.simulate_keystrokes("cmd-a");
        cx.simulate_input("warehouse");
        cx.run_until_parked();
        explorer.read_with(cx, |view, _| {
            assert!(view.rows.len() > 50_000);
            assert_eq!(
                view.search_index.as_ref().unwrap().as_ptr() as usize,
                index_address
            );
            assert_eq!(view.projection_rebuilds, 0);
            assert!(view.last_rendered_row_count > 0 && view.last_rendered_row_count <= 40);
        });
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        explorer.read_with(cx, |view, cx| {
            assert_eq!(view.search_query, "");
            assert_eq!(view.search.read(cx).value(), "");
            assert_eq!(view.rows.len(), 1);
        });
        model.read_with(cx, |model, _| {
            assert!(model.tree.expanded_sources.is_empty());
            assert!(model.tree.expanded_databases.is_empty());
            assert!(model.tree.expanded_groups.is_empty());
            assert!(model.tree.loading.is_empty());
            assert!(model.selected_source.is_none());
            assert!(std::sync::Arc::ptr_eq(model.page.as_ref().unwrap(), &page));
        });
    }

    /// Production Kit popovers render through the window's component Root.
    fn kit_window<T: Render + 'static>(
        cx: &mut TestAppContext,
        build: impl FnOnce(&mut Context<T>) -> T,
    ) -> (Entity<T>, &mut VisualTestContext) {
        // Kit globals must exist before constructing inputs or popup controls.
        cx.update(gpui::init);
        cx.update(crate::desktop::bind_keys);
        let view = cx.new(build);
        let (_, visual) =
            cx.add_window_view(|window, cx| gpui::base::Root::new(view.clone(), window, cx));
        (view, visual)
    }

    fn click(cx: &mut VisualTestContext, id: &str) {
        cx.run_until_parked();
        let bounds = cx
            .debug_bounds(Box::leak(id.to_owned().into_boxed_str()))
            .unwrap_or_else(|| panic!("missing {id}"));
        cx.simulate_click(bounds.center(), Modifiers::default());
        cx.run_until_parked();
    }

    /// Native Kit button activation completes on key release.
    fn press(cx: &mut VisualTestContext, key: &str) {
        let keystroke = gpui::Keystroke::parse(key).unwrap();
        cx.simulate_event(gpui::KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        });
        cx.simulate_event(gpui::KeyUpEvent { keystroke });
        cx.run_until_parked();
    }

    fn page() -> TablePage {
        TablePage {
            columns: vec!["id", "name"]
                .into_iter()
                .map(|name| ColumnInfo {
                    name: name.into(),
                    data_type: "VARCHAR".into(),
                    nullable: true,
                    is_primary_key: name == "id",
                })
                .collect(),
            rows: vec![
                vec![
                    CellValue::Number("1".into()),
                    CellValue::Text("Alice".into()),
                ],
                vec![CellValue::Number("2".into()), CellValue::Null],
            ],
            has_more: true,
            next_offset: Some(2),
            offset: 0,
            truncated: false,
        }
    }

    fn sorting_fixture(
        cx: &mut TestAppContext,
    ) -> (gpui::Entity<SourceModel>, &mut VisualTestContext) {
        let model = cx.new(|_| {
            let mut model = SourceModel::for_tests(vec![SourceProfile::default()]);
            model.selected_database = Some("inventory".into());
            model.selected_table = Some("items".into());
            model.page = Some(std::sync::Arc::new(page()));
            model
        });
        let (_, cx) = kit_window(cx, |cx| SourceBrowser::new(model.clone(), cx));
        cx.simulate_resize(gpui::size(px(1000.), px(800.)));
        cx.refresh().unwrap();
        cx.run_until_parked();
        (model, cx)
    }

    /// Exercise both virtualized views in the same window and shared model.
    struct ScrollingWorkspace {
        explorer: Entity<SourceExplorer>,
        browser: Entity<SourceBrowser>,
    }

    impl Render for ScrollingWorkspace {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .flex()
                .child(
                    div()
                        .w(px(320.))
                        .h_full()
                        .flex_shrink_0()
                        .child(self.explorer.clone()),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .h_full()
                        .child(self.browser.clone()),
                )
        }
    }

    #[gpui::test]
    fn integrated_scrolling_keeps_shared_snapshot_and_projection_stable(cx: &mut TestAppContext) {
        use dalan_app::grid_viewport::{COLUMN_WIDTH, ROW_HEIGHT};
        use gpui::{ScrollDelta, ScrollWheelEvent, point};
        use std::sync::Arc;

        let source = "00000000-0000-4000-8000-000000000001";
        let snapshot = Arc::new(TablePage {
            columns: (0..512)
                .map(|column| ColumnInfo {
                    name: format!("column_{column}"),
                    data_type: "VARCHAR".into(),
                    nullable: true,
                    is_primary_key: false,
                })
                .collect(),
            rows: (0..100)
                .map(|row| {
                    (0..512)
                        .map(|column| CellValue::Text(format!("{row}:{column}")))
                        .collect()
                })
                .collect(),
            has_more: false,
            next_offset: None,
            offset: 0,
            truncated: false,
        });
        let model = cx.new(|_| {
            let mut model = SourceModel::for_tests(vec![SourceProfile {
                id: source.into(),
                name: "Scrolling fixture".into(),
                save_password: false,
                ..SourceProfile::default()
            }]);
            model.tree.databases.insert(
                source.into(),
                (0..1000)
                    .map(|database| format!("database_{database}"))
                    .collect(),
            );
            model.tree.tables.insert(
                (source.into(), "database_0".into()),
                vec![dalan_drivers::TableInfo {
                    name: "wide".into(),
                    kind: "BASE TABLE".into(),
                }],
            );
            model.tree.expand_loaded();
            model.cached_at.insert(source.into(), 123);
            model.selected_source = Some(source.into());
            model.explorer_source = Some(source.into());
            model.selected_database = Some("database_0".into());
            model.selected_table = Some("wide".into());
            model.page = Some(Arc::clone(&snapshot));
            model
        });
        let (workspace, cx) = kit_window(cx, |cx| ScrollingWorkspace {
            explorer: cx.new(|cx| SourceExplorer::new(model.clone(), cx)),
            browser: cx.new(|cx| SourceBrowser::new(model.clone(), cx)),
        });
        cx.simulate_resize(gpui::size(px(1280.), px(720.)));
        cx.refresh().unwrap();
        cx.run_until_parked();
        let (explorer, browser) = workspace.read_with(cx, |workspace, _| {
            (workspace.explorer.clone(), workspace.browser.clone())
        });
        let grid = browser.read_with(cx, |browser, _| browser.grid.clone());
        let rebuilds = explorer.read_with(cx, |explorer, _| {
            assert!(explorer.rows.len() >= 1002);
            assert!(explorer.last_rendered_row_count > 0);
            assert!(explorer.last_rendered_row_count <= 60);
            explorer.projection_rebuilds
        });
        assert!(cx.debug_bounds("cell-0-0").is_some());
        assert!(cx.debug_bounds("cell-50-200").is_none());
        let grid_bounds = cx.debug_bounds("table-grid-scroll").unwrap();
        cx.simulate_event(ScrollWheelEvent {
            position: grid_bounds.center(),
            delta: ScrollDelta::Pixels(point(px(-200. * COLUMN_WIDTH), px(-50. * ROW_HEIGHT))),
            ..Default::default()
        });
        cx.run_until_parked();
        let viewport = grid.read_with(cx, |grid, _| grid.test_viewport());
        assert_eq!(
            (viewport.x, viewport.y),
            (200. * COLUMN_WIDTH, 50. * ROW_HEIGHT)
        );
        let header = cx.debug_bounds("sort-column-200").unwrap();
        let cell = cx.debug_bounds("cell-50-200").unwrap();
        assert_eq!(header.left(), cell.left());
        assert_eq!(header.size.width, px(COLUMN_WIDTH));
        assert_eq!(header.size.width, cell.size.width);
        let materialized = grid.read_with(cx, |grid, _| grid.test_materialized_cells());
        assert_eq!(
            materialized,
            viewport.rows(100).len() * viewport.columns(512).len()
        );
        assert!(materialized > 0 && materialized <= 400);
        assert!(materialized < snapshot.rows.len() * snapshot.columns.len() / 100);
        explorer.read_with(cx, |explorer, _| {
            assert_eq!(explorer.projection_rebuilds, rebuilds)
        });

        // A native wheel event over the sidebar must not reach the data grid or
        // notify the shared model. At 22px/row this advances hundreds of rows.
        let sidebar_bounds = cx.debug_bounds("source-explorer-scroll").unwrap();
        cx.simulate_event(ScrollWheelEvent {
            position: sidebar_bounds.center(),
            delta: ScrollDelta::Pixels(point(px(0.), px(-20_000.))),
            ..Default::default()
        });
        cx.run_until_parked();
        explorer.read_with(cx, |explorer, _| {
            assert_eq!(explorer.projection_rebuilds, rebuilds);
            assert!(explorer.last_rendered_row_count > 0);
            assert!(explorer.last_rendered_row_count <= 60);
        });
        let scroll_offset = explorer.read_with(cx, |explorer, _| {
            explorer.scroll.0.borrow().base_handle.offset()
        });
        assert!(scroll_offset.y <= px(-850.0 * 22.0));
        assert!(
            (850..1000).any(|database| {
                let id = tree_row_id(&TreeKey::Database {
                    source: source.into(),
                    database: format!("database_{database}"),
                });
                cx.debug_bounds(Box::leak(id.into_boxed_str())).is_some()
            }),
            "sidebar wheel must paint a distant database, not just repaint the initial rows"
        );
        assert_eq!(grid.read_with(cx, |grid, _| grid.test_viewport()), viewport);
        assert_eq!(
            grid.read_with(cx, |grid, _| grid.test_materialized_cells()),
            materialized
        );
        model.read_with(cx, |model, _| {
            assert!(Arc::ptr_eq(model.page.as_ref().unwrap(), &snapshot));
            assert_eq!(model.selected_source.as_deref(), Some(source));
            assert_eq!(model.explorer_source.as_deref(), Some(source));
            assert_eq!(model.selected_database.as_deref(), Some("database_0"));
            assert_eq!(model.selected_table.as_deref(), Some("wide"));
            assert!(!model.busy);
            assert!(model.tree.loading.is_empty());
            assert_eq!(model.tree.databases[source].len(), 1000);
        });
        let painted_tree_rows =
            explorer.read_with(cx, |explorer, _| explorer.last_rendered_row_count);
        println!(
            "integrated scrolling: 1000 databases, 100x512 cells, grid viewport {}x{}, materialized {materialized}/51200 cells, sidebar {painted_tree_rows} rows, projection rebuild delta 0, shared page Arc preserved",
            viewport.width, viewport.height,
        );
    }

    #[test]
    fn snapshot_status_is_textual_and_loading_takes_precedence() {
        assert_eq!(cached_status(false, true, false), Some("Cached"));
        assert_eq!(cached_status(false, true, true), Some("Stale"));
        assert_eq!(cached_status(true, true, true), Some("Refreshing…"));
        assert_eq!(cached_status(false, false, false), None);
    }

    #[gpui::test]
    fn refresh_requires_real_explicit_explorer_selection(cx: &mut TestAppContext) {
        let model = cx.new(|_| {
            let mut m = SourceModel::for_tests(vec![SourceProfile {
                id: "refresh-guard".into(),
                save_password: false,
                ..SourceProfile::default()
            }]);
            m.selected_source = Some("refresh-guard".into());
            m.cached_at.insert("refresh-guard".into(), u64::MAX);
            m.tree
                .databases
                .insert("refresh-guard".into(), vec!["cached_db".into()]);
            m
        });
        let (_, cx) = kit_window(cx, |cx| SourceExplorer::new(model.clone(), cx));
        cx.refresh().unwrap();
        click(cx, "refresh-source");
        model.read_with(cx, |m, _| {
            assert!(refresh_disabled(m));
            assert!(m.tree.loading.is_empty());
            assert!(m.explorer_source.is_none());
            assert_eq!(m.cached_at["refresh-guard"], u64::MAX);
            assert_eq!(m.tree.databases["refresh-guard"], ["cached_db"]);
        });
        // Traversal alone must not select a source or start metadata work.
        // Activating every other toolbar/tree stop could intentionally connect
        // a source, so it is not a valid disabled-Refresh regression.
        for _ in 0..8 {
            cx.update(|window, cx| window.focus_next(cx));
            cx.run_until_parked();
        }
        model.update(cx, |m, cx| {
            assert!(m.tree.loading.is_empty());
            m.explorer_source = Some("phantom".into());
            cx.notify();
        });
        click(cx, "refresh-source");
        assert!(model.read_with(cx, |m, _| refresh_disabled(m) && m.tree.loading.is_empty()));
        model.update(cx, |m, _| {
            m.explorer_source = Some("refresh-guard".into());
            m.busy = true;
            assert!(
                !refresh_disabled(m),
                "page loading is independent of metadata"
            );
            m.saving = true;
            assert!(refresh_disabled(m));
            m.saving = false;
            m.tree
                .loading
                .insert(TreeKey::Source("refresh-guard".into()));
            assert!(refresh_disabled(m));
        });
    }

    #[gpui::test]
    fn offline_snapshot_browsing_and_notice_preserve_rows(cx: &mut TestAppContext) {
        let model = cx.new(|_| {
            let mut m = SourceModel::for_tests(vec![SourceProfile {
                id: "offline".into(),
                name: "Full cached source name".into(),
                save_password: false,
                ..SourceProfile::default()
            }]);
            m.cached_at.insert("offline".into(), 123);
            m.cached_offline.insert("offline".into());
            m.metadata_notice = Some("Local metadata cache is unavailable.".into());
            m.tree.databases.insert("offline".into(), vec!["db".into()]);
            m.tree.tables.insert(
                ("offline".into(), "db".into()),
                vec![dalan_drivers::TableInfo {
                    name: "items".into(),
                    kind: "BASE TABLE".into(),
                }],
            );
            m.tree.expanded_sources.insert("offline".into());
            m
        });
        let (explorer, cx) = kit_window(cx, |cx| SourceExplorer::new(model.clone(), cx));
        cx.simulate_resize(gpui::size(px(240.), px(420.)));
        cx.refresh().unwrap();
        cx.run_until_parked();
        assert!(cx.debug_bounds("source-metadata-notice").is_some());
        let marker = cx.debug_bounds("cached-status-offline").unwrap();
        assert_eq!(marker.size, gpui::size(px(18.), px(18.)));
        let db = TreeKey::Database {
            source: "offline".into(),
            database: "db".into(),
        };
        click(cx, Box::leak(tree_row_id(&db).into_boxed_str()));
        let group = TreeKey::Group {
            source: "offline".into(),
            database: "db".into(),
            views: false,
        };
        click(cx, Box::leak(tree_row_id(&group).into_boxed_str()));
        model.read_with(cx, |m, _| {
            assert!(m.page.is_none());
            assert!(!m.busy);
            assert!(m.tree.loading.is_empty());
            assert_eq!(
                m.tree.tables[&("offline".into(), "db".into())][0].name,
                "items"
            );
            assert_eq!(
                cached_status(m.refreshing_source("offline"), true, false),
                Some("Cached")
            );
        });
        for loading in [false, true] {
            model.update(cx, |m, cx| {
                let key = TreeKey::Source("offline".into());
                m.tree.errors.insert(
                    key.clone(),
                    "Refresh failed; keeping cached metadata".into(),
                );
                if loading {
                    m.tree.loading.insert(key);
                }
                cx.notify();
            });
            cx.run_until_parked();
            assert!(cx.debug_bounds("cached-status-offline").is_some());
            explorer.read_with(cx, |view, _| assert_eq!(view.rows.len(), 5));
            model.read_with(cx, |m, _| {
                assert_eq!(
                    cached_status(m.refreshing_source("offline"), true, true),
                    Some(if loading { "Refreshing…" } else { "Stale" })
                );
                assert!(m.page.is_none());
            });
        }
    }

    #[gpui::test]
    fn header_clicks_cycle_sort_and_switch_columns(cx: &mut TestAppContext) {
        let (model, cx) = sorting_fixture(cx);
        for column in ["id", "name"] {
            let id = if column == "id" {
                "sort-column-0"
            } else {
                "sort-column-1"
            };
            for direction in [
                Some(SortDirection::Ascending),
                Some(SortDirection::Descending),
                None,
            ] {
                click(cx, id);
                model.read_with(cx, |model, _| {
                    assert_eq!(model.sort.as_ref().map(|sort| sort.direction), direction);
                    if let Some(sort) = &model.sort {
                        assert_eq!(sort.column, column);
                    }
                    assert!(!model.busy);
                    assert_eq!(model.page.as_ref().unwrap().rows.len(), 2);
                });
            }
        }
        click(cx, "sort-column-0");
        click(cx, "sort-column-1");
        model.read_with(cx, |model, _| {
            let sort = model.sort.as_ref().unwrap();
            assert_eq!(sort.column, "name");
            assert_eq!(sort.direction, SortDirection::Ascending);
        });
    }

    #[gpui::test]
    fn header_keyboard_enter_and_space_activate_sort(cx: &mut TestAppContext) {
        let (model, cx) = sorting_fixture(cx);
        // This isolated browser has no Shell tab action; traverse GPUI's focus tree.
        let mut focused_header = false;
        for _ in 0..20 {
            cx.update(|window, cx| window.focus_next(cx));
            cx.run_until_parked();
            press(cx, "enter");
            cx.run_until_parked();
            if model.read_with(cx, |model, _| model.sort.is_some()) {
                focused_header = true;
                break;
            }
        }
        assert!(focused_header, "header was not reachable by keyboard");
        let column = model.read_with(cx, |model, _| model.sort.as_ref().unwrap().column.clone());
        assert_eq!(
            model.read_with(cx, |model, _| model.sort.as_ref().unwrap().direction),
            SortDirection::Ascending
        );
        press(cx, "space");
        cx.run_until_parked();
        model.read_with(cx, |model, _| {
            let sort = model.sort.as_ref().unwrap();
            assert_eq!(sort.column, column);
            assert_eq!(sort.direction, SortDirection::Descending);
        });
        press(cx, "enter");
        cx.run_until_parked();
        assert!(model.read_with(cx, |model, _| model.sort.is_none()));
    }

    #[gpui::test]
    fn stale_and_busy_headers_ignore_clicks(cx: &mut TestAppContext) {
        let (model, cx) = sorting_fixture(cx);
        for busy in [true, false] {
            model.update(cx, |model, cx| {
                model.busy = busy;
                model.error = (!busy).then(|| "failed reload; old page".into());
                cx.notify();
            });
            click(cx, "sort-column-0");
            click(cx, "sort-column-1");
            assert!(model.read_with(cx, |model, _| model.sort.is_none()));
        }
    }

    #[gpui::test]
    fn export_button_uses_native_picker_and_cancel_clears_busy(cx: &mut TestAppContext) {
        let (model, cx) = sorting_fixture(cx);
        click(cx, "export-loaded-page");
        assert!(cx.did_prompt_for_new_path());
        assert!(model.read_with(cx, |model, _| model.export_busy));
        click(cx, "export-loaded-page");
        cx.simulate_new_path_selection(|_| None);
        cx.run_until_parked();
        assert!(!cx.did_prompt_for_new_path());
        model.read_with(cx, |model, _| {
            assert!(!model.export_busy);
            assert_eq!(
                model.export_feedback.as_deref(),
                Some("Export cancelled; no file written.")
            );
        });
    }

    #[gpui::test]
    fn export_button_is_disabled_for_truncated_stale_and_busy_pages(cx: &mut TestAppContext) {
        let (model, cx) = sorting_fixture(cx);
        for state in 0..3 {
            model.update(cx, |model, cx| {
                model.busy = state == 0;
                model.error = (state == 1).then(|| "failed reload; old page".into());
                let mut snapshot = model.page.as_ref().unwrap().as_ref().clone();
                snapshot.truncated = state == 2;
                model.page = Some(std::sync::Arc::new(snapshot));
                cx.notify();
            });
            click(cx, "export-loaded-page");
            assert!(!cx.did_prompt_for_new_path());
            assert!(!model.read_with(cx, |model, _| model.export_busy));
        }
    }

    #[gpui::test]
    fn clause_inputs_keep_drafts_until_apply_and_clear(cx: &mut TestAppContext) {
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let profile = SourceProfile {
            host: "127.0.0.1".into(),
            port: server.local_addr().unwrap().port(),
            ..Default::default()
        };
        let model = cx.new(|_| {
            let mut model = SourceModel::for_tests(vec![profile.clone()]);
            model.selected_source = Some(profile.id.clone());
            model.selected_database = Some("db".into());
            model.selected_table = Some("items".into());
            model.page = Some(std::sync::Arc::new(page()));
            model
        });
        let (browser, cx) = kit_window(cx, |cx| SourceBrowser::new(model.clone(), cx));
        cx.simulate_resize(gpui::size(px(1000.), px(800.)));
        cx.refresh().unwrap();
        cx.run_until_parked();
        click(cx, "where-clause");
        cx.simulate_input("id > 1");
        click(cx, "order-by-clause");
        cx.simulate_input("name DESC, id ASC");
        model.update(cx, |model, cx| {
            assert_eq!(model.where_clause, "");
            assert_eq!(model.order_by, "");
            cx.notify();
        });
        cx.run_until_parked();
        browser.read_with(cx, |browser, app| {
            assert_eq!(browser.where_input.read(app).value(), "id > 1");
            assert_eq!(browser.order_input.read(app).value(), "name DESC, id ASC");
        });
        press(cx, "enter");
        cx.run_until_parked();
        model.read_with(cx, |model, _| {
            assert_eq!(model.where_clause, "id > 1");
            assert_eq!(model.order_by, "name DESC, id ASC");
        });
        model.update(cx, |model, cx| model.cancel_table(cx));
        cx.run_until_parked();
        click(cx, "clear-table-conditions");
        browser.read_with(cx, |browser, app| {
            assert_eq!(browser.where_input.read(app).value(), "");
            assert_eq!(browser.order_input.read(app).value(), "");
        });
        model.read_with(cx, |model, _| {
            assert_eq!(model.where_clause, "");
            assert_eq!(model.order_by, "");
        });
    }

    #[gpui::test]
    fn explorer_delete_requires_confirmation_and_missing_repository_never_removes(
        cx: &mut TestAppContext,
    ) {
        let profile = SourceProfile::default();
        let id = profile.id.clone();
        let model = cx.new(|_| {
            let mut model = SourceModel::for_tests(vec![profile]);
            model.selected_source = Some(id.clone());
            model
        });
        let (_, cx) = kit_window(cx, |cx| SourceExplorer::new(model.clone(), cx));
        cx.refresh().unwrap();
        cx.run_until_parked();
        assert!(cx.debug_bounds("confirm-delete").is_none());
        click(cx, &format!("source-actions-{id}"));
        click(cx, "source-action-remove");
        assert!(model.read_with(cx, |model, _| model.delete_confirm));
        click(cx, "cancel-delete");
        assert!(!model.read_with(cx, |model, _| model.delete_confirm));
        click(cx, &format!("source-actions-{id}"));
        click(cx, "source-action-remove");
        click(cx, "confirm-delete");
        model.read_with(cx, |model, _| {
            assert_eq!(model.profiles.len(), 1);
            assert_eq!(model.selected_source.as_deref(), Some(id.as_str()));
            assert!(!model.saving);
            assert!(!model.busy);
        });
        click(cx, "cancel-delete");
        click(cx, &format!("source-actions-{id}"));
        click(cx, "source-action-manage");
        assert_eq!(
            model.read_with(cx, |model, _| model
                .form_profile
                .as_ref()
                .unwrap()
                .id
                .clone()),
            id
        );
    }

    #[gpui::test]
    fn compact_toolbar_and_engine_rows(cx: &mut TestAppContext) {
        let mysql = SourceProfile {
            id: "mysql-fixture".into(),
            name: "Inventory".into(),
            color: Some("#ff0000".into()),
            ..SourceProfile::default()
        };
        let maria = SourceProfile {
            id: "maria-fixture".into(),
            name: "Reporting".into(),
            engine: DbEngine::MariaDb,
            ..mysql.clone()
        };
        let model = cx.new(|_| SourceModel::for_tests(vec![mysql, maria]));
        let (_, cx) = kit_window(cx, |cx| SourceExplorer::new(model.clone(), cx));
        cx.simulate_resize(gpui::size(px(320.), px(600.)));
        cx.refresh().unwrap();
        cx.run_until_parked();
        let toolbar = cx.debug_bounds("source-explorer-toolbar").unwrap();
        assert_eq!(toolbar.size.height, px(TOOLBAR_HEIGHT));
        // Reserve the explorer's keyboard-focus outline without shifting layout on focus.
        assert_eq!(toolbar.top(), px(1.));
        let mut previous = toolbar.left();
        for id in [
            "refresh-source",
            "toggle-tree-expansion",
            "explorer-new-console",
        ] {
            let bounds = cx.debug_bounds(id).unwrap();
            assert_eq!(bounds.size.width, px(28.));
            assert_eq!(bounds.size.height, px(28.));
            assert!(bounds.left() >= previous);
            previous = bounds.right();
        }
        for id in [
            "driver-glyph-mysql-fixture",
            "driver-glyph-maria-fixture",
            "color-indicator-mysql-fixture",
            "color-indicator-maria-fixture",
        ] {
            assert!(cx.debug_bounds(id).is_some(), "missing {id}");
        }
        for id in [
            "add-source",
            "edit-source",
            "delete-source",
            "expand-loaded-tree",
            "collapse-all-tree",
        ] {
            assert!(
                cx.debug_bounds(id).is_none(),
                "removed toolbar control {id}"
            );
        }
        click(cx, "refresh-source");
        model.read_with(cx, |m, _| {
            assert!(!m.form_open);
            assert!(!m.delete_confirm);
            assert!(!m.busy);
        });
        click(cx, "source-actions-maria-fixture");
        cx.simulate_keystrokes("down");
        cx.run_until_parked();
        press(cx, "enter");
        cx.run_until_parked();
        model.read_with(cx, |m, _| {
            assert!(m.form_open);
            assert_eq!(m.form_profile.as_ref().unwrap().id, "maria-fixture");
        });
        let fallback: Hsla = rgb(0x808080).into();
        assert_eq!(
            source_color(Some("#ff0000"), fallback),
            Hsla::from(rgb(0xff0000))
        );
        assert_eq!(source_color(None, fallback), fallback);
        for invalid in ["ff0000", "#fff", "#zzzzzz", "#1000000"] {
            assert_eq!(source_color(Some(invalid), fallback), fallback);
        }
    }

    #[gpui::test]
    fn source_actions_copy_exact_row_and_keyboard_dismissal(cx: &mut TestAppContext) {
        let selected = SourceProfile {
            id: "11111111-1111-4111-8111-111111111111".into(),
            name: "Selected".into(),
            ..SourceProfile::default()
        };
        let other = SourceProfile {
            id: "22222222-2222-4222-8222-222222222222".into(),
            name: "Other".into(),
            ..SourceProfile::default()
        };
        let model = cx.new(|_| {
            let mut model = SourceModel::for_tests(vec![selected, other]);
            model.explorer_source = Some("11111111-1111-4111-8111-111111111111".into());
            model
        });
        let (explorer, cx) = kit_window(cx, |cx| SourceExplorer::new(model.clone(), cx));
        cx.refresh().unwrap();
        click(cx, "source-actions-22222222-2222-4222-8222-222222222222");
        model.read_with(cx, |model, _| {
            assert_eq!(
                model.explorer_source.as_deref(),
                Some("11111111-1111-4111-8111-111111111111")
            );
            assert!(model.tree.expanded_sources.is_empty());
            assert!(model.tree.loading.is_empty());
        });
        cx.simulate_keystrokes("down");
        cx.run_until_parked();
        cx.simulate_keystrokes("down");
        cx.run_until_parked();
        press(cx, "enter");
        cx.run_until_parked();
        model.read_with(cx, |model, _| {
            assert!(model.form_open);
            let copy = model.form_profile.as_ref().unwrap();
            assert_eq!(copy.name, "Other copy");
            assert_ne!(copy.id, "22222222-2222-4222-8222-222222222222");
            assert_eq!(model.profiles.len(), 2);
        });
        explorer.read_with(cx, |view, _| assert!(view.menu_source.is_none()));
        click(cx, "source-actions-22222222-2222-4222-8222-222222222222");
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        explorer.read_with(cx, |view, _| assert!(view.menu_source.is_none()));
    }

    #[gpui::test]
    fn virtual_tree_paints_only_viewport_and_scrolls_to_end(cx: &mut TestAppContext) {
        let profile = SourceProfile {
            id: "virtual-source".into(),
            name: "quoted `名字' ASCII".into(),
            ..SourceProfile::default()
        };
        let model = cx.new(|_| {
            let mut m = SourceModel::for_tests(vec![profile]);
            m.tree.databases.insert(
                "virtual-source".into(),
                (0..1000).map(|i| format!("db{i:04}")).collect(),
            );
            m.cached_at.insert("virtual-source".into(), 123);
            m.cached_offline.insert("virtual-source".into());
            m.metadata_notice = Some("Cached metadata warning".into());
            m.tree.expanded_sources.insert("virtual-source".into());
            m
        });
        let (explorer, cx) = kit_window(cx, |cx| SourceExplorer::new(model.clone(), cx));
        cx.simulate_resize(gpui::size(px(200.), px(420.)));
        cx.refresh().unwrap();
        cx.run_until_parked();
        explorer.read_with(cx, |view, _| {
            assert_eq!(view.rows.len(), 1001);
            assert_eq!(view.rows[0].label, "quoted `名字' ASCII");
            assert!(view.last_rendered_row_count > 0);
            assert!(view.last_rendered_row_count <= 40);
        });
        let label = cx
            .debug_bounds("tree-label-connect-source-virtual-source")
            .unwrap();
        assert!(label.size.width > px(80.));
        let row = cx.debug_bounds("connect-source-virtual-source").unwrap();
        assert_eq!(row.size.height, px(22.));
        explorer.update(cx, |view, cx| {
            view.scroll
                .scroll_to_item_strict(901, gpui::ScrollStrategy::Top);
            cx.notify();
        });
        cx.run_until_parked();
        let label_id = format!(
            "tree-label-{}",
            tree_row_id(&TreeKey::Database {
                source: "virtual-source".into(),
                database: "db0900".into(),
            })
        );
        assert!(
            cx.debug_bounds(Box::leak(label_id.into_boxed_str()))
                .unwrap()
                .size
                .width
                > px(80.)
        );
        explorer.read_with(cx, |view, _| assert!(view.last_rendered_row_count <= 40));
        let focus = explorer.read_with(cx, |view, _| view.tree_focus.clone());
        cx.update(|window, cx| focus.focus(window, cx));
        cx.simulate_keystrokes("end");
        cx.run_until_parked();
        explorer.read_with(cx, |view, _| {
            assert_eq!(
                view.active_key,
                Some(TreeKey::Database {
                    source: "virtual-source".into(),
                    database: "db0999".into(),
                })
            );
            assert!(view.last_rendered_row_count <= 40);
        });
        click(cx, "toggle-tree-expansion");
        explorer.read_with(cx, |view, _| assert_eq!(view.rows.len(), 1));
        model.read_with(cx, |model, _| {
            assert_eq!(model.tree.databases["virtual-source"].len(), 1000)
        });
        click(cx, "toggle-tree-expansion");
        explorer.read_with(cx, |view, _| assert_eq!(view.rows.len(), 1001));
        model.read_with(cx, |model, _| assert!(model.tree.loading.is_empty()));
    }

    #[gpui::test]
    fn cached_tree_groups_keyboard_and_unicode_labels(cx: &mut TestAppContext) {
        let model = cx.new(|_| {
            let mut m = SourceModel::for_tests(vec![SourceProfile {
                id: "cached".into(),
                name: "名前 `source'".into(),
                ..SourceProfile::default()
            }]);
            m.tree
                .databases
                .insert("cached".into(), vec!["库存 `db'".into()]);
            m.tree.tables.insert(
                ("cached".into(), "库存 `db'".into()),
                vec![
                    dalan_drivers::TableInfo {
                        name: "明細 `items'".into(),
                        kind: "BASE TABLE".into(),
                    },
                    dalan_drivers::TableInfo {
                        name: "read view".into(),
                        kind: "VIEW".into(),
                    },
                ],
            );
            m.tree.expand_loaded();
            m
        });
        let (explorer, cx) = kit_window(cx, |cx| SourceExplorer::new(model.clone(), cx));
        cx.simulate_resize(gpui::size(px(240.), px(420.)));
        cx.refresh().unwrap();
        cx.run_until_parked();
        explorer.read_with(cx, |view, _| {
            assert_eq!(view.rows.len(), 6);
            assert_eq!(view.rows[3].label, "明細 `items'");
            assert_eq!(view.rows[5].label, "read view");
            assert!(view.rows.iter().all(|row| !row.label.is_empty()));
        });
        let table = TreeKey::Table {
            source: "cached".into(),
            database: "库存 `db'".into(),
            table: "明細 `items'".into(),
            view: false,
        };
        let label_id = format!("tree-label-{}", tree_row_id(&table));
        assert!(
            cx.debug_bounds(Box::leak(label_id.into_boxed_str()))
                .unwrap()
                .size
                .width
                > px(80.)
        );
        explorer.update(cx, |view, cx| {
            view.select_key(table, cx);
        });
        let focus = explorer.read_with(cx, |view, _| view.tree_focus.clone());
        cx.update(|window, cx| focus.focus(window, cx));
        cx.simulate_keystrokes("left");
        cx.run_until_parked();
        explorer.read_with(cx, |view, _| {
            assert!(matches!(
                view.active_key,
                Some(TreeKey::Group { views: false, .. })
            ))
        });
        cx.simulate_keystrokes("left");
        cx.run_until_parked();
        explorer.read_with(cx, |view, _| assert_eq!(view.rows.len(), 5));
        cx.simulate_keystrokes("right");
        cx.run_until_parked();
        explorer.read_with(cx, |view, _| assert_eq!(view.rows.len(), 6));
        model.read_with(cx, |model, _| {
            assert!(model.tree.loading.is_empty());
            assert!(model.selected_table.is_none());
        });
    }

    #[gpui::test]
    fn source_row_manage_and_remove_are_guarded(cx: &mut TestAppContext) {
        let profile = SourceProfile {
            save_password: false,
            ..SourceProfile::default()
        };
        let model = cx.new(|_| {
            let mut m = SourceModel::for_tests(vec![profile.clone()]);
            m.selected_source = Some(profile.id.clone());
            m
        });
        let (_, cx) = kit_window(cx, |cx| SourceExplorer::new(model.clone(), cx));
        cx.refresh().unwrap();
        click(cx, &format!("source-actions-{}", profile.id));
        click(cx, "source-action-manage");
        model.read_with(cx, |m, _| {
            assert_eq!(m.form_profile.as_ref(), Some(&profile))
        });
        click(cx, &format!("source-actions-{}", profile.id));
        click(cx, "source-action-remove");
        assert!(model.read_with(cx, |m, _| m.delete_confirm));
        click(cx, "cancel-delete");
        for saving in [false, true] {
            model.update(cx, |m, cx| {
                m.form_open = false;
                m.busy = !saving;
                m.saving = saving;
                cx.notify();
            });
            click(cx, "refresh-source");
            click(cx, &format!("source-actions-{}", profile.id));
            for id in [
                "source-action-manage",
                "source-action-copy",
                "source-action-remove",
            ] {
                click(cx, id);
            }
            cx.simulate_keystrokes("escape");
            cx.run_until_parked();
            model.read_with(cx, |m, _| {
                assert!(!m.form_open);
                assert!(!m.delete_confirm);
            });
        }
    }

    #[gpui::test]
    fn empty_browser_connect_is_centered_and_keyboard_accessible(cx: &mut TestAppContext) {
        let model = cx.new(|_| SourceModel::for_tests(vec![]));
        let (_, cx) = kit_window(cx, |cx| SourceBrowser::new(model.clone(), cx));
        cx.simulate_resize(gpui::size(px(1000.), px(800.)));
        cx.refresh().unwrap();
        cx.run_until_parked();
        let root = cx.debug_bounds("source-browser").unwrap();
        let connect = cx.debug_bounds("connect-empty-source").unwrap();
        assert!((connect.center().x - root.center().x).abs() < px(1.));
        assert!((connect.center().y - root.center().y).abs() < px(30.));
        click(cx, "connect-empty-source");
        assert!(model.read_with(cx, |m, _| m.form_open && m.form_profile.is_some()));
        for key in ["enter", "space"] {
            model.update(cx, |m, cx| {
                m.form_open = false;
                cx.notify();
            });
            cx.update(|window, cx| window.focus_next(cx));
            press(cx, key);
            cx.run_until_parked();
            assert!(model.read_with(cx, |m, _| m.form_open));
        }
        model.update(cx, |m, cx| {
            m.form_open = false;
            m.saving = true;
            cx.notify();
        });
        click(cx, "connect-empty-source");
        assert!(!model.read_with(cx, |m, _| m.form_open));
    }

    #[gpui::test]
    fn cached_browser_without_selection_has_only_centered_table_prompt(cx: &mut TestAppContext) {
        let model = cx.new(|_| {
            let mut m = SourceModel::for_tests(vec![SourceProfile {
                id: "offline-prompt".into(),
                ..SourceProfile::default()
            }]);
            m.cached_offline.insert("offline-prompt".into());
            m.tree
                .databases
                .insert("offline-prompt".into(), vec!["cached_db".into()]);
            m
        });
        let (_, cx) = kit_window(cx, |cx| SourceBrowser::new(model.clone(), cx));
        cx.simulate_resize(gpui::size(px(1000.), px(800.)));
        cx.refresh().unwrap();
        cx.run_until_parked();
        assert!(cx.debug_bounds("table-browser-header").is_none());
        assert!(cx.debug_bounds("read-only-indicator").is_none());
        assert!(cx.debug_bounds("connect-empty-source").is_none());
        let root = cx.debug_bounds("source-browser").unwrap();
        let prompt = cx.debug_bounds("table-browser-empty").unwrap();
        assert!((prompt.center().x - root.center().x).abs() < px(1.));
        assert!((prompt.center().y - root.center().y).abs() < px(1.));
        model.read_with(cx, |m, _| {
            assert!(m.selected_source.is_none());
            assert!(m.tree.loading.is_empty());
        });
        model.update(cx, |m, cx| {
            m.busy = true;
            cx.notify();
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("table-browser-loading").is_some());
        // GPUI retains debug bounds for removed elements; state and loading
        // marker verify this transition without treating stale bounds as live UI.
        assert!(model.read_with(cx, |m, _| m.busy && m.page.is_none()));
        assert!(cx.debug_bounds("table-browser-header").is_none());
    }

    #[gpui::test]
    fn preview_indicator_is_passive_and_controls_are_compact(cx: &mut TestAppContext) {
        let (model, cx) = sorting_fixture(cx);
        let indicator = cx.debug_bounds("read-only-indicator").unwrap();
        assert_eq!(indicator.size, gpui::size(px(28.), px(28.)));
        click(cx, "read-only-indicator");
        model.read_with(cx, |m, _| {
            assert!(!m.form_open);
            assert!(!m.busy);
            assert_eq!(m.selected_table.as_deref(), Some("items"));
        });
        for id in [
            "export-loaded-page",
            "previous-page",
            "next-page",
            "apply-table-conditions",
            "clear-table-conditions",
        ] {
            let bounds = cx.debug_bounds(id).unwrap();
            assert_eq!(
                bounds.size,
                gpui::size(px(CONTROL_HEIGHT), px(CONTROL_HEIGHT))
            );
        }
    }

    #[gpui::test]
    fn committed_order_changes_sync_without_clobbering_where_draft(cx: &mut TestAppContext) {
        let model = cx.new(|_| SourceModel::for_tests(vec![]));
        let (browser, cx) = kit_window(cx, |cx| SourceBrowser::new(model.clone(), cx));
        browser.update(cx, |browser, cx| {
            browser
                .where_input
                .update(cx, |input, cx| input.set_value("id > 2", cx));
            browser
                .order_input
                .update(cx, |input, cx| input.set_value("draft", cx));
        });
        model.update(cx, |model, cx| {
            model.order_by = "`name` DESC".into();
            cx.notify();
        });
        cx.run_until_parked();
        browser.read_with(cx, |browser, app| {
            assert_eq!(browser.where_input.read(app).value(), "id > 2");
            assert_eq!(browser.order_input.read(app).value(), "`name` DESC");
        });
    }

    #[gpui::test]
    fn selection_observer_resets_clause_drafts(cx: &mut TestAppContext) {
        let model = cx.new(|_| SourceModel::for_tests(vec![]));
        let (browser, cx) = kit_window(cx, |cx| SourceBrowser::new(model.clone(), cx));
        browser.update(cx, |browser, cx| {
            browser
                .where_input
                .update(cx, |input, cx| input.set_value("old", cx));
        });
        model.update(cx, |model, cx| {
            model.selected_database = Some("new_database".into());
            cx.notify();
        });
        cx.run_until_parked();
        browser.read_with(cx, |browser, app| {
            assert_eq!(browser.where_input.read(app).value(), "");
            assert_eq!(browser.order_input.read(app).value(), "");
        });
    }
    #[test]
    fn page_summary_reports_configured_two_hundred_rows() {
        let mut snapshot = page();
        snapshot.rows = vec![snapshot.rows[0].clone(); 200];
        snapshot.offset = 200;
        let (range, tooltip) = page_summary(&snapshot);
        assert_eq!(range, "201–400");
        assert!(tooltip.starts_with("200 loaded rows"));
    }
}

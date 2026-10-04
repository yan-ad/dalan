//! Read-only source explorer and bounded table browser.
use dalan_drivers::mysql::{CellValue, FilterOperator, SortDirection, TableFilter};
use gpui::{Context, Div, Entity, Stateful, Subscription, Window, div, prelude::*, px, rgb};

use super::{
    icons::{Icon, icon},
    input::TextInput,
    source_model::SourceModel,
    theme::*,
};

pub(super) struct SourceExplorer {
    model: Entity<SourceModel>,
    _subscription: Subscription,
}

impl SourceExplorer {
    pub(super) fn new(model: Entity<SourceModel>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&model, |_, _, cx| cx.notify());
        Self {
            model,
            _subscription: subscription,
        }
    }
}

impl Render for SourceExplorer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.model.read(cx);
        let profiles = model.profiles.clone();
        let selected_source = model.selected_source.clone();
        let selected_database = model.selected_database.clone();
        let selected_table = model.selected_table.clone();
        let databases = model.databases.clone();
        let tables = model.tables.clone();
        let busy = model.busy;
        let error = model.error.clone();
        let confirm = model.delete_confirm;
        let source_name = profiles
            .iter()
            .find(|p| Some(&p.id) == selected_source.as_ref())
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "Data sources".into());
        let mut entries = div()
            .id("source-explorer-scroll")
            .debug_selector(|| "source-explorer-scroll".into())
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .p(px(8.))
            .flex()
            .flex_col()
            .gap(px(6.));
        if profiles.is_empty() {
            entries = entries.child(
                div()
                    .p(px(8.))
                    .text_color(rgb(MUTED))
                    .child("No data sources yet. Add a MySQL or MariaDB source to begin."),
            );
        }
        for profile in profiles {
            let selected = Some(&profile.id) == selected_source.as_ref();
            let id = profile.id.clone();
            let mut group = div().flex().flex_col().gap(px(4.)).child(button(
                format!("connect-source-{id}"),
                format!("{} · {}", profile.name, profile.engine.display_name()),
                selected,
                busy,
                cx,
                move |this, cx| {
                    this.model
                        .update(cx, |model, cx| model.connect(id.clone(), cx));
                },
            ));
            if selected {
                group = group.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(4.))
                        .child(button(
                            "edit-source",
                            "Edit",
                            false,
                            busy,
                            cx,
                            |this, cx| this.model.update(cx, |m, cx| m.edit_source(cx)),
                        ))
                        .child(button(
                            "delete-source",
                            "Delete",
                            false,
                            busy,
                            cx,
                            |this, cx| this.model.update(cx, |m, cx| m.request_delete(cx)),
                        ))
                        .child(button(
                            "refresh-source",
                            "Refresh",
                            false,
                            busy,
                            cx,
                            |this, cx| this.model.update(cx, |m, cx| m.refresh(cx)),
                        )),
                );
                if confirm {
                    group = group.child(
                        div()
                            .p(px(6.))
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(format!(
                                "Remove {}? This removes the saved source, not its databases.",
                                profile.name
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
                                        |this, cx| {
                                            this.model.update(cx, |m, cx| m.confirm_delete(cx))
                                        },
                                    ))
                                    .child(button(
                                        "cancel-delete",
                                        "Cancel",
                                        false,
                                        false,
                                        cx,
                                        |this, cx| {
                                            this.model.update(cx, |m, cx| m.request_delete(cx))
                                        },
                                    )),
                            ),
                    );
                }
                let mut hierarchy = div().pl(px(12.)).flex().flex_col().gap(px(4.));
                for database in &databases {
                    let selected = Some(database) == selected_database.as_ref();
                    let db = database.clone();
                    hierarchy = hierarchy.child(button(
                        format!("select-database-{db}"),
                        db.clone(),
                        selected,
                        busy,
                        cx,
                        move |this, cx| {
                            this.model
                                .update(cx, |m, cx| m.select_database(db.clone(), cx));
                        },
                    ));
                    if selected {
                        let mut table_list = div().pl(px(12.)).flex().flex_col().gap(px(3.));
                        for table in &tables {
                            if table.kind == "BASE TABLE" {
                                let name = table.name.clone();
                                table_list = table_list.child(button(
                                    format!("select-table-{database}-{name}"),
                                    name.clone(),
                                    Some(&name) == selected_table.as_ref(),
                                    busy,
                                    cx,
                                    move |this, cx| {
                                        this.model
                                            .update(cx, |m, cx| m.select_table(name.clone(), cx));
                                    },
                                ));
                            } else {
                                table_list = table_list.child(
                                    div().px(px(8.)).py(px(5.)).text_color(rgb(MUTED)).child(
                                        format!("{} · {} (unavailable)", table.name, table.kind),
                                    ),
                                );
                            }
                        }
                        if tables.is_empty() && !busy {
                            table_list = table_list
                                .child(div().text_color(rgb(MUTED)).child("No tables available"));
                        }
                        hierarchy = hierarchy.child(table_list);
                    }
                }
                group = group.child(hierarchy);
            }
            entries = entries.child(group);
        }
        if busy {
            entries = entries.child(
                div()
                    .text_color(rgb(MUTED))
                    .child(format!("Loading {source_name}…")),
            );
        }
        if let Some(error) = error {
            entries = entries.child(
                div()
                    .id("source-load-error")
                    .debug_selector(|| "source-load-error".into())
                    .text_color(rgb(0xf2bf76))
                    .child(format!("{source_name}: {error}")),
            );
        }
        div()
            .id("source-explorer")
            .debug_selector(|| "source-explorer".into())
            .size_full()
            .flex()
            .flex_col()
            .text_size(px(12.))
            .text_color(rgb(TEXT))
            .bg(rgb(PANEL))
            .child(
                div()
                    .flex_shrink_0()
                    .p(px(8.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(4.))
                    .child("Database Explorer")
                    .child(button("add-source", "Add", false, false, cx, |this, cx| {
                        this.model.update(cx, |m, cx| m.new_source(cx))
                    })),
            )
            .child(entries)
    }
}

pub(super) struct SourceBrowser {
    model: Entity<SourceModel>,
    value: Entity<TextInput>,
    column: usize,
    operator: usize,
    selection: (Option<String>, Option<String>, Option<String>),
    _subscriptions: Vec<Subscription>,
}

const OPERATORS: [(FilterOperator, &str); 7] = [
    (FilterOperator::Contains, "Contains"),
    (FilterOperator::Equals, "Equals"),
    (FilterOperator::NotEquals, "Not equals"),
    (FilterOperator::GreaterThan, "Greater than"),
    (FilterOperator::LessThan, "Less than"),
    (FilterOperator::IsNull, "Is NULL"),
    (FilterOperator::IsNotNull, "Is not NULL"),
];

impl SourceBrowser {
    pub(super) fn new(model: Entity<SourceModel>, cx: &mut Context<Self>) -> Self {
        let value = cx.new(|cx| {
            let mut input = TextInput::new("", "Filter value", false, cx);
            input.set_tab_order(20);
            input
        });
        let m = model.read(cx);
        let selection = (
            m.selected_source.clone(),
            m.selected_database.clone(),
            m.selected_table.clone(),
        );
        let subscriptions = vec![
            cx.observe(&model, |this, model, cx| {
                let m = model.read(cx);
                let selection = (
                    m.selected_source.clone(),
                    m.selected_database.clone(),
                    m.selected_table.clone(),
                );
                if this.selection != selection {
                    this.selection = selection;
                    this.column = 0;
                    this.operator = 0;
                    this.value.update(cx, |input, cx| input.set_value("", cx));
                }
                cx.notify();
            }),
            cx.observe(&value, |_, _, cx| cx.notify()),
        ];
        Self {
            model,
            value,
            column: 0,
            operator: 0,
            selection,
            _subscriptions: subscriptions,
        }
    }

    fn apply(&mut self, cx: &mut Context<Self>) {
        if self.model.read(cx).busy {
            return;
        }
        let column = self
            .model
            .read(cx)
            .page
            .as_ref()
            .and_then(|p| p.columns.get(self.column))
            .map(|c| c.name.clone());
        if let Some(column) = column {
            let filter = TableFilter {
                column,
                operator: OPERATORS[self.operator].0,
                value: self.value.read(cx).value(),
            };
            self.model
                .update(cx, |model, cx| model.apply_filter(Some(filter), cx));
        }
    }
}

impl Render for SourceBrowser {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let m = self.model.read(cx);
        let busy = m.busy;
        let error = m.error.clone();
        let stale = busy || error.is_some();
        let page = m.page.clone();
        let sort = m.sort.clone();
        let export_busy = m.export_busy;
        let export_feedback = m.export_feedback.clone();
        let source = m
            .profiles
            .iter()
            .find(|p| Some(&p.id) == m.selected_source.as_ref())
            .map(|p| p.name.clone());
        let title = match (&source, &m.selected_database, &m.selected_table) {
            (Some(source), Some(db), Some(table)) => format!("{source} / {db}.{table}"),
            (Some(source), _, _) => source.clone(),
            _ => "Table browser".into(),
        };
        let empty = if m.profiles.is_empty() {
            "Add a data source in the Database Explorer to browse its tables."
        } else if m.selected_source.is_none() {
            "Connect to a data source in the Database Explorer."
        } else if m.selected_database.is_none() {
            "Select a database in the Database Explorer."
        } else if m.selected_table.is_none() {
            "Select a base table to browse its rows. Views are unavailable."
        } else if busy {
            "Loading table…"
        } else {
            "No table page loaded. Use Refresh to retry."
        };
        let mut body = div()
            .id("source-browser")
            .debug_selector(|| "source-browser".into())
            .size_full()
            .min_w(px(0.))
            .flex()
            .flex_col()
            .bg(rgb(PANEL))
            .text_color(rgb(TEXT))
            .text_size(px(12.))
            .child(
                div()
                    .flex_shrink_0()
                    .p(px(12.))
                    .bg(rgb(HEADER))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .child(div().min_w(px(0.)).text_ellipsis().child(title.clone()))
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_color(rgb(MUTED))
                            .child("Read-only"),
                    ),
            );
        if busy {
            body = body.child(
                div()
                    .px(px(12.))
                    .py(px(6.))
                    .text_color(rgb(MUTED))
                    .child(format!("Loading {title}…")),
            );
        }
        if let Some(error) = error {
            body = body.child(
                div()
                    .px(px(12.))
                    .py(px(6.))
                    .text_color(rgb(0xf2bf76))
                    .child(format!("{title}: {error}")),
            );
        }
        if let Some(page) = page {
            if let Some(feedback) = export_feedback {
                body = body.child(
                    div()
                        .px(px(12.0))
                        .py(px(6.0))
                        .text_color(rgb(MUTED))
                        .child(feedback),
                );
            }
            if stale {
                body = body.child(div().px(px(12.0)).py(px(6.0)).text_color(rgb(MUTED))
                    .child("Previous page shown until the current request succeeds. Pagination is disabled."));
            }
            if self.column >= page.columns.len() {
                self.column = 0;
            }
            let column_label = page
                .columns
                .get(self.column)
                .map(|c| format!("Column: {}", c.name))
                .unwrap_or_else(|| "No columns".into());
            let no_columns = page.columns.is_empty();
            let unary = matches!(
                OPERATORS[self.operator].0,
                FilterOperator::IsNull | FilterOperator::IsNotNull
            );
            body = body.child(
                div()
                    .p(px(10.))
                    .flex_shrink_0()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(8.))
                    .child(button(
                        "filter-column",
                        column_label,
                        false,
                        busy || no_columns,
                        cx,
                        |this, cx| {
                            let count = this
                                .model
                                .read(cx)
                                .page
                                .as_ref()
                                .map_or(0, |p| p.columns.len());
                            if count > 0 {
                                this.column = (this.column + 1) % count;
                                cx.notify();
                            }
                        },
                    ))
                    .child(button(
                        "filter-operator",
                        OPERATORS[self.operator].1,
                        false,
                        busy,
                        cx,
                        |this, cx| {
                            this.operator = (this.operator + 1) % OPERATORS.len();
                            cx.notify();
                        },
                    ))
                    .when(!unary, |el| {
                        el.child(
                            div()
                                .id("filter-value")
                                .debug_selector(|| "filter-value".into())
                                .w(px(200.))
                                .child(self.value.clone()),
                        )
                    })
                    .child(button(
                        "apply-filter",
                        "Apply",
                        false,
                        busy || no_columns,
                        cx,
                        |this, cx| this.apply(cx),
                    ))
                    .child(button(
                        "clear-filter",
                        "Clear",
                        false,
                        busy,
                        cx,
                        |this, cx| {
                            this.column = 0;
                            this.operator = 0;
                            this.value.update(cx, |input, cx| input.set_value("", cx));
                            this.model.update(cx, |m, cx| m.apply_filter(None, cx));
                        },
                    )),
            );
            let mut grid = div()
                .flex()
                .flex_col()
                .w(px(page.columns.len().max(1) as f32 * 180.));
            grid = grid.child(
                div()
                    .flex()
                    .h(px(32.))
                    .flex_shrink_0()
                    .bg(rgb(HEADER))
                    .children(page.columns.iter().enumerate().map(|(index, column)| {
                        let active = sort.as_ref().filter(|sort| sort.column == column.name);
                        let direction = active.map(|sort| sort.direction);
                        let name = column.name.clone();
                        button(
                            format!("sort-column-{index}"),
                            "",
                            active.is_some(),
                            stale,
                            cx,
                            move |this, cx| {
                                this.model
                                    .update(cx, |model, cx| model.cycle_sort(name.clone(), cx))
                            },
                        )
                        .w(px(180.0))
                        .h(px(32.0))
                        .flex_shrink_0()
                        .justify_start()
                        .child(div().flex_1().min_w(px(0.0)).text_ellipsis().child(format!(
                            "{}{} · {}",
                            if column.is_primary_key { "PK " } else { "" },
                            column.name,
                            column.data_type
                        )))
                        .when_some(direction, |header, direction| {
                            header.child(icon(
                                match direction {
                                    SortDirection::Ascending => Icon::SortAscending,
                                    SortDirection::Descending => Icon::SortDescending,
                                },
                                TEXT,
                            ))
                        })
                    })),
            );
            for (index, row) in page.rows.iter().take(100).enumerate() {
                grid = grid.child(
                    div()
                        .flex()
                        .h(px(26.))
                        .flex_shrink_0()
                        .bg(rgb(if index % 2 == 0 { PANEL } else { HEADER }))
                        .children((0..page.columns.len()).map(|index| {
                            let value = row.get(index);
                            cell(
                                value.map(CellValue::display).unwrap_or_default(),
                                matches!(value, Some(CellValue::Null)),
                            )
                        })),
                );
            }
            if page.rows.is_empty() {
                grid = grid.child(
                    div()
                        .p(px(12.))
                        .text_color(rgb(MUTED))
                        .child("No matching rows"),
                );
            }
            body = body.child(
                div()
                    .id("table-grid-scroll")
                    .debug_selector(|| "table-grid-scroll".into())
                    .flex_1()
                    .min_h(px(0.))
                    .min_w(px(0.))
                    .overflow_scroll()
                    .child(grid),
            );
            let count = page.rows.len().min(100);
            let summary = if count == 0 {
                format!("0 rows · offset {}", page.offset)
            } else {
                format!(
                    "{} rows · {}–{}",
                    count,
                    page.offset + 1,
                    page.offset + count as u64
                )
            };
            body = body.child(
                div()
                    .p(px(10.))
                    .flex_shrink_0()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_color(rgb(MUTED))
                            .child(summary)
                            .when(page.truncated, |el| {
                                el.child(" · Values truncated by the reader's safety limits")
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(
                                button(
                                    "export-loaded-page",
                                    "Export loaded CSV",
                                    false,
                                    stale || page.truncated || export_busy,
                                    cx,
                                    |this, cx| {
                                        this.model.update(cx, |model, cx| model.request_export(cx))
                                    },
                                )
                                .child(icon(Icon::Download, MUTED)),
                            )
                            .child(button(
                                "previous-page",
                                "Previous",
                                false,
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
                            .child(button(
                                "next-page",
                                "Next",
                                false,
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
        } else {
            body = body.child(
                div()
                    .flex_1()
                    .p(px(24.))
                    .text_color(rgb(MUTED))
                    .child(empty),
            );
        }
        body
    }
}

fn cell(value: String, null: bool) -> Div {
    div()
        .w(px(180.))
        .h(px(26.))
        .flex_shrink_0()
        .px(px(8.))
        .py(px(4.))
        .overflow_hidden()
        .text_ellipsis()
        .border_r_1()
        .border_color(rgb(CHROME))
        .text_color(rgb(if null { MUTED } else { TEXT }))
        .child(value)
}

/// GPUI creates a focus handle for tab-indexed elements; IDs keep it stable across renders.
fn button<T: 'static>(
    id: impl Into<String>,
    label: impl Into<String>,
    selected: bool,
    disabled: bool,
    cx: &mut Context<T>,
    activate: impl Fn(&mut T, &mut Context<T>) + Clone + 'static,
) -> Stateful<Div> {
    let id: gpui::SharedString = id.into().into();
    let label: String = label.into();
    let debug_id = id.clone();
    let click = activate.clone();
    div()
        .id(id)
        .debug_selector(move || debug_id.clone().into())
        .tab_index(20)
        .tab_stop(!disabled)
        .flex()
        .items_center()
        .min_w(px(0.))
        .px(px(8.))
        .py(px(5.))
        .rounded(px(3.))
        .border_1()
        .border_color(rgb(if selected { FOCUS } else { CHROME }))
        .bg(rgb(if selected { SELECTION } else { CHROME }))
        .text_color(rgb(if disabled { MUTED } else { TEXT }))
        .when(disabled, |el| el.opacity(0.5))
        .when(!disabled, |el| {
            el.cursor_pointer().hover(|style| style.bg(rgb(HOVER)))
        })
        .focus(|style| style.border_color(rgb(FOCUS)))
        .on_click(cx.listener(move |this, _, _, cx| {
            if !disabled {
                click(this, cx);
            }
        }))
        .on_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, _, cx| {
            if !disabled && matches!(event.keystroke.key.as_str(), "enter" | "space") {
                cx.stop_propagation();
                activate(this, cx);
            }
        }))
        .child(
            div()
                .min_w(px(0.))
                .overflow_hidden()
                .text_ellipsis()
                .child(label),
        )
}

#[cfg(all(test, feature = "ui-tests"))]
mod tests {
    use super::*;
    use dalan_drivers::{SourceProfile, TablePage, mysql::ColumnInfo};
    use gpui::{Modifiers, TestAppContext, VisualTestContext};

    fn click(cx: &mut VisualTestContext, id: &'static str) {
        cx.run_until_parked();
        let bounds = cx
            .debug_bounds(id)
            .unwrap_or_else(|| panic!("missing {id}"));
        cx.simulate_click(bounds.center(), Modifiers::default());
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
        cx.update(crate::desktop::bind_keys);
        let model = cx.new(|_| {
            let mut model = SourceModel::for_tests(vec![]);
            model.selected_database = Some("inventory".into());
            model.selected_table = Some("items".into());
            model.page = Some(page());
            model
        });
        let (_, cx) = cx.add_window_view(|_, cx| SourceBrowser::new(model.clone(), cx));
        cx.simulate_resize(gpui::size(px(1000.), px(800.)));
        cx.refresh().unwrap();
        cx.run_until_parked();
        (model, cx)
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
            cx.update(|window, _| window.focus_next());
            cx.run_until_parked();
            cx.simulate_keystrokes("enter");
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
        cx.simulate_keystrokes("space");
        cx.run_until_parked();
        model.read_with(cx, |model, _| {
            let sort = model.sort.as_ref().unwrap();
            assert_eq!(sort.column, column);
            assert_eq!(sort.direction, SortDirection::Descending);
        });
        cx.simulate_keystrokes("enter");
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
                model.page.as_mut().unwrap().truncated = state == 2;
                cx.notify();
            });
            click(cx, "export-loaded-page");
            assert!(!cx.did_prompt_for_new_path());
            assert!(!model.read_with(cx, |model, _| model.export_busy));
        }
    }

    #[gpui::test]
    fn filter_controls_bind_literal_value_cycle_and_clear(cx: &mut TestAppContext) {
        cx.update(crate::desktop::bind_keys);
        let model = cx.new(|_| {
            let mut model = SourceModel::for_tests(vec![SourceProfile::default()]);
            // No selected source means Apply cannot launch a database task.
            model.selected_database = Some("inventory".into());
            model.selected_table = Some("items".into());
            model.page = Some(page());
            model
        });
        let (browser, cx) = cx.add_window_view(|_, cx| SourceBrowser::new(model.clone(), cx));
        cx.simulate_resize(gpui::size(px(1000.), px(800.)));
        cx.refresh().unwrap();
        cx.run_until_parked();
        click(cx, "filter-column");
        assert_eq!(browser.read_with(cx, |browser, _| browser.column), 1);
        click(cx, "filter-operator");
        assert_eq!(browser.read_with(cx, |browser, _| browser.operator), 1);
        click(cx, "filter-value");
        let literal = "' OR 1=1; --";
        cx.simulate_input(literal);
        click(cx, "apply-filter");
        model.read_with(cx, |model, _| {
            let filter = model.test_filter().unwrap();
            assert_eq!(filter.column, "name");
            assert_eq!(filter.operator, FilterOperator::Equals);
            assert_eq!(filter.value, literal);
            assert!(!model.busy);
            assert_eq!(model.page.as_ref().unwrap().rows.len(), 2);
        });
        for _ in 0..4 {
            click(cx, "filter-operator");
        }
        assert_eq!(browser.read_with(cx, |browser, _| browser.operator), 5);
        click(cx, "apply-filter");
        assert_eq!(
            model.read_with(cx, |model, _| model.test_filter().unwrap().operator),
            FilterOperator::IsNull
        );
        click(cx, "clear-filter");
        assert!(model.read_with(cx, |model, _| model.test_filter().is_none()));
        browser.read_with(cx, |browser, app| {
            assert_eq!(browser.column, 0);
            assert_eq!(browser.operator, 0);
            assert_eq!(browser.value.read(app).value(), "");
        });
        for _ in 0..7 {
            click(cx, "filter-operator");
        }
        assert_eq!(browser.read_with(cx, |browser, _| browser.operator), 0);
        click(cx, "filter-column");
        click(cx, "filter-column");
        assert_eq!(browser.read_with(cx, |browser, _| browser.column), 0);
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
        let (_, cx) = cx.add_window_view(|_, cx| SourceExplorer::new(model.clone(), cx));
        cx.refresh().unwrap();
        cx.run_until_parked();
        assert!(cx.debug_bounds("confirm-delete").is_none());
        click(cx, "delete-source");
        assert!(model.read_with(cx, |model, _| model.delete_confirm));
        click(cx, "cancel-delete");
        assert!(!model.read_with(cx, |model, _| model.delete_confirm));
        click(cx, "delete-source");
        click(cx, "confirm-delete");
        model.read_with(cx, |model, _| {
            assert_eq!(model.profiles.len(), 1);
            assert_eq!(model.selected_source.as_deref(), Some(id.as_str()));
            assert!(!model.saving);
            assert!(!model.busy);
        });
        click(cx, "cancel-delete");
        click(cx, "edit-source");
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
    fn selection_observer_resets_filter_draft(cx: &mut TestAppContext) {
        let model = cx.new(|_| SourceModel::for_tests(vec![]));
        let (browser, cx) = cx.add_window_view(|_, cx| SourceBrowser::new(model.clone(), cx));
        browser.update(cx, |browser, cx| {
            browser.column = 1;
            browser.operator = 4;
            browser
                .value
                .update(cx, |input, cx| input.set_value("old", cx));
        });
        model.update(cx, |model, cx| {
            model.selected_database = Some("new_database".into());
            cx.notify();
        });
        cx.run_until_parked();
        browser.read_with(cx, |browser, app| {
            assert_eq!(browser.column, 0);
            assert_eq!(browser.operator, 0);
            assert_eq!(browser.value.read(app).value(), "");
        });
    }
}

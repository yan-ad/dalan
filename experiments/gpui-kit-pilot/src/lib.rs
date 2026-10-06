//! Independent Kit migration probe; no database or production application dependencies.
//! Entry-point/test patterns follow https://github.com/longbridge/gpui-kit (Apache-2.0).
use gpui_kit::component::{
    ActiveTheme, IconName, Sizable,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    input::{Editor, EditorState},
    table::{Column, DataTable, TableDelegate, TableState},
};
use gpui_kit::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

pub const ROWS: usize = 100;
pub const COLS: usize = 512;
pub const SQL: &str =
    "SELECT id, name, created_at\nFROM migration_fixture\nWHERE id > 0\nORDER BY id;";

pub struct FixtureTable {
    pub data: Arc<Vec<Vec<String>>>,
    pub calls: Arc<AtomicUsize>,
}
impl FixtureTable {
    pub fn new() -> Self {
        // A single page, cached once. Delegates never clone the full result page.
        let data = (0..ROWS)
            .map(|r| {
                (0..COLS)
                    .map(|c| format!("r{r:03}/c{c:03}:{}", ((r as u64) << 32) | c as u64))
                    .collect()
            })
            .collect();
        Self {
            data: Arc::new(data),
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }
}
impl Default for FixtureTable {
    fn default() -> Self {
        Self::new()
    }
}
impl TableDelegate for FixtureTable {
    fn columns_count(&self, _: &App) -> usize {
        COLS
    }
    fn rows_count(&self, _: &App) -> usize {
        self.data.len()
    }
    fn column(&self, c: usize, _: &App) -> Column {
        Column::new(format!("c{c}"), format!("column_{c:03}")).width(px(160.))
    }
    fn render_td(
        &mut self,
        r: usize,
        c: usize,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        self.calls.fetch_add(1, Ordering::Relaxed);
        div()
            .id(format!("fixture-{r}-{c}"))
            .test_support()
            .child(self.data[r][c].clone())
    }
    fn cell_text(&self, r: usize, c: usize, _: &App) -> String {
        self.data[r][c].clone()
    }
}

pub struct Pilot {
    pub editor: Entity<EditorState>,
    pub table: Entity<TableState<FixtureTable>>,
    pub render_calls: Arc<AtomicUsize>,
    pub fixture: Arc<Vec<Vec<String>>>,
    pub checked: bool,
    pub clicks: usize,
}
impl Pilot {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let delegate = FixtureTable::new();
        let fixture = delegate.data.clone();
        let render_calls = delegate.calls.clone();
        Self {
            editor: cx.new(|cx| {
                EditorState::new(window, cx)
                    .language("sql")
                    .line_number(true)
                    .default_value(SQL)
            }),
            table: cx.new(|cx| TableState::new(delegate, window, cx)),
            render_calls,
            fixture,
            checked: false,
            clicks: 0,
        }
    }
}
impl Render for Pilot {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("pilot")
            .test_support()
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .overflow_hidden()
            .p_4()
            .gap_3()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .flex_shrink_0()
                    .child(
                        Button::new("run-fixture")
                            .primary()
                            .icon(IconName::Play)
                            .label("Run fixture")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.clicks += 1;
                                cx.notify();
                            })),
                    )
                    .child(
                        Checkbox::new("fixture-toggle")
                            .label("Fixture only — no database")
                            .checked(self.checked)
                            .on_change(cx.listener(|this, checked, _, cx| {
                                this.checked = *checked;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .id("editor-pane")
                    .test_support()
                    .h(px(200.))
                    .flex_shrink_0()
                    .overflow_hidden()
                    .child(Editor::new(&self.editor).h(px(200.))),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .child("100 rows × 512 columns · horizontal/vertical virtualization probe"),
            )
            .child(
                div()
                    .id("result-pane")
                    .test_support()
                    .flex_1()
                    .min_h_0()
                    .max_h(px(420.))
                    .overflow_hidden()
                    .child(DataTable::new(&self.table).with_size(px(22.))),
            )
    }
}

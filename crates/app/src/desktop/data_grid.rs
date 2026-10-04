//! Retained table grid with independent, bounded virtualization on both axes.
//! The page snapshot is shared; only visible cell strings are formatted.
use std::{collections::HashMap, sync::Arc};
use unicode_segmentation::UnicodeSegmentation;

use dalan_app::grid_viewport::{
    COLUMN_WIDTH, GridViewport, HEADER_HEIGHT, ROW_HEIGHT, SCROLLBAR_SIZE, scrollbar,
};
use dalan_drivers::{
    TablePage,
    mysql::{CellValue, SortDirection, TableSort},
};
use gpui::{
    Bounds, Context, Entity, FocusHandle, MouseButton, Pixels, Render, SharedString, Subscription,
    Window, canvas, div, prelude::*, px, rgb,
};

use super::{
    icons::{Icon, icon},
    source_model::SourceModel,
    theme::*,
};

type Selection = (Option<String>, Option<String>, Option<String>);

const DISPLAY_GRAPHEME_LIMIT: usize = 128;

fn display_preview(value: Option<&CellValue>) -> SharedString {
    let text = match value {
        None => "",
        Some(CellValue::Null) => "NULL",
        Some(
            CellValue::Text(text)
            | CellValue::Binary(text)
            | CellValue::Number(text)
            | CellValue::Temporal(text),
        ) => text,
    };
    let mut graphemes = text.graphemes(true);
    let mut preview: String = graphemes.by_ref().take(DISPLAY_GRAPHEME_LIMIT).collect();
    if graphemes.next().is_some() {
        preview.push('…');
    }
    preview.into()
}

#[derive(Clone, Copy)]
enum Axis {
    Horizontal,
    Vertical,
}

struct ThumbDrag {
    axis: Axis,
    pointer: f32,
    offset: f32,
}

pub(super) struct DataGrid {
    model: Entity<SourceModel>,
    page: Option<Arc<TablePage>>,
    sort: Option<TableSort>,
    stale: bool,
    selection: Selection,
    viewport: GridViewport,
    bounds: Bounds<Pixels>,
    focus: FocusHandle,
    drag: Option<ThumbDrag>,
    visible_text: HashMap<(usize, usize), SharedString>,
    visible_headers: HashMap<usize, SharedString>,
    #[cfg(test)]
    last_materialized_cells: usize,
    #[cfg(test)]
    last_formatted_cells: usize,
    _subscription: Subscription,
}

impl DataGrid {
    #[cfg(test)]
    pub(super) fn test_materialized_cells(&self) -> usize {
        self.last_materialized_cells
    }

    #[cfg(test)]
    pub(super) fn test_viewport(&self) -> GridViewport {
        self.viewport
    }

    pub(super) fn new(model: Entity<SourceModel>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&model, |this, model, cx| {
            let model = model.read(cx);
            let selection = Self::selection(model);
            let page_changed = match (&this.page, &model.page) {
                (Some(old), Some(new)) => !Arc::ptr_eq(old, new),
                (None, None) => false,
                _ => true,
            };
            // Busy/error notifications retain the same Arc, so scrolling a
            // stale snapshot is not disrupted while the replacement is fetched.
            if selection != this.selection || page_changed {
                this.viewport.reset();
                this.drag = None;
                this.visible_text.clear();
                this.visible_headers.clear();
            }
            this.selection = selection;
            this.page = model.page.as_ref().map(Arc::clone);
            this.sort = model.sort.clone();
            this.stale = model.busy || model.saving || model.error.is_some();
            let (rows, columns) = this.dimensions();
            this.viewport.clamp(rows, columns);
            cx.notify();
        });
        let snapshot = model.read(cx);
        Self {
            page: snapshot.page.as_ref().map(Arc::clone),
            sort: snapshot.sort.clone(),
            stale: snapshot.busy || snapshot.saving || snapshot.error.is_some(),
            selection: Self::selection(snapshot),
            model,
            viewport: GridViewport::default(),
            bounds: Bounds::default(),
            focus: cx.focus_handle().tab_stop(true).tab_index(30),
            drag: None,
            visible_text: HashMap::new(),
            visible_headers: HashMap::new(),
            #[cfg(test)]
            last_materialized_cells: 0,
            #[cfg(test)]
            last_formatted_cells: 0,
            _subscription: subscription,
        }
    }

    fn selection(model: &SourceModel) -> Selection {
        (
            model.selected_source.clone(),
            model.selected_database.clone(),
            model.selected_table.clone(),
        )
    }

    fn dimensions(&self) -> (usize, usize) {
        self.page
            .as_ref()
            .map_or((0, 0), |page| (page.rows.len(), page.columns.len()))
    }

    fn wheel(&mut self, event: &gpui::ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let delta = event.delta.pixel_delta(px(ROW_HEIGHT));
        let mut dx = f32::from(delta.x);
        let mut dy = f32::from(delta.y);
        if event.modifiers.shift && dx == 0. {
            dx = dy;
            dy = 0.;
        }
        let (rows, columns) = self.dimensions();
        self.viewport.scroll(-dx, -dy, rows, columns);
        cx.stop_propagation();
        cx.notify();
    }

    fn keyboard(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Header events bubble through the grid, but only its own focus scrolls.
        if !self.focus.is_focused(window) {
            return;
        }
        let (rows, columns) = self.dimensions();
        let all = event.keystroke.modifiers.control || event.keystroke.modifiers.platform;
        match event.keystroke.key.as_str() {
            "left" => self.viewport.x -= COLUMN_WIDTH,
            "right" => self.viewport.x += COLUMN_WIDTH,
            "up" => self.viewport.y -= ROW_HEIGHT,
            "down" => self.viewport.y += ROW_HEIGHT,
            "pageup" => self.viewport.y -= self.viewport.height,
            "pagedown" => self.viewport.y += self.viewport.height,
            "home" => {
                self.viewport.x = 0.;
                if all {
                    self.viewport.y = 0.;
                }
            }
            "end" => {
                self.viewport.x = self.viewport.max_x(columns);
                if all {
                    self.viewport.y = self.viewport.max_y(rows);
                }
            }
            _ => return,
        }
        self.viewport.clamp(rows, columns);
        cx.stop_propagation();
        cx.notify();
    }

    fn start_thumb(
        &mut self,
        axis: Axis,
        event: &gpui::MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus.focus(window);
        self.drag = Some(ThumbDrag {
            axis,
            pointer: match axis {
                Axis::Horizontal => event.position.x,
                Axis::Vertical => event.position.y,
            }
            .into(),
            offset: match axis {
                Axis::Horizontal => self.viewport.x,
                Axis::Vertical => self.viewport.y,
            },
        });
        cx.stop_propagation();
    }

    fn move_thumb(&mut self, event: &gpui::MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !event.dragging() {
            self.drag = None;
            return;
        }
        let Some(drag) = &self.drag else {
            return;
        };
        let (rows, columns) = self.dimensions();
        let (pointer, extent, content) = match drag.axis {
            Axis::Horizontal => (
                f32::from(event.position.x),
                self.viewport.width,
                columns as f32 * COLUMN_WIDTH,
            ),
            Axis::Vertical => (
                f32::from(event.position.y),
                self.viewport.height,
                rows as f32 * ROW_HEIGHT,
            ),
        };
        let (_, length) = scrollbar(0., extent, content);
        let travel = extent - length;
        if travel > 0. {
            let offset = drag.offset + (pointer - drag.pointer) * (content - extent) / travel;
            match drag.axis {
                Axis::Horizontal => self.viewport.x = offset,
                Axis::Vertical => self.viewport.y = offset,
            }
            self.viewport.clamp(rows, columns);
            cx.notify();
        }
    }

    fn track_click(
        &mut self,
        axis: Axis,
        event: &gpui::MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (rows, columns) = self.dimensions();
        let (position, extent, content) = match axis {
            Axis::Horizontal => (
                f32::from(event.position.x - self.bounds.origin.x),
                self.viewport.width,
                columns as f32 * COLUMN_WIDTH,
            ),
            Axis::Vertical => (
                f32::from(event.position.y - self.bounds.origin.y) - HEADER_HEIGHT,
                self.viewport.height,
                rows as f32 * ROW_HEIGHT,
            ),
        };
        let (_, length) = scrollbar(0., extent, content);
        if extent > length {
            let offset = ((position - length / 2.) / (extent - length)) * (content - extent);
            match axis {
                Axis::Horizontal => self.viewport.x = offset,
                Axis::Vertical => self.viewport.y = offset,
            }
            self.viewport.clamp(rows, columns);
        }
        self.start_thumb(axis, event, window, cx);
        cx.notify();
    }
}

impl Render for DataGrid {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (row_count, column_count) = self.dimensions();
        let columns = self.viewport.columns(column_count);
        let rows = self.viewport.rows(row_count);
        #[cfg(test)]
        {
            self.last_materialized_cells = rows.len() * columns.len();
            self.last_formatted_cells = 0;
        }
        self.visible_text
            .retain(|(row, column), _| rows.contains(row) && columns.contains(column));
        self.visible_headers
            .retain(|column, _| columns.contains(column));
        let view = cx.entity().downgrade();
        let previous_bounds = self.bounds;
        let measurement = canvas(
            move |bounds, _, cx| {
                // Updating after prepaint avoids invalidating layout while it is
                // executing. Stable bounds schedule no work during scrolling.
                if bounds == previous_bounds {
                    return;
                }
                cx.defer(move |cx| {
                    let _ = view.update(cx, |this, cx| {
                        let width = (f32::from(bounds.size.width) - SCROLLBAR_SIZE).max(0.);
                        let height =
                            (f32::from(bounds.size.height) - HEADER_HEIGHT - SCROLLBAR_SIZE)
                                .max(0.);
                        this.bounds = bounds;
                        if this.viewport.width != width || this.viewport.height != height {
                            let (rows, columns) = this.dimensions();
                            this.viewport.resize(width, height, rows, columns);
                            cx.notify();
                        }
                    });
                });
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();

        let mut header = div()
            .debug_selector(|| "grid-header".into())
            .absolute()
            .top_0()
            .left_0()
            .w(px(self.viewport.width))
            .h(px(HEADER_HEIGHT))
            .overflow_hidden()
            .bg(rgb(HEADER));
        let mut body = div()
            .debug_selector(|| "grid-body".into())
            .absolute()
            .top(px(HEADER_HEIGHT))
            .left_0()
            .w(px(self.viewport.width))
            .h(px(self.viewport.height))
            .overflow_hidden();
        if let Some(page) = &self.page {
            for column_index in columns.clone() {
                let left = column_index as f32 * COLUMN_WIDTH - self.viewport.x;
                let in_view = left < self.viewport.width && left + COLUMN_WIDTH > 0.0;
                let column = &page.columns[column_index];
                let direction = self
                    .sort
                    .as_ref()
                    .filter(|sort| sort.column == column.name)
                    .map(|sort| sort.direction);
                let name = column.name.clone();
                let keyboard_name = name.clone();
                let label = self
                    .visible_headers
                    .entry(column_index)
                    .or_insert_with(|| {
                        format!(
                            "{}{} · {}",
                            if column.is_primary_key { "PK " } else { "" },
                            column.name,
                            column.data_type
                        )
                        .into()
                    })
                    .clone();
                let debug_id = format!("sort-column-{column_index}");
                header = header.child(
                    div()
                        .id(gpui::SharedString::from(format!(
                            "sort-column-{column_index}"
                        )))
                        .debug_selector(move || debug_id.clone())
                        .tab_index(20)
                        .tab_stop(!self.stale && in_view)
                        .border_1()
                        .border_color(gpui::transparent_black())
                        .focus(|style| style.border_color(rgb(FOCUS)))
                        .absolute()
                        .left(px(column_index as f32 * COLUMN_WIDTH - self.viewport.x))
                        .top_0()
                        .w(px(COLUMN_WIDTH))
                        .h(px(HEADER_HEIGHT))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .overflow_hidden()
                        .text_color(rgb(MUTED))
                        .when(!self.stale, |header| {
                            header
                                .cursor_pointer()
                                .hover(|style| style.bg(rgb(HOVER)))
                                .on_key_down(cx.listener(
                                    move |this, event: &gpui::KeyDownEvent, _, cx| {
                                        if !this.stale
                                            && matches!(
                                                event.keystroke.key.as_str(),
                                                "enter" | "space"
                                            )
                                        {
                                            this.model.update(cx, |model, cx| {
                                                model.cycle_sort(keyboard_name.clone(), cx)
                                            });
                                            cx.stop_propagation();
                                        }
                                    },
                                ))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if !this.stale {
                                        this.model.update(cx, |model, cx| {
                                            model.cycle_sort(name.clone(), cx)
                                        });
                                    }
                                }))
                        })
                        .child(div().flex_1().min_w_0().text_ellipsis().child(label))
                        .when_some(direction, |header, direction| {
                            header.child(icon(
                                match direction {
                                    SortDirection::Ascending => Icon::SortAscending,
                                    SortDirection::Descending => Icon::SortDescending,
                                },
                                FOCUS,
                            ))
                        }),
                );
            }
            for row_index in rows {
                let row = &page.rows[row_index];
                let mut row_div = div()
                    .absolute()
                    .left_0()
                    .top(px(row_index as f32 * ROW_HEIGHT - self.viewport.y))
                    .w(px(self.viewport.width))
                    .h(px(GRID_ROW_HEIGHT))
                    .bg(rgb(if row_index % 2 == 0 {
                        PANEL
                    } else {
                        BACKGROUND
                    }));
                for column_index in columns.clone() {
                    let value = row.get(column_index);
                    let text = self
                        .visible_text
                        .entry((row_index, column_index))
                        .or_insert_with(|| {
                            #[cfg(test)]
                            {
                                self.last_formatted_cells += 1;
                            }
                            display_preview(value)
                        })
                        .clone();
                    let cell_id = format!("cell-{row_index}-{column_index}");
                    row_div = row_div.child(
                        div()
                            .debug_selector(move || cell_id.clone())
                            .absolute()
                            .left(px(column_index as f32 * COLUMN_WIDTH - self.viewport.x))
                            .top_0()
                            .w(px(COLUMN_WIDTH))
                            .h(px(ROW_HEIGHT))
                            .px(px(8.))
                            .flex()
                            .items_center()
                            .border_r_1()
                            .border_color(rgb(BORDER))
                            .text_color(rgb(if matches!(value, Some(CellValue::Null)) {
                                MUTED
                            } else {
                                TEXT
                            }))
                            .child(div().flex_1().min_w_0().text_ellipsis().child(text)),
                    );
                }
                body = body.child(row_div);
            }
            if page.rows.is_empty() {
                body = body.child(
                    div()
                        .p(px(12.))
                        .text_color(rgb(MUTED))
                        .child("No matching rows"),
                );
            }
        }
        let (horizontal_position, horizontal_length) = scrollbar(
            self.viewport.x,
            self.viewport.width,
            column_count as f32 * COLUMN_WIDTH,
        );
        let (vertical_position, vertical_length) = scrollbar(
            self.viewport.y,
            self.viewport.height,
            row_count as f32 * ROW_HEIGHT,
        );
        div()
            .id("table-grid-scroll")
            .debug_selector(|| "table-grid-scroll".into())
            .relative()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .bg(rgb(PANEL))
            .text_size(px(12.))
            .track_focus(&self.focus)
            .key_context("DataGrid")
            // track_focus installs GPUI's default mouse focus behavior. An
            // explicitly focusing parent listener would steal focus from headers.
            .on_key_down(cx.listener(Self::keyboard))
            .on_scroll_wheel(cx.listener(Self::wheel))
            .on_mouse_move(cx.listener(Self::move_thumb))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.drag = None),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.drag = None),
            )
            .child(measurement)
            .child(header)
            .child(body)
            .when(self.viewport.max_x(column_count) > 0., |root| {
                root.child(
                    div()
                        .id("grid-horizontal-track")
                        .debug_selector(|| "grid-horizontal-track".into())
                        .absolute()
                        .left_0()
                        .bottom_0()
                        .w(px(self.viewport.width))
                        .h(px(SCROLLBAR_SIZE))
                        .bg(rgb(HEADER))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, event, window, cx| {
                                this.track_click(Axis::Horizontal, event, window, cx)
                            }),
                        )
                        .child(
                            div()
                                .id("grid-horizontal-thumb")
                                .debug_selector(|| "grid-horizontal-thumb".into())
                                .absolute()
                                .left(px(horizontal_position))
                                .top(px(2.))
                                .w(px(horizontal_length))
                                .h(px(6.))
                                .rounded(px(3.))
                                .bg(rgb(INPUT_BORDER))
                                .cursor_pointer()
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, event, window, cx| {
                                        this.start_thumb(Axis::Horizontal, event, window, cx)
                                    }),
                                ),
                        ),
                )
            })
            .when(self.viewport.max_y(row_count) > 0., |root| {
                root.child(
                    div()
                        .id("grid-vertical-track")
                        .debug_selector(|| "grid-vertical-track".into())
                        .absolute()
                        .right_0()
                        .top(px(HEADER_HEIGHT))
                        .w(px(SCROLLBAR_SIZE))
                        .h(px(self.viewport.height))
                        .bg(rgb(HEADER))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, event, window, cx| {
                                this.track_click(Axis::Vertical, event, window, cx)
                            }),
                        )
                        .child(
                            div()
                                .id("grid-vertical-thumb")
                                .debug_selector(|| "grid-vertical-thumb".into())
                                .absolute()
                                .top(px(vertical_position))
                                .left(px(2.))
                                .w(px(6.))
                                .h(px(vertical_length))
                                .rounded(px(3.))
                                .bg(rgb(INPUT_BORDER))
                                .cursor_pointer()
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, event, window, cx| {
                                        this.start_thumb(Axis::Vertical, event, window, cx)
                                    }),
                                ),
                        ),
                )
            })
    }
}

#[cfg(all(test, feature = "ui-tests"))]
mod tests {
    use super::*;
    use dalan_drivers::{SourceProfile, mysql::ColumnInfo};
    use gpui::{
        Modifiers, ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext, point,
    };

    struct GridHarness(Entity<DataGrid>);

    impl Render for GridHarness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().flex().flex_col().child(self.0.clone())
        }
    }

    fn fixture(
        cx: &mut TestAppContext,
    ) -> (
        Entity<SourceModel>,
        Entity<DataGrid>,
        &mut VisualTestContext,
    ) {
        let model = cx.new(|_| {
            let mut model = SourceModel::for_tests(vec![SourceProfile::default()]);
            model.selected_database = Some("inventory".into());
            model.selected_table = Some("wide".into());
            model.page = Some(Arc::new(TablePage {
                columns: (0..512)
                    .map(|i| ColumnInfo {
                        name: format!("column_{i}"),
                        data_type: "VARCHAR".into(),
                        nullable: true,
                        is_primary_key: i == 0,
                    })
                    .collect(),
                rows: (0..200)
                    .map(|r| {
                        (0..512)
                            .map(|c| CellValue::Text(format!("{r}:{c}")))
                            .collect()
                    })
                    .collect(),
                has_more: false,
                next_offset: None,
                offset: 0,
                truncated: false,
            }));
            model
        });
        let grid = cx.new(|cx| DataGrid::new(model.clone(), cx));
        let (_, cx) = cx.add_window_view(|_, _| GridHarness(grid.clone()));
        cx.simulate_resize(gpui::size(px(730.), px(258.)));
        cx.refresh().unwrap();
        cx.run_until_parked();
        (model, grid, cx)
    }

    fn click(cx: &mut VisualTestContext, selector: &'static str) {
        let bounds = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("missing {selector}"));
        cx.simulate_click(bounds.center(), Modifiers::default());
        cx.run_until_parked();
    }

    fn wheel(cx: &mut VisualTestContext, dx: f32, dy: f32, shift: bool) {
        let position = cx.debug_bounds("grid-body").unwrap().center();
        cx.simulate_event(ScrollWheelEvent {
            position,
            delta: ScrollDelta::Pixels(point(px(dx), px(dy))),
            modifiers: Modifiers {
                shift,
                ..Modifiers::default()
            },
            ..Default::default()
        });
        cx.run_until_parked();
    }

    #[gpui::test]
    fn wide_grid_virtualizes_both_axes_and_keeps_header_aligned(cx: &mut TestAppContext) {
        let (_, grid, cx) = fixture(cx);
        assert!(cx.debug_bounds("cell-0-0").is_some());
        assert!(cx.debug_bounds("cell-100-300").is_none());
        wheel(cx, -200. * COLUMN_WIDTH, -50. * ROW_HEIGHT, false);
        grid.read_with(cx, |grid, _| {
            assert_eq!(grid.viewport.columns(512), 198..206);
            assert_eq!(grid.viewport.rows(200), 48..62);
            assert!(grid.viewport.columns(512).len() * grid.viewport.rows(200).len() <= 112);
        });
        assert!(grid.read_with(cx, |g, _| g.viewport.rows(200).start > 0));
        assert_eq!(grid.read_with(cx, |g, _| g.last_materialized_cells), 112);
        let header = cx.debug_bounds("sort-column-200").unwrap();
        let cell = cx.debug_bounds("cell-50-200").unwrap();
        assert_eq!(header.origin.x, cell.origin.x);
        assert_eq!(header.size.width, cell.size.width);
        wheel(cx, 0., -COLUMN_WIDTH, true);
        assert_eq!(
            grid.read_with(cx, |g, _| (g.viewport.x, g.viewport.y)),
            (201. * COLUMN_WIDTH, 50. * ROW_HEIGHT)
        );
        wheel(cx, -f32::MAX, -f32::MAX, false);
        assert!(cx.debug_bounds("cell-199-511").is_some());
        assert!(cx.debug_bounds("sort-column-511").is_some());
    }

    #[gpui::test]
    fn keyboard_scrolls_only_grid_focus_and_header_click_retains_focus(cx: &mut TestAppContext) {
        let (model, grid, cx) = fixture(cx);
        click(cx, "cell-0-0");
        cx.simulate_keystrokes("right down pagedown");
        cx.run_until_parked();
        let viewport = grid.read_with(cx, |g, _| g.viewport);
        assert_eq!(viewport.x, COLUMN_WIDTH);
        assert_eq!(viewport.y, ROW_HEIGHT + viewport.height);
        click(cx, "sort-column-1");
        assert!(model.read_with(cx, |m, _| m.sort.is_some()));
        cx.simulate_keystrokes("left right up down home end");
        cx.run_until_parked();
        assert_eq!(grid.read_with(cx, |g, _| g.viewport), viewport);
        cx.simulate_keystrokes("space");
        cx.run_until_parked();
        assert_eq!(
            model.read_with(cx, |m, _| m.sort.as_ref().unwrap().direction),
            SortDirection::Descending
        );
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert!(model.read_with(cx, |m, _| m.sort.is_none()));
        click(cx, "grid-body");
        cx.simulate_keystrokes("ctrl-end");
        cx.run_until_parked();
        grid.read_with(cx, |g, _| {
            assert_eq!(g.viewport.x, g.viewport.max_x(512));
            assert_eq!(g.viewport.y, g.viewport.max_y(200));
        });
        cx.simulate_keystrokes("ctrl-home");
        cx.run_until_parked();
        assert_eq!(
            grid.read_with(cx, |g, _| (g.viewport.x, g.viewport.y)),
            (0., 0.)
        );
    }

    #[gpui::test]
    fn snapshot_notifications_preserve_offsets_but_page_and_selection_reset(
        cx: &mut TestAppContext,
    ) {
        let (model, grid, cx) = fixture(cx);
        wheel(cx, -200. * COLUMN_WIDTH, -50. * ROW_HEIGHT, false);
        let snapshot = model.read_with(cx, |m, _| m.page.clone().unwrap());
        for state in 0..3 {
            model.update(cx, |m, cx| {
                m.busy = state == 0;
                m.saving = state == 1;
                m.error = (state == 2).then(|| "refresh failed".into());
                cx.notify();
            });
            cx.run_until_parked();
            grid.read_with(cx, |g, _| {
                assert!(Arc::ptr_eq(g.page.as_ref().unwrap(), &snapshot));
                assert_eq!(
                    (g.viewport.x, g.viewport.y),
                    (200. * COLUMN_WIDTH, 50. * ROW_HEIGHT)
                );
            });
            click(cx, "sort-column-200");
            cx.simulate_keystrokes("enter space");
            cx.run_until_parked();
            assert!(model.read_with(cx, |m, _| m.sort.is_none()));
        }
        model.update(cx, |m, cx| {
            m.page = Some(Arc::new((*snapshot).clone()));
            cx.notify();
        });
        cx.run_until_parked();
        assert_eq!(
            grid.read_with(cx, |g, _| (g.viewport.x, g.viewport.y)),
            (0., 0.)
        );
        wheel(cx, -COLUMN_WIDTH, -ROW_HEIGHT, false);
        model.update(cx, |m, cx| {
            m.selected_table = Some("other".into());
            cx.notify();
        });
        cx.run_until_parked();
        assert_eq!(
            grid.read_with(cx, |g, _| (g.viewport.x, g.viewport.y)),
            (0., 0.)
        );
    }

    #[gpui::test]
    fn scrollbar_track_drag_and_resize_reach_edges(cx: &mut TestAppContext) {
        let (_, grid, cx) = fixture(cx);
        let thumb = cx.debug_bounds("grid-horizontal-thumb").unwrap();
        let track = cx.debug_bounds("grid-horizontal-track").unwrap();
        cx.simulate_mouse_down(thumb.center(), MouseButton::Left, Modifiers::default());
        let end = point(track.right() - px(1.), thumb.center().y);
        cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::default());
        cx.run_until_parked();
        grid.read_with(cx, |g, _| {
            assert_eq!(g.viewport.x, g.viewport.max_x(512));
            assert!(g.drag.is_none());
        });
        let track = cx.debug_bounds("grid-vertical-track").unwrap();
        cx.simulate_click(
            point(track.center().x, track.bottom() - px(1.)),
            Modifiers::default(),
        );
        cx.run_until_parked();
        grid.read_with(cx, |g, _| assert_eq!(g.viewport.y, g.viewport.max_y(200)));
        cx.simulate_resize(gpui::size(px(1000.), px(500.)));
        cx.refresh().unwrap();
        cx.run_until_parked();
        grid.read_with(cx, |g, _| {
            assert_eq!(g.viewport.x, g.viewport.max_x(512));
            assert_eq!(g.viewport.y, g.viewport.max_y(200));
        });
    }
    #[gpui::test]
    fn tab_traversal_reaches_headers_and_empty_pages_clear_virtual_cells(cx: &mut TestAppContext) {
        let (model, grid, cx) = fixture(cx);
        cx.update(|window, _| window.focus_next());
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(
            model.read_with(cx, |m, _| m.sort.as_ref().unwrap().column.clone()),
            "column_0"
        );
        let mut page = model.read_with(cx, |m, _| (**m.page.as_ref().unwrap()).clone());
        page.rows.clear();
        model.update(cx, |m, cx| {
            m.page = Some(Arc::new(page));
            cx.notify();
        });
        cx.run_until_parked();
        assert_eq!(grid.read_with(cx, |g, _| g.last_materialized_cells), 0);
        assert!(cx.debug_bounds("sort-column-0").is_some());
        assert_eq!(grid.read_with(cx, |g, _| g.viewport.max_y(0)), 0.0);
        model.update(cx, |m, cx| {
            m.page = None;
            cx.notify();
        });
        cx.run_until_parked();
        assert_eq!(grid.read_with(cx, |g, _| g.dimensions()), (0, 0));
        assert_eq!(grid.read_with(cx, |g, _| g.last_materialized_cells), 0);
        assert!(grid.read_with(cx, |g, _| g.visible_headers.is_empty()
            && g.visible_text.is_empty()));
    }
    #[gpui::test]
    fn small_scrolls_reuse_visible_text_and_cache_stays_viewport_bounded(cx: &mut TestAppContext) {
        let (model, grid, cx) = fixture(cx);
        wheel(cx, -90.0, -11.0, false);
        let before = grid.read_with(cx, |grid, _| grid.viewport);
        wheel(cx, -1.0, -1.0, false);
        grid.read_with(cx, |grid, _| {
            assert_eq!(before.columns(512), grid.viewport.columns(512));
            assert_eq!(before.rows(200), grid.viewport.rows(200));
            assert_eq!(grid.last_formatted_cells, 0);
            assert_eq!(grid.visible_text.len(), grid.last_materialized_cells);
            assert!(grid.visible_text.len() <= 135);
            assert_eq!(grid.visible_headers.len(), grid.viewport.columns(512).len());
        });
        wheel(cx, -200.0 * COLUMN_WIDTH, -50.0 * ROW_HEIGHT, false);
        grid.read_with(cx, |grid, _| {
            assert!(
                grid.visible_text
                    .keys()
                    .all(|(row, column)| grid.viewport.rows(200).contains(row)
                        && grid.viewport.columns(512).contains(column))
            );
            assert_eq!(
                grid.visible_text.len(),
                grid.viewport.rows(200).len() * grid.viewport.columns(512).len()
            );
            assert!(grid.visible_text.len() <= 135);
        });
        let mut replacement =
            model.read_with(cx, |model, _| (**model.page.as_ref().unwrap()).clone());
        replacement.rows[0][0] = CellValue::Text("changed".into());
        model.update(cx, |model, cx| {
            model.page = Some(Arc::new(replacement));
            cx.notify();
        });
        cx.run_until_parked();
        assert_eq!(
            grid.read_with(cx, |grid, _| grid.visible_text[&(0, 0)].to_string()),
            "changed"
        );
    }
    #[gpui::test]
    fn offscreen_headers_are_not_tab_stops_and_hover_cancels_missed_release(
        cx: &mut TestAppContext,
    ) {
        let (model, grid, cx) = fixture(cx);
        wheel(cx, -200.0 * COLUMN_WIDTH, -50.0 * ROW_HEIGHT, false);
        cx.update(|window, _| window.blur());
        cx.update(|window, _| window.focus_next());
        cx.simulate_keystrokes("enter");
        assert_eq!(
            model.read_with(cx, |model, _| model.sort.as_ref().unwrap().column.clone()),
            "column_200"
        );
        let thumb = cx.debug_bounds("grid-horizontal-thumb").unwrap().center();
        cx.simulate_mouse_down(thumb, MouseButton::Left, Modifiers::default());
        assert!(grid.read_with(cx, |grid, _| grid.drag.is_some()));
        let before = grid.read_with(cx, |grid, _| grid.viewport);
        cx.simulate_event(gpui::MouseMoveEvent {
            position: thumb + point(px(80.0), px(0.0)),
            pressed_button: None,
            modifiers: Modifiers::default(),
        });
        cx.run_until_parked();
        assert!(grid.read_with(cx, |grid, _| grid.drag.is_none()));
        assert_eq!(grid.read_with(cx, |grid, _| grid.viewport), before);
    }
    #[test]
    fn display_preview_caps_shaping_without_changing_typed_values() {
        let text = "👨‍👩‍👧‍👦".repeat(256);
        let value = CellValue::Text(text.clone());
        let preview = display_preview(Some(&value));
        assert_eq!(preview.graphemes(true).count(), DISPLAY_GRAPHEME_LIMIT + 1);
        assert!(preview.ends_with('…'));
        assert_eq!(value.display(), text);
        assert_eq!(display_preview(Some(&CellValue::Null)).as_ref(), "NULL");
        assert_eq!(
            display_preview(Some(&CellValue::Number("18446744073709551615".into()))).as_ref(),
            "18446744073709551615"
        );
    }
}

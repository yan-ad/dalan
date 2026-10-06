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
    Bounds, ContentMask, Context, Entity, FocusHandle, Font, Hsla, MouseButton, Pixels, Render,
    ShapedLine, SharedString, Subscription, TextRun, Window, canvas, div, fill, point, prelude::*,
    px, size,
};

use super::{source_model::SourceModel, theme::*};

use gpui::component::{
    ActiveTheme, Disableable, Icon as KitIcon, Selectable, Sizable,
    button::{Button as KitButton, ButtonVariants},
    menu::{ContextMenuExt, PopupMenuItem},
};

/// Capture Kit's keyed button handle during rendering, when its element
/// namespace is active. Kit suppresses default mouse focus, so explicitly
/// focus this handle instead of allowing the ancestor grid to receive keys.
#[derive(IntoElement)]
struct GridHeaderButton {
    id: SharedString,
    enabled: bool,
    button: KitButton,
}

impl RenderOnce for GridHeaderButton {
    fn render(self, window: &mut Window, cx: &mut gpui::App) -> impl IntoElement {
        let focus = window
            .use_keyed_state(self.id, cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        self.button
            .when(self.enabled, |button| {
                button.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    focus.focus(window, cx);
                })
            })
            .render(window, cx)
    }
}

type Selection = (Option<String>, Option<String>, Option<String>);

const DISPLAY_GRAPHEME_LIMIT: usize = 128;
const DISPLAY_BYTE_LIMIT: usize = 4096;
const ROW_GUTTER_WIDTH: f32 = 44.;
const CELL_PADDING: f32 = 8.;
const CELL_FONT_SIZE: f32 = 12.;

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
    single_line_preview(text).into()
}

fn display_control(ch: char) -> bool {
    ch.is_control() || matches!(ch, '\u{2028}' | '\u{2029}')
}

/// Only the rendered preview is escaped. SQL values, exports and the page's
/// truncation metadata remain untouched. GPUI's shape_line requires no LF.
/// The byte cap also bounds a single pathological combining-mark grapheme.
fn single_line_preview(text: &str) -> String {
    let mut preview = String::new();
    for (index, grapheme) in text.graphemes(true).enumerate() {
        if index == DISPLAY_GRAPHEME_LIMIT {
            preview.push('…');
            break;
        }
        if grapheme == "\r\n" {
            if preview.len() + '↵'.len_utf8() > DISPLAY_BYTE_LIMIT - '…'.len_utf8() {
                preview.push('…');
                break;
            }
            preview.push('↵');
            continue;
        }
        let grapheme_start = preview.len();
        for ch in grapheme.chars() {
            match ch {
                '\n' | '\r' | '\u{2028}' | '\u{2029}' => preview.push('↵'),
                '\t' => preview.push('⇥'),
                ch if ch.is_control() => {
                    use std::fmt::Write as _;
                    let _ = write!(preview, "\\u{{{:x}}}", ch as u32);
                }
                ch => preview.push(ch),
            }
            if preview.len() > DISPLAY_BYTE_LIMIT - '…'.len_utf8() {
                // Do not display a partial extended grapheme.
                preview.truncate(grapheme_start);
                preview.push('…');
                return preview;
            }
        }
    }
    preview
}

/// Small, stable type glyphs keep column names readable without embedding
/// potentially long SQL type declarations in each header.
fn column_type_icon(data_type: &str) -> &'static str {
    let kind = data_type.to_ascii_lowercase();
    if kind.contains("json") {
        "icons/braces.svg"
    } else if kind.contains("bool") || kind == "bit" {
        "icons/check.svg"
    } else if ["int", "decimal", "numeric", "float", "double", "real"]
        .iter()
        .any(|value| kind.contains(value))
    {
        "icons/hash.svg"
    } else if ["date", "time", "year"]
        .iter()
        .any(|value| kind.contains(value))
    {
        "icons/calendar-clock.svg"
    } else if ["binary", "blob", "geometry"]
        .iter()
        .any(|value| kind.contains(value))
    {
        "icons/binary.svg"
    } else {
        "icons/text-initial.svg"
    }
}

fn shape_cell(
    text: SharedString,
    font: &Font,
    color: Hsla,
    window: &mut Window,
    truncate: bool,
) -> ShapedLine {
    // Defend the single-line API boundary even if a future caller bypasses
    // display_preview. Keep the cached SharedString for ordinary cell text.
    let text = if text.len() > DISPLAY_BYTE_LIMIT || text.chars().any(display_control) {
        single_line_preview(&text).into()
    } else {
        text
    };
    let shape = |text: SharedString| {
        let run = TextRun {
            len: text.len(),
            font: font.clone(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        window
            .text_system()
            .shape_line(text, px(CELL_FONT_SIZE), &[run], None)
    };
    let line = shape(text.clone());
    let width = px(COLUMN_WIDTH - 2. * CELL_PADDING);
    if !truncate || line.width <= width {
        return line;
    }
    let ellipsis = shape("…".into());
    let available = width - ellipsis.width;
    let end = text
        .grapheme_indices(true)
        .map(|(index, _)| index)
        .take_while(|&index| line.x_for_index(index) <= available)
        .last()
        .unwrap_or(0);
    shape(format!("{}…", &text[..end]).into())
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
    active_cell: Option<(usize, usize)>,
    edit_input: Entity<super::input::TextInput>,
    editing: bool,
    edit_target: Option<(usize, usize)>,
    write_confirm: bool,
    write_alert_open: bool,
    edit_rows: usize,
    edit_revision: u64,
    write_feedback: Option<String>,
    stale: bool,
    selection: Selection,
    viewport: GridViewport,
    bounds: Bounds<Pixels>,
    focus: FocusHandle,
    drag: Option<ThumbDrag>,
    visible_text: HashMap<(usize, usize), SharedString>,
    visible_headers: HashMap<usize, SharedString>,
    visible_lines: HashMap<(usize, usize), ShapedLine>,
    row_numbers: HashMap<usize, ShapedLine>,
    line_font: Option<Font>,
    line_colors: Option<(Hsla, Hsla)>,
    #[cfg(test)]
    last_materialized_cells: usize,
    #[cfg(test)]
    last_formatted_cells: usize,
    #[cfg(test)]
    last_painted_cells: usize,
    #[cfg(test)]
    last_shaped_cells: usize,
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
                (Some(a), Some(b)) => !Arc::ptr_eq(a, b),
                (None, None) => false,
                _ => true,
            };
            let edits_changed = this.edit_revision != model.table_edit_revision;
            let stale = model.busy
                || model.saving
                || model.error.is_some()
                || model.query_console
                || model.has_table_changes()
                || model.write_busy
                || matches!(
                    model.selected_engine(),
                    dalan_drivers::DbEngine::MongoDb
                        | dalan_drivers::DbEngine::Redis
                        | dalan_drivers::DbEngine::Jdbc
                );
            if selection == this.selection
                && !page_changed
                && !edits_changed
                && stale == this.stale
                && this.sort == model.sort
                && this.write_feedback == model.write_feedback
            {
                return;
            }
            this.write_confirm = false;
            this.edit_revision = model.table_edit_revision;
            this.write_feedback = model.write_feedback.clone();
            // Busy/error notifications retain the same Arc, so scrolling a
            // stale snapshot is not disrupted while the replacement is fetched.
            if selection != this.selection || page_changed {
                this.viewport.reset();
                this.active_cell = None;
                this.editing = false;
                this.edit_target = None;
                this.write_confirm = false;
                this.drag = None;
                this.visible_text.clear();
                this.visible_headers.clear();
                this.visible_lines.clear();
                this.row_numbers.clear();
            }
            this.selection = selection;
            this.page = model.page.as_ref().map(Arc::clone);
            if edits_changed {
                this.visible_text.clear();
                this.visible_lines.clear();
            }
            this.edit_rows = model
                .table_edits
                .as_ref()
                .map(|e| e.rows_count())
                .unwrap_or(0);
            this.sort = model.sort.clone();
            this.stale = model.busy
                || model.saving
                || model.error.is_some()
                || model.query_console
                || model.has_table_changes()
                || model.write_busy
                || matches!(
                    model.selected_engine(),
                    dalan_drivers::DbEngine::MongoDb
                        | dalan_drivers::DbEngine::Redis
                        | dalan_drivers::DbEngine::Jdbc
                );
            let (rows, columns) = this.dimensions();
            this.viewport.clamp(rows, columns);
            cx.notify();
        });
        let edit_input = cx.new(|cx| super::input::TextInput::new("", "Cell value", false, cx));
        let snapshot = model.read(cx);
        Self {
            page: snapshot.page.as_ref().map(Arc::clone),
            sort: snapshot.sort.clone(),
            active_cell: None,
            edit_input,
            editing: false,
            edit_target: None,
            write_confirm: false,
            write_alert_open: false,
            edit_rows: 0,
            edit_revision: snapshot.table_edit_revision,
            write_feedback: snapshot.write_feedback.clone(),
            stale: snapshot.busy
                || snapshot.saving
                || snapshot.error.is_some()
                || snapshot.query_console
                || snapshot.has_table_changes()
                || snapshot.write_busy
                || matches!(
                    snapshot.selected_engine(),
                    dalan_drivers::DbEngine::MongoDb
                        | dalan_drivers::DbEngine::Redis
                        | dalan_drivers::DbEngine::Jdbc
                ),
            selection: Self::selection(snapshot),
            model,
            viewport: GridViewport::default(),
            bounds: Bounds::default(),
            focus: cx.focus_handle().tab_stop(true).tab_index(30),
            drag: None,
            visible_text: HashMap::new(),
            visible_headers: HashMap::new(),
            visible_lines: HashMap::new(),
            row_numbers: HashMap::new(),
            line_font: None,
            line_colors: None,
            #[cfg(test)]
            last_materialized_cells: 0,
            #[cfg(test)]
            last_formatted_cells: 0,
            #[cfg(test)]
            last_painted_cells: 0,
            #[cfg(test)]
            last_shaped_cells: 0,
            _subscription: subscription,
        }
    }

    fn hit_cell(&self, position: gpui::Point<Pixels>) -> Option<(usize, usize)> {
        let x = f32::from(position.x - self.bounds.origin.x) - ROW_GUTTER_WIDTH;
        let y = f32::from(position.y - self.bounds.origin.y) - HEADER_HEIGHT;
        if x < 0. || y < 0. || x >= self.viewport.width || y >= self.viewport.height {
            return None;
        }
        let row = ((y + self.viewport.y) / ROW_HEIGHT) as usize;
        let col = ((x + self.viewport.x) / COLUMN_WIDTH) as usize;
        let (rows, cols) = self.dimensions();
        (row < rows && col < cols).then_some((row, col))
    }
    fn cell_value(&self, row: usize, col: usize, cx: &gpui::App) -> Option<CellValue> {
        self.model
            .read(cx)
            .table_edits
            .as_ref()
            .and_then(|e| e.displayed(row, col))
            .cloned()
            .or_else(|| {
                self.page
                    .as_ref()
                    .and_then(|p| p.rows.get(row))
                    .and_then(|r| r.get(col))
                    .cloned()
            })
    }
    fn begin_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((row, col)) = self.active_cell else {
            return;
        };
        if !self.model.read(cx).table_editable() {
            return;
        }
        if self
            .page
            .as_ref()
            .and_then(|p| p.columns.get(col))
            .is_some_and(|c| c.is_primary_key)
            && !self
                .model
                .read(cx)
                .table_edits
                .as_ref()
                .is_some_and(|e| e.is_insert(row))
        {
            return;
        }
        if self
            .cell_value(row, col, cx)
            .is_some_and(|v| v.display().contains(['\n', '\r', '\t']))
        {
            self.model.update(cx,|m,cx|{m.write_feedback=Some("Multiline/tab-containing cell editing requires the future full-value editor; cell unchanged.".into());cx.notify();});
            return;
        }
        let text = self
            .cell_value(row, col, cx)
            .map(|v| {
                if matches!(v, CellValue::Null) {
                    String::new()
                } else {
                    v.display()
                }
            })
            .unwrap_or_default();
        self.edit_input.update(cx, |i, cx| i.set_value(text, cx));
        self.edit_target = Some((row, col));
        self.editing = true;
        self.edit_input.read(cx).focus_handle().focus(window, cx);
        cx.notify();
    }
    fn grid_action(&mut self, action: &str, window: &mut Window, cx: &mut Context<Self>) {
        let (row, col) = self.active_cell.unwrap_or((0, 0));
        match action {
            "edit" => self.begin_edit(window, cx),
            "copy" => {
                if let Some(value) = self.cell_value(row, col, cx) {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(value.display()));
                }
            }
            "null" => self
                .model
                .update(cx, |m, cx| m.stage_cell(row, col, None, cx)),
            "add" | "clone" | "delete" | "restore" => {
                self.model.update(cx, |m, cx| m.stage_row(row, action, cx))
            }
            "discard" => {
                self.open_write_alert(false, window, cx);
            }
            "apply"
                if !self.write_alert_open
                    && self.model.read(cx).table_editable()
                    && self.model.read(cx).has_table_changes() =>
            {
                self.write_confirm = true;
            }
            _ => {}
        }
        cx.notify();
    }

    /// Snapshot the exact staging target. A stale alert can never apply/discard
    /// edits from a replacement page, tab model, or revision.
    fn open_write_alert(&mut self, apply: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.write_alert_open {
            return;
        }
        let model = self.model.read(cx);
        if model.write_busy || !model.has_table_changes() || (apply && !model.table_editable()) {
            self.write_confirm = false;
            return;
        }
        let Some(page) = model.page.clone() else {
            return;
        };
        let selection = Self::selection(model);
        let revision = model.table_edit_revision;
        let model_id = self.model.entity_id();
        let count = model
            .table_edits
            .as_ref()
            .map(|e| e.operation_count())
            .unwrap_or(0);
        let source = model
            .profiles
            .iter()
            .find(|p| Some(&p.id) == model.selected_source.as_ref())
            .map(|p| p.name.as_str())
            .unwrap_or("source");
        let description = format!(
            "{} {count} staged operation(s) {} {} / {} / {}. {}",
            if apply { "Apply" } else { "Discard" },
            if apply { "to" } else { "for" },
            source,
            selection.1.as_deref().unwrap_or("database"),
            selection.2.as_deref().unwrap_or("table"),
            if apply {
                "Updates, inserts and deletes will modify the database."
            } else {
                "Local staged changes will be lost; the database is unchanged."
            }
        );
        let weak = cx.entity().downgrade();
        let cancel = weak.clone();
        self.write_alert_open = super::confirm::open(
            window,
            cx,
            super::confirm::ConfirmAlert {
                title: if apply {
                    "Apply staged changes?"
                } else {
                    "Discard staged changes?"
                }
                .into(),
                description: description.into(),
                confirm_id: if apply {
                    "grid-confirm-apply"
                } else {
                    "grid-confirm-discard"
                },
                confirm_label: if apply {
                    "Apply to database"
                } else {
                    "Discard changes"
                },
                cancel_id: if apply {
                    "grid-keep-staging"
                } else {
                    "grid-keep-edits"
                },
                cancel_label: "Keep editing",
            },
            move |_, cx| {
                weak.update(cx, |this, cx| {
                    let model = this.model.read(cx);
                    let valid = this.model.entity_id() == model_id
                        && !model.write_busy
                        && model.has_table_changes()
                        && model.table_edit_revision == revision
                        && Self::selection(model) == selection
                        && model.page.as_ref().is_some_and(|p| Arc::ptr_eq(p, &page))
                        && model.table_edits.as_ref().is_some_and(|e| {
                            Arc::ptr_eq(e.base(), &page)
                                && e.request().map_or(!apply, |r| {
                                    Some(&r.database) == selection.1.as_ref()
                                        && Some(&r.table) == selection.2.as_ref()
                                })
                        })
                        && (!apply || (this.write_confirm && model.table_editable()));
                    this.write_confirm = false;
                    if !valid {
                        cx.notify();
                        return false;
                    }
                    this.write_alert_open = false;
                    this.model.update(cx, |m, cx| {
                        if apply {
                            m.apply_table_changes(cx);
                        } else {
                            m.discard_table_changes(cx);
                        }
                    });
                    if !apply {
                        this.editing = false;
                        this.edit_target = None;
                    }
                    cx.notify();
                    true
                })
                .unwrap_or(true)
            },
            move |_, cx| {
                let _ = cancel.update(cx, |this, cx| {
                    this.write_confirm = false;
                    this.write_alert_open = false;
                    cx.notify();
                });
                true
            },
        );
    }

    fn selection(model: &SourceModel) -> Selection {
        (
            model.selected_source.clone(),
            model.selected_database.clone(),
            model.selected_table.clone(),
        )
    }

    fn dimensions(&self) -> (usize, usize) {
        self.page.as_ref().map_or((0, 0), |page| {
            (page.rows.len().max(self.edit_rows), page.columns.len())
        })
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
        self.focus.focus(window, cx);
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
                f32::from(event.position.x - self.bounds.origin.x) - ROW_GUTTER_WIDTH,
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = colors(cx);
        let (row_count, column_count) = self.dimensions();
        let columns = self.viewport.columns(column_count);
        let rows = self.viewport.rows(row_count);
        #[cfg(test)]
        {
            self.last_materialized_cells = rows.len() * columns.len();
            self.last_formatted_cells = 0;
            self.last_painted_cells = 0;
            self.last_shaped_cells = 0;
        }
        self.visible_text
            .retain(|(row, column), _| rows.contains(row) && columns.contains(column));
        self.visible_headers
            .retain(|column, _| columns.contains(column));
        self.visible_lines
            .retain(|(row, column), _| rows.contains(row) && columns.contains(column));
        self.row_numbers.retain(|row, _| rows.contains(row));
        let mut font = window.text_style().font();
        font.family = "Menlo".into();
        if self.line_font.as_ref() != Some(&font)
            || self.line_colors != Some((palette.text, palette.muted))
        {
            self.visible_lines.clear();
            self.row_numbers.clear();
            self.line_font = Some(font.clone());
            self.line_colors = Some((palette.text, palette.muted));
        }
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
                        let width =
                            (f32::from(bounds.size.width) - SCROLLBAR_SIZE - ROW_GUTTER_WIDTH)
                                .max(0.);
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
            .left(px(ROW_GUTTER_WIDTH))
            .w(px(self.viewport.width))
            .h(px(HEADER_HEIGHT))
            .overflow_hidden()
            .bg(palette.header);
        let mut body = div()
            .debug_selector(|| "grid-body".into())
            .absolute()
            .top(px(HEADER_HEIGHT))
            .left(px(ROW_GUTTER_WIDTH))
            .w(px(self.viewport.width))
            .h(px(self.viewport.height))
            .overflow_hidden();
        let mut gutter = div()
            .id("grid-row-gutter")
            .debug_selector(|| "grid-row-gutter".into())
            .absolute()
            .left_0()
            .top(px(HEADER_HEIGHT))
            .w(px(ROW_GUTTER_WIDTH))
            .h(px(self.viewport.height))
            .overflow_hidden()
            .bg(palette.header);
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
                let metadata_tip = format!(
                    "{} · {}{}{} · {}",
                    column.name,
                    column.data_type,
                    if column.is_primary_key {
                        " · primary key"
                    } else {
                        ""
                    },
                    if column.nullable {
                        " · nullable"
                    } else {
                        " · not null"
                    },
                    if self.page.as_ref().is_some_and(|_| self.stale) {
                        "Read-only result; sorting unavailable"
                    } else {
                        "Sort ascending / descending / default"
                    }
                );
                let label = self
                    .visible_headers
                    .entry(column_index)
                    .or_insert_with(|| column.name.clone().into())
                    .clone();
                let debug_id = format!("sort-column-{column_index}");
                header = header.child(GridHeaderButton {
                    id: debug_id.clone().into(),
                    enabled: !self.stale && in_view,
                    button: KitButton::new(gpui::SharedString::from(format!(
                        "sort-column-{column_index}"
                    )))
                    .debug_selector(move || debug_id.clone())
                    .ghost()
                    .selected(direction.is_some())
                    .disabled(self.stale)
                    .tab_index(20)
                    .tab_stop(!self.stale && in_view)
                    .absolute()
                    .left(px(left))
                    .top_0()
                    .w(px(COLUMN_WIDTH))
                    .h(px(HEADER_HEIGHT))
                    .rounded(px(0.))
                    .tooltip(metadata_tip)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.stale {
                            this.model
                                .update(cx, |model, cx| model.cycle_sort(name.clone(), cx));
                        }
                    }))
                    .child(
                        div()
                            .id(gpui::SharedString::from(format!(
                                "column-indicator-{column_index}"
                            )))
                            .child(KitIcon::empty().path(if column.is_primary_key {
                                "icons/key-round.svg"
                            } else {
                                column_type_icon(&column.data_type)
                            })),
                    )
                    .child(div().flex_1().min_w_0().text_ellipsis().child(label))
                    .child(
                        div()
                            .id(gpui::SharedString::from(format!(
                                "column-sort-button-{column_index}"
                            )))
                            .debug_selector(move || format!("column-sort-button-{column_index}"))
                            .child(KitIcon::empty().path(match direction {
                                Some(SortDirection::Descending) => "icons/arrow-down.svg",
                                Some(SortDirection::Ascending) => "icons/arrow-up.svg",
                                None => "icons/arrow-down-up.svg",
                            })),
                    ),
                });
            }
            let mut highlights = Vec::new();
            if let Some((row, col)) = self.active_cell {
                highlights.push((
                    col as f32 * COLUMN_WIDTH - self.viewport.x,
                    row as f32 * ROW_HEIGHT - self.viewport.y,
                    COLUMN_WIDTH,
                    ROW_HEIGHT,
                    palette.selection,
                ));
            }
            if let Some(edits) = &self.model.read(cx).table_edits {
                for row in rows.clone() {
                    if edits.is_deleted(row) {
                        highlights.push((
                            0.,
                            row as f32 * ROW_HEIGHT - self.viewport.y,
                            self.viewport.width,
                            ROW_HEIGHT,
                            palette.error.opacity(0.16),
                        ));
                    } else {
                        for col in columns.clone() {
                            if edits.is_changed(row, col) || edits.is_insert(row) {
                                highlights.push((
                                    col as f32 * COLUMN_WIDTH - self.viewport.x,
                                    row as f32 * ROW_HEIGHT - self.viewport.y,
                                    COLUMN_WIDTH,
                                    ROW_HEIGHT,
                                    palette.selection.opacity(0.6),
                                ));
                            }
                        }
                    }
                }
            }
            let mut painted_cells = Vec::new();
            let mut painted_numbers = Vec::new();
            for row_index in rows.clone() {
                let inserted = self
                    .model
                    .read(cx)
                    .table_edits
                    .as_ref()
                    .is_some_and(|e| e.is_insert(row_index));
                let top = row_index as f32 * ROW_HEIGHT - self.viewport.y;
                let visible_row = top < self.viewport.height && top + ROW_HEIGHT > 0.;
                if visible_row {
                    let number = self.row_numbers.entry(row_index).or_insert_with(|| {
                        shape_cell(
                            format!(
                                "{}",
                                page.offset
                                    .saturating_add(row_index as u64)
                                    .saturating_add(1)
                            )
                            .into(),
                            &font,
                            palette.muted,
                            window,
                            false,
                        )
                    });
                    painted_numbers.push((top, number.clone()));
                }
                for column_index in columns.clone() {
                    let value = self.cell_value(row_index, column_index, cx);
                    let value = value.as_ref();
                    let key = (row_index, column_index);
                    let text = self.visible_text.entry(key).or_insert_with(|| {
                        #[cfg(test)]
                        {
                            self.last_formatted_cells += 1;
                        }
                        if inserted && value.is_none() {
                            "DEFAULT".into()
                        } else {
                            display_preview(value)
                        }
                    });
                    let left = column_index as f32 * COLUMN_WIDTH - self.viewport.x;
                    if visible_row && left < self.viewport.width && left + COLUMN_WIDTH > 0. {
                        let line = self.visible_lines.entry(key).or_insert_with(|| {
                            #[cfg(test)]
                            {
                                self.last_shaped_cells += 1;
                            }
                            shape_cell(
                                text.clone(),
                                &font,
                                if matches!(value, Some(CellValue::Null)) {
                                    palette.muted
                                } else {
                                    palette.text
                                },
                                window,
                                true,
                            )
                        });
                        painted_cells.push((left, top, line.clone()));
                    }
                    // Debug-only geometry preserves UI-test selectors without
                    // creating a retained element per cell in production.
                    #[cfg(all(test, feature = "ui-tests"))]
                    {
                        let cell_id = format!("cell-{row_index}-{column_index}");
                        body = body.child(
                            div()
                                .debug_selector(move || cell_id.clone())
                                .absolute()
                                .left(px(left))
                                .top(px(top))
                                .w(px(COLUMN_WIDTH))
                                .h(px(ROW_HEIGHT)),
                        );
                    }
                }
            }
            #[cfg(test)]
            {
                self.last_painted_cells = painted_cells.len();
            }
            let viewport = self.viewport;
            let paint_rows = self.viewport.painted_rows(row_count);
            let paint_columns = self.viewport.painted_columns(column_count);
            body = body.child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, cx| {
                        window.with_content_mask(Some(ContentMask { bounds }), |window| {
                            // One background and horizontal separator per visible row.
                            for row in paint_rows {
                                let top = row as f32 * ROW_HEIGHT - viewport.y;
                                if top >= viewport.height || top + ROW_HEIGHT <= 0. {
                                    continue;
                                }
                                let origin = bounds.origin + point(px(0.), px(top));
                                window.paint_quad(fill(
                                    Bounds::new(
                                        origin,
                                        size(bounds.size.width, px(GRID_ROW_HEIGHT)),
                                    ),
                                    if row % 2 == 0 {
                                        palette.table_even
                                    } else {
                                        palette.table
                                    },
                                ));
                                window.paint_quad(fill(
                                    Bounds::new(
                                        origin + point(px(0.), px(ROW_HEIGHT - 1.)),
                                        size(bounds.size.width, px(1.)),
                                    ),
                                    palette.table_row_border,
                                ));
                            }
                            // One full-height separator per column, not per cell.
                            for column in paint_columns {
                                let right = (column + 1) as f32 * COLUMN_WIDTH - viewport.x;
                                if right > 0. && right <= viewport.width {
                                    window.paint_quad(fill(
                                        Bounds::new(
                                            bounds.origin + point(px(right - 1.), px(0.)),
                                            size(px(1.), bounds.size.height),
                                        ),
                                        palette.table_row_border,
                                    ));
                                }
                            }
                            for (left, top, width, height, color) in highlights {
                                window.paint_quad(fill(
                                    Bounds::new(
                                        bounds.origin + point(px(left), px(top)),
                                        size(px(width), px(height)),
                                    ),
                                    color,
                                ));
                            }
                            for (left, top, line) in painted_cells {
                                let cell_bounds = Bounds::new(
                                    bounds.origin + point(px(left + CELL_PADDING), px(top)),
                                    size(px(COLUMN_WIDTH - CELL_PADDING * 2.), px(ROW_HEIGHT)),
                                );
                                window.with_content_mask(
                                    Some(ContentMask {
                                        bounds: cell_bounds,
                                    }),
                                    |window| {
                                        let _ = line.paint(
                                            cell_bounds.origin,
                                            px(ROW_HEIGHT),
                                            gpui::TextAlign::Left,
                                            None,
                                            window,
                                            cx,
                                        );
                                    },
                                );
                            }
                        });
                    },
                )
                .absolute()
                .size_full(),
            );
            // The gutter is fixed horizontally and follows the same row offset.
            gutter = gutter.child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, cx| {
                        window.with_content_mask(Some(ContentMask { bounds }), |window| {
                            window.paint_quad(fill(bounds, palette.header));
                            for (top, line) in painted_numbers {
                                let _ = line.paint(
                                    point(
                                        bounds.right() - px(8.) - line.width,
                                        bounds.top() + px(top),
                                    ),
                                    px(ROW_HEIGHT),
                                    gpui::TextAlign::Left,
                                    None,
                                    window,
                                    cx,
                                );
                            }
                            window.paint_quad(fill(
                                Bounds::new(
                                    point(bounds.right() - px(1.), bounds.top()),
                                    size(px(1.), bounds.size.height),
                                ),
                                palette.table_row_border,
                            ));
                        });
                    },
                )
                .absolute()
                .size_full(),
            );
            if page.rows.is_empty() {
                body = body.child(
                    div()
                        .p(px(12.))
                        .text_color(palette.muted)
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
        let mut root = div()
            .id("table-grid-scroll")
            .debug_selector(|| "table-grid-scroll".into())
            .relative()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .bg(palette.table)
            .text_size(px(12.))
            .track_focus(&self.focus)
            .key_context("DataGrid")
            // track_focus installs GPUI's default mouse focus behavior. An
            // explicitly focusing parent listener would steal focus from headers.
            .on_key_down(cx.listener(Self::keyboard))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    this.active_cell = this.hit_cell(event.position);
                    if this.active_cell.is_some() {
                        if event.click_count == 2 {
                            this.begin_edit(window, cx);
                        }
                        cx.notify();
                    }
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                    this.active_cell = this.hit_cell(event.position);
                    cx.notify();
                }),
            )
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
            .child(gutter)
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .w(px(ROW_GUTTER_WIDTH))
                    .h(px(HEADER_HEIGHT))
                    .bg(palette.header)
                    .text_color(palette.muted)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child("#"),
            )
            .when(self.viewport.max_x(column_count) > 0., |root| {
                root.child(
                    div()
                        .id("grid-horizontal-track")
                        .debug_selector(|| "grid-horizontal-track".into())
                        .absolute()
                        .left(px(ROW_GUTTER_WIDTH))
                        .bottom_0()
                        .w(px(self.viewport.width))
                        .h(px(SCROLLBAR_SIZE))
                        .bg(palette.scrollbar)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
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
                                .bg(palette.scrollbar_thumb)
                                .cursor_pointer()
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(
                                        |this, event: &gpui::MouseDownEvent, window, cx| {
                                            this.start_thumb(Axis::Horizontal, event, window, cx)
                                        },
                                    ),
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
                        .bg(palette.scrollbar)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
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
                                .bg(palette.scrollbar_thumb)
                                .cursor_pointer()
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(
                                        |this, event: &gpui::MouseDownEvent, window, cx| {
                                            this.start_thumb(Axis::Vertical, event, window, cx)
                                        },
                                    ),
                                ),
                        ),
                )
            });
        let pending = self.model.read(cx).has_table_changes();
        let writable = self.model.read(cx).table_editable();
        let busy = self.model.read(cx).write_busy;
        if writable || pending || self.model.read(cx).write_uncertain {
            let mut bar = div()
                .id("grid-write-tools")
                .debug_selector(|| "grid-write-tools".into())
                .absolute()
                .bottom_0()
                .left_0()
                .right_0()
                .p_2()
                .bg(cx.theme().background)
                .border_t_1()
                .border_color(cx.theme().border)
                .flex()
                .flex_wrap()
                .gap_2();
            for (id, label, action, disabled) in [
                ("grid-add-row", "Add row", "add", !writable),
                (
                    "grid-edit-cell",
                    "Edit cell",
                    "edit",
                    !writable || self.active_cell.is_none(),
                ),
                (
                    "grid-clone-row",
                    "Clone row",
                    "clone",
                    !writable || self.active_cell.is_none(),
                ),
                (
                    "grid-delete-row",
                    "Delete row",
                    "delete",
                    !writable || self.active_cell.is_none(),
                ),
                (
                    "grid-apply",
                    "Apply changes",
                    "apply",
                    !writable || !pending,
                ),
                ("grid-discard", "Discard", "discard", busy || !pending),
            ] {
                bar = bar.child(
                    KitButton::new(id)
                        .debug_selector(move || id.into())
                        .small()
                        .ghost()
                        .label(label)
                        .tooltip(label)
                        .disabled(disabled)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.grid_action(action, window, cx)
                        })),
                );
            }
            root = root.child(bar);
        }
        if let Some(message) = self.model.read(cx).write_feedback.clone() {
            root = root.child(
                div()
                    .id("grid-write-feedback")
                    .debug_selector(|| "grid-write-feedback".into())
                    .absolute()
                    .top(px(HEADER_HEIGHT))
                    .left(px(ROW_GUTTER_WIDTH))
                    .right_0()
                    .p_2()
                    .bg(cx.theme().popover)
                    .child(message),
            );
        }
        if self.editing {
            root = root.child(
                div()
                    .id("grid-cell-editor")
                    .debug_selector(|| "grid-cell-editor".into())
                    .absolute()
                    .top(px(HEADER_HEIGHT))
                    .left(px(ROW_GUTTER_WIDTH))
                    .w(px(340.))
                    .p_2()
                    .bg(cx.theme().popover)
                    .border_1()
                    .border_color(cx.theme().border)
                    .flex()
                    .gap_2()
                    .child(self.edit_input.clone())
                    .child(
                        KitButton::new("grid-stage-cell")
                            .debug_selector(|| "grid-stage-cell".into())
                            .small()
                            .label("Stage")
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some((row, col)) = this.edit_target {
                                    let text = this.edit_input.read(cx).value();
                                    this.model
                                        .update(cx, |m, cx| m.stage_cell(row, col, Some(text), cx));
                                }
                                this.editing = false;
                                this.edit_target = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        KitButton::new("grid-cancel-cell")
                            .small()
                            .label("Cancel")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.editing = false;
                                cx.notify();
                            })),
                    ),
            );
        }
        if self.write_confirm && !self.write_alert_open {
            self.open_write_alert(true, window, cx);
        }
        let entity = cx.entity().downgrade();
        root.context_menu(move |mut menu, _, cx| {
            let target = entity.upgrade().and_then(|grid| grid.read(cx).active_cell);
            let allowed = entity
                .upgrade()
                .is_some_and(|grid| grid.read(cx).model.read(cx).table_editable());
            for (action, label) in [
                ("copy", "Copy cell"),
                ("edit", "Edit cell"),
                ("null", "Set NULL"),
                ("add", "Add row"),
                ("clone", "Clone row"),
                ("delete", "Delete row"),
                ("restore", "Restore row"),
                ("discard", "Discard staged changes"),
            ] {
                let entity = entity.clone();
                menu = menu.item(
                    PopupMenuItem::new(label)
                        .disabled(
                            (action != "copy" && !allowed)
                                || (target.is_none() && !matches!(action, "add" | "discard")),
                        )
                        .on_click(move |_, window, cx| {
                            let _ = entity.update(cx, |grid, cx| {
                                if grid.active_cell == target {
                                    grid.grid_action(action, window, cx)
                                }
                            });
                        }),
                );
            }
            menu
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
        cx.update(gpui::init);
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
        let (_, cx) = cx.add_window_view(|window, cx| {
            let harness = cx.new(|_| GridHarness(grid.clone()));
            gpui::base::Root::new(harness, window, cx)
        });
        cx.simulate_resize(gpui::size(px(730.), px(258.)));
        cx.refresh().unwrap();
        cx.run_until_parked();
        (model, grid, cx)
    }

    fn editable_fixture(
        cx: &mut TestAppContext,
    ) -> (
        Entity<SourceModel>,
        Entity<DataGrid>,
        &mut VisualTestContext,
    ) {
        let (model, grid, cx) = fixture(cx);
        model.update(cx, |m, cx| {
            m.selected_source = Some(m.profiles[0].id.clone());
            m.tables = vec![dalan_drivers::TableInfo {
                name: "wide".into(),
                kind: "BASE TABLE".into(),
            }];
            m.page = Some(Arc::new(TablePage {
                columns: vec![
                    ColumnInfo {
                        name: "id".into(),
                        data_type: "int".into(),
                        nullable: false,
                        is_primary_key: true,
                    },
                    ColumnInfo {
                        name: "name".into(),
                        data_type: "varchar(255)".into(),
                        nullable: true,
                        is_primary_key: false,
                    },
                ],
                rows: vec![
                    vec![CellValue::Number("1".into()), CellValue::Text("one".into())],
                    vec![CellValue::Number("2".into()), CellValue::Text("two".into())],
                ],
                has_more: false,
                next_offset: None,
                offset: 0,
                truncated: false,
            }));
            cx.notify();
        });
        cx.run_until_parked();
        (model, grid, cx)
    }
    #[gpui::test]
    fn query_draft_notifications_keep_retained_grid_caches_without_repaint(
        cx: &mut TestAppContext,
    ) {
        let (model, grid, cx) = fixture(cx);
        model.update(cx, |m, cx| {
            m.query_console = true;
            cx.notify();
        });
        cx.run_until_parked();
        cx.refresh().unwrap();
        let cached = grid.read_with(cx, |g, _| g.visible_lines.len());
        assert!(cached > 0);
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let subscription = cx.update(|_, app| {
            let count = count.clone();
            app.observe(&grid, move |_, _| {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            })
        });
        cx.run_until_parked();
        count.store(0, std::sync::atomic::Ordering::SeqCst);
        for i in 0..30 {
            model.update(cx, |m, cx| m.set_query_sql(format!("SELECT {i}"), cx));
            cx.run_until_parked();
        }
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(grid.read_with(cx, |g, _| g.visible_lines.len()), cached);
        drop(subscription);
    }

    #[gpui::test]
    fn cell_editor_stages_captured_target_and_discard_preserves_original_page(
        cx: &mut TestAppContext,
    ) {
        let (model, grid, cx) = editable_fixture(cx);
        let base = model.read_with(cx, |m, _| m.page.clone().unwrap());
        grid.update_in(cx, |g, window, cx| {
            g.active_cell = Some((0, 1));
            g.begin_edit(window, cx);
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("grid-cell-editor").is_some());
        grid.update(cx, |g, cx| {
            g.active_cell = Some((1, 1));
            g.edit_input.update(cx, |i, cx| i.set_value("changed", cx));
        });
        cx.run_until_parked();
        click(cx, "grid-stage-cell");
        model.read_with(cx, |m, _| {
            let edits = m.table_edits.as_ref().unwrap();
            assert_eq!(
                edits.displayed(0, 1),
                Some(&CellValue::Text("changed".into()))
            );
            assert_eq!(edits.displayed(1, 1), Some(&CellValue::Text("two".into())));
            assert!(Arc::ptr_eq(m.page.as_ref().unwrap(), &base));
            assert!(!m.write_busy && !m.busy);
        });
        click(cx, "grid-apply");
        assert!(cx.debug_bounds("grid-confirm-apply").is_some());
        click(cx, "grid-keep-staging");
        assert!(!model.read_with(cx, |m, _| m.write_busy));
        grid.update_in(cx, |g, window, cx| g.grid_action("discard", window, cx));
        cx.run_until_parked();
        assert!(model.read_with(cx, |m, _| m.has_table_changes()));
        click(cx, "grid-confirm-discard");
        assert!(!model.read_with(cx, |m, _| m.has_table_changes()));
        assert!(Arc::ptr_eq(
            &base,
            &model.read_with(cx, |m, _| m.page.clone().unwrap())
        ));
    }
    #[gpui::test]
    fn staged_alerts_cancel_safely_and_reject_changed_revisions(cx: &mut TestAppContext) {
        use gpui::component::WindowExt;
        let (model, grid, cx) = editable_fixture(cx);
        model.update(cx, |m, cx| m.stage_cell(0, 1, Some("changed".into()), cx));
        cx.run_until_parked();
        let body = cx.debug_bounds("grid-body").unwrap();
        grid.update_in(cx, |g, w, cx| g.grid_action("discard", w, cx));
        cx.run_until_parked();
        assert!(cx.update(|w, cx| w.has_active_dialog(cx)));
        assert_eq!(cx.debug_bounds("grid-body").unwrap(), body);
        click(cx, "grid-keep-edits");
        assert!(!cx.update(|w, cx| w.has_active_dialog(cx)));
        assert!(model.read_with(cx, |m, _| m.has_table_changes()));
        grid.update_in(cx, |g, w, cx| g.grid_action("apply", w, cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("grid-write-confirm").is_none());
        assert_eq!(cx.debug_bounds("grid-body").unwrap(), body);
        cx.simulate_click(point(px(2.), px(2.)), Modifiers::default());
        cx.run_until_parked();
        assert!(cx.update(|w, cx| w.has_active_dialog(cx)));
        assert!(model.read_with(cx, |m, _| m.has_table_changes()));
        press(cx, "escape");
        assert!(!cx.update(|w, cx| w.has_active_dialog(cx)));
        grid.update_in(cx, |g, w, cx| g.grid_action("discard", w, cx));
        cx.run_until_parked();
        model.update(cx, |m, cx| m.stage_cell(1, 1, Some("newer".into()), cx));
        cx.run_until_parked();
        click(cx, "grid-confirm-discard");
        assert!(model.read_with(cx, |m, _| m.has_table_changes()));
        assert!(cx.update(|w, cx| w.has_active_dialog(cx)));
        click(cx, "grid-keep-edits");
        grid.update_in(cx, |g, w, cx| g.grid_action("apply", w, cx));
        cx.run_until_parked();
        model.update(cx, |m, cx| m.stage_cell(1, 1, Some("latest".into()), cx));
        cx.run_until_parked();
        click(cx, "grid-confirm-apply");
        assert!(!model.read_with(cx, |m, _| m.write_busy));
        assert!(model.read_with(cx, |m, _| m.has_table_changes()));
        click(cx, "grid-keep-staging");
    }

    #[gpui::test]
    fn staged_rows_render_default_clone_deleted_restore_without_network(cx: &mut TestAppContext) {
        let (model, grid, cx) = editable_fixture(cx);
        grid.update_in(cx, |g, window, cx| {
            g.active_cell = Some((0, 1));
            g.grid_action("clone", window, cx);
        });
        cx.run_until_parked();
        model.read_with(cx, |m, _| {
            let edits = m.table_edits.as_ref().unwrap();
            assert_eq!(edits.rows_count(), 3);
            assert_eq!(edits.displayed(2, 0), None);
            assert_eq!(edits.displayed(2, 1), Some(&CellValue::Text("one".into())));
        });
        model.update(cx, |m, cx| m.stage_row(0, "delete", cx));
        cx.run_until_parked();
        assert!(model.read_with(cx, |m, _| m.table_edits.as_ref().unwrap().is_deleted(0)));
        model.update(cx, |m, cx| m.stage_row(0, "restore", cx));
        cx.run_until_parked();
        assert!(!model.read_with(cx, |m, _| m.table_edits.as_ref().unwrap().is_deleted(0)));
        assert!(grid.read_with(cx, |g, _| g.last_materialized_cells < 100));
        model.update(cx, |m, cx| {
            let mut p = (**m.page.as_ref().unwrap()).clone();
            p.truncated = true;
            m.discard_table_changes(cx);
            m.page = Some(Arc::new(p));
            cx.notify();
        });
        assert!(!model.read_with(cx, |m, _| m.table_editable()));
    }

    fn click(cx: &mut VisualTestContext, selector: &'static str) {
        let bounds = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("missing {selector}"));
        cx.simulate_click(bounds.center(), Modifiers::default());
        cx.run_until_parked();
    }

    /// Kit buttons activate on key release; exercise a complete native press.
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

    #[test]
    fn previews_escape_controls_and_bound_graphemes_and_bytes() {
        assert_eq!(
            single_line_preview("a\r\nb\rc\nd\te\u{2028}f\u{2029}\0\u{1b}"),
            "a↵b↵c↵d⇥e↵f↵\\u{0}\\u{1b}"
        );
        let family = "👨‍👩‍👧‍👦";
        assert_eq!(single_line_preview(&family.repeat(128)), family.repeat(128));
        assert_eq!(
            single_line_preview(&family.repeat(129)),
            format!("{}…", family.repeat(128))
        );
        assert_eq!(
            single_line_preview(&format!("a{}", "\u{301}".repeat(8192))),
            "…"
        );
        assert_eq!(display_preview(None).as_ref(), "");
        assert_eq!(display_preview(Some(&CellValue::Null)).as_ref(), "NULL");
    }

    #[gpui::test]
    fn shape_cell_defensively_escapes_raw_multiline_text(cx: &mut TestAppContext) {
        let (_, _, visual) = fixture(cx);
        visual.update(|window, _| {
            let font = window.text_style().font();
            let line = shape_cell("raw\n\t文".into(), &font, gpui::black(), window, false);
            assert_eq!(line.text.as_ref(), "raw↵⇥文");
        });
    }

    #[gpui::test]
    fn multiline_query_results_render_without_changing_typed_values(cx: &mut TestAppContext) {
        let (model, grid, cx) = fixture(cx);
        let values = [
            CellValue::Text("中文\nHTML\r\nJSON\t👨‍👩‍👧‍👦".into()),
            CellValue::Binary("bytes\0\n\u{1b}".into()),
            CellValue::Number("1\n2".into()),
            CellValue::Temporal("date\rtime\u{2028}end".into()),
        ];
        let mut page = model.read_with(cx, |m, _| (**m.page.as_ref().unwrap()).clone());
        page.rows = (0..100)
            .map(|_| (0..128).map(|i| values[i % values.len()].clone()).collect())
            .collect();
        page.columns.truncate(128);
        model.update(cx, |m, cx| {
            m.page = Some(Arc::new(page));
            cx.notify();
        });
        cx.run_until_parked();
        cx.refresh().unwrap();
        cx.run_until_parked();
        grid.read_with(cx, |grid, _| {
            assert!(!grid.visible_lines.is_empty());
            assert_eq!(grid.visible_text[&(0, 0)].as_ref(), "中文↵HTML↵JSON⇥👨‍👩‍👧‍👦");
            assert!(
                grid.visible_lines
                    .values()
                    .all(|line| !line.text.contains('\n'))
            );
        });
        model.read_with(cx, |model, _| {
            for (i, value) in values.iter().enumerate() {
                assert_eq!(&model.page.as_ref().unwrap().rows[0][i], value);
            }
            assert!(!model.page.as_ref().unwrap().truncated);
        });
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
        let header_focus = cx.update(|window, app| {
            assert!(!grid.read(app).focus.is_focused(window));
            window.focused(app).expect("clicked header must own focus")
        });
        assert!(model.read_with(cx, |m, _| m.sort.is_some()));
        cx.simulate_keystrokes("left right up down home end");
        cx.run_until_parked();
        assert_eq!(grid.read_with(cx, |g, _| g.viewport), viewport);
        assert!(cx.update(|window, _| header_focus.is_focused(window)));
        press(cx, "space");
        cx.run_until_parked();
        assert_eq!(
            model.read_with(cx, |m, _| m.sort.as_ref().unwrap().direction),
            SortDirection::Descending
        );
        press(cx, "enter");
        cx.run_until_parked();
        assert!(model.read_with(cx, |m, _| m.sort.is_none()));
        assert!(cx.update(|window, _| header_focus.is_focused(window)));
        assert_eq!(grid.read_with(cx, |g, _| g.viewport), viewport);
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
            press(cx, "enter");
            press(cx, "space");
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
        cx.update(|window, app| window.focus_next(app));
        press(cx, "enter");
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
        cx.update(|window, app| window.blur(app));
        cx.update(|window, app| window.focus_next(app));
        press(cx, "enter");
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
    #[gpui::test]
    fn query_result_headers_do_not_offer_generated_table_sorting(cx: &mut TestAppContext) {
        let (model, _, cx) = fixture(cx);
        model.update(cx, |model, cx| {
            model.query_console = true;
            cx.notify();
        });
        cx.run_until_parked();
        let header = cx.debug_bounds("sort-column-0").unwrap();
        cx.simulate_click(header.center(), Modifiers::default());
        press(cx, "enter");
        assert!(model.read_with(cx, |model, _| model.sort.is_none()));
    }
    #[gpui::test]
    fn canvas_paints_only_visible_cells_and_reuses_shaped_text(cx: &mut TestAppContext) {
        let (model, grid, cx) = fixture(cx);
        let original = model.read_with(cx, |model, _| Arc::clone(model.page.as_ref().unwrap()));
        wheel(cx, -90.0, -11.0, false);
        wheel(cx, -1.0, -1.0, false);
        grid.read_with(cx, |grid, _| {
            assert_eq!(grid.last_shaped_cells, 0);
            assert_eq!(
                grid.last_painted_cells,
                grid.viewport.painted_rows(200).len() * grid.viewport.painted_columns(512).len()
            );
            assert!(grid.last_painted_cells <= 60);
            assert!(grid.visible_lines.len() <= grid.last_materialized_cells);
        });
        let gutter = cx.debug_bounds("grid-row-gutter").unwrap();
        assert_eq!(gutter.size.width, px(ROW_GUTTER_WIDTH));
        assert_eq!(gutter.origin.x, px(0.0));
        wheel(cx, -200.0 * COLUMN_WIDTH, -50.0 * ROW_HEIGHT, false);
        assert_eq!(
            cx.debug_bounds("grid-row-gutter").unwrap().origin.x,
            px(0.0)
        );
        assert!(model.read_with(cx, |model, _| Arc::ptr_eq(
            model.page.as_ref().unwrap(),
            &original
        )));
        let mut next = original.as_ref().clone();
        next.offset = 100;
        model.update(cx, |model, cx| {
            model.page = Some(Arc::new(next));
            cx.notify();
        });
        cx.run_until_parked();
        assert_eq!(
            grid.read_with(cx, |grid, _| grid.row_numbers[&0].text.to_string()),
            "101"
        );
        grid.read_with(cx, |grid, _| println!("canvas grid: painted {} cells, cached {} shaped cells, header controls {}, production cell elements 0", grid.last_painted_cells, grid.visible_lines.len(), grid.visible_headers.len()));
    }

    #[gpui::test]
    fn active_theme_changes_reshape_cached_cells_and_row_numbers(cx: &mut TestAppContext) {
        let (_, grid, cx) = fixture(cx);
        let previous_colors = grid.read_with(cx, |grid, _| grid.line_colors.unwrap());
        let viewport = grid.read_with(cx, |grid, _| grid.viewport);
        cx.update(|window, app| {
            use gpui::component::{Theme, ThemeMode};
            let mode = if Theme::global(app).mode.is_dark() {
                ThemeMode::Light
            } else {
                ThemeMode::Dark
            };
            Theme::change(mode, Some(window), app);
        });
        cx.refresh().unwrap();
        cx.run_until_parked();
        grid.read_with(cx, |grid, app| {
            let palette = colors(app);
            assert_ne!(grid.line_colors.unwrap(), previous_colors);
            assert_eq!(grid.line_colors, Some((palette.text, palette.muted)));
            assert_eq!(grid.viewport, viewport);
            assert!(grid.last_shaped_cells > 0);
            assert!(!grid.row_numbers.is_empty());
        });
    }

    #[test]
    fn column_icons_have_stable_type_categories() {
        assert!(matches!(column_type_icon("BIGINT"), "icons/hash.svg"));
        assert!(matches!(column_type_icon("JSON"), "icons/braces.svg"));
        assert!(matches!(
            column_type_icon("timestamp"),
            "icons/calendar-clock.svg"
        ));
        assert!(matches!(column_type_icon("VARBINARY"), "icons/binary.svg"));
        assert!(matches!(
            column_type_icon("VARCHAR"),
            "icons/text-initial.svg"
        ));
        assert!(matches!(column_type_icon("BOOL"), "icons/check.svg"));
    }
    #[gpui::test]
    fn explicit_sort_button_cycles_once_and_preserves_where(cx: &mut TestAppContext) {
        let (model, _, cx) = fixture(cx);
        model.update(cx, |model, cx| {
            model.where_clause = "column_0 = 'value'".into();
            cx.notify();
        });
        cx.run_until_parked();
        click(cx, "column-sort-button-0");
        model.read_with(cx, |model, _| {
            assert_eq!(
                model.sort.as_ref().unwrap().direction,
                SortDirection::Ascending
            );
            assert_eq!(model.where_clause, "column_0 = 'value'");
            assert_eq!(model.order_by, "`column_0` ASC");
        });
    }
}

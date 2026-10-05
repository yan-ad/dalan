// Native input-handler and custom-element scaffolding adapted from GPUI 0.2.2
// examples/input.rs (Copyright Zed Industries, licensed Apache-2.0).
// See the project's retained third-party notices. Multiline editor implementation
// is local; no Zed editor implementation or syntax assets are used.
//! Plain-text SQL editing, with virtualized, unwrapped lines. Double-click selects
//! a word; triple-click selects a line. Cmd-Enter deliberately belongs to the owner.

use super::theme::*;
use gpui::{
    App, Bounds, ClipboardItem, ContentMask, Context, CursorStyle, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, FocusHandle, Focusable, GlobalElementId, KeyBinding, LayoutId,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollWheelEvent,
    ShapedLine, Style, TextRun, UTF16Selection, UnderlineStyle, Window, actions, div, fill, point,
    prelude::*, px, relative, rgb, size,
};
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

const MAX_BYTES: usize = 64 * 1024;
const HISTORY_LIMIT: usize = 100;
const LINE_HEIGHT: f32 = 22.;
const GUTTER: f32 = 44.;

actions!(
    dalan_sql_editor,
    [
        Backspace,
        Delete,
        Left,
        Right,
        Up,
        Down,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectAll,
        Home,
        End,
        SelectHome,
        SelectEnd,
        DocumentHome,
        DocumentEnd,
        SelectDocumentHome,
        SelectDocumentEnd,
        PageUp,
        PageDown,
        SelectPageUp,
        SelectPageDown,
        Enter,
        Tab,
        Outdent,
        Copy,
        Cut,
        Paste,
        Undo,
        Redo
    ]
);

#[derive(Clone)]
struct Snapshot {
    content: String,
    selection: Range<usize>,
    reversed: bool,
}

pub(super) struct SqlEditor {
    focus_handle: FocusHandle,
    content: String,
    lines: Vec<Range<usize>>,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    scroll: Point<Pixels>,
    bounds: Option<Bounds<Pixels>>,
    layouts: Vec<(usize, ShapedLine)>,
    preferred_column: Option<usize>,
    reveal_cursor: bool,
    is_selecting: bool,
    /// Oversized replacements are rejected atomically, without truncating SQL.
    pub(super) validation_error: Option<String>,
}

impl SqlEditor {
    pub(super) fn new(value: &str, cx: &mut Context<Self>) -> Self {
        let value = normalize(value);
        let oversized = value.len() > MAX_BYTES;
        let content = if oversized { String::new() } else { value };
        Self {
            focus_handle: cx.focus_handle().tab_stop(true).tab_index(20),
            lines: line_ranges(&content),
            content,
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            undo: Vec::new(),
            redo: Vec::new(),
            scroll: point(px(0.), px(0.)),
            bounds: None,
            layouts: Vec::new(),
            preferred_column: None,
            reveal_cursor: true,
            is_selecting: false,
            validation_error: oversized.then(limit_message),
        }
    }

    pub(super) fn value(&self) -> String {
        self.content.clone()
    }
    pub(super) fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }
    pub(super) fn selected_text(&self) -> Option<String> {
        (!self.selected_range.is_empty())
            .then(|| self.content[self.selected_range.clone()].to_owned())
    }
    /// Programmatic loads start a new history; oversized loads leave the old SQL intact.
    pub(super) fn set_value(&mut self, value: String, cx: &mut Context<Self>) {
        let value = normalize(&value);
        if value.len() > MAX_BYTES {
            self.validation_error = Some(limit_message());
        } else {
            self.content = value;
            self.selected_range = self.content.len()..self.content.len();
            self.selection_reversed = false;
            self.marked_range = None;
            self.undo.clear();
            self.redo.clear();
            self.scroll = point(px(0.), px(0.));
            self.validation_error = None;
            self.changed();
        }
        cx.notify();
    }
    fn changed(&mut self) {
        self.lines = line_ranges(&self.content);
        self.layouts.clear();
        self.preferred_column = None;
        self.reveal_cursor = true;
    }
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            content: self.content.clone(),
            selection: self.selected_range.clone(),
            reversed: self.selection_reversed,
        }
    }
    fn restore(&mut self, state: Snapshot) {
        self.content = state.content;
        self.selected_range = state.selection;
        self.selection_reversed = state.reversed;
        self.marked_range = None;
        self.validation_error = None;
        self.changed();
    }
    fn replace(
        &mut self,
        range: Range<usize>,
        text: &str,
        mark: bool,
        selection: Option<Range<usize>>,
        cx: &mut Context<Self>,
    ) {
        let text = normalize(text);
        if self.content.len() - range.len() + text.len() > MAX_BYTES {
            self.validation_error = Some(limit_message());
            cx.notify();
            return;
        }
        // Consecutive IME updates form one undo transaction, including the commit.
        if self.marked_range.is_none() && self.content[range.clone()] != text {
            let state = self.snapshot();
            push_history(&mut self.undo, state);
        }
        self.redo.clear();
        self.content.replace_range(range.clone(), &text);
        self.marked_range =
            (mark && !text.is_empty()).then_some(range.start..range.start + text.len());
        self.selected_range = selection
            .map(|r| utf16_range(&text, &r))
            .map(|r| range.start + r.start..range.start + r.end)
            .unwrap_or(range.start + text.len()..range.start + text.len());
        self.selection_reversed = false;
        self.validation_error = None;
        self.changed();
        cx.notify();
    }
    fn cursor(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }
    fn move_cursor(&mut self, offset: usize, select: bool, cx: &mut Context<Self>) {
        if select {
            let anchor = if self.selection_reversed {
                self.selected_range.end
            } else {
                self.selected_range.start
            };
            self.selected_range = anchor.min(offset)..anchor.max(offset);
            self.selection_reversed = offset < anchor;
        } else {
            self.selected_range = offset..offset;
            self.selection_reversed = false;
        }
        self.preferred_column = None;
        self.reveal_cursor = true;
        cx.notify();
    }
    fn row(&self, byte: usize) -> usize {
        self.lines
            .partition_point(|r| r.start <= byte)
            .saturating_sub(1)
    }
    fn horizontal(&mut self, right: bool, select: bool, cx: &mut Context<Self>) {
        let offset = if !select && !self.selected_range.is_empty() {
            if right {
                self.selected_range.end
            } else {
                self.selected_range.start
            }
        } else if right {
            self.content
                .grapheme_indices(true)
                .find(|(i, _)| *i > self.cursor())
                .map(|(i, _)| i)
                .unwrap_or(self.content.len())
        } else {
            self.content
                .grapheme_indices(true)
                .rev()
                .find(|(i, _)| *i < self.cursor())
                .map(|(i, _)| i)
                .unwrap_or(0)
        };
        self.move_cursor(offset, select, cx);
    }
    fn vertical(&mut self, delta: isize, select: bool, cx: &mut Context<Self>) {
        let row = self.row(self.cursor());
        let current = &self.lines[row];
        let column = self.preferred_column.unwrap_or_else(|| {
            self.content[current.start..self.cursor()]
                .graphemes(true)
                .count()
        });
        let target = row.saturating_add_signed(delta).min(self.lines.len() - 1);
        let range = &self.lines[target];
        let offset = self.content[range.clone()]
            .grapheme_indices(true)
            .nth(column)
            .map(|(i, _)| range.start + i)
            .unwrap_or(range.end);
        self.move_cursor(offset, select, cx);
        self.preferred_column = Some(column);
    }
    fn page(&mut self, down: bool, select: bool, cx: &mut Context<Self>) {
        let rows = self
            .bounds
            .map(|b| (f32::from(b.size.height) / LINE_HEIGHT).floor() as isize)
            .unwrap_or(10)
            .max(1);
        self.vertical(if down { rows } else { -rows }, select, cx);
    }
    fn edge(&mut self, end: bool, document: bool, select: bool, cx: &mut Context<Self>) {
        let r = &self.lines[self.row(self.cursor())];
        let offset = match (end, document) {
            (true, true) => self.content.len(),
            (false, true) => 0,
            (true, false) => r.end,
            (false, false) => r.start,
        };
        self.move_cursor(offset, select, cx);
    }
    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_cursor(0, false, cx);
        self.move_cursor(self.content.len(), true, cx);
    }
    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() && self.marked_range.is_none() {
            self.horizontal(false, true, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }
    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() && self.marked_range.is_none() {
            self.horizontal(true, true, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }
    fn enter(&mut self, _: &Enter, window: &mut Window, cx: &mut Context<Self>) {
        let start = self.lines[self.row(self.selected_range.start)].start;
        let indent: String = self.content[start..self.selected_range.start]
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();
        self.replace_text_in_range(None, &format!("\n{indent}"), window, cx);
    }
    fn tab(&mut self, _: &Tab, window: &mut Window, cx: &mut Context<Self>) {
        self.replace_text_in_range(None, "    ", window, cx);
    }
    fn outdent(&mut self, _: &Outdent, _: &mut Window, cx: &mut Context<Self>) {
        let start = self.lines[self.row(self.cursor())].start;
        let n = if self.content[start..].starts_with('\t') {
            1
        } else {
            self.content[start..]
                .bytes()
                .take(4)
                .take_while(|b| *b == b' ')
                .count()
        };
        if n > 0 {
            self.replace(start..start + n, "", false, None, cx);
        }
    }
    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.selected_text() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }
    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.selected_text() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            self.replace_text_in_range(None, "", window, cx);
        }
    }
    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text, window, cx);
        }
    }
    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(state) = self.undo.pop() {
            let current = self.snapshot();
            push_history(&mut self.redo, current);
            self.restore(state);
            cx.notify();
        }
    }
    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(state) = self.redo.pop() {
            let current = self.snapshot();
            push_history(&mut self.undo, current);
            self.restore(state);
            cx.notify();
        }
    }
    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        self.horizontal(false, false, cx);
    }
    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.horizontal(false, true, cx);
    }
    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        self.horizontal(true, false, cx);
    }
    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.horizontal(true, true, cx);
    }
    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(-1, false, cx);
    }
    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(-1, true, cx);
    }
    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(1, false, cx);
    }
    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(1, true, cx);
    }
    fn page_up(&mut self, _: &PageUp, _: &mut Window, cx: &mut Context<Self>) {
        self.page(false, false, cx);
    }
    fn select_page_up(&mut self, _: &SelectPageUp, _: &mut Window, cx: &mut Context<Self>) {
        self.page(false, true, cx);
    }
    fn page_down(&mut self, _: &PageDown, _: &mut Window, cx: &mut Context<Self>) {
        self.page(true, false, cx);
    }
    fn select_page_down(&mut self, _: &SelectPageDown, _: &mut Window, cx: &mut Context<Self>) {
        self.page(true, true, cx);
    }
    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.edge(false, false, false, cx);
    }
    fn select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        self.edge(false, false, true, cx);
    }
    fn document_home(&mut self, _: &DocumentHome, _: &mut Window, cx: &mut Context<Self>) {
        self.edge(false, true, false, cx);
    }
    fn select_document_home(
        &mut self,
        _: &SelectDocumentHome,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edge(false, true, true, cx);
    }
    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.edge(true, false, false, cx);
    }
    fn select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.edge(true, false, true, cx);
    }
    fn document_end(&mut self, _: &DocumentEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.edge(true, true, false, cx);
    }
    fn select_document_end(
        &mut self,
        _: &SelectDocumentEnd,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edge(true, true, true, cx);
    }

    fn index_for_point(&self, position: Point<Pixels>) -> usize {
        let Some(bounds) = self.bounds else {
            return self.cursor();
        };
        let y = f32::from(position.y - bounds.top() + self.scroll.y);
        let row = (y.max(0.) / LINE_HEIGHT).floor() as usize;
        let row = row.min(self.lines.len() - 1);
        let range = &self.lines[row];
        let Some((_, line)) = self.layouts.iter().find(|(i, _)| *i == row) else {
            return range.start;
        };
        let byte =
            line.closest_index_for_x(position.x - bounds.left() - px(GUTTER) + self.scroll.x);
        let local = &self.content[range.clone()];
        let byte = local
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .chain(std::iter::once(local.len()))
            .min_by_key(|i| i.abs_diff(byte))
            .unwrap_or(0);
        range.start + byte
    }
    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_handle.focus(window);
        let byte = self.index_for_point(event.position);
        self.is_selecting = event.click_count == 1;
        if event.click_count >= 3 {
            let row = self.row(byte);
            let r = self.lines[row].clone();
            let end = self.lines.get(row + 1).map(|r| r.start).unwrap_or(r.end);
            self.move_cursor(r.start, false, cx);
            self.move_cursor(end, true, cx);
        } else if event.click_count == 2 {
            let r = self.lines[self.row(byte)].clone();
            let word = self.content[r.clone()]
                .split_word_bound_indices()
                .find(|(i, word)| r.start + i <= byte && byte < r.start + i + word.len())
                .map(|(i, word)| r.start + i..r.start + i + word.len())
                .unwrap_or(byte..byte);
            self.move_cursor(word.start, false, cx);
            self.move_cursor(word.end, true, cx);
        } else {
            self.move_cursor(byte, event.modifiers.shift, cx);
        }
    }
    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }
    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            // Scroll while dragging past the viewport, so long selections remain possible.
            if let Some(b) = self.bounds {
                if event.position.y < b.top() {
                    self.scroll.y = (self.scroll.y - px(LINE_HEIGHT)).max(px(0.));
                }
                if event.position.y > b.bottom() {
                    self.scroll.y += px(LINE_HEIGHT);
                }
            }
            self.move_cursor(self.index_for_point(event.position), true, cx);
        }
    }
    fn scroll_wheel(&mut self, event: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let delta = event.delta.pixel_delta(px(LINE_HEIGHT));
        self.scroll.x = (self.scroll.x - delta.x).max(px(0.));
        self.scroll.y = (self.scroll.y - delta.y).max(px(0.));
        self.reveal_cursor = false;
        cx.stop_propagation();
        cx.notify();
    }
    fn offset_to_utf16(&self, byte: usize) -> usize {
        self.content[..byte].encode_utf16().count()
    }
    fn range_to_utf16(&self, r: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(r.start)..self.offset_to_utf16(r.end)
    }
}

impl EntityInputHandler for SqlEditor {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let r = utf16_range(&self.content, &range);
        *actual_range = Some(self.range_to_utf16(&r));
        Some(self.content[r].to_owned())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked_range.as_ref().map(|r| self.range_to_utf16(r))
    }
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.marked_range = None;
        cx.notify();
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range
            .map(|r| utf16_range(&self.content, &r))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        self.replace(range, text, false, None, cx);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selection: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range
            .map(|r| utf16_range(&self.content, &r))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        self.replace(range, text, true, selection, cx);
    }
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let bounds = self.bounds?;
        let r = utf16_range(&self.content, &range);
        let row = self.row(r.start);
        let (_, line) = self.layouts.iter().find(|(i, _)| *i == row)?;
        let start = self.lines[row].start;
        let x = bounds.left() + px(GUTTER) - self.scroll.x;
        let y = bounds.top() + px(row as f32 * LINE_HEIGHT) - self.scroll.y;
        Some(Bounds::new(
            point(x + line.x_for_index(r.start - start), y),
            size(
                (line.x_for_index(r.end.min(self.lines[row].end) - start)
                    - line.x_for_index(r.start - start))
                .max(px(1.)),
                px(LINE_HEIGHT),
            ),
        ))
    }
    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        self.bounds?.localize(&point)?;
        Some(self.offset_to_utf16(self.index_for_point(point)))
    }
}

struct SqlTextElement {
    editor: Entity<SqlEditor>,
}
struct VisibleLine {
    row: usize,
    text: ShapedLine,
    number: ShapedLine,
}
struct PrepaintState {
    lines: Vec<VisibleLine>,
}
impl IntoElement for SqlTextElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for SqlTextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> PrepaintState {
        self.editor.update(cx, |editor, _| {
            let style = window.text_style();
            let base = TextRun {
                len: 0,
                font: style.font(),
                color: rgb(TEXT).into(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let shape = |row: usize| {
                let range = &editor.lines[row];
                let text = &editor.content[range.clone()];
                let mut runs = vec![TextRun {
                    len: text.len(),
                    ..base.clone()
                }];
                if let Some(mark) = &editor.marked_range {
                    let a = mark.start.max(range.start).min(range.end) - range.start;
                    let b = mark.end.max(range.start).min(range.end) - range.start;
                    if a < b {
                        runs = vec![
                            TextRun {
                                len: a,
                                ..base.clone()
                            },
                            TextRun {
                                len: b - a,
                                underline: Some(UnderlineStyle {
                                    color: Some(base.color),
                                    thickness: px(1.),
                                    wavy: false,
                                }),
                                ..base.clone()
                            },
                            TextRun {
                                len: text.len() - b,
                                ..base.clone()
                            },
                        ];
                        runs.retain(|r| r.len > 0);
                    }
                }
                window
                    .text_system()
                    .shape_line(text.to_owned().into(), px(13.), &runs, None)
            };
            let row = editor.row(editor.cursor());
            let width = (bounds.size.width - px(GUTTER) - px(2.)).max(px(1.));
            if editor.reveal_cursor {
                let top = px(row as f32 * LINE_HEIGHT);
                if top < editor.scroll.y {
                    editor.scroll.y = top;
                }
                if top + px(LINE_HEIGHT) > editor.scroll.y + bounds.size.height {
                    editor.scroll.y = (top + px(LINE_HEIGHT) - bounds.size.height).max(px(0.));
                }
                let line = shape(row);
                let x = line.x_for_index(editor.cursor() - editor.lines[row].start);
                if x < editor.scroll.x {
                    editor.scroll.x = x;
                }
                if x > editor.scroll.x + width {
                    editor.scroll.x = x - width;
                }
            }
            editor.scroll.y = editor.scroll.y.min(
                (px(editor.lines.len() as f32 * LINE_HEIGHT) - bounds.size.height).max(px(0.)),
            );
            let first = (f32::from(editor.scroll.y) / LINE_HEIGHT).floor() as usize;
            let count = (f32::from(bounds.size.height).max(0.) / LINE_HEIGHT).ceil() as usize + 1;
            let mut lines = Vec::new();
            for row in first..(first + count).min(editor.lines.len()) {
                let number = (row + 1).to_string();
                let number = window.text_system().shape_line(
                    number.clone().into(),
                    px(13.),
                    &[TextRun {
                        len: number.len(),
                        color: rgb(MUTED).into(),
                        ..base.clone()
                    }],
                    None,
                );
                lines.push(VisibleLine {
                    row,
                    text: shape(row),
                    number,
                });
            }
            editor.bounds = Some(bounds);
            editor.reveal_cursor = false;
            PrepaintState { lines }
        })
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        state: &mut PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let editor = self.editor.read(cx);
        let focus = editor.focus_handle.clone();
        let scroll = editor.scroll;
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );
        let editor = self.editor.read(cx);
        let selection = editor.selected_range.clone();
        let content_len = editor.content.len();
        let cursor = editor.cursor();
        let cursor_row = editor.row(cursor);
        let ranges: Vec<_> = state
            .lines
            .iter()
            .map(|v| editor.lines[v.row].clone())
            .collect();
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            let text_bounds = Bounds::new(
                point(bounds.left() + px(GUTTER), bounds.top()),
                size(
                    (bounds.size.width - px(GUTTER)).max(px(0.)),
                    bounds.size.height,
                ),
            );
            window.with_content_mask(
                Some(ContentMask {
                    bounds: text_bounds,
                }),
                |window| {
                    for (visible, range) in state.lines.iter().zip(&ranges) {
                        let x = bounds.left() + px(GUTTER) - scroll.x;
                        let y = bounds.top() + px(visible.row as f32 * LINE_HEIGHT) - scroll.y;

                        if selection.start < range.end + usize::from(range.end < content_len)
                            && selection.end > range.start
                        {
                            let a = selection.start.max(range.start).min(range.end) - range.start;
                            let b = selection.end.min(range.end) - range.start;
                            let end = visible.text.x_for_index(b)
                                + if selection.end > range.end {
                                    px(8.)
                                } else {
                                    px(0.)
                                };
                            window.paint_quad(fill(
                                Bounds::new(
                                    point(x + visible.text.x_for_index(a), y),
                                    size(end - visible.text.x_for_index(a), px(LINE_HEIGHT)),
                                ),
                                rgb(TEXT_SELECTION),
                            ));
                        }
                        // Text is clipped rather than wrapped; empty lines still have a caret and number.
                        let _ = visible.text.paint(point(x, y), px(LINE_HEIGHT), window, cx);
                        if focus.is_focused(window) && cursor_row == visible.row {
                            let caret = visible.text.x_for_index(cursor - range.start);
                            window.paint_quad(fill(
                                Bounds::new(point(x + caret, y), size(px(2.), px(LINE_HEIGHT))),
                                rgb(FOCUS),
                            ));
                        }
                    }
                },
            );
            for visible in &state.lines {
                let y = bounds.top() + px(visible.row as f32 * LINE_HEIGHT) - scroll.y;
                let _ = visible.number.paint(
                    point(bounds.left() + px(GUTTER - 8.) - visible.number.width, y),
                    px(LINE_HEIGHT),
                    window,
                    cx,
                );
            }
        });
        self.editor.update(cx, |editor, _| {
            editor.layouts = state.lines.drain(..).map(|v| (v.row, v.text)).collect();
        });
    }
}

impl Focusable for SqlEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
impl Render for SqlEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("dalan-sql-editor")
            .flex_1()
            .min_h_0()
            .w_full()
            .h_full()
            .bg(rgb(BACKGROUND))
            .text_color(rgb(TEXT))
            .font_family("Menlo")
            .text_size(px(13.))
            .line_height(px(LINE_HEIGHT))
            .overflow_hidden()
            .key_context("DalanSqlEditor")
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::page_up))
            .on_action(cx.listener(Self::select_page_up))
            .on_action(cx.listener(Self::page_down))
            .on_action(cx.listener(Self::select_page_down))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::select_home))
            .on_action(cx.listener(Self::document_home))
            .on_action(cx.listener(Self::select_document_home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_end))
            .on_action(cx.listener(Self::document_end))
            .on_action(cx.listener(Self::select_document_end))
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::enter))
            .on_action(cx.listener(Self::tab))
            .on_action(cx.listener(Self::outdent))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_scroll_wheel(cx.listener(Self::scroll_wheel))
            .child(SqlTextElement {
                editor: cx.entity(),
            })
    }
}

/// Cmd-Enter is intentionally unbound so the query console owns execution.
pub(super) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some("DalanSqlEditor")),
        KeyBinding::new("delete", Delete, Some("DalanSqlEditor")),
        KeyBinding::new("left", Left, Some("DalanSqlEditor")),
        KeyBinding::new("right", Right, Some("DalanSqlEditor")),
        KeyBinding::new("up", Up, Some("DalanSqlEditor")),
        KeyBinding::new("down", Down, Some("DalanSqlEditor")),
        KeyBinding::new("home", Home, Some("DalanSqlEditor")),
        KeyBinding::new("end", End, Some("DalanSqlEditor")),
        KeyBinding::new("pageup", PageUp, Some("DalanSqlEditor")),
        KeyBinding::new("pagedown", PageDown, Some("DalanSqlEditor")),
        KeyBinding::new("cmd-left", Home, Some("DalanSqlEditor")),
        KeyBinding::new("cmd-right", End, Some("DalanSqlEditor")),
        KeyBinding::new("cmd-home", DocumentHome, Some("DalanSqlEditor")),
        KeyBinding::new("cmd-end", DocumentEnd, Some("DalanSqlEditor")),
        KeyBinding::new("cmd-up", DocumentHome, Some("DalanSqlEditor")),
        KeyBinding::new("cmd-down", DocumentEnd, Some("DalanSqlEditor")),
        KeyBinding::new("shift-left", SelectLeft, Some("DalanSqlEditor")),
        KeyBinding::new("shift-right", SelectRight, Some("DalanSqlEditor")),
        KeyBinding::new("shift-up", SelectUp, Some("DalanSqlEditor")),
        KeyBinding::new("shift-down", SelectDown, Some("DalanSqlEditor")),
        KeyBinding::new("shift-home", SelectHome, Some("DalanSqlEditor")),
        KeyBinding::new("shift-end", SelectEnd, Some("DalanSqlEditor")),
        KeyBinding::new("shift-pageup", SelectPageUp, Some("DalanSqlEditor")),
        KeyBinding::new("shift-pagedown", SelectPageDown, Some("DalanSqlEditor")),
        KeyBinding::new("shift-cmd-left", SelectHome, Some("DalanSqlEditor")),
        KeyBinding::new("shift-cmd-right", SelectEnd, Some("DalanSqlEditor")),
        KeyBinding::new("shift-cmd-home", SelectDocumentHome, Some("DalanSqlEditor")),
        KeyBinding::new("shift-cmd-end", SelectDocumentEnd, Some("DalanSqlEditor")),
        KeyBinding::new("shift-cmd-up", SelectDocumentHome, Some("DalanSqlEditor")),
        KeyBinding::new("shift-cmd-down", SelectDocumentEnd, Some("DalanSqlEditor")),
        KeyBinding::new("cmd-a", SelectAll, Some("DalanSqlEditor")),
        KeyBinding::new("cmd-c", Copy, Some("DalanSqlEditor")),
        KeyBinding::new("cmd-x", Cut, Some("DalanSqlEditor")),
        KeyBinding::new("cmd-v", Paste, Some("DalanSqlEditor")),
        KeyBinding::new("cmd-z", Undo, Some("DalanSqlEditor")),
        KeyBinding::new("cmd-shift-z", Redo, Some("DalanSqlEditor")),
        KeyBinding::new("enter", Enter, Some("DalanSqlEditor")),
        KeyBinding::new("tab", Tab, Some("DalanSqlEditor")),
        KeyBinding::new("shift-tab", Outdent, Some("DalanSqlEditor")),
    ]);
}

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}
fn limit_message() -> String {
    "SQL is limited to 64 KiB; the edit was not applied.".into()
}
fn push_history(history: &mut Vec<Snapshot>, state: Snapshot) {
    if history.len() == HISTORY_LIMIT {
        history.remove(0);
    }
    history.push(state);
}
fn line_ranges(text: &str) -> Vec<Range<usize>> {
    let mut start = 0;
    let mut lines = Vec::new();
    for (i, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            lines.push(start..i);
            start = i + 1;
        }
    }
    lines.push(start..text.len());
    lines
}
// Round into the next scalar boundary, including a request inside a surrogate pair.
// Always return a valid, ordered UTF-8 range, including oversized OS requests.
fn utf16_range(text: &str, range: &Range<usize>) -> Range<usize> {
    fn offset(text: &str, requested: usize) -> usize {
        let mut units = 0;
        for (byte, ch) in text.char_indices() {
            if units >= requested {
                return byte;
            }
            units += ch.len_utf16();
        }
        text.len()
    }
    let start = offset(text, range.start);
    start..offset(text, range.end).max(start)
}

#[cfg(all(test, feature = "ui-tests"))]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    #[gpui::test]
    fn multiline_clipboard_indent_and_history(cx: &mut TestAppContext) {
        cx.update(bind_keys);
        let (editor, visual) = cx.add_window_view(|_, cx| SqlEditor::new("  select 😀", cx));
        visual.update(|window, app| {
            app.write_to_clipboard(ClipboardItem::new_string("a\r\nb\rc\td".into()));
            editor.update(app, |e, cx| {
                e.document_end(&DocumentEnd, window, cx);
                e.enter(&Enter, window, cx);
                e.tab(&Tab, window, cx);
                e.paste(&Paste, window, cx);
                assert_eq!(e.value(), "  select 😀\n      a\nb\nc\td");
                e.undo(&Undo, window, cx);
                assert_eq!(e.value(), "  select 😀\n      ");
                e.redo(&Redo, window, cx);
                assert_eq!(e.lines.len(), 4);
                e.select_all(&SelectAll, window, cx);
                e.copy(&Copy, window, cx);
                assert_eq!(cx.read_from_clipboard().unwrap().text(), e.selected_text());
                e.cut(&Cut, window, cx);
                assert_eq!(e.value(), "");
                e.undo(&Undo, window, cx);
                assert_eq!(e.value(), "  select 😀\n      a\nb\nc\td");
            });
        });
    }

    #[gpui::test]
    fn graphemes_vertical_selection_and_utf16_ime(cx: &mut TestAppContext) {
        let (editor, visual) =
            cx.add_window_view(|_, cx| SqlEditor::new("a👩‍💻e\u{301}\nx\n12345", cx));
        visual.update(|window, app| {
            editor.update(app, |e, cx| {
                e.end(&End, window, cx);
                e.backspace(&Backspace, window, cx);
                assert_eq!(e.value(), "a👩‍💻\nx\n12345");
                e.down(&Down, window, cx);
                assert_eq!(e.cursor(), "a👩‍💻\nx".len());
                e.down(&Down, window, cx);
                assert_eq!(e.cursor(), "a👩‍💻\nx\n12".len());
                e.select_up(&SelectUp, window, cx);
                assert_eq!(e.selected_text().as_deref(), Some("\n12"));
                e.set_value("prefix\n".into(), cx);
                e.replace_and_mark_text_in_range(None, "😀é", Some(2..3), window, cx);
                assert_eq!(e.range_to_utf16(&e.selected_range), 9..10);
                assert_eq!(e.marked_text_range(window, cx), Some(7..10));
                let mut actual = None;
                assert_eq!(
                    e.text_for_range(7..9, &mut actual, window, cx).as_deref(),
                    Some("😀")
                );
                assert_eq!(actual, Some(7..9));
                e.replace_and_mark_text_in_range(None, "文", Some(0..1), window, cx);
                e.replace_text_in_range(None, "文章", window, cx);
                assert_eq!(e.value(), "prefix\n文章");
                assert!(e.marked_range.is_none());
                e.undo(&Undo, window, cx);
                assert_eq!(e.value(), "prefix\n");
            });
        });
    }

    #[gpui::test]
    fn limit_is_atomic_and_history_is_bounded(cx: &mut TestAppContext) {
        let (editor, visual) = cx.add_window_view(|_, cx| SqlEditor::new("", cx));
        visual.update(|window, app| {
            editor.update(app, |e, cx| {
                e.set_value("a".repeat(MAX_BYTES), cx);
                e.replace_text_in_range(None, "😀", window, cx);
                assert_eq!(e.content.len(), MAX_BYTES);
                assert!(e.validation_error.is_some());
                e.set_value("b".repeat(MAX_BYTES + 1), cx);
                assert_eq!(e.content.len(), MAX_BYTES);
                e.set_value(String::new(), cx);
                for _ in 0..110 {
                    e.replace_text_in_range(None, "x", window, cx);
                }
                assert_eq!(e.undo.len(), HISTORY_LIMIT);
                for _ in 0..110 {
                    e.undo(&Undo, window, cx);
                }
                assert_eq!(e.value(), "x".repeat(10));
            });
        });
    }

    #[gpui::test]
    fn visible_lines_mouse_hit_and_scrolled_caret(cx: &mut TestAppContext) {
        let sql = (0..1000)
            .map(|i| format!("select {i};\n"))
            .collect::<String>();
        let (editor, visual) = cx.add_window_view(|_, cx| SqlEditor::new(&sql, cx));
        visual.run_until_parked();
        visual.update(|window, app| {
            editor.update(app, |e, cx| {
                let bounds = e.bounds.unwrap();
                assert!(e.layouts.len() < 100);
                let (_, line) = e.layouts.iter().find(|(row, _)| *row == 1).unwrap();
                let position = point(
                    bounds.left() + px(GUTTER) + line.x_for_index(7),
                    bounds.top() + px(LINE_HEIGHT + 5.),
                );
                assert_eq!(e.index_for_point(position), e.lines[1].start + 7);
                e.focus_handle.focus(window);
                e.document_end(&DocumentEnd, window, cx);
            });
        });
        visual.run_until_parked();
        editor.read_with(visual, |e, _| {
            assert!(e.scroll.y > px(0.));
            assert!(e.layouts.iter().any(|(row, _)| *row == 1000));
            assert_eq!(e.cursor(), sql.len());
        });
    }

    #[gpui::test]
    fn native_input_and_key_bindings(cx: &mut TestAppContext) {
        cx.update(bind_keys);
        let (editor, visual) = cx.add_window_view(|_, cx| SqlEditor::new("", cx));
        visual.update(|window, app| editor.update(app, |e, _| e.focus_handle.focus(window)));
        visual.run_until_parked();
        visual.simulate_input("select 😀");
        visual.simulate_keystrokes("enter tab");
        visual.simulate_input("from t");
        editor.read_with(visual, |e, _| {
            assert_eq!(e.value(), "select 😀\n    from t")
        });
        visual.simulate_keystrokes("cmd-z");
        editor.read_with(visual, |e, _| assert_eq!(e.value(), "select 😀\n    from "));
        visual.simulate_keystrokes("cmd-shift-z shift-home");
        editor.read_with(visual, |e, _| {
            assert_eq!(e.selected_text().as_deref(), Some("    from t"))
        });
    }

    #[test]
    fn newline_ranges_and_utf16_clamping() {
        assert_eq!(line_ranges("a\n\n"), vec![0..1, 2..2, 3..3]);
        assert_eq!(normalize("a\r\nb\rc\td"), "a\nb\nc\td");
        assert_eq!(utf16_range("a😀b", &(2..99)), 5..6);
        assert_eq!(utf16_range("a😀b", &Range { start: 99, end: 0 }), 6..6);
    }
}

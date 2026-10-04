// Adapted from GPUI 0.2.2 examples/input.rs (Zed Industries, Apache-2.0).
use std::ops::Range;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, Focusable, GlobalElementId, KeyBinding, LayoutId, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, ShapedLine,
    SharedString, Style, TextRun, UTF16Selection, UnderlineStyle, Window, actions, div, fill,
    point, prelude::*, px, relative, rgb, rgba, size,
};
use unicode_segmentation::*;

actions!(
    dalan_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        ShowCharacterPalette,
        Paste,
        Cut,
        Copy,
    ]
);

pub(super) struct TextInput {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    secret: bool,
    scroll_offset: Pixels,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
}

impl TextInput {
    pub(super) fn new(
        value: impl Into<SharedString>,
        placeholder: &'static str,
        secret: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let content = single_line(&value.into()).into();
        Self {
            focus_handle: cx.focus_handle(),
            content,
            placeholder: placeholder.into(),
            secret,
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
            scroll_offset: px(0.),
        }
    }

    pub(super) fn value(&self) -> String {
        self.content.to_string()
    }

    pub(super) fn set_value(&mut self, value: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.content = single_line(&value.into()).into();
        self.selected_range = self.content.len()..self.content.len();
        self.selection_reversed = false;
        self.marked_range = None;
        self.last_layout = None;
        self.last_bounds = None;
        self.scroll_offset = px(0.);
        cx.notify();
    }

    pub(super) fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    fn display_text(&self) -> SharedString {
        if self.secret {
            "•".repeat(self.content.graphemes(true).count()).into()
        } else {
            self.content.clone()
        }
    }

    // Layout byte offsets differ from the model for passwords; no secret reaches shaping.
    fn display_offset(&self, offset: usize) -> usize {
        if self.secret {
            self.content
                .grapheme_indices(true)
                .take_while(|(i, _)| *i < offset)
                .count()
                * "•".len()
        } else {
            offset
        }
    }

    fn model_offset(&self, offset: usize) -> usize {
        if self.secret {
            self.content
                .grapheme_indices(true)
                .nth(offset / "•".len())
                .map(|(i, _)| i)
                .unwrap_or(self.content.len())
        } else {
            self.content
                .grapheme_indices(true)
                .map(|(i, _)| i)
                .chain(std::iter::once(self.content.len()))
                .min_by_key(|i| i.abs_diff(offset))
                .unwrap_or(0)
        }
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx)
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_handle.focus(window);
        self.is_selecting = true;

        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx)
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text, window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.secret && !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }
    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.secret && !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }

        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        self.model_offset(line.closest_index_for_x(position.x - bounds.left() + self.scroll_offset))
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify()
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;

        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }

        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        utf16_range(&self.content, range_utf16)
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        // Never expose password text through OS surrounding-text/extraction APIs.
        // IMEs may lose reconversion/context, but insertion and marked replacement work.
        if self.secret {
            *actual_range = None;
            return None;
        }
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
        _cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        let new_text = single_line(new_text);
        self.content =
            (self.content[0..range.start].to_owned() + &new_text + &self.content[range.end..])
                .into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.marked_range.take();
        self.selection_reversed = false;
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        let new_text = single_line(new_text);
        self.content =
            (self.content[0..range.start].to_owned() + &new_text + &self.content[range.end..])
                .into();
        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| utf16_range(&new_text, range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.start)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());

        self.selection_reversed = false;
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let last_layout = self.last_layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        Some(Bounds::from_corners(
            point(
                bounds.left() + last_layout.x_for_index(self.display_offset(range.start))
                    - self.scroll_offset,
                bounds.top(),
            ),
            point(
                bounds.left() + last_layout.x_for_index(self.display_offset(range.end))
                    - self.scroll_offset,
                bounds.bottom(),
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let bounds = self.last_bounds?;
        bounds.localize(&point)?;
        let line = self.last_layout.as_ref()?;
        let index = line.closest_index_for_x(point.x - bounds.left() + self.scroll_offset);
        Some(self.offset_to_utf16(self.model_offset(index)))
    }
}

struct TextElement {
    input: Entity<TextInput>,
}

struct PrepaintState {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
    scroll_offset: Pixels,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
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
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content = input.display_text();
        let selected_range = input.display_offset(input.selected_range.start)
            ..input.display_offset(input.selected_range.end);
        let cursor = input.display_offset(input.cursor_offset());
        let style = window.text_style();

        let (display_text, text_color) = if content.is_empty() {
            (input.placeholder.clone(), rgb(0x9298a2).into())
        } else {
            (content, style.color)
        };

        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let marked_range = input
            .marked_range
            .as_ref()
            .map(|r| input.display_offset(r.start)..input.display_offset(r.end));
        let runs = if let Some(marked_range) = marked_range.as_ref() {
            vec![
                TextRun {
                    len: marked_range.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked_range.end - marked_range.start,
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display_text.len() - marked_range.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect()
        } else {
            vec![run]
        };

        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);

        let cursor_pos = line.x_for_index(cursor);
        let mut scroll_offset = input.scroll_offset;
        if cursor_pos - scroll_offset > bounds.size.width - px(2.) {
            scroll_offset = (cursor_pos - bounds.size.width + px(2.)).max(px(0.));
        } else if cursor_pos < scroll_offset {
            scroll_offset = cursor_pos;
        }
        scroll_offset = scroll_offset.min((line.width - bounds.size.width + px(2.)).max(px(0.)));
        let origin_x = bounds.left() - scroll_offset;
        let (selection, cursor) = if selected_range.is_empty() {
            (
                None,
                Some(fill(
                    Bounds::new(
                        point(origin_x + cursor_pos, bounds.top()),
                        size(px(2.), bounds.bottom() - bounds.top()),
                    ),
                    rgb(0x8ab4f8),
                )),
            )
        } else {
            (
                Some(fill(
                    Bounds::from_corners(
                        point(
                            origin_x + line.x_for_index(selected_range.start),
                            bounds.top(),
                        ),
                        point(
                            origin_x + line.x_for_index(selected_range.end),
                            bounds.bottom(),
                        ),
                    ),
                    rgba(0x8ab4f850),
                )),
                None,
            )
        };
        PrepaintState {
            line: Some(line),
            cursor,
            selection,
            scroll_offset,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection)
        }
        let line = prepaint.line.take().unwrap();
        line.paint(
            point(bounds.left() - prepaint.scroll_offset, bounds.top()),
            window.line_height(),
            window,
            cx,
        )
        .unwrap();

        if focus_handle.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }

        self.input.update(cx, |input, _cx| {
            input.scroll_offset = prepaint.scroll_offset;
            input.last_layout = Some(line);
            input.last_bounds = Some(bounds);
        });
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("dalan-input")
            .debug_selector(|| "dalan-input".into())
            .flex()
            .items_center()
            .w_full()
            .h(px(28.))
            .px(px(7.))
            .border_1()
            .rounded(px(4.))
            .border_color(rgb(if self.focus_handle.is_focused(window) {
                0x8ab4f8
            } else {
                0x4a4d53
            }))
            .bg(rgb(0x191a1c))
            .text_color(rgb(0xe6e8eb))
            .text_size(px(13.))
            .line_height(px(18.))
            .overflow_hidden()
            .key_context("DalanInput")
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::show_character_palette))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .child(TextElement { input: cx.entity() })
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

// Deliberately no Tab or Enter binding: the owner controls navigation/submission.
pub(super) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some("DalanInput")),
        KeyBinding::new("delete", Delete, Some("DalanInput")),
        KeyBinding::new("left", Left, Some("DalanInput")),
        KeyBinding::new("right", Right, Some("DalanInput")),
        KeyBinding::new("shift-left", SelectLeft, Some("DalanInput")),
        KeyBinding::new("shift-right", SelectRight, Some("DalanInput")),
        KeyBinding::new("cmd-a", SelectAll, Some("DalanInput")),
        KeyBinding::new("cmd-v", Paste, Some("DalanInput")),
        KeyBinding::new("cmd-c", Copy, Some("DalanInput")),
        KeyBinding::new("cmd-x", Cut, Some("DalanInput")),
        KeyBinding::new("home", Home, Some("DalanInput")),
        KeyBinding::new("end", End, Some("DalanInput")),
        KeyBinding::new("cmd-left", Home, Some("DalanInput")),
        KeyBinding::new("cmd-right", End, Some("DalanInput")),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, Some("DalanInput")),
    ]);
}

fn single_line(value: &str) -> String {
    value
        .replace("\r\n", " ")
        .replace(['\r', '\n', '\t', '\u{2028}', '\u{2029}'], " ")
}

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
    fn unicode_navigation_and_backspace(cx: &mut TestAppContext) {
        cx.update(bind_keys);
        let (input, visual) =
            cx.add_window_view(|_, cx| TextInput::new("aé👩‍💻e\u{301}", "", false, cx));
        visual.update(|window, app| {
            input.update(app, |input, cx| {
                input.focus_handle.focus(window);
                input.end(&End, window, cx);
                input.backspace(&Backspace, window, cx);
                assert_eq!(input.value(), "aé👩‍💻");
                input.left(&Left, window, cx);
                assert_eq!(input.cursor_offset(), "aé".len());
                input.delete(&Delete, window, cx);
                assert_eq!(input.value(), "aé");
                assert_eq!(input.range_from_utf16(&(1..2)), 1..3);
            })
        });
    }

    #[gpui::test]
    fn password_mask_does_not_copy_or_cut(cx: &mut TestAppContext) {
        let (input, visual) = cx.add_window_view(|_, cx| TextInput::new("é👩‍💻", "", true, cx));
        visual.update(|window, app| {
            app.write_to_clipboard(ClipboardItem::new_string("sentinel".into()));
            input.update(app, |input, cx| {
                assert_eq!(input.display_text().as_ref(), "••");
                assert_eq!(input.value(), "é👩‍💻");
                input.select_all(&SelectAll, window, cx);
                input.copy(&Copy, window, cx);
                input.cut(&Cut, window, cx);
                assert_eq!(input.value(), "é👩‍💻");
                assert_eq!(
                    cx.read_from_clipboard().unwrap().text().as_deref(),
                    Some("sentinel")
                );
                let mut actual = None;
                assert!(
                    input
                        .text_for_range(0..7, &mut actual, window, cx)
                        .is_none()
                );
            });
        });
        visual.refresh().unwrap();
        visual.run_until_parked();
        input.read_with(visual, |input, _| {
            assert_eq!(input.last_layout.as_ref().unwrap().text.as_ref(), "••")
        });
    }

    #[gpui::test]
    fn paste_scrubs_lines_and_ime_selection_is_relative(cx: &mut TestAppContext) {
        let (input, visual) = cx.add_window_view(|_, cx| TextInput::new("x", "", false, cx));
        visual.update(|window, app| {
            app.write_to_clipboard(ClipboardItem::new_string("a\r\nb\nc\rd\te\u{2028}f".into()));
            input.update(app, |input, cx| {
                input.end(&End, window, cx);
                input.paste(&Paste, window, cx);
                assert_eq!(input.value(), "xa b c d e f");
                input.set_value("prefix", cx);
                input.replace_and_mark_text_in_range(None, "😀é", Some(2..3), window, cx);
                assert_eq!(input.value(), "prefix😀é");
                assert_eq!(input.selected_range, 10..12);
                assert_eq!(input.range_to_utf16(&input.selected_range), 8..9);
                input.replace_text_in_range(None, "文", window, cx);
                assert_eq!(input.value(), "prefix文");
                assert!(input.marked_range.is_none());
            });
        });
    }
}

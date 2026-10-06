//! Compatibility bridge for window-less field construction and Kit's input state.
//!
//! Kit owns editing, selection, IME, painting and clipboard actions. The cached
//! value keeps existing form observers synchronous; only value changes notify
//! those observers, not cursor movement or focus changes.
use gpui::component::{
    Sizable,
    input::{Copy, Cut, Input, InputEvent, InputState},
};
use gpui::{
    AnyWindowHandle, App, Bounds, ClipboardItem, Context, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, Focusable, Pixels, Point, SharedString,
    TextInputConfiguration, UTF16Selection, Window, canvas, div, prelude::*,
};
use std::{ops::Range, time::Duration};

pub(super) struct TextInput {
    fallback_focus: FocusHandle,
    input_focus: Option<FocusHandle>,
    state: Option<Entity<InputState>>,
    password_handler: Option<Entity<PasswordInputHandler>>,
    content: SharedString,
    pending_value: Option<SharedString>,
    placeholder: &'static str,
    secret: bool,
    revealed: bool,
    reveal_generation: u64,
    reveal_task: Option<gpui::Task<()>>,
    window: Option<AnyWindowHandle>,
    tab_order: isize,
}

impl TextInput {
    pub(super) fn new(
        value: impl Into<SharedString>,
        placeholder: &'static str,
        secret: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let content: SharedString = single_line(&value.into()).into();
        Self {
            // This handle is only a pre-render focus target, never a tab stop.
            fallback_focus: cx.focus_handle(),
            input_focus: None,
            state: None,
            password_handler: None,
            pending_value: Some(content.clone()),
            content,
            placeholder,
            secret,
            revealed: false,
            reveal_generation: 0,
            reveal_task: None,
            window: None,
            tab_order: 0,
        }
    }

    pub(super) fn password_revealed(&self) -> bool {
        self.secret && self.revealed
    }

    /// Toggle presentation only: never replace text, selection or editing history.
    pub(super) fn reveal_password(&mut self, cx: &mut Context<Self>) {
        if !self.secret {
            return;
        }
        if self.revealed {
            self.hide_password(cx);
            return;
        }
        self.revealed = true;
        self.reveal_generation = self.reveal_generation.wrapping_add(1);
        let generation = self.reveal_generation;
        self.sync_mask(cx);
        let timer = cx.background_executor().timer(Duration::from_secs(3));
        self.reveal_task = Some(cx.spawn(async move |this, cx| {
            timer.await;
            let _ = this.update(cx, |this, cx| {
                if this.reveal_generation == generation {
                    this.hide_password(cx);
                }
            });
        }));
        cx.notify();
    }

    pub(super) fn hide_password(&mut self, cx: &mut Context<Self>) {
        self.reveal_generation = self.reveal_generation.wrapping_add(1);
        self.reveal_task = None;
        if self.revealed {
            self.revealed = false;
            self.sync_mask(cx);
            cx.notify();
        }
    }

    fn sync_mask(&self, cx: &mut Context<Self>) {
        if let (Some(window), Some(state)) = (self.window, self.state.as_ref()) {
            let masked = self.secret && !self.revealed;
            let state = state.clone();
            // Button callbacks may already borrow this window. Apply after the
            // event; render also reconciles the current presentation below.
            cx.defer(move |cx| {
                let _ = window.update(cx, |_, window, cx| {
                    state.update(cx, |state, cx| state.set_masked(masked, window, cx));
                });
            });
        }
    }

    pub(super) fn value(&self) -> String {
        self.content.to_string()
    }

    pub(super) fn set_value(&mut self, value: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.hide_password(cx);
        let value: SharedString = single_line(&value.into()).into();
        self.pending_value = Some(value.clone());
        self.content = value;
        // A Window is deliberately not required here. Render flushes the write
        // to Kit, which also resets selection, composition and editing history.
        cx.notify();
    }

    pub(super) fn focus_handle(&self) -> FocusHandle {
        self.input_focus
            .as_ref()
            .unwrap_or(&self.fallback_focus)
            .clone()
    }

    pub(super) fn set_tab_order(&mut self, index: isize) {
        self.tab_order = index;
    }

    fn ensure_state(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<InputState> {
        self.window = Some(window.window_handle());
        if self.state.is_none() {
            let value = self.content.clone();
            let placeholder = self.placeholder;
            let secret = self.secret;
            let masked = secret && !self.revealed;
            let state = cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(value)
                    .placeholder(placeholder)
                    .masked(masked)
            });
            self.input_focus = Some(state.read(cx).focus_handle(cx));
            cx.subscribe(&state, |this, state, event: &InputEvent, cx| {
                if !matches!(event, InputEvent::Change) || this.pending_value.is_some() {
                    return;
                }
                let raw = state.read(cx).value();
                let value: SharedString = single_line(&raw).into();
                if raw != value {
                    this.pending_value = Some(value.clone());
                }
                if this.content != value {
                    this.content = value;
                    cx.notify();
                }
            })
            .detach();
            if secret {
                let input_state = state.clone();
                self.password_handler =
                    Some(cx.new(|_| PasswordInputHandler { state: input_state }));
            }
            self.state = Some(state);
        }
        let state = self.state.as_ref().unwrap().clone();
        let masked = self.secret && !self.revealed;
        if state.read(cx).presentation().is_masked() != masked {
            state.update(cx, |state, cx| state.set_masked(masked, window, cx));
        }
        if let Some(value) = self.pending_value.take() {
            state.update(cx, |state, cx| state.set_value(value, window, cx));
        }
        if self.fallback_focus.is_focused(window) {
            self.input_focus.as_ref().unwrap().focus(window, cx);
        }
        state
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.ensure_state(window, cx);
        let paste_state = state.clone();
        let password_handler = self.password_handler.clone();
        let focus_handle = self.focus_handle();
        div()
            .id("dalan-input")
            .debug_selector(|| "dalan-input".into())
            .relative()
            .w_full()
            // The Kit state handle is private. An ordered tab group orders its
            // native tab stop without adding a second focusable element.
            .tab_group()
            .tab_index(self.tab_order)
            .track_focus(&self.fallback_focus)
            .tab_stop(false)
            .when(self.secret, |this| {
                // Capture before Kit's handlers: revealing must not enable
                // clipboard extraction (including context-menu actions).
                this.capture_action(|_: &Copy, _, cx| cx.stop_propagation())
                    .capture_action(|_: &Cut, _, cx| cx.stop_propagation())
            })
            .child(
                Input::new(&state)
                    .small()
                    .on_paste(move |item, window, cx| {
                        let Some(text) = item.text() else {
                            return false;
                        };
                        paste_state.update(cx, |state, cx| {
                            state.replace_text_in_range(None, &single_line(&text), window, cx);
                        });
                        true
                    }),
            )
            .when_some(password_handler, |this, handler| {
                // Kit registers its native handler during paint. Register after
                // that child so password extraction cannot reach its raw rope.
                // This canvas neither paints nor installs a mouse hitbox: all
                // editing, selection and rendering remain owned by Kit.
                this.child(
                    canvas(
                        |_, _, _| (),
                        move |bounds, (), window, cx| {
                            window.handle_input(
                                &focus_handle,
                                ElementInputHandler::new(bounds, handler),
                                cx,
                            );
                        },
                    )
                    .absolute()
                    .size_full(),
                )
            })
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle()
    }
}

/// A native-only privacy adapter. Kit still owns the password and every edit;
/// only OS surrounding-text extraction is suppressed.
struct PasswordInputHandler {
    state: Entity<InputState>,
}

impl EntityInputHandler for PasswordInputHandler {
    fn text_for_range(
        &mut self,
        _range: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        *adjusted_range = None;
        None
    }

    fn selected_text_range(
        &mut self,
        ignore_disabled_input: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        self.state.update(cx, |state, cx| {
            EntityInputHandler::selected_text_range(state, ignore_disabled_input, window, cx)
        })
    }

    fn marked_text_range(
        &self,

        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.state.update(cx, |state, cx| {
            EntityInputHandler::marked_text_range(state, window, cx)
        })
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            EntityInputHandler::unmark_text(state, window, cx)
        })
    }

    fn paste(&mut self, item: ClipboardItem, window: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            EntityInputHandler::paste(state, item, window, cx)
        })
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, cx| {
            EntityInputHandler::replace_text_in_range(state, range, text, window, cx)
        })
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected_range: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, cx| {
            EntityInputHandler::replace_and_mark_text_in_range(
                state,
                range,
                text,
                selected_range,
                window,
                cx,
            )
        })
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        self.state.update(cx, |state, cx| {
            EntityInputHandler::bounds_for_range(state, range, bounds, window, cx)
        })
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        self.state.update(cx, |state, cx| {
            EntityInputHandler::character_index_for_point(state, point, window, cx)
        })
    }

    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, cx| {
            EntityInputHandler::set_selected_text_range(state, range, window, cx)
        })
    }

    fn text_length_utf16(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<usize> {
        self.state.update(cx, |state, cx| {
            EntityInputHandler::text_length_utf16(state, window, cx)
        })
    }

    fn accepts_text_input(&self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.state.update(cx, |state, cx| {
            EntityInputHandler::accepts_text_input(state, window, cx)
        })
    }

    fn text_input_configuration(
        &mut self,

        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> TextInputConfiguration {
        self.state.update(cx, |state, cx| {
            EntityInputHandler::text_input_configuration(state, window, cx)
        })
    }

    fn text_input_editable_range(
        &mut self,

        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.state.update(cx, |state, cx| {
            EntityInputHandler::text_input_editable_range(state, window, cx)
        })
    }
}

// Existing callers may keep this hook; gpui::init installs Kit's key bindings.
pub(super) fn bind_keys(_: &mut App) {}

fn single_line(value: &str) -> String {
    value
        .replace("\r\n", " ")
        .replace(['\r', '\n', '\t', '\u{2028}', '\u{2029}'], " ")
}

#[cfg(all(test, feature = "ui-tests"))]
mod tests {
    use super::*;
    use gpui::component::input::{Copy, Cut, SelectAll};
    use gpui::{ClipboardItem, TestAppContext};

    #[gpui::test]
    fn cached_values_work_without_a_window(cx: &mut TestAppContext) {
        let input = cx.new(|cx| TextInput::new("é\r\n😀\t文", "Name", false, cx));
        input.read_with(cx, |input, _| assert_eq!(input.value(), "é 😀 文"));
        input.update(cx, |input, cx| {
            input.set_value("new\nvalue", cx);
            assert_eq!(input.value(), "new value");
            assert!(input.state.is_none());
        });
    }

    #[gpui::test]
    fn native_ime_commit_updates_the_unicode_cache(cx: &mut TestAppContext) {
        cx.update(gpui::init);
        let (input, visual) = cx.add_window_view(|_, cx| TextInput::new("prefix", "", false, cx));
        visual.update(|window, app| {
            input.update(app, |input, cx| {
                let state = input.ensure_state(window, cx);
                state.update(cx, |state, cx| {
                    state.replace_and_mark_text_in_range(None, "😀é", Some(2..3), window, cx);
                    state.replace_text_in_range(None, "文", window, cx);
                    assert!(state.marked_text_range(window, cx).is_none());
                });
            });
        });
        visual.run_until_parked();
        input.read_with(visual, |input, _| assert_eq!(input.value(), "prefix文"));
    }

    #[gpui::test]
    fn password_native_handler_hides_text_and_preserves_unicode_ime(cx: &mut TestAppContext) {
        cx.update(gpui::init);
        let (input, visual) = cx.add_window_view(|_, cx| TextInput::new("A😀Z", "", true, cx));
        visual.refresh().unwrap();
        visual.run_until_parked();
        visual.update(|window, app| {
            input.read(app).focus_handle().focus(window, app);
            let handler = input.read(app).password_handler.clone().unwrap();
            // GPUI keeps the installed platform handler private. Exercise its
            // public native bridge against the same retained adapter entity.
            handler.update(app, |handler, cx| {
                let mut adjusted = Some(1..3);
                assert_eq!(
                    handler.text_for_range(0..4, &mut adjusted, window, cx),
                    None
                );
                assert_eq!(adjusted, None);
                handler.replace_and_mark_text_in_range(Some(1..3), "に😀", Some(1..3), window, cx);
                assert_eq!(handler.marked_text_range(window, cx), Some(1..4));
                assert_eq!(
                    handler
                        .selected_text_range(false, window, cx)
                        .unwrap()
                        .range,
                    2..4
                );
                assert_eq!(
                    handler.text_for_range(1..4, &mut adjusted, window, cx),
                    None
                );
                assert_eq!(adjusted, None);
                handler.replace_text_in_range(None, "日本語", window, cx);
                assert_eq!(handler.marked_text_range(window, cx), None);
                assert_eq!(
                    handler
                        .selected_text_range(false, window, cx)
                        .unwrap()
                        .range,
                    4..4
                );
                handler.replace_and_mark_text_in_range(None, "é", None, window, cx);
                handler.unmark_text(window, cx);
                assert_eq!(handler.marked_text_range(window, cx), None);
                assert_eq!(handler.state.read(cx).value().as_ref(), "A日本語éZ");
            });
        });
        visual.run_until_parked();
        input.read_with(visual, |input, _| assert_eq!(input.value(), "A日本語éZ"));
        // Ordinary native input still routes through the installed adapter.
        visual.simulate_input("文");
        visual.run_until_parked();
        input.read_with(visual, |input, _| assert_eq!(input.value(), "A日本語é文Z"));
    }

    #[gpui::test]
    fn plain_native_handler_keeps_surrounding_text(cx: &mut TestAppContext) {
        cx.update(gpui::init);
        let (input, visual) = cx.add_window_view(|_, cx| TextInput::new("A😀Z", "", false, cx));
        visual.update(|window, app| {
            input.update(app, |input, cx| {
                let state = input.ensure_state(window, cx);
                assert!(input.password_handler.is_none());
                state.update(cx, |state, cx| {
                    let mut adjusted = None;
                    assert_eq!(
                        state.text_for_range(1..3, &mut adjusted, window, cx),
                        Some("😀".into())
                    );
                    assert_eq!(adjusted, Some(1..3));
                });
            });
        });
    }

    #[gpui::test]
    fn plain_kit_input_copies_unicode(cx: &mut TestAppContext) {
        cx.update(gpui::init);
        let (input, visual) = cx.add_window_view(|_, cx| TextInput::new("é😀文", "", false, cx));
        visual.refresh().unwrap();
        visual.run_until_parked();
        visual.update(|window, app| {
            input.read(app).focus_handle().focus(window, app);
        });
        visual.dispatch_action(SelectAll);
        visual.dispatch_action(Copy);
        visual.update(|_, app| {
            assert_eq!(
                app.read_from_clipboard().unwrap().text().as_deref(),
                Some("é😀文")
            );
        });
    }

    #[gpui::test]
    fn kit_changes_update_the_form_cache(cx: &mut TestAppContext) {
        cx.update(gpui::init);
        let (input, visual) = cx.add_window_view(|_, cx| TextInput::new("", "Name", false, cx));
        visual.update(|window, app| {
            input.update(app, |input, cx| {
                let state = input.ensure_state(window, cx);
                state.update(cx, |state, cx| {
                    state.replace_text_in_range(None, "é😀文", window, cx);
                });
            });
        });
        visual.run_until_parked();
        input.read_with(visual, |input, _| assert_eq!(input.value(), "é😀文"));
    }

    #[gpui::test]
    fn programmatic_write_is_flushed_to_kit(cx: &mut TestAppContext) {
        cx.update(gpui::init);
        let (input, visual) = cx.add_window_view(|_, cx| TextInput::new("old", "", false, cx));
        visual.update(|window, app| {
            input.update(app, |input, cx| {
                input.ensure_state(window, cx);
                input.set_value("new\r\nvalue", cx);
                assert_eq!(input.value(), "new value");
                let state = input.ensure_state(window, cx);
                assert_eq!(state.read(cx).value().as_ref(), "new value");
                assert!(input.pending_value.is_none());
            });
        });
    }

    #[gpui::test]
    fn focus_requested_before_render_transfers_to_kit(cx: &mut TestAppContext) {
        cx.update(gpui::init);
        let (input, visual) = cx.add_window_view(|_, cx| TextInput::new("", "", false, cx));
        visual.update(|window, app| {
            input.update(app, |input, cx| {
                input.fallback_focus.focus(window, cx);
                input.ensure_state(window, cx);
                assert!(input.focus_handle().is_focused(window));
                assert!(!input.fallback_focus.is_focused(window));
            });
        });
    }

    #[gpui::test]
    fn reveal_expires_without_rerender_and_keeps_clipboard_and_native_text_private(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui::init);
        let (input, visual) = cx.add_window_view(|_, cx| TextInput::new("é😀secret", "", true, cx));
        visual.refresh().unwrap();
        visual.run_until_parked();
        visual.update(|window, app| {
            app.write_to_clipboard(ClipboardItem::new_string("sentinel".into()));
            input.read(app).focus_handle().focus(window, app);
        });
        visual.dispatch_action(SelectAll);
        input.update(visual, |input, cx| input.reveal_password(cx));
        visual.run_until_parked();
        visual.refresh().unwrap();
        input.read_with(visual, |input, cx| {
            assert!(input.password_revealed());
            assert!(
                !input
                    .state
                    .as_ref()
                    .unwrap()
                    .read(cx)
                    .presentation()
                    .is_masked()
            );
        });
        visual.dispatch_action(Copy);
        visual.dispatch_action(Cut);
        visual.update(|window, app| {
            let handler = input.read(app).password_handler.clone().unwrap();
            handler.update(app, |handler, cx| {
                let mut adjusted = Some(0..1);
                assert!(
                    handler
                        .text_for_range(0..10, &mut adjusted, window, cx)
                        .is_none()
                );
                assert!(adjusted.is_none());
                assert_eq!(
                    handler
                        .selected_text_range(false, window, cx)
                        .unwrap()
                        .range,
                    0..9
                );
            });
            assert_eq!(
                app.read_from_clipboard().unwrap().text().as_deref(),
                Some("sentinel")
            );
        });
        visual.executor().advance_clock(Duration::from_millis(2999));
        visual.run_until_parked();
        input.read_with(visual, |input, _| assert!(input.password_revealed()));
        visual.executor().advance_clock(Duration::from_millis(1));
        visual.run_until_parked();
        input.read_with(visual, |input, cx| {
            assert!(!input.password_revealed());
            assert!(
                input
                    .state
                    .as_ref()
                    .unwrap()
                    .read(cx)
                    .presentation()
                    .is_masked()
            );
            assert_eq!(input.value(), "é😀secret");
        });
    }

    #[gpui::test]
    fn hide_and_set_value_invalidate_prior_reveal_deadlines(cx: &mut TestAppContext) {
        cx.update(gpui::init);
        let (input, visual) = cx.add_window_view(|_, cx| TextInput::new("fixture", "", true, cx));
        visual.refresh().unwrap();
        input.update(visual, |input, cx| input.reveal_password(cx));
        visual.run_until_parked();
        visual.executor().advance_clock(Duration::from_secs(2));
        input.update(visual, |input, cx| {
            input.reveal_password(cx);
            assert!(!input.password_revealed());
            input.reveal_password(cx);
        });
        visual.run_until_parked();
        visual.executor().advance_clock(Duration::from_secs(1));
        visual.run_until_parked();
        input.read_with(visual, |input, _| assert!(input.password_revealed()));
        input.update(visual, |input, cx| input.set_value("replacement", cx));
        visual.run_until_parked();
        input.read_with(visual, |input, cx| {
            assert!(!input.password_revealed());
            assert!(
                input
                    .state
                    .as_ref()
                    .unwrap()
                    .read(cx)
                    .presentation()
                    .is_masked()
            );
        });
    }

    #[gpui::test]
    fn reveal_preserves_caret_focus_and_undo(cx: &mut TestAppContext) {
        cx.update(gpui::init);
        let (input, visual) = cx.add_window_view(|_, cx| TextInput::new("", "", true, cx));
        visual.refresh().unwrap();
        visual.run_until_parked();
        visual.update(|window, app| input.read(app).focus_handle().focus(window, app));
        visual.simulate_input("fixture");
        visual.run_until_parked();
        input.update(visual, |input, cx| input.reveal_password(cx));
        visual.run_until_parked();
        visual.update(|window, app| {
            assert!(input.read(app).focus_handle().is_focused(window));
            let handler = input.read(app).password_handler.clone().unwrap();
            handler.update(app, |handler, cx| {
                assert_eq!(
                    handler
                        .selected_text_range(false, window, cx)
                        .unwrap()
                        .range,
                    7..7
                );
            });
        });
        input.update(visual, |input, cx| input.hide_password(cx));
        visual.run_until_parked();
        visual.dispatch_action(gpui::component::input::Undo);
        visual.run_until_parked();
        input.read_with(visual, |input, _| assert!(input.value().is_empty()));
    }

    #[gpui::test]
    fn masked_kit_input_blocks_copy_and_cut(cx: &mut TestAppContext) {
        cx.update(gpui::init);
        let (input, visual) = cx.add_window_view(|_, cx| TextInput::new("é😀secret", "", true, cx));
        visual.refresh().unwrap();
        visual.run_until_parked();
        visual.update(|window, app| {
            app.write_to_clipboard(ClipboardItem::new_string("sentinel".into()));
            input.read(app).focus_handle().focus(window, app);
        });
        visual.dispatch_action(SelectAll);
        visual.dispatch_action(Copy);
        visual.dispatch_action(Cut);
        visual.run_until_parked();
        input.read_with(visual, |input, _| assert_eq!(input.value(), "é😀secret"));
        visual.update(|_, app| {
            assert_eq!(
                app.read_from_clipboard().unwrap().text().as_deref(),
                Some("sentinel")
            );
        });
    }
}

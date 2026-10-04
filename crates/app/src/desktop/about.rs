use super::{CloseWindow, Dismiss, NextFocus, PreviousFocus, ShowAbout, theme::*};
use gpui::{
    App, Bounds, Context, FocusHandle, SharedString, TitlebarOptions, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, rgb, size,
};

pub(super) struct AboutWindow {
    focus: FocusHandle,
}

impl AboutWindow {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle().tab_stop(true);
        focus.focus(window);
        Self { focus }
    }
}

impl Render for AboutWindow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("about-dalan")
            .debug_selector(|| "about-dalan".into())
            .key_context("About")
            .size_full()
            .flex()
            .flex_col()
            .p(px(24.0))
            .gap(px(10.0))
            .bg(rgb(PANEL))
            .font_family(".SystemUIFont")
            .text_size(px(13.0))
            .text_color(rgb(TEXT))
            .on_action(|_: &Dismiss, window, _| window.remove_window())
            .on_action(|_: &CloseWindow, window, _| window.remove_window())
            .on_action(|_: &NextFocus, window, _| window.focus_next())
            .on_action(|_: &PreviousFocus, window, _| window.focus_prev())
            .child(
                div()
                    .text_size(px(24.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("Dalan"),
            )
            .child(format!("Version {}", env!("CARGO_PKG_VERSION")))
            .child(div().text_color(rgb(MUTED)).child("“Ways” in Javanese."))
            .child(
                div()
                    .text_color(rgb(MUTED))
                    .child("A database workspace built with Rust and GPUI."),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(rgb(MUTED))
                    .child("Development build. MySQL/MariaDB browsing is experimental; ACP agent connections are not implemented."),
            )
            .child(div().flex_1())
            .child(
                div().flex().justify_end().child(
                    div()
                        .id("about-done")
                        .debug_selector(|| "about-done".into())
                        .track_focus(&self.focus)
                        .tab_stop(true)
                        .h(px(28.0))
                        .px(px(14.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.0))
                        .border_1()
                        .border_color(rgb(MUTED))
                        .bg(rgb(CHROME))
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(HOVER)))
                        .focus(|style| style.border_color(rgb(FOCUS)))
                        .on_click(|_, window, _| window.remove_window())
                        .on_key_down(|event, window, cx| {
                            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                cx.stop_propagation();
                                window.remove_window();
                            }
                        })
                        .child("Done"),
                ),
            )
    }
}

pub(super) fn show_about(_: &ShowAbout, cx: &mut App) {
    if let Some(handle) = cx
        .windows()
        .into_iter()
        .find_map(|handle| handle.downcast::<AboutWindow>())
    {
        let _ = handle.update(cx, |_, window, _| window.activate_window());
        return;
    }
    let bounds = Bounds::centered(None, size(px(420.0), px(280.0)), cx);
    if let Err(error) = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some(SharedString::from("About Dalan")),
                ..Default::default()
            }),
            is_resizable: false,
            is_minimizable: false,
            ..Default::default()
        },
        |window, cx| cx.new(|cx| AboutWindow::new(window, cx)),
    ) {
        eprintln!("Could not open About Dalan: {error}");
    }
}

use super::{CloseWindow, Dismiss, NextFocus, PreviousFocus, ShowAbout};
use gpui::{
    AnyWindowHandle, App, Bounds, Context, Entity, FocusHandle, Global, SharedString,
    TitlebarOptions, WeakEntity, Window, WindowBounds, WindowOptions, div, prelude::*, px, size,
};

use gpui::component::{
    ActiveTheme, Sizable,
    button::{Button as KitButton, ButtonVariants},
};
struct AboutSlot {
    window: AnyWindowHandle,
    view: WeakEntity<AboutWindow>,
}
impl Global for AboutSlot {}
pub(super) fn current_view(cx: &App) -> Option<Entity<AboutWindow>> {
    cx.try_global::<AboutSlot>()?.view.upgrade()
}
pub(super) fn current_window(cx: &App) -> Option<AnyWindowHandle> {
    let slot = cx.try_global::<AboutSlot>()?;
    slot.view.upgrade()?;
    cx.windows()
        .into_iter()
        .find(|window| *window == slot.window)
}
pub(super) struct AboutWindow {
    focus: FocusHandle,
}

impl AboutWindow {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle().tab_stop(false);
        focus.focus(window, cx);
        Self { focus }
    }
}

impl Render for AboutWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("about-dalan")
            .debug_selector(|| "about-dalan".into())
            .key_context("About")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .p(px(20.0))
            .gap(px(10.0))
            .bg(cx.theme().background)
            .font_family(".SystemUIFont")
            .text_size(px(13.0))
            .text_color(cx.theme().foreground)
            .on_action(|_: &Dismiss, window, _| window.remove_window())
            .on_action(|_: &CloseWindow, window, _| window.remove_window())
            .on_action(|_: &NextFocus, window, cx| window.focus_next(cx))
            .on_action(|_: &PreviousFocus, window, cx| window.focus_prev(cx))
            .child(
                div()
                    .text_size(px(24.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("Dalan"),
            )
            .child(format!("Version {}", env!("CARGO_PKG_VERSION")))
            .child(div().text_color(cx.theme().muted_foreground).child("“Ways” in Javanese."))
            .child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child("A database workspace built with Rust and GPUI."),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(cx.theme().muted_foreground)
                    .child("Development build. MySQL/MariaDB browsing is experimental; ACP agent connections are not implemented."),
            )
            .child(div().flex_1())
            .child(
                div().flex().justify_end().child(
                    KitButton::new("about-done").debug_selector(|| "about-done".into())
                        .small().primary().label("Done")
                        .on_click(|_, window, _| window.remove_window()),
                ),
            )
    }
}

pub(super) fn show_about(_: &ShowAbout, cx: &mut App) {
    if current_view(cx).is_some()
        && let Some(window) = current_window(cx)
        && cx
            .update_window(window, |_, window, _| window.activate_window())
            .is_ok()
    {
        return;
    }
    let bounds = Bounds::centered(None, size(px(420.0), px(280.0)), cx);
    match gpui::open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_background: gpui::WindowBackgroundAppearance::Opaque,
            titlebar: Some(TitlebarOptions {
                title: Some(SharedString::from("About Dalan")),
                ..Default::default()
            }),
            is_resizable: false,
            is_minimizable: false,
            ..Default::default()
        },
        cx,
        |window, cx| cx.new(|cx| AboutWindow::new(window, cx)),
    ) {
        Ok((window, view)) => cx.set_global(AboutSlot {
            window,
            view: view.downgrade(),
        }),
        Err(error) => eprintln!("Could not open About Dalan: {error}"),
    }
}

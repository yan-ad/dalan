//! Shared destructive confirmations using Kit's modal AlertDialog layer.
use gpui::component::{
    WindowExt,
    button::{Button, ButtonVariants},
    dialog::DialogFooter,
};
use gpui::{App, IntoElement, RenderOnce, SharedString, Window, prelude::*, px};
use std::{cell::Cell, rc::Rc};

type ActionCallback = Rc<dyn Fn(&mut Window, &mut App) -> bool>;

#[derive(Clone)]
pub(super) struct ConfirmAlert {
    pub title: SharedString,
    pub description: SharedString,
    pub confirm_id: &'static str,
    pub confirm_label: &'static str,
    pub cancel_id: &'static str,
    pub cancel_label: &'static str,
}

#[derive(IntoElement)]
struct SafeButton {
    id: &'static str,
    label: &'static str,
    focused: Rc<Cell<bool>>,
    cancel: ActionCallback,
}
impl RenderOnce for SafeButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let handle = window
            .use_keyed_state(self.id, cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        if !self.focused.replace(true) {
            window.defer(cx, move |window, cx| handle.focus(window, cx));
        }
        Button::new(self.id)
            .debug_selector(move || self.id.into())
            .label(self.label)
            .outline()
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                let handle = window.window_handle();
                if (self.cancel)(window, cx) && cx.windows().into_iter().any(|w| w == handle) {
                    window.close_dialog(cx);
                }
            })
            .render(window, cx)
    }
}
/// Repeated shortcut presses do not stack alerts. Escape/cancel invoke only the
/// safe callback; outside clicks never confirm or dismiss a destructive action.
pub(super) fn open(
    window: &mut Window,
    cx: &mut App,
    alert: ConfirmAlert,
    on_confirm: impl Fn(&mut Window, &mut App) -> bool + 'static,
    on_cancel: impl Fn(&mut Window, &mut App) -> bool + 'static,
) -> bool {
    if window.has_active_dialog(cx) {
        return false;
    }
    let focused = Rc::new(Cell::new(false));
    let on_confirm: ActionCallback = Rc::new(on_confirm);
    let on_cancel: ActionCallback = Rc::new(on_cancel);
    window.open_alert_dialog(cx, move |dialog, _, _| {
        let confirm = on_confirm.clone();
        let cancel = on_cancel.clone();
        dialog
            .title(alert.title.clone())
            .description(alert.description.clone())
            .close_button(false)
            .width(px(480.))
            .on_ok(move |_, window, cx| {
                let handle = window.window_handle();
                let res = confirm(window, cx);
                if !cx.windows().into_iter().any(|w| w == handle) {
                    return false;
                }
                res
            })
            .on_cancel(move |_, window, cx| {
                let handle = window.window_handle();
                let res = cancel(window, cx);
                if !cx.windows().into_iter().any(|w| w == handle) {
                    return false;
                }
                res
            })
            .footer(
                DialogFooter::new()
                    .child(SafeButton {
                        id: alert.cancel_id,
                        label: alert.cancel_label,
                        focused: focused.clone(),
                        cancel: on_cancel.clone(),
                    })
                    .child({
                        let confirm = on_confirm.clone();
                        let confirm_id = alert.confirm_id;
                        Button::new(confirm_id)
                            .debug_selector(move || confirm_id.into())
                            .label(alert.confirm_label)
                            .danger()
                            .on_click(move |_, window, cx| {
                                cx.stop_propagation();
                                let handle = window.window_handle();
                                if confirm(window, cx)
                                    && cx.windows().into_iter().any(|w| w == handle)
                                {
                                    let active_focus = window.focused(cx);
                                    window.close_dialog(cx);
                                    if let Some(ref focus) = active_focus {
                                        window.focus(focus, cx);
                                    }
                                }
                            })
                    }),
            )
    });
    true
}

mod about;
mod icons;
mod input;
mod source_browser;
mod source_dialog;
mod source_form;
mod source_model;
mod source_workspace;
mod theme;

use std::collections::HashMap;

use dalan_app::shell_state::{Control, OUTER_PADDING, PANE_GAP, ShellState};
use gpui::{
    App, Application, Bounds, Context, Div, FocusHandle, KeyBinding, Menu, MenuItem, MouseButton,
    SharedString, Stateful, TitlebarOptions, Window, WindowBounds, WindowOptions, actions, div,
    point, prelude::*, px, rgb, size,
};

use icons::{Icon, icon};
use theme::*;

actions!(
    shell,
    [
        NextFocus,
        PreviousFocus,
        Dismiss,
        ToggleDatabase,
        ToggleAcp,
        ShowAbout,
        ResetLayout,
        CloseWindow,
        Quit
    ]
);

const MENU_CONTROLS: [(Control, &str); 4] = [
    (Control::ToggleDatabase, "Database sidebar"),
    (Control::NarrowDatabase, "Narrow database sidebar"),
    (Control::WidenDatabase, "Widen database sidebar"),
    (Control::ResetLayout, "Reset layout"),
];

#[derive(Clone, Copy)]
struct DragState {
    start_x: f32,
    start_width: f32,
}

fn control_label(id: &str) -> &'static str {
    match id {
        "layout-menu" => "Workspace layout",
        "database-toggle" | "toggle-database" => "Toggle database sidebar (Cmd-B)",
        "database-resize" => "Resize database sidebar (Left/Right)",
        "narrow-database" => "Narrow database sidebar",
        "widen-database" => "Widen database sidebar",
        "reset-layout" => "Reset layout (Cmd-Alt-0)",
        "acp-toggle" => "Toggle AI panel (ACP only, Cmd-Shift-A)",
        "acp-close" => "Close AI panel",
        _ => "UI foundation",
    }
}

struct ControlTooltip(&'static str);

impl Render for ControlTooltip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(8.0))
            .py(px(5.0))
            .rounded(px(4.0))
            .border_1()
            .border_color(rgb(MUTED))
            .bg(rgb(CHROME))
            .text_color(rgb(TEXT))
            .text_size(px(12.0))
            .child(self.0)
    }
}

struct Shell {
    state: ShellState,
    root_focus: FocusHandle,
    acp_focus: FocusHandle,
    controls: HashMap<&'static str, FocusHandle>,
    drag: Option<DragState>,
    explorer: gpui::Entity<source_browser::SourceExplorer>,
    workspace: gpui::Entity<source_workspace::SourceWorkspace>,
}

impl Shell {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let root_focus = cx.focus_handle().tab_stop(false);
        root_focus.focus(window);
        let ids = [
            "layout-menu",
            "database-toggle",
            "database-resize",
            "acp-toggle",
            "acp-close",
            "toggle-database",
            "narrow-database",
            "widen-database",
            "reset-layout",
        ];
        let controls = ids
            .into_iter()
            .enumerate()
            .map(|(index, id)| {
                (
                    id,
                    cx.focus_handle()
                        .tab_index(index as isize + 1)
                        .tab_stop(true),
                )
            })
            .collect();
        #[cfg(all(test, feature = "ui-tests"))]
        let model = cx.new(|_| source_model::SourceModel::for_tests(vec![]));
        #[cfg(not(all(test, feature = "ui-tests")))]
        let model = cx.new(source_model::SourceModel::new);
        let explorer = cx.new(|cx| source_browser::SourceExplorer::new(model.clone(), cx));
        let workspace = cx.new(|cx| source_workspace::SourceWorkspace::new(model, cx));
        Self {
            state: ShellState::default(),
            root_focus,
            acp_focus: cx.focus_handle().tab_stop(false),
            controls,
            drag: None,
            explorer,
            workspace,
        }
    }

    fn apply(&mut self, control: Control, window: &mut Window, cx: &mut Context<Self>) {
        self.state.apply(control);
        self.drag = None;
        if self.state.menu_open {
            self.controls["toggle-database"].focus(window);
        } else if control == Control::ToggleAcp {
            if self.state.acp_visible {
                self.controls["acp-close"].focus(window);
            } else {
                self.controls["acp-toggle"].focus(window);
            }
        } else if control == Control::LayoutMenu || control == Control::ResetLayout {
            self.controls["layout-menu"].focus(window);
        } else {
            self.root_focus.focus(window);
        }
        cx.notify();
    }

    fn dismiss(&mut self, _: &Dismiss, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.menu_open {
            self.state.menu_open = false;
            self.controls["layout-menu"].focus(window);
        } else if self.state.acp_visible && self.acp_focus.contains_focused(window, cx) {
            self.state.acp_visible = false;
            self.controls["acp-toggle"].focus(window);
        }
        self.drag = None;
        cx.notify();
    }

    fn move_focus(&mut self, backwards: bool, window: &mut Window) {
        if self.state.menu_open {
            let index = MENU_CONTROLS
                .iter()
                .position(|(control, _)| self.controls[control.id()].is_focused(window));
            let count = MENU_CONTROLS.len();
            let next = match index {
                Some(index) if backwards => (index + count - 1) % count,
                Some(index) => (index + 1) % count,
                None if backwards => count - 1,
                None => 0,
            };
            self.controls[MENU_CONTROLS[next].0.id()].focus(window);
        } else if backwards {
            window.focus_prev();
        } else {
            window.focus_next();
        }
    }

    fn button(
        &self,
        id: &'static str,
        control: Control,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        div()
            .id(id)
            .debug_selector(|| id.to_owned())
            .track_focus(&self.controls[id])
            .tab_stop(true)
            .flex()
            .items_center()
            .justify_center()
            .h(px(28.0))
            .min_w(px(28.0))
            .rounded(px(CONTROL_RADIUS))
            .border_1()
            .border_color(rgb(if selected { SELECTION } else { CHROME }))
            .bg(rgb(if selected { SELECTION } else { CHROME }))
            .text_color(rgb(if selected { FOCUS } else { TEXT }))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(HOVER)))
            .focus(|style| style.border_color(rgb(FOCUS)))
            .tooltip(move |_, cx| cx.new(|_| ControlTooltip(control_label(id))).into())
            .on_click(cx.listener(move |this, _, window, cx| this.apply(control, window, cx)))
            .on_key_down(
                cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        cx.stop_propagation();
                        this.apply(control, window, cx);
                    }
                }),
            )
    }

    fn titlebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("titlebar")
            .debug_selector(|| "titlebar".into())
            .h(px(TITLEBAR_HEIGHT))
            .flex_shrink_0()
            .flex()
            .items_center()
            .bg(rgb(CHROME))
            .child(div().w(px(84.0)).h_full().flex_shrink_0())
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .window_control_area(gpui::WindowControlArea::Drag)
                    .on_mouse_down(MouseButton::Left, |event, window, _| {
                        if event.click_count == 2 {
                            window.zoom_window();
                        } else {
                            window.start_window_move();
                        }
                    })
                    .child(
                        div()
                            .id("product-name")
                            .debug_selector(|| "product-name".into())
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Dalan"),
                    )
                    .child(div().text_color(rgb(MUTED)).child("Database workspace")),
            )
            .child(
                self.button("layout-menu", Control::LayoutMenu, self.state.menu_open, cx)
                    .w(px(96.0))
                    .gap(px(6.0))
                    .child(icon(Icon::Layout, MUTED))
                    .child("Layout")
                    .child(icon(Icon::Chevron, MUTED)),
            )
            .child(div().w(px(8.0)))
    }

    fn sidebar(&self, width: f32) -> impl IntoElement {
        div()
            .id("database-pane")
            .debug_selector(|| "database-pane".into())
            .w(px(width))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(PANE_RADIUS))
            .bg(rgb(PANEL))
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_hidden()
                    .child(self.explorer.clone()),
            )
    }

    fn separator(&self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let id = "database-resize";
        div()
            .id(id)
            .debug_selector(|| id.into())
            .track_focus(&self.controls[id])
            .tab_stop(true)
            .w(px(PANE_GAP))
            .h_full()
            .flex_shrink_0()
            .flex()
            .justify_center()
            .bg(rgb(CHROME))
            .cursor_col_resize()
            .hover(|style| style.bg(rgb(FOCUS)))
            .focus(|style| style.bg(rgb(FOCUS)))
            .tooltip(move |_, cx| cx.new(|_| ControlTooltip(control_label(id))).into())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                    this.controls[id].focus(window);
                    this.drag = Some(DragState {
                        start_x: event.position.x.into(),
                        start_width: width,
                    });
                    cx.stop_propagation();
                }),
            )
            .on_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, _, cx| {
                let delta = match event.keystroke.key.as_str() {
                    "left" => -16.0,
                    "right" => 16.0,
                    _ => return,
                };
                this.state.resize(this.state.requested_width() + delta);
                cx.stop_propagation();
                cx.notify();
            }))
            .child(div().w(px(1.0)).h_full().bg(rgb(BORDER)))
    }

    fn layout_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("layout-popover")
            .debug_selector(|| "layout-popover".into())
            .absolute()
            .top(px(TITLEBAR_HEIGHT + 2.0))
            .right(px(8.0))
            .w(px(260.0))
            .p(px(6.0))
            .flex()
            .flex_col()
            .gap(px(2.0))
            .rounded(px(CONTROL_RADIUS))
            .border_1()
            .border_color(rgb(BORDER))
            .bg(rgb(CHROME))
            .occlude()
            .on_mouse_down_out(
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    // The trigger handles its own toggle on click; closing it on
                    // mouse-down would reopen the popover on the following click.
                    if event.position.y < px(TITLEBAR_HEIGHT)
                        && event.position.x > window.viewport_size().width - px(104.0)
                    {
                        return;
                    }
                    this.state.menu_open = false;
                    this.controls["layout-menu"].focus(window);
                    cx.notify();
                }),
            )
            .children(MENU_CONTROLS.into_iter().map(|(control, label)| {
                let suffix = match control {
                    Control::ToggleDatabase if self.state.database_visible => "Shown",
                    Control::ToggleDatabase => "Hidden",
                    _ => "",
                };
                self.button(control.id(), control, false, cx)
                    .h(px(CONTROL_HEIGHT))
                    .w_full()
                    .px(px(8.0))
                    .justify_between()
                    .child(label)
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgb(MUTED))
                            .child(suffix),
                    )
            }))
    }

    fn acp_panel(&self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        div().id("acp-panel").debug_selector(|| "acp-panel".into())
            .track_focus(&self.acp_focus).tab_stop(false)
            .w(px(width)).h_full().flex_shrink_0().flex().flex_col()
            .overflow_hidden().rounded(px(PANE_RADIUS)).bg(rgb(PANEL))
            .child(div().h(px(PANEL_HEADER_HEIGHT)).flex_shrink_0()
                .flex().items_center().justify_between().pl(px(10.0)).pr(px(4.0)).bg(rgb(HEADER))
                .child(div().font_weight(gpui::FontWeight::MEDIUM).child("AI · ACP"))
                .child(self.button("acp-close", Control::ToggleAcp, false, cx)
                    .child(icon(Icon::Hide, MUTED))))
            .child(div().id("acp-empty").debug_selector(|| "acp-empty".into())
                .flex_1().min_h(px(0.0)).overflow_y_scroll().p(px(12.0))
                .flex().flex_col().gap(px(8.0))
                .child("Not connected")
                .child(div().text_size(px(12.0)).text_color(rgb(MUTED))
                    .child("Agent Client Protocol connections are not implemented yet."))
                .child(div().text_size(px(12.0)).text_color(rgb(MUTED))
                    .child("Dalan connects to external ACP agents. Agents manage their own login and billing; no provider API keys are stored here.")))
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = self.state.layout(window.viewport_size().width.into());
        let focus_help = self
            .controls
            .iter()
            .find(|(_, handle)| handle.is_focused(window))
            .map_or(NAME, |(id, _)| control_label(id));
        div()
            .id("shell")
            .track_focus(&self.root_focus)
            .key_context("Shell")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(CHROME))
            .text_color(rgb(TEXT))
            .font_family(".SystemUIFont")
            .text_size(px(13.0))
            .on_action(cx.listener(|this, _: &NextFocus, window, _| this.move_focus(false, window)))
            .on_action(
                cx.listener(|this, _: &PreviousFocus, window, _| this.move_focus(true, window)),
            )
            .on_action(cx.listener(Self::dismiss))
            .on_action(cx.listener(|this, _: &ToggleDatabase, window, cx| {
                this.apply(Control::ToggleDatabase, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ToggleAcp, window, cx| {
                this.apply(Control::ToggleAcp, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ResetLayout, window, cx| {
                this.apply(Control::ResetLayout, window, cx)
            }))
            .on_action(|_: &CloseWindow, window, _| window.remove_window())
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                if let Some(drag) = this.drag {
                    if !event.dragging() {
                        this.drag = None;
                        return;
                    }
                    let x: f32 = event.position.x.into();
                    this.state.resize(drag.start_width + x - drag.start_x);
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.drag = None),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.drag = None),
            )
            .child(self.titlebar(cx))
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .flex()
                    .pb(px(0.0))
                    .px(px(OUTER_PADDING))
                    .when_some(layout.database, |body, width| {
                        body.child(self.sidebar(width))
                            .child(self.separator(width, cx))
                    })
                    .child(
                        div()
                            .id("main-content")
                            .debug_selector(|| "main-content".into())
                            .flex_1()
                            .min_w(px(0.0))
                            .h_full()
                            .rounded(px(PANE_RADIUS))
                            .bg(rgb(BACKGROUND))
                            .overflow_hidden()
                            .child(self.workspace.clone()),
                    )
                    .when_some(layout.acp, |body, width| {
                        body.child(
                            div()
                                .w(px(PANE_GAP))
                                .h_full()
                                .flex_shrink_0()
                                .flex()
                                .justify_center()
                                .child(div().w(px(1.0)).h_full().bg(rgb(BORDER))),
                        )
                        .child(self.acp_panel(width, cx))
                    }),
            )
            .child(
                div()
                    .id("shell-status")
                    .debug_selector(|| "shell-status".into())
                    .h(px(STATUS_HEIGHT))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(12.0))
                    .text_size(px(11.0))
                    .text_color(rgb(MUTED))
                    .gap(px(8.0))
                    .child(
                        self.button(
                            "database-toggle",
                            Control::ToggleDatabase,
                            layout.database.is_some(),
                            cx,
                        )
                        .w(px(28.0))
                        .child(icon(Icon::Layout, TEXT)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .overflow_hidden()
                            .child(focus_help),
                    )
                    .child(
                        self.button("acp-toggle", Control::ToggleAcp, self.state.acp_visible, cx)
                            .w(px(28.0))
                            .child(
                                div()
                                    .id("acp-ai-icon")
                                    .debug_selector(|| "acp-ai-icon".into())
                                    .child(icon(Icon::Ai, TEXT)),
                            ),
                    ),
            )
            .when(self.state.menu_open, |root| {
                root.child(self.layout_menu(cx))
            })
    }
}

fn bind_keys(cx: &mut App) {
    input::bind_keys(cx);
    cx.bind_keys([
        KeyBinding::new("tab", NextFocus, Some("Shell")),
        KeyBinding::new("shift-tab", PreviousFocus, Some("Shell")),
        KeyBinding::new("escape", Dismiss, Some("Shell")),
        KeyBinding::new("cmd-b", ToggleDatabase, Some("Shell")),
        KeyBinding::new("cmd-shift-a", ToggleAcp, Some("Shell")),
        KeyBinding::new("cmd-alt-0", ResetLayout, Some("Shell")),
        KeyBinding::new("cmd-w", CloseWindow, Some("Shell")),
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("tab", NextFocus, Some("About")),
        KeyBinding::new("shift-tab", PreviousFocus, Some("About")),
        KeyBinding::new("escape", Dismiss, Some("About")),
        KeyBinding::new("cmd-w", CloseWindow, Some("About")),
        KeyBinding::new("tab", NextFocus, Some("SourceForm")),
        KeyBinding::new("shift-tab", PreviousFocus, Some("SourceForm")),
        KeyBinding::new("escape", Dismiss, Some("SourceForm")),
        KeyBinding::new("cmd-w", CloseWindow, Some("SourceDialog")),
    ]);
}

pub fn run() {
    Application::new()
        .with_assets(icons::IconAssets)
        .run(|cx: &mut App| {
            bind_keys(cx);
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.on_action(about::show_about);
            cx.on_window_closed(|cx| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            cx.set_menus(vec![
                Menu {
                    name: "Dalan".into(),
                    items: vec![
                        MenuItem::action("About Dalan", ShowAbout),
                        MenuItem::separator(),
                        MenuItem::action("Quit Dalan", Quit),
                    ],
                },
                Menu {
                    name: "View".into(),
                    items: vec![
                        MenuItem::action("Toggle Database Sidebar", ToggleDatabase),
                        MenuItem::action("Toggle AI Panel (ACP)", ToggleAcp),
                        MenuItem::action("Reset Layout", ResetLayout),
                    ],
                },
            ]);
            let bounds = Bounds::centered(None, size(px(1280.0), px(800.0)), cx);
            if let Err(error) = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(720.0), px(480.0))),
                    window_background: gpui::WindowBackgroundAppearance::Opaque,
                    titlebar: Some(TitlebarOptions {
                        title: Some(SharedString::from("Dalan")),
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(12.0), px(12.0))),
                    }),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| Shell::new(window, cx)),
            ) {
                eprintln!("Could not open Dalan window: {error}");
                cx.quit();
                return;
            }
            cx.activate(true);
        });
}

#[cfg(all(test, feature = "ui-tests"))]
mod tests;

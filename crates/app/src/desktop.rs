mod about;
mod data_grid;
mod icons;
mod input;
mod query_console;
mod source_browser;
mod source_dialog;
mod source_form;
mod source_model;
mod source_workspace;
mod sql_editor;
mod ssh_manager;
mod theme;

use std::collections::HashMap;

use dalan_app::shell_state::{Control, OUTER_PADDING, PANE_GAP, ShellState};
use gpui::{
    App, Bounds, Context, FocusHandle, KeyBinding, Menu, MenuItem, MouseButton, SharedString,
    TitlebarOptions, Window, WindowBounds, WindowOptions, actions, div, point, prelude::*, px,
    size,
};

use gpui::component::{
    Disableable, Icon as KitIcon, Selectable, Sizable,
    button::{Button as KitButton, ButtonVariants},
    menu::{DropdownMenu, PopupMenuItem},
};
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        gpui::component::tooltip::Tooltip::new(self.0).build(window, cx)
    }
}

// Capture the actual keyed focus state used by Kit, rather than attaching a
// second focus handle to the button's outer styling div.
#[derive(IntoElement)]
struct ShellButton {
    id: &'static str,
    button: KitButton,
    owner: gpui::WeakEntity<Shell>,
}

impl RenderOnce for ShellButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let handle = window
            .use_keyed_state(self.id, cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        let _ = self.owner.update(cx, |shell, cx| {
            shell.controls.insert(self.id, handle.clone());
            if shell.pending_focus_control == Some(self.id) {
                shell.pending_focus_control = None;
                handle.focus(window, cx);
            }
        });
        self.button.render(window, cx)
    }
}

struct Shell {
    state: ShellState,
    root_focus: FocusHandle,
    acp_focus: FocusHandle,
    controls: HashMap<&'static str, FocusHandle>,
    pending_focus_control: Option<&'static str>,
    drag: Option<DragState>,
    explorer: gpui::Entity<source_browser::SourceExplorer>,
    workspace: gpui::Entity<source_workspace::SourceWorkspace>,
    sources: gpui::Entity<source_model::SourceModel>,
    _source_subscription: gpui::Subscription,
}

impl Shell {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let root_focus = cx.focus_handle().tab_stop(false);
        root_focus.focus(window, cx);
        let controls = HashMap::from([("database-resize", cx.focus_handle().tab_stop(true))]);
        #[cfg(all(test, feature = "ui-tests"))]
        let model = cx.new(|_| source_model::SourceModel::for_tests(vec![]));
        #[cfg(not(all(test, feature = "ui-tests")))]
        let model = cx.new(source_model::SourceModel::new);
        let explorer = cx.new(|cx| source_browser::SourceExplorer::new(model.clone(), cx));
        let workspace = cx.new(|cx| source_workspace::SourceWorkspace::new(model.clone(), cx));
        let source_subscription = cx.observe(&model, |_, _, cx| cx.notify());
        Self {
            state: ShellState::default(),
            root_focus,
            acp_focus: cx.focus_handle().tab_stop(false),
            controls,
            pending_focus_control: None,
            drag: None,
            explorer,
            workspace,
            sources: model,
            _source_subscription: source_subscription,
        }
    }

    fn apply(&mut self, control: Control, window: &mut Window, cx: &mut Context<Self>) {
        self.state.apply(control);
        self.drag = None;
        if control == Control::ToggleAcp {
            let id = if self.state.acp_visible {
                "acp-close"
            } else {
                "acp-toggle"
            };
            // The close control only exists after the newly opened panel renders.
            // Resolve focus as it mounts, rather than relying on another frame.
            self.pending_focus_control = Some(id);
        } else if control != Control::LayoutMenu {
            self.root_focus.focus(window, cx);
        }
        cx.notify();
    }

    fn dismiss(&mut self, _: &Dismiss, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.acp_visible && self.acp_focus.contains_focused(window, cx) {
            self.state.acp_visible = false;
            if let Some(handle) = self.controls.get("acp-toggle") {
                handle.focus(window, cx);
            }
        }
        self.drag = None;
        cx.notify();
    }

    fn move_focus(&mut self, backwards: bool, window: &mut Window, cx: &mut Context<Self>) {
        if backwards {
            window.focus_prev(cx);
        } else {
            window.focus_next(cx);
        }
    }

    fn kit_button(&self, id: &'static str) -> KitButton {
        KitButton::new(id)
            .small()
            .ghost()
            .debug_selector(move || id.into())
            .tooltip(control_label(id))
    }

    fn tracked_button(
        &self,
        id: &'static str,
        button: KitButton,
        cx: &Context<Self>,
    ) -> ShellButton {
        ShellButton {
            id,
            button,
            owner: cx.entity().downgrade(),
        }
    }

    fn button(
        &self,
        id: &'static str,
        control: Control,
        selected: bool,
        icon: &'static str,
        cx: &mut Context<Self>,
    ) -> ShellButton {
        let button = self
            .kit_button(id)
            .selected(selected)
            .icon(KitIcon::empty().path(icon))
            .w(px(CONTROL_HEIGHT))
            .when(id == "acp-toggle", |button| {
                button.child(
                    div()
                        .id("acp-ai-icon")
                        .debug_selector(|| "acp-ai-icon".into()),
                )
            })
            .on_click(cx.listener(move |this, _, window, cx| this.apply(control, window, cx)));
        self.tracked_button(id, button, cx)
    }

    fn titlebar(&self, database_visible: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let disabled = self.sources.read(cx).saving
            || (self.sources.read(cx).busy && self.sources.read(cx).profiles.is_empty());
        div()
            .id("titlebar")
            .debug_selector(|| "titlebar".into())
            .h(px(TITLEBAR_HEIGHT))
            .flex_shrink_0()
            .flex()
            .items_center()
            .bg(colors(cx).chrome)
            .child(div().w(px(84.0)).h_full().flex_shrink_0())
            .child(self.button(
                "database-toggle",
                Control::ToggleDatabase,
                database_visible,
                "icons/database.svg",
                cx,
            ))
            .child(
                self.tracked_button(
                    "new-connection",
                    self.kit_button("new-connection")
                        .label("New Connection")
                        .icon(KitIcon::empty().path("icons/plus.svg"))
                        .disabled(disabled)
                        .tooltip("Create a new database connection")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.sources.update(cx, |model, cx| model.new_source(cx));
                        })),
                    cx,
                ),
            )
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
                    }),
            )
            .child(self.layout_menu(cx))
            .child(div().w(px(8.0)))
    }

    fn sidebar(&self, width: f32, cx: &Context<Self>) -> impl IntoElement {
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
            .bg(colors(cx).panel)
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
            .bg(colors(cx).chrome)
            .cursor_col_resize()
            .hover(|style| style.bg(colors(cx).focus))
            .focus(|style| style.bg(colors(cx).focus))
            .tooltip(move |window, cx| {
                gpui::component::tooltip::Tooltip::new(control_label(id)).build(window, cx)
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                    this.controls[id].focus(window, cx);
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
            .child(div().w(px(1.0)).h_full().bg(colors(cx).border))
    }

    fn layout_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let observer = owner.clone();
        self.kit_button("layout-menu")
            .icon(KitIcon::empty().path("icons/panel-left.svg"))
            .w(px(CONTROL_HEIGHT))
            .dropdown_menu_with_anchor(gpui::Anchor::TopRight, move |mut menu, _, cx| {
                let shown = owner
                    .upgrade()
                    .is_some_and(|shell| shell.read(cx).state.database_visible);
                for (control, label) in MENU_CONTROLS {
                    let owner = owner.clone();
                    menu = menu.item(
                        PopupMenuItem::new(label)
                            .checked(control == Control::ToggleDatabase && shown)
                            .on_click(move |_, window, cx| {
                                let _ =
                                    owner.update(cx, |this, cx| this.apply(control, window, cx));
                            }),
                    );
                }
                menu
            })
            .on_open_change(move |open, _, cx| {
                let _ = observer.update(cx, |this, cx| {
                    this.state.menu_open = *open;
                    cx.notify();
                });
            })
    }

    fn acp_panel(&self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        div().id("acp-panel").debug_selector(|| "acp-panel".into())
            .track_focus(&self.acp_focus).tab_stop(false)
            .w(px(width)).h_full().flex_shrink_0().flex().flex_col()
            .overflow_hidden().rounded(px(PANE_RADIUS)).bg(colors(cx).panel)
            .child(div().h(px(PANEL_HEADER_HEIGHT)).flex_shrink_0()
                .flex().items_center().justify_between().pl(px(10.0)).pr(px(4.0)).bg(colors(cx).header)
                .child(div().font_weight(gpui::FontWeight::MEDIUM).child("AI · ACP"))
                .child(self.button("acp-close", Control::ToggleAcp, false, "icons/minus.svg", cx)))
            .child(div().id("acp-empty").debug_selector(|| "acp-empty".into())
                .flex_1().min_h(px(0.0)).overflow_y_scroll().p(px(12.0))
                .flex().flex_col().gap(px(8.0))
                .child("Not connected")
                .child(div().text_size(px(12.0)).text_color(colors(cx).muted)
                    .child("Agent Client Protocol connections are not implemented yet."))
                .child(div().text_size(px(12.0)).text_color(colors(cx).muted)
                    .child("Dalan connects to external ACP agents. Agents manage their own login and billing; no provider API keys are stored here.")))
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = self.state.layout(window.viewport_size().width.into());
        div()
            .id("shell")
            .track_focus(&self.root_focus)
            .key_context("Shell")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(colors(cx).chrome)
            .text_color(colors(cx).text)
            .font_family(".SystemUIFont")
            .text_size(px(13.0))
            .on_action(
                cx.listener(|this, _: &NextFocus, window, cx| this.move_focus(false, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &PreviousFocus, window, cx| {
                    this.move_focus(true, window, cx)
                }),
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
            .on_action(
                cx.listener(|this, _: &source_workspace::NewConsole, _, cx| {
                    this.workspace
                        .update(cx, |workspace, cx| workspace.new_console(cx));
                }),
            )
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
            .child(self.titlebar(layout.database.is_some(), cx))
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .flex()
                    .pb(px(0.0))
                    .px(px(OUTER_PADDING))
                    .when_some(layout.database, |body, width| {
                        body.child(self.sidebar(width, cx))
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
                            .bg(colors(cx).background)
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
                                .child(div().w(px(1.0)).h_full().bg(colors(cx).border)),
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
                    .text_color(colors(cx).muted)
                    .gap(px(8.0))
                    .child(
                        div()
                            .id("status-theme-hint")
                            .flex_1()
                            .min_w(px(0.0))
                            .overflow_hidden()
                            .tooltip(|window, cx| {
                                gpui::component::tooltip::Tooltip::new(
                                    "Appearance follows the system theme",
                                )
                                .build(window, cx)
                            }),
                    )
                    .child(self.button(
                        "acp-toggle",
                        Control::ToggleAcp,
                        self.state.acp_visible,
                        "icons/bot-message-square.svg",
                        cx,
                    )),
            )
    }
}

fn bind_keys(cx: &mut App) {
    input::bind_keys(cx);
    sql_editor::bind_keys(cx);
    query_console::bind_keys(cx);
    source_workspace::bind_keys(cx);
    ssh_manager::bind_keys(cx);
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
    gpui::application()
        .with_assets(icons::IconAssets)
        .run(|cx: &mut App| {
            gpui::init(cx);
            bind_keys(cx);
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.on_action(about::show_about);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            cx.set_menus(vec![
                Menu {
                    name: "Dalan".into(),
                    disabled: false,
                    items: vec![
                        MenuItem::action("About Dalan", ShowAbout),
                        MenuItem::separator(),
                        MenuItem::action("Quit Dalan", Quit),
                    ],
                },
                Menu {
                    name: "View".into(),
                    disabled: false,
                    items: vec![
                        MenuItem::action("Toggle Database Sidebar", ToggleDatabase),
                        MenuItem::action("Toggle AI Panel (ACP)", ToggleAcp),
                        MenuItem::action("Reset Layout", ResetLayout),
                    ],
                },
            ]);
            let bounds = Bounds::centered(None, size(px(1280.0), px(800.0)), cx);
            if let Err(error) = gpui::open_window(
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
                cx,
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

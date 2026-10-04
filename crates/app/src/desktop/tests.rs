use super::*;
use gpui::{Modifiers, TestAppContext, VisualTestContext};

fn state(view: &gpui::Entity<Shell>, cx: &VisualTestContext) -> ShellState {
    view.read_with(cx, |shell, _| shell.state.clone())
}

#[gpui::test]
fn center_connect_action_works_with_explorer_hidden(cx: &mut TestAppContext) {
    let (view, cx) = fixture(cx);
    click(cx, "database-toggle");
    assert!(!state(&view, cx).database_visible);
    let content = cx.debug_bounds("main-content").unwrap();
    let action = cx.debug_bounds("connect-empty-source").unwrap();
    assert!((f32::from(action.center().x) - f32::from(content.center().x)).abs() < 2.0);
    click(cx, "connect-empty-source");
    let mut dialog = source_dialog_context(cx);
    assert!(dialog.debug_bounds("source-name").is_some());
    assert!(dialog.debug_bounds("source-test").is_some());
    dialog.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(!state(&view, cx).database_visible);
    assert!(cx.debug_bounds("connect-empty-source").is_some());
}

#[gpui::test]
fn add_source_opens_real_form_and_cancel_returns_browser(cx: &mut TestAppContext) {
    let (_, cx) = fixture(cx);
    click(cx, "add-source");
    assert!(
        cx.debug_bounds("source-form").is_none(),
        "Form must not replace the main workspace"
    );
    let mut dialog = source_dialog_context(cx);
    assert!(dialog.debug_bounds("source-form").is_some());
    assert!(dialog.debug_bounds("source-host").is_some());
    assert!(dialog.debug_bounds("source-test").is_some());
    dialog.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(cx.debug_bounds("source-browser").is_some());
    assert_eq!(cx.cx.read(|app| app.windows().len()), 1);
    click(cx, "add-source");
    let mut dialog = source_dialog_context(cx);
    click(&mut dialog, "source-cancel");
    cx.run_until_parked();
    assert_eq!(cx.cx.read(|app| app.windows().len()), 1);
}

fn source_dialog_context(cx: &VisualTestContext) -> VisualTestContext {
    let handle = cx.cx.read(|app| {
        app.windows()
            .into_iter()
            .find(|handle| handle.downcast::<source_dialog::SourceDialog>().is_some())
            .expect("source dialog opened")
    });
    let dialog = VisualTestContext::from_window(handle, &cx.cx);
    dialog.run_until_parked();
    dialog
}

#[gpui::test]
fn acp_button_is_bottom_right_and_panel_close_restores_focus(cx: &mut TestAppContext) {
    let (view, cx) = fixture(cx);
    assert!(!state(&view, cx).acp_visible);
    let trigger = cx.debug_bounds("acp-toggle").unwrap();
    assert_eq!(trigger.size.width, px(28.0));
    assert!(cx.debug_bounds("acp-ai-icon").is_some());
    assert!(trigger.origin.x > px(1_100.0));
    assert!(trigger.origin.y >= px(768.0));
    click(cx, "acp-toggle");
    assert!(state(&view, cx).acp_visible);
    assert!(cx.debug_bounds("acp-empty").is_some());
    assert!(cx.update(|window, app| view.read(app).controls["acp-close"].is_focused(window)));
    click(cx, "acp-close");
    assert!(!state(&view, cx).acp_visible);
    assert!(cx.update(|window, app| view.read(app).controls["acp-toggle"].is_focused(window)));
    cx.simulate_keystrokes("space");
    assert!(state(&view, cx).acp_visible);
    assert!(
        cx.update(|window, app| view.read(app).controls["acp-close"].is_focused(window)),
        "Space should focus panel close"
    );
    assert!(
        cx.update(|window, app| view.read(app).acp_focus.contains_focused(window, app)),
        "Panel should contain focus"
    );
    cx.simulate_keystrokes("escape");
    assert!(!state(&view, cx).acp_visible);
    cx.simulate_keystrokes("enter cmd-shift-a");
    assert!(!state(&view, cx).acp_visible);
}

#[gpui::test]
fn acp_compact_layout_keeps_content_and_restores_database(cx: &mut TestAppContext) {
    let (view, cx) = fixture(cx);
    cx.simulate_keystrokes("cmd-shift-a");
    cx.simulate_resize(size(px(720.0), px(480.0)));
    cx.refresh().unwrap();
    cx.run_until_parked();
    assert_eq!(state(&view, cx).layout(720.0).database, None);
    assert!(state(&view, cx).database_visible);
    assert_eq!(cx.debug_bounds("acp-panel").unwrap().size.width, px(300.0));
    assert!(cx.debug_bounds("main-content").unwrap().size.width >= px(240.0));
    assert!(cx.debug_bounds("acp-toggle").unwrap().origin.y >= px(448.0));
    cx.update(|window, app| view.read(app).root_focus.focus(window));
    cx.simulate_keystrokes("escape");
    assert!(
        state(&view, cx).acp_visible,
        "Escape outside the panel must not close it"
    );
    click(cx, "acp-close");
    assert_eq!(
        cx.debug_bounds("database-pane").unwrap().size.width,
        px(320.0)
    );
}

#[gpui::test]
fn about_action_opens_one_window_and_escape_closes(cx: &mut TestAppContext) {
    cx.update(|app| {
        bind_keys(app);
        app.on_action(about::show_about);
        app.dispatch_action(&ShowAbout);
    });
    cx.run_until_parked();
    let handle = cx.read(|app| app.windows()[0].downcast::<about::AboutWindow>().unwrap());
    assert_eq!(cx.read(|app| app.windows().len()), 1);
    cx.update(|app| app.dispatch_action(&ShowAbout));
    cx.run_until_parked();
    assert_eq!(cx.read(|app| app.windows().len()), 1);
    handle
        .update(cx, |_, window, app| {
            assert_eq!(window.viewport_size(), size(px(420.0), px(280.0)));
            window.dispatch_action(Box::new(Dismiss), app);
        })
        .unwrap();
    cx.run_until_parked();
    assert!(cx.read(|app| app.windows().is_empty()));
}

#[gpui::test]
fn about_done_button_and_close_shortcut_work(cx: &mut TestAppContext) {
    cx.update(bind_keys);
    let (_, visual) = cx.add_window_view(about::AboutWindow::new);
    visual.simulate_resize(size(px(420.0), px(280.0)));
    visual.refresh().unwrap();
    visual.run_until_parked();
    assert!(visual.debug_bounds("about-dalan").is_some());
    click(visual, "about-done");
    assert!(visual.cx.read(|app| app.windows().is_empty()));
    let (_, visual) = cx.add_window_view(about::AboutWindow::new);
    visual.simulate_keystrokes("cmd-w");
    assert!(visual.cx.read(|app| app.windows().is_empty()));
}

#[gpui::test]
fn tab_order_reaches_every_visible_control_in_both_directions(cx: &mut TestAppContext) {
    let (view, cx) = fixture(cx);
    let order = [
        "layout-menu",
        "database-toggle",
        "database-resize",
        "acp-toggle",
    ];
    for id in order {
        cx.simulate_keystrokes("tab");
        assert!(
            cx.update(|window, app| view.read(app).controls[id].is_focused(window)),
            "{id}"
        );
    }
    for id in order.into_iter().rev().skip(1) {
        cx.simulate_keystrokes("shift-tab");
        assert!(
            cx.update(|window, app| view.read(app).controls[id].is_focused(window)),
            "{id}"
        );
    }
}

#[gpui::test]
fn close_shortcut_removes_window(cx: &mut TestAppContext) {
    let (_, cx) = fixture(cx);
    cx.simulate_keystrokes("cmd-w");
    assert!(cx.cx.read(|app| app.windows().is_empty()));
}

fn fixture(cx: &mut TestAppContext) -> (gpui::Entity<Shell>, &mut VisualTestContext) {
    cx.update(bind_keys);
    let (view, cx) = cx.add_window_view(Shell::new);
    cx.simulate_resize(size(px(1280.0), px(800.0)));
    cx.refresh().unwrap();
    cx.run_until_parked();
    (view, cx)
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    cx.run_until_parked();
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("missing {selector}"));
    cx.simulate_click(bounds.center(), Modifiers::default());
    cx.run_until_parked();
}

#[gpui::test]
fn titlebar_database_toggle_replaces_brand_without_duplicate_controls(cx: &mut TestAppContext) {
    let (view, cx) = fixture(cx);
    click(cx, "database-toggle");
    assert!(!state(&view, cx).database_visible);
    click(cx, "database-toggle");
    let button = cx.debug_bounds("database-toggle").unwrap();
    assert_eq!(button.origin.x, px(84.0));
    assert!(button.origin.y < px(TITLEBAR_HEIGHT));
    assert_eq!(button.size.width, px(CONTROL_HEIGHT));
    assert!(cx.debug_bounds("product-name").is_none());
    click(cx, "database-toggle");
    assert!(!state(&view, cx).database_visible);
    for id in [
        "database-rail",
        "hide-database",
        "files-rail",
        "files-pane",
        "files-resize",
        "hide-files",
    ] {
        assert!(!view.read_with(cx, |shell, _| shell.controls.contains_key(id)));
        assert!(cx.debug_bounds(id).is_none());
    }
}

#[gpui::test]
fn layout_menu_controls_work_and_restore_defaults(cx: &mut TestAppContext) {
    let (view, cx) = fixture(cx);
    for control in [Control::NarrowDatabase, Control::WidenDatabase] {
        let before = state(&view, cx);
        let mut expected = before;
        expected.apply(control);
        click(cx, "layout-menu");
        assert!(state(&view, cx).menu_open);
        click(cx, control.id());
        assert_eq!(state(&view, cx), expected);
    }
    click(cx, "layout-menu");
    click(cx, "toggle-database");
    click(cx, "layout-menu");
    click(cx, "reset-layout");
    assert_eq!(state(&view, cx), ShellState::default());
}

#[gpui::test]
fn menu_trigger_escape_and_outside_click_dismiss(cx: &mut TestAppContext) {
    let (view, cx) = fixture(cx);
    click(cx, "layout-menu");
    click(cx, "layout-menu");
    assert!(!state(&view, cx).menu_open);
    click(cx, "layout-menu");
    cx.simulate_keystrokes("escape");
    assert!(!state(&view, cx).menu_open);
    click(cx, "layout-menu");
    click(cx, "main-content");
    assert!(!state(&view, cx).menu_open);
}

#[gpui::test]
fn keyboard_activation_menu_focus_trap_and_shortcuts(cx: &mut TestAppContext) {
    let (view, cx) = fixture(cx);
    cx.simulate_keystrokes("tab");
    assert!(cx.update(|window, app| view.read(app).controls["layout-menu"].is_focused(window)));
    cx.simulate_keystrokes("enter");
    assert!(state(&view, cx).menu_open);
    assert!(cx.update(|window, app| view.read(app).controls["toggle-database"].is_focused(window)));
    cx.simulate_keystrokes("shift-tab");
    assert!(cx.update(|window, app| view.read(app).controls["reset-layout"].is_focused(window)));
    cx.simulate_keystrokes("tab space");
    assert!(!state(&view, cx).database_visible);
    assert!(!state(&view, cx).menu_open);
    cx.simulate_keystrokes("cmd-b");
    assert!(state(&view, cx).database_visible);
    cx.simulate_keystrokes("cmd-alt-0");
    assert_eq!(state(&view, cx), ShellState::default());
}

#[gpui::test]
fn separator_drag_keyboard_and_compact_resize(cx: &mut TestAppContext) {
    let (view, cx) = fixture(cx);
    let bounds = cx.debug_bounds("database-resize").unwrap();
    let start = bounds.center();
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(
        start + point(px(64.0), px(0.0)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_up(
        start + point(px(64.0), px(0.0)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.run_until_parked();
    assert_eq!(state(&view, cx).requested_width(), 384.0);
    cx.simulate_keystrokes("left");
    assert_eq!(state(&view, cx).requested_width(), 368.0);
    cx.simulate_keystrokes("cmd-alt-0");
    click(cx, "layout-menu");
    click(cx, "widen-database");
    click(cx, "layout-menu");
    click(cx, "widen-database");
    click(cx, "layout-menu");
    click(cx, "widen-database");
    click(cx, "layout-menu");
    click(cx, "widen-database");
    click(cx, "layout-menu");
    click(cx, "widen-database");
    assert_eq!(state(&view, cx).requested_width(), 480.0);
    cx.simulate_resize(size(px(720.0), px(480.0)));
    cx.refresh().unwrap();
    cx.run_until_parked();
    assert_eq!(
        cx.update(|window, _| window.viewport_size().width),
        px(720.0)
    );
    assert_eq!(state(&view, cx).database_width(720.0), Some(476.0));
    assert_eq!(
        cx.debug_bounds("database-pane").unwrap().size.width,
        px(476.0)
    );
    assert!(cx.debug_bounds("main-content").unwrap().size.width >= px(240.0));
    cx.simulate_resize(size(px(1280.0), px(800.0)));
    cx.refresh().unwrap();
    cx.run_until_parked();
    assert_eq!(
        cx.debug_bounds("database-pane").unwrap().size.width,
        px(480.0)
    );
    assert_eq!(state(&view, cx).requested_width(), 480.0);
}

#[gpui::test]
fn carbonfox_shell_is_compact_and_flush(cx: &mut TestAppContext) {
    let (_, cx) = fixture(cx);
    let title = cx.debug_bounds("titlebar").unwrap();
    let status = cx.debug_bounds("shell-status").unwrap();
    let pane = cx.debug_bounds("database-pane").unwrap();
    let content = cx.debug_bounds("main-content").unwrap();
    assert_eq!(title.size.height, px(TITLEBAR_HEIGHT));
    assert_eq!(status.size.height, px(STATUS_HEIGHT));
    assert_eq!(pane.origin.x, px(0.0));
    assert_eq!(pane.origin.y, title.size.height);
    assert_eq!(content.origin.x, pane.size.width + px(PANE_GAP));
    assert_eq!(content.origin.x + content.size.width, px(1280.0));
    assert_eq!(content.origin.y + content.size.height, status.origin.y);
    assert_eq!(status.size.height, px(28.0));
    assert_eq!(TITLEBAR_HEIGHT, 34.0);
    assert_eq!(NAME, "Carbonfox - opaque");
}

#[gpui::test]
fn layout_trigger_is_icon_sized_and_popover_still_operates(cx: &mut TestAppContext) {
    let (view, cx) = fixture(cx);
    let trigger = cx.debug_bounds("layout-menu").unwrap();
    assert_eq!(trigger.size.width, px(CONTROL_HEIGHT));
    assert_eq!(trigger.size.height, px(CONTROL_HEIGHT));
    click(cx, "layout-menu");
    assert!(state(&view, cx).menu_open);
    click(cx, "layout-menu");
    assert!(!state(&view, cx).menu_open);
    cx.update(|window, app| view.read(app).controls["layout-menu"].focus(window));
    cx.simulate_keystrokes("enter");
    assert!(state(&view, cx).menu_open);
    cx.simulate_keystrokes("escape");
    assert!(!state(&view, cx).menu_open);
}

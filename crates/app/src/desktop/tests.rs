use super::*;
use gpui::{Modifiers, TestAppContext, VisualTestContext};

fn state(view: &gpui::Entity<Shell>, cx: &VisualTestContext) -> ShellState {
    view.read_with(cx, |shell, _| shell.state.clone())
}

// Native Kit buttons activate Space on release, not on key-down.
fn press(cx: &mut VisualTestContext, key: &str) {
    cx.run_until_parked();
    let keystroke = gpui::Keystroke::parse(key).unwrap();
    cx.simulate_event(gpui::KeyDownEvent {
        keystroke: keystroke.clone(),
        is_held: false,
        prefer_character_input: false,
    });
    cx.simulate_event(gpui::KeyUpEvent { keystroke });
    cx.run_until_parked();
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
    let toggle = cx.debug_bounds("database-toggle").unwrap();
    let create = cx.debug_bounds("new-connection").unwrap();
    assert_eq!(create.left(), toggle.right());
    assert!(create.size.height <= px(TITLEBAR_HEIGHT));
    assert!(create.size.width > px(CONTROL_HEIGHT));
    click(cx, "new-connection");
    click(cx, "connection-create-manually");
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
    click(cx, "new-connection");
    click(cx, "connection-create-manually");
    let mut dialog = source_dialog_context(cx);
    click(&mut dialog, "source-cancel");
    cx.run_until_parked();
    assert_eq!(cx.cx.read(|app| app.windows().len()), 1);
}

#[gpui::test]
fn new_connection_is_keyboard_operable_with_sidebar_hidden(cx: &mut TestAppContext) {
    let (shell, cx) = fixture(cx);
    click(cx, "database-toggle");
    cx.update(|window, app| {
        window.blur(app);
        window.focus_next(app);
        window.focus_next(app);
    });
    press(cx, "space");
    cx.run_until_parked();
    assert!(cx.debug_bounds("connection-create-manually").is_some());
    cx.simulate_keystrokes("down");
    press(cx, "enter");
    cx.run_until_parked();
    let mut dialog = source_dialog_context(cx);
    assert!(dialog.debug_bounds("source-name").is_some());
    dialog.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(!state(&shell, cx).database_visible);
    let sources = shell.read_with(cx, |shell, _| shell.sources.clone());
    sources.update(cx, |model, cx| {
        model.saving = true;
        cx.notify();
    });
    cx.run_until_parked();
    click(cx, "new-connection");
    assert!(!sources.read_with(cx, |model, _| model.form_open));
}

#[gpui::test]
fn connection_dropdown_import_actions_and_native_equivalents_use_file_pickers(
    cx: &mut TestAppContext,
) {
    let (shell, cx) = fixture(cx);
    click(cx, "new-connection");
    for id in [
        "connection-create-manually",
        "connection-import-dbx",
        "connection-import-navicat",
        "connection-import-datagrip",
    ] {
        assert!(cx.debug_bounds(id).is_some());
    }
    click(cx, "connection-import-dbx");
    assert!(cx.did_prompt_for_paths());
    cx.simulate_path_prompt_response(|options| {
        assert!(!options.multiple);
        None
    });
    cx.run_until_parked();
    let model = shell.read_with(cx, |s, _| s.sources.clone());
    assert!(model.read_with(cx, |m, _| !m.connector_busy
        && m.profiles.is_empty()
        && !m.form_open));
    for action in [
        Box::new(ImportNavicat) as Box<dyn gpui::Action>,
        Box::new(ImportDataGrip),
        Box::new(ImportConnectors),
    ] {
        cx.update(|window, app| window.dispatch_action(action, app));
        cx.run_until_parked();
        assert!(cx.did_prompt_for_paths());
        cx.simulate_path_prompt_response(|_| None);
        cx.run_until_parked();
    }
    cx.update(|window, app| window.dispatch_action(Box::new(ExportConnectors), app));
    cx.run_until_parked();
    assert!(!cx.did_prompt_for_new_path()); // Empty saved list is not exportable.
    model.update(cx, |m, cx| {
        m.profiles.push(dalan_drivers::SourceProfile::default());
        cx.notify();
    });
    cx.update(|window, app| window.dispatch_action(Box::new(ExportConnectors), app));
    cx.run_until_parked();
    assert!(cx.did_prompt_for_new_path());
    cx.simulate_new_path_selection(|_| None);
    cx.run_until_parked();
    assert!(model.read_with(cx, |m, _| !m.connector_busy));
    let source = include_str!("../desktop.rs");
    assert!(source.contains("name: \"File\".into()") || source.contains("name: \"File\""));
}

#[gpui::test]
fn importing_from_dropdown_reads_only_selected_fixture_and_opens_review_not_saved_connections(
    cx: &mut TestAppContext,
) {
    let (shell, cx) = fixture(cx);
    let dir = std::env::temp_dir().join(format!("dalan-import-ui-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let file = std::fs::canonicalize(&dir).unwrap().join("connectors.json");
    std::fs::write(&file,br#"{"connections":[{"db_type":"mysql","name":"Imported fixture","host":"db.example.invalid","username":"reader","password":"do-not-import"}]}"#).unwrap();
    click(cx, "new-connection");
    click(cx, "connection-import-dbx");
    cx.simulate_path_prompt_response(|_| Some(vec![file.clone()]));
    cx.run_until_parked();
    let model = shell.read_with(cx, |s, _| s.sources.clone());
    model.read_with(cx, |m, _| {
        assert!(!m.connector_busy);
        assert!(m.form_open, "{:?}", m.connector_feedback);
        assert!(m.profiles.is_empty());
        assert!(m.connector_feedback.as_ref().unwrap().contains("unsaved"));
    });
    let mut dialog = source_dialog_context(cx);
    assert!(dialog.debug_bounds("settings-source-0").is_some());
    assert!(dialog.debug_bounds("settings-source-1").is_none());
    click(&mut dialog, "source-cancel");
    click(&mut dialog, "settings-discard-close");
    std::fs::remove_dir_all(dir).unwrap();
}

fn source_dialog_context(cx: &VisualTestContext) -> VisualTestContext {
    let handle = cx.cx.read(|app| {
        app.windows()
            .into_iter()
            .find(|handle| Some(*handle) == source_dialog::SourceDialog::current_window(app))
            .expect("source dialog opened")
    });
    let dialog = VisualTestContext::from_window(handle, &cx.cx);
    dialog.run_until_parked();
    dialog
}

#[gpui::test]
fn acp_button_is_top_right_and_panel_close_restores_focus(cx: &mut TestAppContext) {
    let (view, cx) = fixture(cx);
    assert!(!state(&view, cx).acp_visible);
    let trigger = cx.debug_bounds("acp-toggle").unwrap();
    assert_eq!(trigger.size.width, px(28.0));
    assert!(cx.debug_bounds("acp-ai-icon").is_some());
    assert!(trigger.origin.x > px(1_100.0));
    assert!(trigger.origin.y < px(TITLEBAR_HEIGHT));
    click(cx, "acp-toggle");
    assert!(state(&view, cx).acp_visible);
    assert!(cx.debug_bounds("acp-empty").is_some());
    assert!(cx.update(|window, app| view.read(app).controls["acp-close"].is_focused(window)));
    click(cx, "acp-close");
    assert!(!state(&view, cx).acp_visible);
    assert!(cx.update(|window, app| view.read(app).controls["acp-toggle"].is_focused(window)));
    press(cx, "space");
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
    press(cx, "enter");
    cx.simulate_keystrokes("cmd-shift-a");
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
    assert!(cx.debug_bounds("acp-toggle").unwrap().origin.y < px(TITLEBAR_HEIGHT));
    cx.update(|window, app| view.read(app).root_focus.clone().focus(window, app));
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
        gpui::init(app);
        bind_keys(app);
        app.on_action(about::show_about);
        app.dispatch_action(&ShowAbout);
    });
    cx.run_until_parked();
    let handle = cx.read(|app| about::current_window(app).unwrap());
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
    cx.update(gpui::init);
    cx.update(bind_keys);
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| about::AboutWindow::new(window, cx));
        gpui::base::Root::new(view, window, cx)
    });
    visual.simulate_resize(size(px(420.0), px(280.0)));
    visual.refresh().unwrap();
    visual.run_until_parked();
    assert!(visual.debug_bounds("about-dalan").is_some());
    click(visual, "about-done");
    assert!(visual.cx.read(|app| app.windows().is_empty()));
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| about::AboutWindow::new(window, cx));
        gpui::base::Root::new(view, window, cx)
    });
    visual.simulate_keystrokes("cmd-w");
    assert!(visual.cx.read(|app| app.windows().is_empty()));
}

#[gpui::test]
fn tab_order_reaches_every_visible_control_in_both_directions(cx: &mut TestAppContext) {
    let (view, cx) = fixture(cx);
    let order = [
        "database-toggle",
        "new-connection",
        "acp-toggle",
        "theme-toggle",
        "layout-menu",
        "database-resize",
    ];
    let mut handles = Vec::new();
    for id in order {
        cx.simulate_keystrokes("tab");
        handles.push(cx.update(|window, app| window.focused(app).expect("tab focus")));
        if !matches!(id, "layout-menu" | "new-connection") {
            assert!(
                cx.update(|window, app| view.read(app).controls[id].is_focused(window)),
                "{id}"
            );
        }
    }
    for (id, handle) in order.into_iter().zip(handles).rev().skip(1) {
        cx.simulate_keystrokes("shift-tab");
        assert!(cx.update(|window, _| handle.is_focused(window)), "{id}");
    }
}

#[gpui::test]
fn close_shortcut_removes_window(cx: &mut TestAppContext) {
    let (_, cx) = fixture(cx);
    cx.simulate_keystrokes("cmd-w");
    assert!(cx.cx.read(|app| app.windows().is_empty()));
}

fn fixture(cx: &mut TestAppContext) -> (gpui::Entity<Shell>, &mut VisualTestContext) {
    cx.update(gpui::init);
    cx.update(bind_keys);
    let mut shell = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let content = cx.new(|cx| Shell::new(window, cx));
        shell = Some(content.clone());
        gpui::base::Root::new(content, window, cx)
    });
    let view = shell.unwrap();
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
        select_layout_item(cx, control);
        assert_eq!(state(&view, cx), expected);
    }
    click(cx, "layout-menu");
    select_layout_item(cx, Control::ToggleDatabase);
    click(cx, "layout-menu");
    select_layout_item(cx, Control::ResetLayout);
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
fn keyboard_activation_native_menu_navigation_and_shortcuts(cx: &mut TestAppContext) {
    let (view, cx) = fixture(cx);
    cx.simulate_keystrokes("tab tab tab tab tab");
    press(cx, "enter");
    cx.run_until_parked();
    assert!(state(&view, cx).menu_open);
    select_layout_item(cx, Control::ToggleDatabase);
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
    select_layout_item(cx, Control::WidenDatabase);
    click(cx, "layout-menu");
    select_layout_item(cx, Control::WidenDatabase);
    click(cx, "layout-menu");
    select_layout_item(cx, Control::WidenDatabase);
    click(cx, "layout-menu");
    select_layout_item(cx, Control::WidenDatabase);
    click(cx, "layout-menu");
    select_layout_item(cx, Control::WidenDatabase);
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
fn system_theme_shell_is_compact_and_flush(cx: &mut TestAppContext) {
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
}

#[gpui::test]
fn layout_trigger_is_icon_sized_and_popover_still_operates(cx: &mut TestAppContext) {
    let (view, cx) = fixture(cx);
    let trigger = cx.debug_bounds("layout-menu").unwrap();
    assert_eq!(trigger.size.width, px(CONTROL_HEIGHT));
    assert!(trigger.size.height <= px(TITLEBAR_HEIGHT));
    click(cx, "layout-menu");
    assert!(state(&view, cx).menu_open);
    click(cx, "layout-menu");
    assert!(!state(&view, cx).menu_open);
    cx.update(|window, app| view.read(app).root_focus.clone().focus(window, app));
    cx.simulate_keystrokes("tab tab tab tab tab");
    press(cx, "enter");
    assert!(state(&view, cx).menu_open);
    cx.simulate_keystrokes("escape");
    assert!(!state(&view, cx).menu_open);
}

#[gpui::test]
fn new_console_shortcut_opens_once_and_uses_selected_database(cx: &mut TestAppContext) {
    let (shell, cx) = fixture(cx);
    let profile = dalan_drivers::SourceProfile::default();
    let id = profile.id.clone();
    let sources = shell.read_with(cx, |shell, _| shell.sources.clone());
    sources.update(cx, |model, cx| {
        model.profiles = vec![profile];
        model.explorer_source = Some(id.clone());
        model.explorer_database = Some("inventory".into());
        model.tree.databases.insert(id, vec!["inventory".into()]);
        cx.notify();
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("cmd-shift-n");
    cx.run_until_parked();
    let workspace = shell.read_with(cx, |shell, _| shell.workspace.clone());
    assert_eq!(
        workspace.read_with(cx, |workspace, _| workspace.tab_count()),
        1
    );
    assert!(cx.debug_bounds("query-run").is_some());
    assert!(cx.debug_bounds("query-database").is_some());
    sources.read_with(cx, |model, _| {
        assert!(matches!(&model.workspace_open, Some(dalan_app::workspace_tabs::WorkspaceOpen::Console { database: Some(database), .. }) if database == "inventory"));
        assert!(!model.busy && model.page.is_none());
    });
    cx.simulate_input("SELECT 1");
    cx.run_until_parked();
    cx.simulate_keystrokes("cmd-w");
    cx.run_until_parked();
    assert!(cx.debug_bounds("confirm-close-console").is_some());
    cx.simulate_keystrokes("space");
    cx.run_until_parked();
    assert_eq!(
        workspace.read_with(cx, |workspace, _| workspace.tab_count()),
        1
    );
}

fn select_layout_item(cx: &mut VisualTestContext, control: Control) {
    // PopupMenu owns selection and keyboard focus. Its first ArrowDown selects
    // the first enabled item; the application does not synthesize menu buttons.
    let index = MENU_CONTROLS
        .iter()
        .position(|(item, _)| *item == control)
        .unwrap();
    for _ in 0..=index {
        cx.simulate_keystrokes("down");
    }
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
}

#[gpui::test]
fn titlebar_theme_control_switches_kit_modes_without_changing_workspace(cx: &mut TestAppContext) {
    let (shell, cx) = fixture(cx);
    let before = state(&shell, cx);
    let mode = cx.cx.read(|app| app.theme().mode);
    let theme = cx.debug_bounds("theme-toggle").unwrap();
    let ai = cx.debug_bounds("acp-toggle").unwrap();
    let layout = cx.debug_bounds("layout-menu").unwrap();
    assert!(ai.origin.y < px(TITLEBAR_HEIGHT));
    assert!(ai.right() <= theme.left() && theme.right() <= layout.left());
    click(cx, "theme-toggle");
    assert_ne!(cx.cx.read(|app| app.theme().mode), mode);
    assert_eq!(state(&shell, cx), before);
    click(cx, "theme-toggle");
    assert_eq!(cx.cx.read(|app| app.theme().mode), mode);
}

use super::{CloseWindow, source_form::SourceForm, source_model::SourceModel};
use gpui::{
    App, Bounds, Context, Entity, Subscription, TitlebarOptions, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, size,
};

pub(super) struct SourceDialog {
    model: Entity<SourceModel>,
    form: Entity<SourceForm>,
    generation: u64,
    _subscription: Subscription,
}

impl SourceDialog {
    fn new(model: Entity<SourceModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let profile = model
            .read(cx)
            .form_profile
            .clone()
            .expect("source dialog requires a draft");
        let generation = model.read(cx).form_generation;
        let form = cx.new(|cx| SourceForm::new(profile, model.clone(), cx));
        form.read(cx).focus(window, cx);
        let subscription = cx.observe_in(&model, window, |this, _, window, cx| {
            let model = this.model.read(cx);
            if !model.form_open {
                window.remove_window();
                return;
            }
            if model.form_generation != this.generation {
                let profile = model
                    .form_profile
                    .clone()
                    .expect("open source dialog has draft");
                this.generation = model.form_generation;
                let model = this.model.clone();
                this.form = cx.new(|cx| SourceForm::new(profile, model, cx));
                this.form.read(cx).focus(window, cx);
            }
            cx.notify();
        });
        let close_model = model.clone();
        window.on_window_should_close(cx, move |_, app| {
            close_model.update(app, |model, cx| {
                if model.saving {
                    return false;
                }
                model.close_form(cx);
                true
            })
        });
        Self {
            model,
            form,
            generation,
            _subscription: subscription,
        }
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.model.read(cx).saving {
            return;
        }
        self.model.update(cx, |model, cx| model.close_form(cx));
        window.remove_window();
    }
}
impl Render for SourceDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("source-dialog")
            .debug_selector(|| "source-dialog".into())
            .key_context("SourceDialog")
            .size_full()
            .on_action(cx.listener(|this, _: &CloseWindow, window, cx| this.close(window, cx)))
            .child(self.form.clone())
    }
}

pub(super) fn show(model: Entity<SourceModel>, cx: &mut App) {
    if !model.read(cx).form_open {
        return;
    }
    if let Some(handle) = cx
        .windows()
        .into_iter()
        .find_map(|handle| handle.downcast::<SourceDialog>())
    {
        let _ = handle.update(cx, |_, window, _| window.activate_window());
        return;
    }
    let bounds = Bounds::centered(None, size(px(1040.0), px(760.0)), cx);
    if let Err(error) = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(780.0), px(560.0))),
            window_background: gpui::WindowBackgroundAppearance::Opaque,
            titlebar: Some(TitlebarOptions {
                title: Some("".into()),
                appears_transparent: true,
                traffic_light_position: Some(gpui::point(px(12.0), px(12.0))),
            }),
            ..Default::default()
        },
        |window, cx| cx.new(|cx| SourceDialog::new(model.clone(), window, cx)),
    ) {
        model.update(cx, |model, cx| {
            model.close_form(cx);
            model.error = Some(format!("Could not open source dialog: {error}"));
            cx.notify();
        });
    }
}

#[cfg(all(test, feature = "ui-tests"))]
mod tests {
    use super::*;
    use dalan_drivers::sources::{DbEngine, SourceProfile, TlsMode};
    use gpui::{Modifiers, TestAppContext, VisualTestContext, WindowHandle};

    fn open(
        model: &Entity<SourceModel>,
        cx: &mut TestAppContext,
    ) -> (WindowHandle<SourceDialog>, VisualTestContext) {
        cx.update(|app| show(model.clone(), app));
        let handle = cx.update(|app| {
            let windows = app.windows();
            assert_eq!(windows.len(), 1);
            windows[0].downcast::<SourceDialog>().unwrap()
        });
        let mut visual = VisualTestContext::from_window(handle.into(), cx);
        visual.refresh().unwrap();
        visual.run_until_parked();
        (handle, visual)
    }

    fn new_model(cx: &mut TestAppContext) -> Entity<SourceModel> {
        cx.update(crate::desktop::bind_keys);
        assert!(cx.update(|app| app.windows().is_empty()));
        let model = cx.new(|_| SourceModel::for_tests(vec![]));
        model.update(cx, |model, cx| model.new_source(cx));
        model
    }

    fn assert_closed(model: &Entity<SourceModel>, cx: &mut TestAppContext) {
        cx.run_until_parked();
        assert!(cx.update(|app| app.windows().is_empty()));
        model.read_with(cx, |model, _| {
            assert!(!model.form_open);
            assert!(model.form_profile.is_none());
            assert!(!model.form_busy);
        });
    }

    fn click(visual: &mut VisualTestContext, id: &'static str) {
        visual.run_until_parked();
        let bounds = visual
            .debug_bounds(id)
            .unwrap_or_else(|| panic!("missing {id}"));
        visual.simulate_click(bounds.center(), Modifiers::default());
        visual.run_until_parked();
    }

    fn edit(visual: &mut VisualTestContext, id: &'static str, value: &str) {
        click(visual, id);
        visual.simulate_keystrokes("cmd-a");
        visual.simulate_input(value);
        visual.run_until_parked();
    }

    #[gpui::test]
    fn tab_strip_occupies_transparent_titlebar_and_reserves_traffic_lights(
        cx: &mut TestAppContext,
    ) {
        // GPUI's test window does not expose native titlebar metadata. Check only
        // the production show() definition, never the assertion's own literals.
        let source = include_str!("source_dialog.rs");
        let show = source
            .split("pub(super) fn show(")
            .nth(1)
            .unwrap()
            .split("#[cfg(all(test,")
            .next()
            .unwrap();
        assert!(show.contains("title: Some(\"\".into())"));
        assert!(show.contains("appears_transparent: true"));
        assert!(show.contains("window_min_size: Some(size(px(780.0), px(560.0)))"));
        assert!(show.contains("window_background: gpui::WindowBackgroundAppearance::Opaque"));
        assert!(!show.contains("Data Sources"));
        assert!(!show.contains("Dalan"));

        let model = new_model(cx);
        let (_, mut visual) = open(&model, cx);
        visual.update(|window, _| {
            assert_eq!(window.bounds().size, size(px(1040.), px(760.)));
        });
        for viewport in [size(px(1040.), px(760.)), size(px(780.), px(560.))] {
            visual.simulate_resize(viewport);
            visual.run_until_parked();
            let bar = visual.debug_bounds("source-tab-bar").unwrap();
            assert_eq!(bar.origin, gpui::point(px(0.), px(0.)));
            assert_eq!(bar.size, size(viewport.width, px(34.)));
            let mut right = px(84.);
            for id in [
                "source-tab-general",
                "source-tab-options",
                "source-tab-ssh",
                "source-tab-schemas",
            ] {
                let tab = visual.debug_bounds(id).unwrap();
                assert!(
                    tab.left() >= right,
                    "overlap or traffic-light intrusion: {id}"
                );
                assert_eq!(tab.top(), px(0.));
                assert_eq!(tab.size.height, px(34.));
                assert!(tab.right() <= bar.right());
                right = tab.right();
            }
        }
        visual.simulate_keystrokes("cmd-w");
        assert_closed(&model, cx);
    }

    #[gpui::test]
    fn native_tab_clicks_preserve_one_draft_and_page_edits(cx: &mut TestAppContext) {
        let model = new_model(cx);
        let (handle, mut visual) = open(&model, cx);
        let form_id = handle
            .update(cx, |dialog, _, _| dialog.form.entity_id())
            .unwrap();
        // No click: creation must still prefer Name even if Driver renders first.
        visual.simulate_keystrokes("cmd-a");
        visual.simulate_input("Tab draft");
        visual.run_until_parked();
        edit(&mut visual, "source-color", "#123456");
        click(&mut visual, "source-tab-options");
        edit(&mut visual, "source-connect-timeout", "31");
        for (tab, page_field) in [
            ("source-tab-ssh", "source-direct"),
            ("source-tab-schemas", "source-schemas-all"),
            ("source-tab-general", "source-host"),
            ("source-tab-options", "source-query-timeout"),
        ] {
            click(&mut visual, tab);
            assert!(
                visual.debug_bounds(page_field).is_some(),
                "missing page control {page_field}"
            );
            model.read_with(cx, |model, _| {
                assert!(model.form_open);
                assert!(!model.form_busy);
            });
            handle
                .update(cx, |dialog, _, app| {
                    assert_eq!(dialog.form.entity_id(), form_id);
                    let profile = dialog.form.read(app).profile(app).unwrap();
                    assert_eq!(profile.name, "Tab draft");
                    assert_eq!(profile.color.as_deref(), Some("#123456"));
                    assert_eq!(profile.options.connect_timeout_seconds, 31);
                })
                .unwrap();
        }
        visual.simulate_keystrokes("cmd-w");
        assert_closed(&model, cx);
    }

    #[gpui::test]
    fn minimum_dialog_keeps_aligned_editors_footer_and_local_validation(cx: &mut TestAppContext) {
        let model = new_model(cx);
        let (_, mut visual) = open(&model, cx);
        visual.simulate_resize(size(px(780.), px(560.)));
        visual.run_until_parked();
        edit(&mut visual, "source-port", "not-a-port");
        for tab in [
            "source-tab-options",
            "source-tab-ssh",
            "source-tab-schemas",
            "source-tab-general",
        ] {
            click(&mut visual, tab);
            let body = visual.debug_bounds("source-form-body").unwrap();
            assert!(body.size.width <= px(720.));
            assert!(body.left() >= px(16.));
            assert!(body.right() <= px(764.));
            let footer = visual.debug_bounds("source-form-footer").unwrap();
            assert!(footer.bottom() <= px(560.));
            assert!(footer.top() >= px(34.));
            for id in ["source-test", "source-save", "source-cancel"] {
                let button = visual.debug_bounds(id).unwrap();
                assert!(button.top() >= footer.top());
                assert!(button.bottom() <= footer.bottom());
            }
            if tab == "source-tab-options" {
                let mut left = None;
                for id in [
                    "source-connect-timeout",
                    "source-query-timeout",
                    "source-page-size",
                ] {
                    let field = visual.debug_bounds(id).unwrap();
                    assert_eq!(field.size.height, px(28.));
                    assert_eq!(*left.get_or_insert(field.left()), field.left());
                }
            }
        }
        // Invalid input stops before SourceModel::test, so no network task is spawned.
        click(&mut visual, "source-test");
        model.read_with(cx, |model, _| {
            assert!(model.form_open);
            assert!(!model.form_busy);
            assert!(
                model
                    .form_feedback
                    .as_deref()
                    .is_some_and(|feedback| feedback.contains("port"))
            );
        });
        visual.simulate_keystrokes("cmd-w");
        assert_closed(&model, cx);
    }

    #[gpui::test]
    fn reuses_dialog_and_preserves_native_draft_edits(cx: &mut TestAppContext) {
        let model = new_model(cx);
        let (handle, mut visual) = open(&model, cx);
        visual.update(|window, _| {
            assert_eq!(window.bounds().size, size(px(1040.), px(760.)));
        });
        for id in [
            "source-dialog",
            "source-name",
            "source-host",
            "source-save",
            "source-cancel",
        ] {
            assert!(visual.debug_bounds(id).is_some(), "missing {id}");
        }
        let form = handle
            .update(cx, |dialog, _, _| dialog.form.clone())
            .unwrap();
        let generation = model.read_with(cx, |model, _| model.form_generation);
        // The dialog focuses Name on creation: native input must reach that field.
        visual.simulate_keystrokes("cmd-a");
        visual.simulate_input("Unsaved draft");
        visual.run_until_parked();
        visual.update(|_, app| {
            assert_eq!(form.read(app).profile(app).unwrap().name, "Unsaved draft");
        });
        model.update(cx, |model, cx| model.new_source(cx));
        cx.update(|app| show(model.clone(), app));
        visual.run_until_parked();
        assert_eq!(cx.update(|app| app.windows().len()), 1);
        handle
            .update(cx, |dialog, _, app| {
                assert_eq!(dialog.form.entity_id(), form.entity_id());
                assert_eq!(dialog.generation, generation);
                assert_eq!(
                    dialog.form.read(app).profile(app).unwrap().name,
                    "Unsaved draft"
                );
            })
            .unwrap();
        visual.simulate_keystrokes("cmd-w");
        assert_closed(&model, cx);
    }

    #[gpui::test]
    fn saving_guards_action_and_native_close_until_model_dismisses(cx: &mut TestAppContext) {
        let model = new_model(cx);
        let (_, mut visual) = open(&model, cx);
        model.update(cx, |model, cx| {
            model.saving = true;
            cx.notify();
        });
        visual.run_until_parked();
        visual.update(|window, app| window.dispatch_action(Box::new(CloseWindow), app));
        visual.run_until_parked();
        assert_eq!(cx.update(|app| app.windows().len()), 1);
        assert!(model.read_with(cx, |model, _| model.form_open));
        assert!(!visual.simulate_close());
        visual.run_until_parked();
        assert_eq!(cx.update(|app| app.windows().len()), 1);
        assert!(model.read_with(cx, |model, _| model.form_open));
        model.update(cx, |model, cx| {
            model.saving = false;
            model.close_form(cx);
        });
        assert_closed(&model, cx);
    }

    #[gpui::test]
    fn edit_profile_refreshes_only_when_form_generation_changes(cx: &mut TestAppContext) {
        cx.update(crate::desktop::bind_keys);
        let profile = SourceProfile {
            id: "00000000-0000-4000-8000-000000000001".into(),
            name: "Saved MariaDB".into(),
            engine: DbEngine::MariaDb,
            host: "db.internal".into(),
            port: 3307,
            username: "reader".into(),
            database: Some("inventory".into()),
            tls: TlsMode::Disabled,
            save_password: false,
            ..Default::default()
        };
        let model = cx.new(|_| SourceModel::for_tests(vec![profile.clone()]));
        model.update(cx, |model, cx| {
            model.selected_source = Some(profile.id.clone());
            model.edit_explorer_source(cx);
        });
        let (handle, mut visual) = open(&model, cx);
        let form = handle
            .update(cx, |dialog, _, app| {
                let actual = dialog.form.read(app).profile(app).unwrap();
                assert_eq!(actual.id, profile.id);
                assert_eq!(actual.name, profile.name);
                assert_eq!(actual.engine, profile.engine);
                assert_eq!(actual.host, profile.host);
                assert_eq!(actual.port, profile.port);
                assert_eq!(actual.username, profile.username);
                assert_eq!(actual.database, profile.database);
                assert_eq!(actual.tls, profile.tls);
                dialog.form.clone()
            })
            .unwrap();
        model.update(cx, |model, cx| {
            model.form_busy = true;
            model.form_feedback = Some("Testing connection…".into());
            cx.notify();
        });
        visual.run_until_parked();
        visual.simulate_keystrokes("cmd-a");
        visual.simulate_input("Edited draft");
        visual.run_until_parked();
        model.read_with(cx, |model, _| {
            assert!(!model.form_busy);
            assert!(model.form_feedback.is_none());
        });
        handle
            .update(cx, |dialog, _, app| {
                assert_eq!(dialog.form.entity_id(), form.entity_id());
                assert_eq!(
                    dialog.form.read(app).profile(app).unwrap().name,
                    "Edited draft"
                );
            })
            .unwrap();
        // Reproduce a loaded-draft notification without touching the Keychain.
        model.update(cx, |model, cx| {
            model.form_profile.as_mut().unwrap().name = "Refreshed draft".into();
            model.form_generation += 1;
            cx.notify();
        });
        visual.run_until_parked();
        handle
            .update(cx, |dialog, _, app| {
                assert_ne!(dialog.form.entity_id(), form.entity_id());
                assert_eq!(dialog.generation, model.read(app).form_generation);
                assert_eq!(
                    dialog.form.read(app).profile(app).unwrap().name,
                    "Refreshed draft"
                );
            })
            .unwrap();
        visual.simulate_keystrokes("cmd-a");
        visual.simulate_input("Refocused draft");
        visual.run_until_parked();
        handle
            .update(cx, |dialog, _, app| {
                assert_eq!(
                    dialog.form.read(app).profile(app).unwrap().name,
                    "Refocused draft"
                );
            })
            .unwrap();
        model.update(cx, |model, cx| model.close_form(cx));
        assert_closed(&model, cx);
    }

    #[gpui::test]
    fn save_completion_notification_releases_dialog_and_allows_reopen(cx: &mut TestAppContext) {
        let model = new_model(cx);
        let (handle, visual) = open(&model, cx);
        let weak_form = handle
            .update(cx, |dialog, _, _| dialog.form.downgrade())
            .unwrap();
        // The fixture has no repository. Simulate the successful save's UI-state
        // transition, not storage or a native secret-store operation.
        model.update(cx, |model, cx| {
            model.form_open = false;
            model.form_profile = None;
            model.saving = false;
            cx.notify();
        });
        assert_closed(&model, cx);
        assert!(weak_form.upgrade().is_none());
        drop(visual);
        cx.update(|app| show(model.clone(), app));
        assert!(cx.update(|app| app.windows().is_empty()));
        model.update(cx, |model, cx| model.new_source(cx));
        let (_, mut visual) = open(&model, cx);
        visual.simulate_keystrokes("cmd-w");
        assert_closed(&model, cx);
    }
}

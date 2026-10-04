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
            titlebar: Some(TitlebarOptions {
                title: Some("Data Sources · Dalan".into()),
                ..Default::default()
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
    use gpui::{TestAppContext, VisualTestContext, WindowHandle};

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
            model.edit_source(cx);
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

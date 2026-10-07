//! Lazy background local-JDK detection, shared by JDBC settings and source forms.
use super::input::TextInput;
use gpui::component::{Sizable, button::Button};
use gpui::{Context, Entity, Subscription, Window, div, prelude::*};

pub(super) struct JavaRuntimeField {
    input: Entity<TextInput>,
    started: bool,
    pending: bool,
    revision: u64,
    last_value: String,
    detected: Option<String>,
    status: Option<String>,
    _subscription: Subscription,
}
impl JavaRuntimeField {
    pub(super) fn new(input: Entity<TextInput>, cx: &mut Context<Self>) -> Self {
        let last_value = input.read(cx).value();
        let subscription = cx.observe(&input, |this, input, cx| {
            let value = input.read(cx).value();
            if value != this.last_value {
                this.last_value = value;
                this.revision += 1;
            }
            cx.notify();
        });
        Self {
            input,
            started: false,
            pending: false,
            revision: 0,
            last_value,
            detected: None,
            status: None,
            _subscription: subscription,
        }
    }
    fn start(&mut self, cx: &mut Context<Self>) {
        if self.started {
            return;
        }
        self.started = true;
        // Never replace a saved/custom runtime. Detection itself is not a save.
        if !self.input.read(cx).value().trim().is_empty() {
            return;
        }
        self.pending = true;
        let revision = self.revision;
        #[cfg(not(feature = "ui-tests"))]
        {
            let task = cx.background_executor().spawn(async {
                super::source_model::runtime().block_on(dalan_drivers::jdbc::detect_local_java())
            });
            cx.spawn(async move |this, cx| {
                let result = task.await;
                let _ = this.update(cx, |this, cx| this.complete(revision, result, cx));
            })
            .detach();
        }
        #[cfg(feature = "ui-tests")]
        self.complete(
            revision,
            Err(anyhow::anyhow!("Synthetic JDK not detected")),
            cx,
        );
    }
    fn complete(
        &mut self,
        revision: u64,
        result: anyhow::Result<dalan_drivers::jdbc::DetectedJava>,
        cx: &mut Context<Self>,
    ) {
        self.pending = false;
        if self.revision != revision || !self.input.read(cx).value().trim().is_empty() {
            cx.notify();
            return;
        }
        match result {
            Ok(java) => {
                self.detected = Some(java.executable.clone());
                self.status = Some(format!(
                    "Detected Java {} · {}",
                    java.runtime.major, java.executable
                ));
                self.input
                    .update(cx, |input, cx| input.set_value(java.executable, cx));
            }
            Err(_) => {
                self.status = Some(
                    "No usable JDK detected. Enter an absolute path to a Java 17+ executable."
                        .into(),
                );
            }
        }
        cx.notify();
    }
}
impl gpui::Render for JavaRuntimeField {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.start(cx);
        let value = self.input.read(cx).value();
        let detected = self.detected.as_ref().is_some_and(|path| path == &value);
        div()
            .id("java-runtime-field")
            .flex()
            .flex_col()
            .gap_2()
            .when(self.pending, |body| {
                body.child("Detecting installed Java 17+…")
            })
            .when_some(self.status.clone(), |body, status| {
                body.child(
                    div()
                        .debug_selector(|| "java-runtime-status".into())
                        .child(status),
                )
            })
            .when(!detected && !self.pending, |body| {
                body.child(
                    div()
                        .debug_selector(|| "java-runtime-manual".into())
                        .child("Java executable (absolute path)")
                        .child(self.input.clone()),
                )
            })
            .when(detected, |body| {
                body.child(
                    Button::new("java-runtime-custom")
                        .debug_selector(|| "java-runtime-custom".into())
                        .label("Use a different Java runtime")
                        .small()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.detected = None;
                            this.status = None;
                            cx.notify();
                        })),
                )
            })
    }
}

#[cfg(all(test, feature = "ui-tests"))]
mod tests {
    use super::*;
    use gpui::{TestAppContext, VisualTestContext};
    fn fixture<'a>(
        cx: &'a mut TestAppContext,
        path: &str,
    ) -> (
        Entity<JavaRuntimeField>,
        Entity<TextInput>,
        &'a mut VisualTestContext,
    ) {
        cx.update(gpui::init);
        let input = cx.new(|cx| TextInput::new(path, "Java", false, cx));
        let view = cx.new(|cx| JavaRuntimeField::new(input.clone(), cx));
        let (_, visual) =
            cx.add_window_view(|window, cx| gpui::base::Root::new(view.clone(), window, cx));
        visual.refresh().unwrap();
        visual.run_until_parked();
        (view, input, visual)
    }
    fn detected() -> dalan_drivers::jdbc::DetectedJava {
        dalan_drivers::jdbc::DetectedJava {
            executable: "/synthetic/jdk/bin/java".into(),
            runtime: dalan_drivers::jdbc::RuntimeInfo {
                major: 21,
                version: "21.0.1".into(),
            },
        }
    }
    #[gpui::test]
    fn detection_fallback_and_success_toggle_manual_entry(cx: &mut TestAppContext) {
        let (view, input, visual) = fixture(cx, "");
        assert!(visual.debug_bounds("java-runtime-manual").is_some());
        view.update(visual, |v, cx| v.complete(0, Ok(detected()), cx));
        visual.run_until_parked();
        assert_eq!(
            input.read_with(visual, |i, _| i.value()),
            "/synthetic/jdk/bin/java"
        );
        assert!(visual.debug_bounds("java-runtime-manual").is_none());
        assert!(visual.debug_bounds("java-runtime-custom").is_some());
        let bounds = visual.debug_bounds("java-runtime-custom").unwrap();
        visual.simulate_click(bounds.center(), gpui::Modifiers::default());
        visual.run_until_parked();
        assert!(visual.debug_bounds("java-runtime-manual").is_some());
    }
    #[gpui::test]
    fn explicit_or_changed_runtime_never_overwritten(cx: &mut TestAppContext) {
        let (view, input, visual) = fixture(cx, "/custom/java");
        view.read_with(visual, |v, _| assert!(!v.pending && v.started));
        view.update(visual, |v, cx| v.complete(0, Ok(detected()), cx));
        assert_eq!(input.read_with(visual, |i, _| i.value()), "/custom/java");
        input.update(visual, |i, cx| i.set_value("", cx));
        visual.run_until_parked();
        view.update(visual, |v, cx| v.complete(0, Ok(detected()), cx));
        assert!(input.read_with(visual, |i, _| i.value().is_empty()));
    }
}

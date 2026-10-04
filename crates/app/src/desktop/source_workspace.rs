use super::{source_browser::SourceBrowser, source_form::SourceForm, source_model::SourceModel};
use gpui::{Context, Entity, Subscription, Window, prelude::*};

pub(super) struct SourceWorkspace {
    model: Entity<SourceModel>,
    browser: Entity<SourceBrowser>,
    form: Option<Entity<SourceForm>>,
    form_generation: u64,
    _subscription: Subscription,
}
impl SourceWorkspace {
    pub(super) fn new(model: Entity<SourceModel>, cx: &mut Context<Self>) -> Self {
        let browser = cx.new(|cx| SourceBrowser::new(model.clone(), cx));
        let subscription = cx.observe(&model, |_, _, cx| cx.notify());
        Self {
            model,
            browser,
            form: None,
            form_generation: 0,
            _subscription: subscription,
        }
    }
}
impl Render for SourceWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.model.read(cx);
        if model.form_open {
            if self.form.is_none() || self.form_generation != model.form_generation {
                let profile = model.form_profile.clone().expect("open form has profile");
                self.form_generation = model.form_generation;
                let model = self.model.clone();
                self.form = Some(cx.new(|cx| SourceForm::new(profile, model, cx)));
                self.form.as_ref().unwrap().read(cx).focus(window, cx);
            }
            self.form.clone().unwrap().into_any_element()
        } else {
            self.form = None;
            self.browser.clone().into_any_element()
        }
    }
}

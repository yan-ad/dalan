use super::{source_browser::SourceBrowser, source_dialog, source_model::SourceModel};
use gpui::{Context, Entity, Subscription, Window, prelude::*};

pub(super) struct SourceWorkspace {
    model: Entity<SourceModel>,
    browser: Entity<SourceBrowser>,
    _subscription: Subscription,
}
impl SourceWorkspace {
    pub(super) fn new(model: Entity<SourceModel>, cx: &mut Context<Self>) -> Self {
        let browser = cx.new(|cx| SourceBrowser::new(model.clone(), cx));
        let subscription = cx.observe(&model, |this, _, cx| {
            if this.model.read(cx).form_open {
                let model = this.model.clone();
                cx.defer(move |cx| source_dialog::show(model, cx));
            }
            cx.notify();
        });
        Self {
            model,
            browser,
            _subscription: subscription,
        }
    }
}
impl Render for SourceWorkspace {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.browser.clone()
    }
}

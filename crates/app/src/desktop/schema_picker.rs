//! Display-only schema selection from a materialized catalog. Never connects.
use super::{input::TextInput, source_model::SourceModel};
use dalan_drivers::{SchemaSelection, SourceProfile};
use gpui::component::{Disableable, Sizable, button::Button, checkbox::Checkbox};
use gpui::{Context, Entity, Subscription, Window, div, prelude::*, px};
use std::collections::HashSet;

gpui::actions!(schema_picker, [CancelSchemaPicker]);

pub(super) struct SchemaPicker {
    model: Entity<SourceModel>,
    original: SourceProfile,
    catalog: Vec<String>,
    all: bool,
    selected: HashSet<String>,
    search: Entity<TextInput>,
    cancelled: bool,
    focus: gpui::FocusHandle,
    focus_pending: bool,
    scroll: gpui::UniformListScrollHandle,
    _subscription: Subscription,
}
impl SchemaPicker {
    pub(super) fn new(
        model: Entity<SourceModel>,
        original: SourceProfile,
        catalog: Vec<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.bind_keys([gpui::KeyBinding::new(
            "escape",
            CancelSchemaPicker,
            Some("SchemaPicker"),
        )]);
        let all = matches!(original.schemas, SchemaSelection::All);
        let selected = match &original.schemas {
            SchemaSelection::All => catalog.iter().cloned().collect(),
            SchemaSelection::Selected(names) => names.iter().cloned().collect(),
        };
        let search = cx.new(|cx| TextInput::new("", "Filter schemas", false, cx));
        let subscription = cx.observe(&search, |this, _, cx| {
            this.scroll
                .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);
            cx.notify();
        });
        Self {
            model,
            original,
            catalog,
            all,
            selected,
            search,
            cancelled: false,
            focus: cx.focus_handle().tab_stop(false),
            focus_pending: true,
            scroll: gpui::UniformListScrollHandle::new(),
            _subscription: subscription,
        }
    }
    pub(super) fn cancel(&mut self) {
        self.cancelled = true;
    }
    pub(super) fn finish(&self, cx: &mut Context<Self>) {
        if self.cancelled {
            return;
        }
        let m = self.model.read(cx);
        // A source edit/removal or refreshed catalog invalidates the popup snapshot.
        if !m.profiles.iter().any(|p| p == &self.original)
            || m.tree.databases.get(&self.original.id) != Some(&self.catalog)
        {
            return;
        }
        let selection = if self.all {
            SchemaSelection::All
        } else {
            SchemaSelection::Selected(
                self.catalog
                    .iter()
                    .filter(|name| self.selected.contains(*name))
                    .cloned()
                    .collect(),
            )
        };
        if selection != self.original.schemas {
            self.model.update(cx, |m, cx| {
                m.set_visible_schemas(self.original.id.clone(), selection, cx)
            });
        }
    }
    fn toggle(&mut self, name: String, cx: &mut Context<Self>) {
        self.all = false;
        if !self.selected.remove(&name) {
            self.selected.insert(name);
        }
        cx.notify();
    }
}
impl gpui::Render for SchemaPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.focus_pending {
            self.focus_pending = false;
            let focus = self.focus.clone();
            window.defer(cx, move |window, cx| focus.focus(window, cx));
        }
        let blocked = self.model.read(cx).saving || self.model.read(cx).form_open;
        let query = self.search.read(cx).value().to_lowercase();
        let names: Vec<_> = self
            .catalog
            .iter()
            .enumerate()
            .filter(|(_, name)| name.to_lowercase().contains(&query))
            .map(|(i, name)| (i, name.clone()))
            .collect();
        let count = names.len();
        let mut root = div()
            .id("schema-picker")
            .debug_selector(|| "schema-picker".into())
            .w(px(300.))
            .max_w_full()
            .track_focus(&self.focus)
            .capture_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "escape" => {
                        this.cancel();
                        window.dispatch_action(Box::new(gpui::base::actions::Cancel), cx);
                        cx.stop_propagation();
                    }
                    "enter" => {
                        window.dispatch_action(
                            Box::new(gpui::base::actions::Confirm { secondary: false }),
                            cx,
                        );
                        cx.stop_propagation();
                    }
                    _ => {}
                }
            }))
            .key_context("SchemaPicker")
            .on_action(cx.listener(|this, _: &CancelSchemaPicker, window, cx| {
                this.cancel();
                window.dispatch_action(Box::new(gpui::base::actions::Cancel), cx);
            }))
            .flex()
            .flex_col()
            .gap_2()
            .child("Visible schemas / databases")
            .child(self.search.clone())
            .child(
                Checkbox::new("schema-picker-all")
                    .debug_selector(|| "schema-picker-all".into())
                    .label(format!("All databases ({})", self.catalog.len()))
                    .checked(self.all)
                    .disabled(blocked)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.all = !this.all;
                        this.selected = if this.all {
                            this.catalog.iter().cloned().collect()
                        } else {
                            HashSet::new()
                        };
                        cx.notify();
                    })),
            );
        let default = self
            .original
            .connection_target()
            .ok()
            .and_then(|t| match t {
                dalan_drivers::ConnectionTarget::Tcp { database, .. }
                | dalan_drivers::ConnectionTarget::UnixSocket { database, .. } => database,
            });
        if let Some(default) = default.filter(|d| self.catalog.contains(d)) {
            root = root.child(
                Checkbox::new("schema-picker-default")
                    .debug_selector(|| "schema-picker-default".into())
                    .label(format!("Default database ({default})"))
                    .checked(self.selected.contains(&default))
                    .disabled(blocked)
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle(default.clone(), cx))),
            );
        }
        root.child(
            gpui::uniform_list(
                "schema-picker-list",
                count,
                cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                    range
                        .map(|index| {
                            let (original_index, name) = &names[index];
                            let name = name.clone();
                            let selector = format!("schema-picker-item-{original_index}");
                            div().h(px(28.)).w_full().child(
                                Checkbox::new(gpui::SharedString::from(selector.clone()))
                                    .debug_selector(move || selector.clone())
                                    .label(name.clone())
                                    .checked(this.selected.contains(&name))
                                    .disabled(blocked)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.toggle(name.clone(), cx)
                                    })),
                            )
                        })
                        .collect::<Vec<_>>()
                }),
            )
            .h(px((count * 28).clamp(28, 252) as f32))
            .track_scroll(&self.scroll),
        )
        .when(count == 0, |root| {
            root.child("No loaded schemas match. Refresh the source to discover schemas.")
        })
        .child(
            div().flex().justify_end().child(
                Button::new("schema-picker-apply")
                    .debug_selector(|| "schema-picker-apply".into())
                    .label("Apply")
                    .small()
                    .outline()
                    .disabled(blocked)
                    .on_click(|_, window, cx| {
                        window.dispatch_action(
                            Box::new(gpui::base::actions::Confirm { secondary: false }),
                            cx,
                        )
                    }),
            ),
        )
        .child("Enter or click outside to apply · Escape to cancel")
    }
}

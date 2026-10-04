use std::collections::HashMap;

use anyhow::{Context as _, Result, ensure};
use dalan_drivers::sources::{DbEngine, SourceProfile, TlsMode, Transport};
use gpui::{
    App, Context, Div, Entity, FocusHandle, Stateful, Subscription, Window, div, prelude::*, px,
    rgb,
};

use super::{
    Dismiss, NextFocus, PreviousFocus,
    icons::{Icon, icon},
    input::TextInput,
    source_model::SourceModel,
    theme::*,
};

/// A draft is kept separate from the persisted model until Save is activated.
pub(super) struct SourceForm {
    original: SourceProfile,
    model: Entity<SourceModel>,
    inputs: HashMap<&'static str, Entity<TextInput>>,
    last_values: HashMap<&'static str, String>,
    controls: HashMap<&'static str, FocusHandle>,
    root_focus: FocusHandle,
    engine: DbEngine,
    transport: u8,
    tls: TlsMode,
    save_password: bool,
    ssh_keys: Vec<dalan_app::ssh_keys::SshKeyCandidate>,
    key_picker_open: bool,
    key_picker_busy: bool,
    key_picker_error: Option<String>,
    ca_picker_open: bool,
    _subscriptions: Vec<Subscription>,
}

impl SourceForm {
    pub(super) fn new(
        profile: SourceProfile,
        model: Entity<SourceModel>,
        cx: &mut Context<Self>,
    ) -> Self {
        let password = model.read(cx).password(&profile.id);
        let mut fields = vec![
            ("source-name", profile.name.clone(), "Source name", false),
            ("source-host", profile.host.clone(), "localhost", false),
            ("source-port", profile.port.to_string(), "3306", false),
            ("source-user", profile.username.clone(), "root", false),
            ("source-password", password, "Password", true),
            (
                "source-database",
                profile.database.clone().unwrap_or_default(),
                "Optional database",
                false,
            ),
            (
                "source-ca",
                profile.ca_path.clone().unwrap_or_default(),
                "Optional CA file path",
                false,
            ),
            ("source-tunnel-host", "localhost".into(), "localhost", false),
            ("source-tunnel-port", "22".into(), "22", false),
            (
                "source-tunnel-user",
                std::env::var("USER").unwrap_or_default(),
                "SSH user",
                false,
            ),
            (
                "source-tunnel-key",
                String::new(),
                "Optional identity file path",
                false,
            ),
            ("source-proxy-host", "localhost".into(), "localhost", false),
            ("source-proxy-port", "8080".into(), "8080", false),
            ("source-https-port", "443".into(), "443", false),
            (
                "source-known-hosts",
                String::new(),
                "Optional known_hosts file",
                false,
            ),
        ];
        let transport = match &profile.transport {
            Transport::Direct => 0,
            Transport::Ssh {
                host,
                port,
                user,
                identity_file,
                known_hosts_file,
            } => {
                fields[7].1 = host.clone();
                fields[8].1 = port.to_string();
                fields[9].1 = user.clone();
                fields[10].1 = identity_file.clone().unwrap_or_default();
                fields[14].1 = known_hosts_file.clone().unwrap_or_default();
                1
            }

            Transport::HttpConnect { host, port, https } => {
                fields[11].1 = host.clone();
                fields[if *https { 13 } else { 12 }].1 = port.to_string();
                if *https { 3 } else { 2 }
            }
        };
        let mut inputs = HashMap::new();
        let mut last_values = HashMap::new();
        let mut subscriptions = Vec::new();
        for (id, value, placeholder, secret) in fields {
            let input = cx.new(|cx| TextInput::new(value.clone(), placeholder, secret, cx));
            last_values.insert(id, value);
            subscriptions.push(cx.observe(&input, move |this, input, cx| {
                let value = input.read(cx).value();
                if this.model.read(cx).saving {
                    if let Some(previous) = this
                        .last_values
                        .get(id)
                        .filter(|previous| **previous != value)
                        .cloned()
                    {
                        input.update(cx, |input, cx| input.set_value(previous, cx));
                    }
                    return;
                }
                // TextInput also notifies for cursor/selection movement. Only text edits
                // invalidate a running test; no draft values are logged or formatted.
                if this.last_values.get(id) != Some(&value) {
                    this.last_values.insert(id, value);
                    this.model.update(cx, |model, cx| model.edit_form(cx));
                }
                cx.notify();
            }));
            inputs.insert(id, input);
        }
        subscriptions.push(cx.observe(&model, |_, _, cx| cx.notify()));
        let controls = [
            "source-engine-mysql",
            "source-engine-mariadb",
            "source-direct",
            "source-ssh",
            "source-http",
            "source-https",
            "source-tls-verify",
            "source-tls-disabled",
            "source-save-password",
            "source-test",
            "source-save",
            "source-cancel",
            "source-keys",
            "source-ca-browse",
        ]
        .into_iter()
        .map(|id| (id, cx.focus_handle().tab_stop(true)))
        .collect();
        Self {
            engine: profile.engine,
            tls: profile.tls,
            save_password: profile.save_password,
            original: profile,
            model,
            inputs,
            last_values,
            controls,
            root_focus: cx.focus_handle().tab_stop(false),
            transport,
            ssh_keys: vec![],
            key_picker_open: false,
            key_picker_busy: false,
            key_picker_error: None,
            ca_picker_open: false,
            _subscriptions: subscriptions,
        }
    }

    fn value(&self, id: &'static str, cx: &App) -> String {
        self.inputs[id].read(cx).value().trim().to_owned()
    }

    pub(super) fn password(&self, cx: &App) -> String {
        self.inputs["source-password"].read(cx).value()
    }

    pub(super) fn focus(&self, window: &mut Window, cx: &App) {
        self.inputs["source-name"]
            .read(cx)
            .focus_handle()
            .focus(window);
    }

    pub(super) fn profile(&self, cx: &App) -> Result<SourceProfile> {
        let mut profile = self.original.clone();
        profile.name = self.value("source-name", cx);
        profile.host = self.value("source-host", cx);
        profile.port = parse_port(&self.value("source-port", cx), "Database")?;
        profile.username = self.value("source-user", cx);
        profile.database = optional(self.value("source-database", cx));
        profile.ca_path = optional(self.value("source-ca", cx));
        profile.engine = self.engine;
        profile.tls = self.tls;
        profile.save_password = self.save_password;
        profile.transport = match self.transport {
            1 => Transport::Ssh {
                host: self.value("source-tunnel-host", cx),
                port: parse_port(&self.value("source-tunnel-port", cx), "SSH")?,
                user: self.value("source-tunnel-user", cx),
                identity_file: optional(self.value("source-tunnel-key", cx)),
                known_hosts_file: optional(self.value("source-known-hosts", cx)),
            },
            2 | 3 => Transport::HttpConnect {
                host: self.value("source-proxy-host", cx),
                port: parse_port(
                    &self.value(
                        if self.transport == 3 {
                            "source-https-port"
                        } else {
                            "source-proxy-port"
                        },
                        cx,
                    ),
                    "Proxy",
                )?,
                https: self.transport == 3,
            },
            _ => Transport::Direct,
        };
        profile.validate()?;
        Ok(profile)
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        if self.model.read(cx).saving {
            return;
        }
        self.inputs["source-password"].update(cx, |input, cx| input.set_value("", cx));
        self.last_values.insert("source-password", String::new());
        self.model.update(cx, |model, cx| model.close_form(cx));
    }

    fn activate(&mut self, id: &'static str, cx: &mut Context<Self>) {
        if self.model.read(cx).saving {
            return;
        }
        match id {
            "source-keys" => {
                self.key_picker_open = !self.key_picker_open;
                if self.key_picker_open && !self.key_picker_busy {
                    self.key_picker_busy = true;
                    self.key_picker_error = None;
                    let task = cx.background_executor().spawn(async move {
                        let directory = dalan_app::ssh_keys::user_ssh_directory()?;
                        dalan_app::ssh_keys::discover(&directory)
                    });
                    cx.spawn(async move |this, cx| {
                        let result = task.await;
                        let _ = this.update(cx, |this, cx| {
                            this.key_picker_busy = false;
                            match result {
                                Ok(keys) => this.ssh_keys = keys,
                                Err(error) => this.key_picker_error = Some(error.to_string()),
                            }
                            cx.notify();
                        });
                    })
                    .detach();
                }
                cx.notify();
                return;
            }
            "source-cancel" => {
                self.cancel(cx);
                return;
            }
            "source-test" | "source-save" => {
                if self.model.read(cx).form_busy {
                    return;
                }
                match self.profile(cx) {
                    Ok(profile) => {
                        let password = self.password(cx);
                        self.model.update(cx, |model, cx| {
                            if id == "source-save" {
                                model.save(profile, password, cx);
                            } else {
                                model.test(profile, password, cx);
                            }
                        });
                    }
                    Err(error) => self.model.update(cx, |model, cx| {
                        model.form_feedback = Some(error.to_string());
                        cx.notify();
                    }),
                }
                return;
            }
            "source-engine-mysql" => self.engine = DbEngine::MySql,
            "source-engine-mariadb" => self.engine = DbEngine::MariaDb,
            "source-direct" => self.transport = 0,
            "source-ssh" => self.transport = 1,
            "source-http" => self.transport = 2,
            "source-https" => self.transport = 3,
            "source-tls-verify" => self.tls = TlsMode::VerifyIdentity,
            "source-tls-disabled" => self.tls = TlsMode::Disabled,
            "source-save-password" => self.save_password = !self.save_password,
            _ => return,
        }
        self.model.update(cx, |model, cx| model.edit_form(cx));
        cx.notify();
    }

    fn button(
        &self,
        id: &'static str,
        label: &'static str,
        selected: bool,
        _index: isize,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let disabled = matches!(id, "source-save" | "source-test") && self.model.read(cx).form_busy;
        div()
            .id(id)
            .debug_selector(move || id.into())
            .track_focus(&self.controls[id])
            .flex()
            .items_center()
            .justify_center()
            .px(px(10.))
            .h(px(30.))
            .rounded(px(4.))
            .border_1()
            .border_color(rgb(if selected { FOCUS } else { CHROME }))
            .bg(rgb(if selected { SELECTION } else { CHROME }))
            .text_color(rgb(if disabled { MUTED } else { TEXT }))
            .when(disabled, |el| el.opacity(0.5))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(HOVER)))
            .focus(|style| style.border_color(rgb(FOCUS)))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.controls[id].focus(window);
                this.activate(id, cx);
            }))
            .on_key_down(
                cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                    if this.controls[id].is_focused(window)
                        && matches!(event.keystroke.key.as_str(), "enter" | "space")
                    {
                        cx.stop_propagation();
                        this.activate(id, cx);
                    }
                }),
            )
            .child(label)
    }

    fn row(&self, label: &'static str, content: impl IntoElement) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .w_full()
            .child(
                div()
                    .w(px(140.))
                    .flex_shrink_0()
                    .text_color(rgb(MUTED))
                    .child(label),
            )
            .child(div().flex_1().min_w(px(0.)).child(content))
    }

    fn field(&self, id: &'static str, label: &'static str, _index: isize, _cx: &App) -> Div {
        // Only TextInput tracks its handle. Equal indices follow the visual tree order.
        self.row(
            label,
            div()
                .id(id)
                .debug_selector(move || id.into())
                .w_full()
                .child(self.inputs[id].clone()),
        )
    }

    fn checkbox(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let disabled = self.model.read(cx).saving;
        div()
            .id("source-save-password")
            .debug_selector(|| "source-save-password".into())
            .track_focus(&self.controls["source-save-password"])
            .h(px(30.0))
            .px(px(4.0))
            .flex()
            .items_center()
            .gap(px(7.0))
            .flex_shrink_0()
            .rounded(px(4.0))
            .border_1()
            .border_color(rgb(PANEL))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(HEADER)))
            .focus(|style| style.border_color(rgb(FOCUS)))
            .when(disabled, |style| style.opacity(0.5))
            .on_click(cx.listener(|this, _, window, cx| {
                this.controls["source-save-password"].focus(window);
                this.activate("source-save-password", cx);
            }))
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "space" | "enter") {
                    cx.stop_propagation();
                    this.activate("source-save-password", cx);
                }
            }))
            .child(
                div()
                    .id("keychain-checkbox-indicator")
                    .debug_selector(|| "keychain-checkbox-indicator".into())
                    .size(px(18.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(3.0))
                    .border_1()
                    .border_color(rgb(if self.save_password { FOCUS } else { MUTED }))
                    .bg(rgb(if self.save_password { SELECTION } else { PANEL }))
                    .when(self.save_password, |indicator| {
                        indicator.child(
                            div()
                                .id("keychain-checkbox-check")
                                .child(icon(Icon::Check, TEXT)),
                        )
                    }),
            )
            .child(
                div()
                    .id("keychain-checkbox-label")
                    .debug_selector(|| "keychain-checkbox-label".into())
                    .text_size(px(12.0))
                    .child("Save in Keychain"),
            )
    }

    fn ca_picker_options() -> gpui::PathPromptOptions {
        gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose a database CA certificate file".into()),
        }
    }

    fn finish_ca_pick(
        &mut self,
        selection: Result<Option<Vec<std::path::PathBuf>>>,
        original: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ca_picker_open = false;
        if self.model.read(cx).saving || !self.model.read(cx).form_open {
            cx.notify();
            return;
        }
        match selection {
            Ok(Some(paths)) if paths.len() == 1 => {
                if self.inputs["source-ca"].read(cx).value() != original {
                    self.model.update(cx, |model, cx| { model.form_feedback = Some("CA selection ignored because the path was edited while the dialog was open.".into()); cx.notify(); });
                } else if let Some(path) = paths[0].to_str() {
                    self.inputs["source-ca"]
                        .update(cx, |input, cx| input.set_value(path.to_owned(), cx));
                    self.last_values.insert("source-ca", path.to_owned());
                    self.model.update(cx, |model, cx| model.edit_form(cx));
                } else {
                    self.model.update(cx, |model, cx| {
                        model.form_feedback = Some(
                            "The selected CA path is not UTF-8; enter another path manually."
                                .into(),
                        );
                        cx.notify();
                    });
                }
            }
            Ok(None) => {}
            Ok(Some(_)) => self.model.update(cx, |model, cx| {
                model.form_feedback = Some("Choose one CA certificate file.".into());
                cx.notify();
            }),
            Err(_) => self.model.update(cx, |model, cx| {
                model.form_feedback = Some(
                    "Could not open the CA file dialog. You can enter its path manually.".into(),
                );
                cx.notify();
            }),
        }
        self.inputs["source-ca"]
            .read(cx)
            .focus_handle()
            .focus(window);
        cx.notify();
    }

    fn browse_ca(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ca_picker_open || self.model.read(cx).saving {
            return;
        }
        self.ca_picker_open = true;
        let original = self.inputs["source-ca"].read(cx).value();
        let picker = cx.prompt_for_paths(Self::ca_picker_options());
        cx.spawn_in(window, async move |this, cx| {
            let selection = picker
                .await
                .map_err(|_| anyhow::anyhow!("File dialog stopped"))
                .and_then(|result| result);
            let _ = this.update_in(cx, |this, window, cx| {
                this.finish_ca_pick(selection, original, window, cx)
            });
        })
        .detach();
        cx.notify();
    }

    fn ca_browse_button(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("source-ca-browse")
            .debug_selector(|| "source-ca-browse".into())
            .track_focus(&self.controls["source-ca-browse"])
            .h(px(30.0))
            .px(px(10.0))
            .flex()
            .items_center()
            .justify_center()
            .flex_shrink_0()
            .border_1()
            .border_color(rgb(CHROME))
            .rounded(px(4.0))
            .bg(rgb(CHROME))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(HOVER)))
            .focus(|style| style.border_color(rgb(FOCUS)))
            .when(self.ca_picker_open || self.model.read(cx).saving, |style| {
                style.opacity(0.5)
            })
            .on_click(cx.listener(|this, _, window, cx| this.browse_ca(window, cx)))
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "space" | "enter") {
                    cx.stop_propagation();
                    this.browse_ca(window, cx);
                }
            }))
            .child("Browse…")
    }

    fn choose_key(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.model.read(cx).saving {
            return;
        }
        self.inputs["source-tunnel-key"].update(cx, |input, cx| input.set_value(path.clone(), cx));
        self.last_values.insert("source-tunnel-key", path);
        self.model.update(cx, |model, cx| model.edit_form(cx));
        self.key_picker_open = false;
        self.inputs["source-tunnel-key"]
            .read(cx)
            .focus_handle()
            .focus(window);
        cx.notify();
    }

    fn key_row(
        &self,
        id: impl Into<gpui::SharedString>,
        label: String,
        path: String,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let id = id.into();
        let selector = id.clone();
        let keyboard_path = path.clone();
        div()
            .id(id)
            .debug_selector(move || selector.to_string())
            .tab_index(0)
            .w_full()
            .h(px(30.0))
            .flex()
            .items_center()
            .px(px(8.0))
            .rounded(px(4.0))
            .border_1()
            .border_color(rgb(CHROME))
            .bg(rgb(CHROME))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(HOVER)))
            .focus(|style| style.border_color(rgb(FOCUS)))
            .on_click(
                cx.listener(move |this, _, window, cx| this.choose_key(path.clone(), window, cx)),
            )
            .on_key_down(
                cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        cx.stop_propagation();
                        this.choose_key(keyboard_path.clone(), window, cx);
                    }
                }),
            )
            .child(label)
    }
}

impl Render for SourceForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let feedback = self.model.read(cx).form_feedback.clone();
        let busy = self.model.read(cx).form_busy;
        let mut body = div()
            .w_full()
            .max_w(px(720.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                self.row(
                    "Engine",
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(self.button(
                            "source-engine-mysql",
                            "MySQL",
                            self.engine == DbEngine::MySql,
                            1,
                            cx,
                        ))
                        .child(self.button(
                            "source-engine-mariadb",
                            "MariaDB",
                            self.engine == DbEngine::MariaDb,
                            2,
                            cx,
                        )),
                ),
            )
            .child(self.field("source-name", "Name", 3, cx))
            .child(self.field("source-host", "Host", 4, cx))
            .child(self.field("source-port", "Port", 5, cx))
            .child(self.field("source-user", "User", 6, cx))
            .child(
                self.row(
                    "Password",
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .id("source-password")
                                .debug_selector(|| "source-password".into())
                                .flex_1()
                                .min_w(px(120.0))
                                .child(self.inputs["source-password"].clone()),
                        )
                        .child(self.checkbox(cx)),
                ),
            )
            .child(self.field("source-database", "Database (optional)", 8, cx))
            .child(
                self.row(
                    "Transport",
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(8.))
                        .child(self.button("source-direct", "Direct", self.transport == 0, 9, cx))
                        .child(self.button("source-ssh", "SSH", self.transport == 1, 10, cx))
                        .child(self.button(
                            "source-http",
                            "HTTP CONNECT",
                            self.transport == 2,
                            11,
                            cx,
                        ))
                        .child(self.button(
                            "source-https",
                            "HTTPS CONNECT",
                            self.transport == 3,
                            12,
                            cx,
                        )),
                ),
            );
        if self.transport == 1 {
            body = body
                .child(self.field("source-tunnel-host", "SSH host", 13, cx))
                .child(self.field("source-tunnel-port", "SSH port", 14, cx))
                .child(self.field("source-tunnel-user", "SSH user", 15, cx))
                .child(self.field("source-tunnel-key", "Identity file", 16, cx))
                .child(self.field("source-known-hosts", "Known hosts file", 17, cx))
                .child(self.row(
                    "",
                    self.button("source-keys", "SSH keys…", self.key_picker_open, 16, cx),
                ));
            if self.key_picker_open {
                let mut picker = div()
                    .id("ssh-key-picker")
                    .debug_selector(|| "ssh-key-picker".into())
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .p(px(8.0))
                    .bg(rgb(HEADER))
                    .child(self.key_row(
                        "ssh-use-agent",
                        "Use SSH agent (no explicit identity file)".into(),
                        String::new(),
                        cx,
                    ));
                if self.key_picker_busy {
                    picker = picker.child("Listing ~/.ssh identity filenames…");
                }
                if let Some(error) = &self.key_picker_error {
                    picker = picker.child(error.clone());
                }
                if !self.key_picker_busy && self.ssh_keys.is_empty() {
                    picker = picker
                        .child("No candidate keys found. You can enter an identity path manually.");
                }
                picker = picker.child(
                    div()
                        .id("ssh-key-list")
                        .max_h(px(150.0))
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .children(self.ssh_keys.iter().enumerate().map(|(index, key)| {
                            self.key_row(
                                format!("ssh-key-{index}"),
                                key.name.clone(),
                                key.path.to_string_lossy().into_owned(),
                                cx,
                            )
                        })),
                );
                picker = picker.child(div().text_size(px(12.0)).text_color(rgb(MUTED))
                    .child("Candidates are listed by filename only. Unlock encrypted keys with ssh-add; private key contents are not read by this picker."));
                body = body.child(self.row("", picker));
            }
        } else if self.transport == 2 || self.transport == 3 {
            body = body
                .child(self.field("source-proxy-host", "Proxy host", 13, cx))
                .child(self.field(
                    if self.transport == 3 {
                        "source-https-port"
                    } else {
                        "source-proxy-port"
                    },
                    "Proxy port",
                    14,
                    cx,
                ));
        }
        body = body
            .child(
                self.row(
                    "TLS",
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(self.button(
                            "source-tls-verify",
                            "Verify identity",
                            self.tls == TlsMode::VerifyIdentity,
                            17,
                            cx,
                        ))
                        .child(self.button(
                            "source-tls-disabled",
                            "Disabled",
                            self.tls == TlsMode::Disabled,
                            18,
                            cx,
                        )),
                ),
            )
            .when(self.tls == TlsMode::Disabled, |body| {
                body.child(div().text_color(rgb(0xf2bf76)).child(
                    "Warning: database TLS is disabled. Traffic is not protected by database TLS.",
                ))
            })
            .child(
                self.row(
                    "CA file (optional)",
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .id("source-ca")
                                .debug_selector(|| "source-ca".into())
                                .flex_1()
                                .min_w(px(0.0))
                                .child(self.inputs["source-ca"].clone()),
                        )
                        .child(self.ca_browse_button(cx)),
                ),
            )
            .when_some(feedback, |body, feedback| {
                body.child(
                    div()
                        .id("source-feedback")
                        .debug_selector(|| "source-feedback".into())
                        .text_color(rgb(MUTED))
                        .child(feedback),
                )
            })
            .when(busy, |body| {
                body.child(div().text_color(rgb(MUTED)).child("Working…"))
            });
        div()
            .id("source-form")
            .debug_selector(|| "source-form".into())
            .track_focus(&self.root_focus)
            .tab_group()
            .tab_stop(false)
            .key_context("SourceForm")
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(PANEL))
            .text_color(rgb(TEXT))
            .text_size(px(13.))
            .on_action(cx.listener(|_, _: &NextFocus, window, cx| {
                cx.stop_propagation();
                window.focus_next();
            }))
            .on_action(cx.listener(|_, _: &PreviousFocus, window, cx| {
                cx.stop_propagation();
                window.focus_prev();
            }))
            .on_action(cx.listener(|this, _: &Dismiss, _, cx| {
                cx.stop_propagation();
                this.cancel(cx);
            }))
            .child(
                div()
                    .id("source-form-scroll")
                    .debug_selector(|| "source-form-scroll".into())
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .p(px(20.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(body),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .p(px(16.))
                    .flex()
                    .justify_between()
                    .gap(px(8.))
                    .child(self.button("source-test", "Test Connection", false, 21, cx))
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(self.button("source-save", "Save", true, 22, cx))
                            .child(self.button("source-cancel", "Cancel", false, 23, cx)),
                    ),
            )
    }
}

fn optional(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

fn parse_port(value: &str, label: &str) -> Result<u16> {
    let port = value
        .trim()
        .parse::<u16>()
        .with_context(|| format!("{label} port must be a number from 1 to 65535"))?;
    ensure!(port != 0, "{label} port must be a number from 1 to 65535");
    Ok(port)
}

#[cfg(all(test, feature = "ui-tests"))]
mod tests {
    use super::*;

    #[test]
    fn empty_database_is_nullable_and_ports_are_checked() {
        assert_eq!(optional(String::new()), None);
        assert_eq!(optional("db".into()), Some("db".into()));
        assert_eq!(parse_port(" 3306 ", "Database").unwrap(), 3306);
        for value in ["", "zero", "0", "65536", "-1", "22.5"] {
            assert!(parse_port(value, "Database").is_err());
        }
        let profile = SourceProfile::default();
        assert_eq!(profile.database, None);
        assert_eq!(profile.tls, TlsMode::VerifyIdentity);
        assert!(profile.validate().is_ok());
    }
    use gpui::{Modifiers, TestAppContext, VisualTestContext};

    fn fixture(
        cx: &mut TestAppContext,
    ) -> (
        Entity<SourceForm>,
        Entity<SourceModel>,
        &mut VisualTestContext,
    ) {
        cx.update(crate::desktop::bind_keys);
        let model = cx.new(|_| SourceModel::for_tests(vec![]));
        model.update(cx, |model, cx| model.new_source(cx));
        let profile = model.read_with(cx, |model, _| model.form_profile.clone().unwrap());
        let (form, visual) =
            cx.add_window_view(|_, cx| SourceForm::new(profile, model.clone(), cx));
        visual.simulate_resize(gpui::size(px(1000.), px(1100.)));
        visual.update(|window, app| form.read(app).root_focus.focus(window));
        visual.refresh().unwrap();
        visual.run_until_parked();
        (form, model, visual)
    }

    fn click(cx: &mut VisualTestContext, id: &'static str) {
        cx.run_until_parked();
        let bounds = cx
            .debug_bounds(id)
            .unwrap_or_else(|| panic!("missing {id}"));
        cx.simulate_click(bounds.center(), Modifiers::default());
        cx.run_until_parked();
    }

    fn set(form: &Entity<SourceForm>, cx: &mut VisualTestContext, id: &'static str, value: &str) {
        let input = form.read_with(cx, |form, _| form.inputs[id].clone());
        input.update(cx, |input, cx| input.set_value(value.to_owned(), cx));
        cx.run_until_parked();
    }

    #[gpui::test]
    fn default_draft_and_native_field_edits(cx: &mut TestAppContext) {
        let (form, model, cx) = fixture(cx);
        let profile = cx.update(|_, app| form.read(app).profile(app).unwrap());
        assert_eq!(profile.host, "localhost");
        assert_eq!(profile.port, 3306);
        assert_eq!(profile.database, None);
        assert_eq!(profile.ca_path, None);
        assert!(model.read_with(cx, |model, _| model.form_open));
        click(cx, "source-host");
        cx.simulate_keystrokes("cmd-a");
        cx.simulate_input("db.internal");
        for (id, value) in [
            ("source-name", "  Development  "),
            ("source-port", "3307"),
            ("source-user", "reader"),
            ("source-database", "  inventory  "),
        ] {
            set(&form, cx, id, value);
        }
        let profile = cx.update(|_, app| form.read(app).profile(app).unwrap());
        assert_eq!(profile.host, "db.internal");
        assert_eq!(profile.port, 3307);
        assert_eq!(profile.name, "Development");
        assert_eq!(profile.username, "reader");
        assert_eq!(profile.database.as_deref(), Some("inventory"));
        assert!(model.read_with(cx, |model, _| model.profiles.is_empty()));
    }

    #[gpui::test]
    fn engine_transport_tls_and_credential_controls(cx: &mut TestAppContext) {
        let (form, _, cx) = fixture(cx);
        click(cx, "source-engine-mariadb");
        assert_eq!(form.read_with(cx, |form, _| form.engine), DbEngine::MariaDb);
        click(cx, "source-engine-mysql");
        click(cx, "source-ssh");
        assert!(cx.debug_bounds("source-tunnel-host").is_some());
        set(&form, cx, "source-tunnel-user", "tunnel-user");
        assert!(matches!(
            cx.update(|_, app| form.read(app).profile(app).unwrap().transport),
            Transport::Ssh { port: 22, .. }
        ));
        click(cx, "source-http");
        assert!(matches!(
            cx.update(|_, app| form.read(app).profile(app).unwrap().transport),
            Transport::HttpConnect {
                port: 8080,
                https: false,
                ..
            }
        ));
        click(cx, "source-https");
        assert!(matches!(
            cx.update(|_, app| form.read(app).profile(app).unwrap().transport),
            Transport::HttpConnect {
                port: 443,
                https: true,
                ..
            }
        ));
        click(cx, "source-direct");
        click(cx, "source-tls-disabled");
        assert_eq!(form.read_with(cx, |form, _| form.tls), TlsMode::Disabled);
        click(cx, "source-tls-verify");
        click(cx, "source-save-password");
        assert!(form.read_with(cx, |form, _| form.save_password));
        click(cx, "source-save-password");
        let profile = cx.update(|_, app| form.read(app).profile(app).unwrap());
        assert_eq!(profile.engine, DbEngine::MySql);
        assert_eq!(profile.tls, TlsMode::VerifyIdentity);
        assert_eq!(profile.transport, Transport::Direct);
        assert!(!profile.save_password);
    }

    #[gpui::test]
    fn invalid_test_and_unavailable_save_are_local_and_cancel_clears_secret(
        cx: &mut TestAppContext,
    ) {
        let (form, model, cx) = fixture(cx);
        set(&form, cx, "source-password", "session-only-secret");
        for value in ["0", "65536", "not-a-port"] {
            set(&form, cx, "source-port", value);
            click(cx, "source-test");
            model.read_with(cx, |model, _| {
                assert!(
                    model
                        .form_feedback
                        .as_deref()
                        .unwrap()
                        .contains("Database port")
                );
                assert!(!model.form_busy);
                assert!(!model.busy);
            });
        }
        set(&form, cx, "source-port", "3306");
        click(cx, "source-save-password");
        click(cx, "source-save");
        model.read_with(cx, |model, _| {
            assert_eq!(
                model.form_feedback.as_deref(),
                Some("Profile storage is unavailable; nothing was saved.")
            );
            assert!(model.form_open);
            assert!(!model.saving);
            assert!(model.profiles.is_empty());
        });
        click(cx, "source-cancel");
        assert_eq!(cx.update(|_, app| form.read(app).password(app)), "");
        model.read_with(cx, |model, _| {
            assert!(!model.form_open);
            assert!(model.form_profile.is_none());
            assert!(model.form_feedback.is_none());
        });
    }

    fn assert_input_focus(form: &Entity<SourceForm>, cx: &mut VisualTestContext, id: &'static str) {
        cx.run_until_parked();
        assert!(
            cx.update(|window, app| {
                form.read(app).inputs[id]
                    .read(app)
                    .focus_handle()
                    .is_focused(window)
            }),
            "expected focus on {id}"
        );
    }

    #[gpui::test]
    fn tab_and_shift_tab_follow_inputs_without_duplicate_stops(cx: &mut TestAppContext) {
        let (form, _, cx) = fixture(cx);
        let fields = [
            "source-name",
            "source-host",
            "source-port",
            "source-user",
            "source-password",
            "source-database",
        ];
        click(cx, fields[0]);
        for (i, id) in fields.iter().enumerate() {
            assert_input_focus(&form, cx, id);
            cx.simulate_keystrokes("cmd-a");
            cx.simulate_input(&format!("field-{i}"));
            // Rerendering must not replace the focused handle.
            form.update(cx, |_, cx| cx.notify());
            cx.run_until_parked();
            assert_input_focus(&form, cx, id);
            if i + 1 < fields.len() {
                cx.simulate_keystrokes("tab");
                if *id == "source-password" {
                    assert!(cx.update(|window, app| {
                        form.read(app).controls["source-save-password"].is_focused(window)
                    }));
                    cx.simulate_keystrokes("tab");
                }
            }
        }
        for id in fields.iter().rev().skip(1) {
            cx.simulate_keystrokes("shift-tab");
            if *id == "source-password" {
                assert!(cx.update(|window, app| {
                    form.read(app).controls["source-save-password"].is_focused(window)
                }));
                cx.simulate_keystrokes("shift-tab");
            }
            assert_input_focus(&form, cx, id);
        }
        for (i, id) in fields.iter().enumerate() {
            assert_eq!(
                form.read_with(cx, |form, app| form.inputs[id].read(app).value()),
                format!("field-{i}")
            );
        }
    }

    #[gpui::test]
    fn conditional_transport_inputs_follow_visual_order(cx: &mut TestAppContext) {
        let (form, _, cx) = fixture(cx);
        click(cx, "source-ssh");
        let fields = [
            "source-tunnel-host",
            "source-tunnel-port",
            "source-tunnel-user",
            "source-tunnel-key",
            "source-known-hosts",
        ];
        click(cx, fields[0]);
        for id in fields.iter().skip(1) {
            cx.simulate_keystrokes("tab");
            assert_input_focus(&form, cx, id);
        }
        for id in fields.iter().rev().skip(1) {
            cx.simulate_keystrokes("shift-tab");
            assert_input_focus(&form, cx, id);
        }
        click(cx, "source-direct");
        assert_eq!(form.read_with(cx, |form, _| form.transport), 0);
        click(cx, "source-database");
        // Transport and TLS controls remain keyboard-accessible, but hidden SSH
        // fields are not part of the tab sequence.
        for _ in 0..7 {
            cx.simulate_keystrokes("tab");
        }
        assert_input_focus(&form, cx, "source-ca");
        click(cx, "source-http");
        click(cx, "source-proxy-host");
        cx.simulate_keystrokes("tab");
        assert_input_focus(&form, cx, "source-proxy-port");
        click(cx, "source-https");
        click(cx, "source-proxy-host");
        cx.simulate_keystrokes("tab");
        assert_input_focus(&form, cx, "source-https-port");
    }

    #[gpui::test]
    fn keyboard_controls_and_escape_belong_to_form(cx: &mut TestAppContext) {
        let (form, model, cx) = fixture(cx);
        cx.simulate_keystrokes("tab");
        assert!(cx.update(|window, app| {
            form.read(app).controls["source-engine-mysql"].is_focused(window)
        }));
        cx.simulate_keystrokes("tab space");
        assert_eq!(form.read_with(cx, |form, _| form.engine), DbEngine::MariaDb);
        set(&form, cx, "source-password", "discard-me");
        cx.simulate_keystrokes("escape");
        assert!(!model.read_with(cx, |model, _| model.form_open));
        assert_eq!(cx.update(|_, app| form.read(app).password(app)), "");
    }
    #[gpui::test]
    fn key_picker_selects_identity_and_agent_without_reading_user_files(cx: &mut TestAppContext) {
        let (form, _, cx) = fixture(cx);
        click(cx, "source-ssh");
        form.update(cx, |form, cx| {
            form.key_picker_open = true;
            form.ssh_keys = vec![dalan_app::ssh_keys::SshKeyCandidate {
                name: "work key.pem".into(),
                path: std::path::PathBuf::from("/tmp/dalan-test/work key.pem"),
            }];
            cx.notify();
        });
        cx.run_until_parked();
        click(cx, "ssh-key-0");
        assert_eq!(
            form.read_with(cx, |form, app| form.inputs["source-tunnel-key"]
                .read(app)
                .value()),
            "/tmp/dalan-test/work key.pem"
        );
        assert_input_focus(&form, cx, "source-tunnel-key");
        assert!(!form.read_with(cx, |form, _| form.key_picker_open));
        form.update(cx, |form, cx| {
            form.key_picker_open = true;
            cx.notify();
        });
        cx.run_until_parked();
        let agent = cx.debug_bounds("ssh-use-agent").unwrap();
        cx.simulate_mouse_down(
            agent.center(),
            gpui::MouseButton::Left,
            Modifiers::default(),
        );
        cx.simulate_mouse_up(
            agent.center(),
            gpui::MouseButton::Left,
            Modifiers::default(),
        );
        cx.run_until_parked();
        assert_eq!(
            form.read_with(cx, |form, app| form.inputs["source-tunnel-key"]
                .read(app)
                .value()),
            ""
        );
        assert_input_focus(&form, cx, "source-tunnel-key");
    }
    #[gpui::test]
    fn checkbox_is_inline_clickable_and_keyboard_operable(cx: &mut TestAppContext) {
        let (form, model, cx) = fixture(cx);
        let password = cx.debug_bounds("source-password").unwrap();
        let checkbox = cx.debug_bounds("source-save-password").unwrap();
        assert!(checkbox.origin.x >= password.origin.x + password.size.width);
        assert!((f32::from(checkbox.origin.y) - f32::from(password.origin.y)).abs() <= 2.0);
        assert_eq!(
            cx.debug_bounds("keychain-checkbox-indicator").unwrap().size,
            gpui::size(px(18.0), px(18.0))
        );
        assert!(cx.debug_bounds("keychain-checkbox-label").is_some());
        assert!(!form.read_with(cx, |form, _| form.save_password));
        click(cx, "keychain-checkbox-label");
        assert!(form.read_with(cx, |form, _| form.save_password));
        assert!(cx.update(|_, app| form.read(app).profile(app).unwrap().save_password));
        cx.simulate_keystrokes("space");
        assert!(!form.read_with(cx, |form, _| form.save_password));
        cx.simulate_keystrokes("enter");
        assert!(form.read_with(cx, |form, _| form.save_password));
        model.update(cx, |model, cx| {
            model.saving = true;
            cx.notify();
        });
        click(cx, "source-save-password");
        cx.simulate_keystrokes("space");
        assert!(form.read_with(cx, |form, _| form.save_password));
    }

    #[gpui::test]
    fn ca_picked_path_stays_editable_and_cancel_preserves_manual_path(cx: &mut TestAppContext) {
        let (form, _, cx) = fixture(cx);
        let options = SourceForm::ca_picker_options();
        assert!(options.files && !options.directories && !options.multiple);
        assert!(cx.debug_bounds("source-ca-browse").is_some());
        let old = "/tmp/manual root.pem";
        let picked = "/tmp/selected root.pem";
        set(&form, cx, "source-ca", old);
        form.update(cx, |form, _| form.ca_picker_open = true);
        cx.update(|window, app| {
            form.update(app, |form, cx| {
                form.finish_ca_pick(Ok(Some(vec![picked.into()])), old.into(), window, cx)
            })
        });
        assert_eq!(
            form.read_with(cx, |form, app| form.inputs["source-ca"].read(app).value()),
            picked
        );
        assert_input_focus(&form, cx, "source-ca");
        cx.simulate_keystrokes("cmd-a");
        cx.simulate_input(old);
        assert_eq!(
            form.read_with(cx, |form, app| form.inputs["source-ca"].read(app).value()),
            old
        );
        cx.update(|window, app| {
            form.update(app, |form, cx| {
                form.finish_ca_pick(Ok(None), old.into(), window, cx)
            })
        });
        assert_eq!(
            form.read_with(cx, |form, app| form.inputs["source-ca"].read(app).value()),
            old
        );
        assert!(!form.read_with(cx, |form, _| form.ca_picker_open));
    }

    #[gpui::test]
    fn ca_picker_rejects_stale_selection_and_reports_dialog_errors(cx: &mut TestAppContext) {
        let (form, model, cx) = fixture(cx);
        set(&form, cx, "source-ca", "/tmp/newer.pem");
        cx.update(|window, app| {
            form.update(app, |form, cx| {
                form.finish_ca_pick(
                    Ok(Some(vec!["/tmp/picked.pem".into()])),
                    "/tmp/old.pem".into(),
                    window,
                    cx,
                )
            })
        });
        assert_eq!(
            form.read_with(cx, |form, app| form.inputs["source-ca"].read(app).value()),
            "/tmp/newer.pem"
        );
        assert!(model.read_with(cx, |model, _| {
            model.form_feedback.as_ref().unwrap().contains("edited")
        }));
        cx.update(|window, app| {
            form.update(app, |form, cx| {
                form.finish_ca_pick(
                    Err(anyhow::anyhow!("fixture")),
                    "/tmp/newer.pem".into(),
                    window,
                    cx,
                )
            })
        });
        assert!(model.read_with(cx, |model, _| {
            model
                .form_feedback
                .as_ref()
                .unwrap()
                .contains("enter its path manually")
        }));
        assert_eq!(
            form.read_with(cx, |form, app| form.inputs["source-ca"].read(app).value()),
            "/tmp/newer.pem"
        );
    }
}

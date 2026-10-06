use dalan_app::ssh_config_store::SshProfile;
#[cfg(not(feature = "ui-tests"))]
use dalan_app::ssh_config_store::SshRepository;
use std::collections::{HashMap, HashSet};

use anyhow::{Context as _, Result, ensure};
use dalan_drivers::sources::{
    Authentication, ConnectionMode, DbEngine, SchemaSelection, SourceProfile, TlsMode, Transport,
};
use gpui::{
    App, Context, Div, Entity, FocusHandle, Subscription, Window, div, prelude::*, px, rgb,
};

use gpui::component::{
    Disableable, Selectable, Sizable,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    menu::{DropdownMenu, PopupMenuItem},
    tab::{Tab, TabBar},
};

use super::{
    Dismiss, NextFocus, PreviousFocus, input::TextInput, source_model::SourceModel, theme::*,
};

const COLOR_PRESETS: [(&str, &str, &str, Option<u32>); 6] = [
    ("source-color-default", "Default", "", None),
    ("source-color-blue", "Blue", "#8AB4F8", Some(0x8AB4F8)),
    ("source-color-green", "Green", "#8CD4A4", Some(0x8CD4A4)),
    ("source-color-amber", "Amber", "#E7BD6A", Some(0xE7BD6A)),
    ("source-color-red", "Red", "#EE9296", Some(0xEE9296)),
    ("source-color-purple", "Purple", "#C7AAF5", Some(0xC7AAF5)),
];

/// A draft is kept separate from the persisted model until Save is activated.
pub(super) struct SourceForm {
    original: SourceProfile,
    model: Entity<SourceModel>,
    inputs: HashMap<&'static str, Entity<TextInput>>,
    last_values: HashMap<&'static str, String>,
    root_focus: FocusHandle,
    engine: DbEngine,
    transport: u8,
    tls: TlsMode,
    save_password: bool,
    ssh_profiles: Vec<SshProfile>,
    ssh_load_error: Option<String>,
    ssh_selected: Option<String>,
    ssh_catalog_revision: u64,
    ssh_combo_open: bool,
    selected_schema_names: HashSet<String>,
    schema_scroll: gpui::UniformListScrollHandle,
    ca_picker_open: bool,
    active_tab: u8,
    endpoint_mode: u8,
    authentication: Authentication,
    schemas_all: bool,
    driver_open: bool,
    authentication_open: bool,
    tls_open: bool,
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
            ("source-proxy-host", "localhost".into(), "localhost", false),
            ("source-proxy-port", "8080".into(), "8080", false),
            ("source-https-port", "443".into(), "443", false),
            (
                "source-color",
                profile.color.clone().unwrap_or_default(),
                "Default",
                false,
            ),
        ];
        let transport = match &profile.transport {
            Transport::Direct => 0,
            Transport::Ssh { .. } => 1,
            Transport::HttpConnect { host, port, https } => {
                fields[7].1 = host.clone();
                fields[if *https { 9 } else { 8 }].1 = port.to_string();
                if *https { 3 } else { 2 }
            }
        };
        // Append new fields: the transport initialization above deliberately keeps its indices.
        let endpoint_mode = match &profile.endpoint {
            ConnectionMode::Default => 0,
            ConnectionMode::UnixSocket { .. } => 1,
            ConnectionMode::UrlOnly { .. } => 2,
        };
        let socket = match &profile.endpoint {
            ConnectionMode::UnixSocket { path } => path.clone(),
            _ => String::new(),
        };
        let url = match &profile.endpoint {
            ConnectionMode::UrlOnly { url } => url.clone(),
            _ => profile.canonical_url().unwrap_or_default(),
        };
        let schemas = match &profile.schemas {
            SchemaSelection::All => String::new(),
            SchemaSelection::Selected(names) => names.join(", "),
        };
        fields.extend([
            ("source-socket", socket, "/tmp/mysql.sock", false),
            ("source-url", url, "mysql://localhost:3306/", false),
            (
                "source-connect-timeout",
                profile.options.connect_timeout_seconds.to_string(),
                "10",
                false,
            ),
            (
                "source-query-timeout",
                profile.options.query_timeout_seconds.to_string(),
                "20",
                false,
            ),
            (
                "source-page-size",
                profile.options.page_size.to_string(),
                "100",
                false,
            ),
            (
                "source-client-cert",
                profile.ssl_client_cert.clone().unwrap_or_default(),
                "Optional client certificate path",
                false,
            ),
            (
                "source-client-key",
                profile.ssl_client_key.clone().unwrap_or_default(),
                "Optional client key path",
                false,
            ),
            ("source-schemas", schemas, "schema_one, schema_two", false),
            (
                "source-schema-search",
                String::new(),
                "Filter schemas",
                false,
            ),
        ]);
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
                    if id == "source-schemas" {
                        this.selected_schema_names = value
                            .split(',')
                            .map(str::trim)
                            .filter(|name| !name.is_empty())
                            .map(str::to_owned)
                            .collect();
                    }
                    this.last_values.insert(id, value);
                    if id == "source-url" {
                        this.endpoint_mode = 2;
                    } else if matches!(id, "source-host" | "source-port" | "source-database") {
                        this.refresh_generated_url(cx);
                    }
                    if id != "source-schema-search" {
                        this.model.update(cx, |model, cx| model.edit_form(cx));
                    }
                }
                cx.notify();
            }));
            inputs.insert(id, input);
        }
        subscriptions.push(cx.observe(&model, |_, _, cx| cx.notify()));
        #[cfg(not(feature = "ui-tests"))]
        {
            let task = cx.background_executor().spawn(async {
                SshRepository::default_path().and_then(|path| SshRepository::new(path).load())
            });
            cx.spawn(async move |this, cx| {
                let result = task.await;
                let _ = this.update(cx, |this, cx| {
                    match result {
                        Ok(profiles) => {
                            // A manager Apply/Use is newer than this startup load.
                            if this.ssh_catalog_revision == 0 {
                                this.refresh_ssh_configurations(profiles, cx);
                            }
                        }
                        Err(error) if this.ssh_catalog_revision == 0 => {
                            this.ssh_load_error = Some(error.to_string())
                        }
                        Err(_) => {}
                    }
                    cx.notify();
                });
            })
            .detach();
        }
        Self {
            ssh_catalog_revision: 0,
            ssh_selected: profile.ssh_configuration_id.clone(),
            ssh_profiles: Vec::new(),
            ssh_load_error: None,
            ssh_combo_open: false,
            selected_schema_names: match &profile.schemas {
                SchemaSelection::All => HashSet::new(),
                SchemaSelection::Selected(names) => names.iter().cloned().collect(),
            },
            schema_scroll: gpui::UniformListScrollHandle::new(),
            engine: profile.engine,
            tls: profile.tls,
            save_password: profile.save_password,
            active_tab: 0,
            endpoint_mode,
            authentication: profile.authentication,
            schemas_all: matches!(profile.schemas, SchemaSelection::All),
            driver_open: false,
            authentication_open: false,
            tls_open: false,
            original: profile,
            model,
            inputs,
            last_values,
            root_focus: cx.focus_handle().tab_stop(false),
            transport,
            ca_picker_open: false,
            _subscriptions: subscriptions,
        }
    }

    /// Publish persisted metadata without enabling SSH or choosing another session.
    pub(super) fn refresh_ssh_configurations(
        &mut self,
        profiles: Vec<SshProfile>,
        cx: &mut Context<Self>,
    ) {
        self.ssh_catalog_revision += 1;
        let selected_changed = self.ssh_selected.as_ref().is_some_and(|id| {
            self.ssh_profiles.iter().find(|p| &p.id == id) != profiles.iter().find(|p| &p.id == id)
        });
        self.ssh_profiles = profiles;
        self.ssh_load_error = None;
        if selected_changed && self.transport == 1 && !self.model.read(cx).saving {
            // Apply invalidates a completed/in-flight test, but never changes the route.
            self.model.update(cx, |model, cx| model.edit_form(cx));
        }
        cx.notify();
    }

    pub(super) fn can_use_ssh_session(&self, cx: &App) -> bool {
        !self.model.read(cx).saving && self.endpoint_mode != 1
    }

    /// The manager calls this only after persisting a configuration and choosing Use.
    pub(super) fn set_ssh_configuration(&mut self, profile: SshProfile, cx: &mut Context<Self>) {
        if !self.can_use_ssh_session(cx) {
            return;
        }
        self.ssh_catalog_revision += 1;
        self.transport = 1;
        self.ssh_selected = Some(profile.id.clone());
        self.ssh_combo_open = false;
        if let Some(existing) = self
            .ssh_profiles
            .iter_mut()
            .find(|existing| existing.id == profile.id)
        {
            *existing = profile;
        } else {
            self.ssh_profiles.push(profile);
        }
        self.model.update(cx, |model, cx| model.edit_form(cx));
        cx.notify();
    }

    fn ssh_profile_control(&self, cx: &mut Context<Self>) -> Div {
        let palette = colors(cx);
        let label = self
            .ssh_profiles
            .iter()
            .find(|profile| Some(&profile.id) == self.ssh_selected.as_ref())
            .map(|profile| {
                format!(
                    "{} · {}@{}:{}",
                    profile.name, profile.user, profile.host, profile.port
                )
            })
            .unwrap_or_else(|| {
                if self.ssh_selected.is_some() {
                    "Saved SSH session (unavailable)".into()
                } else {
                    "Select SSH session…".into()
                }
            });
        let profiles = self.ssh_profiles.clone();
        let selected = self.ssh_selected.clone();
        let entity = cx.entity().downgrade();
        let disabled = self.model.read(cx).saving || self.transport != 1 || self.endpoint_mode == 1;
        let trigger = Button::new("source-ssh-profile")
            .debug_selector(|| "source-ssh-profile".into())
            .label(label.clone())
            .tooltip(label)
            .small()
            .disabled(disabled)
            .dropdown_caret(true)
            .dropdown_menu(move |menu, _, _| {
                let mut menu = menu;
                if profiles.is_empty() {
                    menu = menu.item(
                        PopupMenuItem::element(|_, _| {
                            div().child("No SSH sessions — create one in Manage SSH Sessions")
                        })
                        .disabled(true),
                    );
                }
                for (index, profile) in profiles.iter().enumerate() {
                    let profile = profile.clone();
                    let entity = entity.clone();
                    menu = menu.item(
                        PopupMenuItem::element({
                            let label = format!(
                                "{} · {}@{}:{}",
                                profile.name, profile.user, profile.host, profile.port
                            );
                            move |_, _| {
                                div()
                                    .debug_selector(move || format!("source-ssh-profile-{index}"))
                                    .child(label.clone())
                            }
                        })
                        .checked(Some(&profile.id) == selected.as_ref())
                        .disabled(disabled)
                        .on_click(move |_, _, cx| {
                            let _ = entity.update(cx, |this, cx| {
                                this.set_ssh_configuration(profile.clone(), cx)
                            });
                        }),
                    );
                }
                menu
            });
        self.row(
            "SSH configuration",
            div()
                .child(
                    div()
                        .w_full()
                        .flex()
                        .flex_wrap()
                        .gap(px(8.))
                        .child(trigger)
                        .child(self.button(
                            "source-manage-ssh",
                            "Manage SSH Sessions",
                            false,
                            0,
                            cx,
                        )),
                )
                .when_some(self.ssh_load_error.clone(), |container, error| {
                    container.child(div().text_color(palette.error).child(error))
                }),
            cx,
        )
    }

    fn available_schemas(&self, cx: &App) -> Vec<String> {
        let model = self.model.read(cx);
        let mut names = if !model.form_databases.is_empty() {
            model.form_databases.clone()
        } else {
            model
                .tree
                .databases
                .get(&self.original.id)
                .cloned()
                .unwrap_or_default()
        };
        names.extend(self.selected_schema_names.iter().cloned());
        if let Some(database) = optional(self.value("source-database", cx)) {
            names.push(database);
        }
        names.sort();
        names.dedup();
        names
    }

    fn filtered_schemas(&self, cx: &App) -> Vec<String> {
        let search = self.value("source-schema-search", cx).to_lowercase();
        self.available_schemas(cx)
            .into_iter()
            .filter(|name| name.to_lowercase().contains(&search))
            .collect()
    }

    fn toggle_schema(&mut self, name: String, cx: &mut Context<Self>) {
        if self.model.read(cx).saving {
            return;
        }
        if self.schemas_all {
            self.selected_schema_names = self.available_schemas(cx).into_iter().collect();
            self.schemas_all = false;
        }
        if !self.selected_schema_names.remove(&name) {
            self.selected_schema_names.insert(name);
        }
        self.model.update(cx, |model, cx| model.edit_form(cx));
        cx.notify();
    }

    fn schema_list(&self, cx: &mut Context<Self>) -> Div {
        let palette = colors(cx);
        let count = self.filtered_schemas(cx).len();
        div()
            .child(
                gpui::uniform_list(
                    "source-schema-list",
                    count,
                    cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                        let names = this.filtered_schemas(cx);
                        range
                            .map(|index| {
                                let name = names[index].clone();
                                let checked =
                                    this.schemas_all || this.selected_schema_names.contains(&name);
                                let label = name.clone();
                                Checkbox::new(gpui::SharedString::from(format!(
                                    "source-schema-{index}"
                                )))
                                .debug_selector(move || format!("source-schema-{index}"))
                                .label(label)
                                .checked(checked)
                                .small()
                                .h(px(30.))
                                .w_full()
                                .disabled(this.model.read(cx).saving)
                                .on_change(cx.listener(
                                    move |this, _, _, cx| this.toggle_schema(name.clone(), cx),
                                ))
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .debug_selector(|| "source-schema-list".into())
                .h(px((count * 30).clamp(120, 260) as f32))
                .track_scroll(&self.schema_scroll),
            )
            .when(count == 0, |container| {
                container.child(
                    div()
                        .text_color(palette.muted)
                        .child("No matching schemas. Fetch schemas or add names below."),
                )
            })
    }

    fn value(&self, id: &'static str, cx: &App) -> String {
        self.inputs[id].read(cx).value().trim().to_owned()
    }

    pub(super) fn password(&self, cx: &App) -> String {
        if self.authentication == Authentication::NoAuth {
            String::new()
        } else {
            self.inputs["source-password"].read(cx).value()
        }
    }

    pub(super) fn preferred_first_focus(&self, cx: &App) -> FocusHandle {
        self.inputs["source-name"].read(cx).focus_handle()
    }

    pub(super) fn profile(&self, cx: &App) -> Result<SourceProfile> {
        let mut profile = self.original.clone();
        profile.name = self.value("source-name", cx);
        profile.color = optional(self.value("source-color", cx));
        profile.host = self.value("source-host", cx);
        profile.port = if self.endpoint_mode == 0 {
            parse_port(&self.value("source-port", cx), "Database")?
        } else {
            self.original.port
        };
        profile.authentication = self.authentication;
        profile.username = if self.authentication == Authentication::NoAuth {
            String::new()
        } else {
            self.value("source-user", cx)
        };
        profile.endpoint = match self.endpoint_mode {
            1 => ConnectionMode::UnixSocket {
                path: self.value("source-socket", cx),
            },
            2 => ConnectionMode::UrlOnly {
                url: self.value("source-url", cx),
            },
            _ => ConnectionMode::Default,
        };
        profile.options.connect_timeout_seconds = self
            .value("source-connect-timeout", cx)
            .parse()
            .context("Connect timeout must be a whole number of seconds")?;
        profile.options.query_timeout_seconds = self
            .value("source-query-timeout", cx)
            .parse()
            .context("Query timeout must be a whole number of seconds")?;
        profile.options.page_size = self
            .value("source-page-size", cx)
            .parse()
            .context("Page size must be a whole number")?;
        profile.ssl_client_cert = optional(self.value("source-client-cert", cx));
        profile.ssl_client_key = optional(self.value("source-client-key", cx));
        profile.ssh_configuration_id = if self.transport == 1 {
            self.ssh_selected.clone()
        } else {
            None
        };
        profile.schemas = if self.schemas_all {
            SchemaSelection::All
        } else {
            let mut names: Vec<_> = self.selected_schema_names.iter().cloned().collect();
            names.sort();
            SchemaSelection::Selected(names)
        };
        profile.database = optional(self.value("source-database", cx));
        profile.ca_path = optional(self.value("source-ca", cx));
        profile.engine = self.engine;
        profile.tls = self.tls;
        profile.save_password =
            self.save_password && self.authentication == Authentication::UserPassword;
        ensure!(
            self.transport != 1 || self.ssh_selected.is_some(),
            "Select an SSH session or create one in Manage SSH Sessions before testing or saving"
        );
        profile.transport = match self.transport {
            1 => {
                let id = self.ssh_selected.as_ref().expect("selection checked above");
                let session = self.ssh_profiles.iter().find(|session| &session.id == id)
                    .context("Selected SSH session is unavailable or still loading; choose one in Manage SSH Sessions")?;
                session.validate()?;
                session.transport()
            }
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

    fn refresh_generated_url(&mut self, cx: &mut Context<Self>) {
        if self.endpoint_mode != 0 {
            return;
        }
        let Ok(port) = parse_port(&self.value("source-port", cx), "Database") else {
            return;
        };
        let draft = SourceProfile {
            host: self.value("source-host", cx),
            port,
            database: optional(self.value("source-database", cx)),
            ..Default::default()
        };
        if let Ok(url) = draft.canonical_url() {
            // Set expected text before notifying; observers must not treat this as a manual edit.
            self.last_values.insert("source-url", url.clone());
            self.inputs["source-url"].update(cx, |input, cx| input.set_value(url, cx));
        }
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
        if let Some((_, _, value, _)) = COLOR_PRESETS.iter().find(|(preset, _, _, _)| *preset == id)
        {
            self.inputs["source-color"].update(cx, |input, cx| input.set_value(*value, cx));
            self.last_values.insert("source-color", (*value).to_owned());
            self.model.update(cx, |model, cx| model.edit_form(cx));
            cx.notify();
            return;
        }
        match id {
            "source-tab-general" | "source-tab-options" | "source-tab-ssh"
            | "source-tab-schemas" => {
                self.active_tab = match id {
                    "source-tab-options" => 1,
                    "source-tab-ssh" => 2,
                    "source-tab-schemas" => 3,
                    _ => 0,
                };
                self.driver_open = false;
                self.authentication_open = false;
                self.ssh_combo_open = false;
                self.tls_open = false;
                cx.notify();
                return;
            }
            "source-tls-mode" => {
                self.tls_open = !self.tls_open;
                cx.notify();
                return;
            }
            "source-tls-required" => {
                self.tls = TlsMode::Required;
                self.tls_open = false;
            }
            "source-tls-verify-ca" => {
                self.tls = TlsMode::VerifyCa;
                self.tls_open = false;
            }
            "source-ssh-profile" => {
                self.ssh_combo_open = !self.ssh_combo_open;
                cx.notify();
                return;
            }
            "source-manage-ssh" => {
                self.ssh_combo_open = false;
                super::ssh_manager::show(cx.entity(), self.ssh_selected.clone(), cx);
                return;
            }
            "source-driver" => {
                self.driver_open = !self.driver_open;
                self.authentication_open = false;
                cx.notify();
                return;
            }
            "source-authentication" => {
                self.authentication_open = !self.authentication_open;
                self.driver_open = false;
                cx.notify();
                return;
            }
            "source-mode-default" => {
                self.endpoint_mode = 0;
                self.refresh_generated_url(cx);
            }
            "source-mode-socket" => {
                self.endpoint_mode = 1;
                self.transport = 0;
                self.tls = TlsMode::Disabled;
            }
            "source-mode-url" => self.endpoint_mode = 2,
            "source-auth-user-password" => {
                self.authentication = Authentication::UserPassword;
                self.authentication_open = false;
            }
            "source-auth-none" => {
                self.authentication = Authentication::NoAuth;
                self.save_password = false;
                self.authentication_open = false;
            }
            "source-schemas-all" => self.schemas_all = true,
            "source-schemas-selected" => self.schemas_all = false,
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
            "source-engine-mysql" => {
                self.engine = DbEngine::MySql;
                self.driver_open = false;
            }
            "source-engine-mariadb" => {
                self.engine = DbEngine::MariaDb;
                self.driver_open = false;
            }
            "source-direct" => self.transport = 0,
            "source-ssh" if self.endpoint_mode != 1 => {
                self.transport = if self.transport == 1 { 0 } else { 1 };
                self.ssh_combo_open = false;
            }
            "source-http" if self.endpoint_mode != 1 => self.transport = 2,
            "source-https" if self.endpoint_mode != 1 => self.transport = 3,
            "source-tls-verify" if self.endpoint_mode != 1 => {
                self.tls = TlsMode::VerifyIdentity;
                self.tls_open = false;
            }
            "source-tls-disabled" => {
                self.tls = TlsMode::Disabled;
                self.tls_open = false;
            }
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
    ) -> Button {
        let disabled = self.model.read(cx).saving
            || (matches!(id, "source-save" | "source-test") && self.model.read(cx).form_busy);
        Button::new(id)
            .debug_selector(move || id.into())
            .label(label)
            .tooltip(label)
            .selected(selected)
            .small()
            .disabled(disabled)
            .when(id == "source-save", |button| button.primary())
            .on_click(cx.listener(move |this, _, _, cx| this.activate(id, cx)))
    }

    fn row(&self, label: &'static str, content: impl IntoElement, cx: &App) -> Div {
        let palette = colors(cx);
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .w_full()
            .min_w(px(0.))
            .flex_shrink_0()
            .child(
                div()
                    .w(px(140.))
                    .flex_shrink_0()
                    .text_color(palette.muted)
                    .child(label),
            )
            .child(div().flex_1().min_w(px(0.)).child(content))
    }

    fn field(&self, id: &'static str, label: &'static str, _index: isize, cx: &App) -> Div {
        // Only TextInput tracks its handle. Equal indices follow the visual tree order.
        self.row(
            label,
            div()
                .id(id)
                .debug_selector(move || id.into())
                .w_full()
                .min_w(px(0.))
                .child(self.inputs[id].clone()),
            cx,
        )
    }

    // Wrappers deliberately do not track focus: native traversal visits each input once.
    fn host_port_row(
        &self,
        label: &'static str,
        host: &'static str,
        port: &'static str,
        cx: &App,
    ) -> Div {
        self.row(
            label,
            div()
                .flex()
                .items_center()
                .w_full()
                .min_w(px(0.))
                .h(px(30.))
                .gap(px(12.))
                .child(
                    div()
                        .id(host)
                        .debug_selector(move || host.into())
                        .flex_1()
                        .min_w(px(120.))
                        .child(self.inputs[host].clone()),
                )
                .child(div().w(px(30.)).flex_shrink_0().child("Port"))
                .child(
                    div()
                        .id(port)
                        .debug_selector(move || port.into())
                        .w(px(96.))
                        .min_w(px(80.))
                        .flex_shrink_0()
                        .child(self.inputs[port].clone()),
                ),
            cx,
        )
    }

    fn color_row(&self, cx: &mut Context<Self>) -> Div {
        let palette = colors(cx);
        let value = self.value("source-color", cx);
        let swatch = value
            .strip_prefix('#')
            .filter(|hex| hex.len() == 6)
            .and_then(|hex| u32::from_str_radix(hex, 16).ok());
        let entity = cx.entity().downgrade();
        let input = self.inputs["source-color"].clone();
        let disabled = self.model.read(cx).saving;
        div().flex_shrink_0().child(
            Button::new("source-color-menu")
                .debug_selector(|| "source-color-menu".into())
                .label("Color")
                .tooltip("Source color")
                .small()
                .disabled(disabled)
                .child(
                    div()
                        .size(px(12.))
                        .flex_shrink_0()
                        .rounded(px(CONTROL_RADIUS))
                        .border_1()
                        .border_color(palette.muted)
                        .bg(swatch
                            .map(|color| rgb(color).into())
                            .unwrap_or(palette.panel)),
                )
                .dropdown_caret(true)
                .dropdown_menu(move |mut menu, _, _| {
                    for &(id, label, hex, swatch) in &COLOR_PRESETS {
                        let entity = entity.clone();
                        menu = menu.item(
                            PopupMenuItem::element(move |_, cx| {
                                let palette = colors(cx);
                                div()
                                    .debug_selector(move || id.into())
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .size(px(12.))
                                            .flex_shrink_0()
                                            .rounded(px(CONTROL_RADIUS))
                                            .border_1()
                                            .border_color(palette.muted)
                                            .bg(swatch
                                                .map(|color| rgb(color).into())
                                                .unwrap_or(palette.panel)),
                                    )
                                    .child(label)
                            })
                            .checked(value.eq_ignore_ascii_case(hex))
                            .disabled(disabled)
                            .on_click(move |_, _, cx| {
                                let _ = entity.update(cx, |this, cx| this.activate(id, cx));
                            }),
                        );
                    }
                    let input = input.clone();
                    menu.min_w(px(220.)).separator().item(
                        PopupMenuItem::element(move |_, _| {
                            div()
                                .id("source-color-custom-editor")
                                .flex()
                                .flex_col()
                                .gap(px(6.))
                                .w_full()
                                .child("Custom hex (#RRGGBB)")
                                .child(
                                    div()
                                        .id("source-color")
                                        .debug_selector(|| "source-color".into())
                                        .w_full()
                                        .min_w(px(0.))
                                        .child(input.clone()),
                                )
                                // Editing custom color must not activate/dismiss a menu item.
                                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                                    cx.stop_propagation()
                                })
                                .on_click(|_, _, cx| cx.stop_propagation())
                        })
                        .disabled(disabled),
                    )
                }),
        )
    }

    fn checkbox(&self, cx: &mut Context<Self>) -> Checkbox {
        Checkbox::new("source-save-password")
            .debug_selector(|| "source-save-password".into())
            .label("Save in Keychain")
            .checked(self.save_password)
            .small()
            .disabled(self.model.read(cx).saving)
            .on_change(cx.listener(|this, _, _, cx| this.activate("source-save-password", cx)))
    }

    fn ca_picker_options() -> gpui::PathPromptOptions {
        gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose a database CA certificate file".into()),
        }
    }

    fn finish_path_pick(
        &mut self,
        field: &'static str,
        selection: Result<Option<Vec<std::path::PathBuf>>>,
        original: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.finish_path_pick_impl(field, selection, original, window, cx);
    }

    fn finish_ca_pick(
        &mut self,
        selection: Result<Option<Vec<std::path::PathBuf>>>,
        original: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.finish_path_pick("source-ca", selection, original, window, cx);
    }

    fn finish_path_pick_impl(
        &mut self,
        field: &'static str,
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
                if self.inputs[field].read(cx).value() != original {
                    self.model.update(cx, |model, cx| { model.form_feedback = Some("Path selection ignored because the path was edited while the dialog was open.".into()); cx.notify(); });
                } else if let Some(path) = paths[0].to_str() {
                    self.inputs[field].update(cx, |input, cx| input.set_value(path.to_owned(), cx));
                    self.last_values.insert(field, path.to_owned());
                    self.model.update(cx, |model, cx| model.edit_form(cx));
                } else {
                    self.model.update(cx, |model, cx| {
                        model.form_feedback = Some(
                            "The selected path is not UTF-8; enter another path manually.".into(),
                        );
                        cx.notify();
                    });
                }
            }
            Ok(None) => {}
            Ok(Some(_)) => self.model.update(cx, |model, cx| {
                model.form_feedback = Some("Choose one file.".into());
                cx.notify();
            }),
            Err(_) => self.model.update(cx, |model, cx| {
                model.form_feedback =
                    Some("Could not open the file dialog. You can enter its path manually.".into());
                cx.notify();
            }),
        }
        self.inputs[field].read(cx).focus_handle().focus(window, cx);
        cx.notify();
    }

    fn browse_ca(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.browse_path("source-ca", window, cx);
    }

    fn browse_path(&mut self, field: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        if self.ca_picker_open || self.model.read(cx).saving {
            return;
        }
        self.ca_picker_open = true;
        let original = self.inputs[field].read(cx).value();
        let mut options = Self::ca_picker_options();
        if field != "source-ca" {
            options.prompt = Some("Choose a connection file".into());
        }
        let picker = cx.prompt_for_paths(options);
        cx.spawn_in(window, async move |this, cx| {
            let selection = picker
                .await
                .map_err(|_| anyhow::anyhow!("File dialog stopped"))
                .and_then(|result| result);
            let _ = this.update_in(cx, |this, window, cx| {
                if field == "source-ca" {
                    this.finish_ca_pick(selection, original, window, cx);
                } else {
                    this.finish_path_pick(field, selection, original, window, cx);
                }
            });
        })
        .detach();
        cx.notify();
    }

    fn ca_browse_button(&self, cx: &mut Context<Self>) -> Button {
        Button::new("source-ca-browse")
            .debug_selector(|| "source-ca-browse".into())
            .label("Browse…")
            .tooltip("Choose a CA certificate file")
            .small()
            .disabled(self.ca_picker_open || self.model.read(cx).saving)
            .on_click(cx.listener(|this, _, window, cx| this.browse_ca(window, cx)))
    }

    fn path_field(
        &self,
        field: &'static str,
        label: &'static str,
        browse: &'static str,
        cx: &mut Context<Self>,
    ) -> Div {
        self.row(
            label,
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .id(field)
                        .debug_selector(move || field.into())
                        .flex_1()
                        .min_w(px(0.))
                        .child(self.inputs[field].clone()),
                )
                .child(
                    Button::new(browse)
                        .debug_selector(move || browse.into())
                        .label("Browse…")
                        .tooltip(label)
                        .small()
                        .disabled(self.model.read(cx).saving || self.ca_picker_open)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.browse_path(field, window, cx)
                        })),
                ),
            cx,
        )
    }

    fn combo(
        &self,
        id: &'static str,
        label: &'static str,
        _open: bool,
        choices: &[(&'static str, &'static str, bool)],
        cx: &mut Context<Self>,
    ) -> Div {
        let choices = choices.to_vec();
        let entity = cx.entity().downgrade();
        let disabled = self.model.read(cx).saving;
        div().child(
            Button::new(id)
                .debug_selector(move || id.into())
                .label(label)
                .tooltip(label)
                .small()
                .disabled(disabled)
                .dropdown_caret(true)
                .dropdown_menu(move |mut menu, _, _| {
                    for &(choice, label, selected) in &choices {
                        let entity = entity.clone();
                        menu = menu.item(
                            PopupMenuItem::element(move |_, _| {
                                div().debug_selector(move || choice.into()).child(label)
                            })
                            .checked(selected)
                            .disabled(disabled)
                            .on_click(move |_, _, cx| {
                                let _ = entity.update(cx, |this, cx| this.activate(choice, cx));
                            }),
                        );
                    }
                    menu
                }),
        )
    }
}

impl Render for SourceForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = colors(cx);
        let feedback = self.model.read(cx).form_feedback.clone();
        let busy = self.model.read(cx).form_busy;
        let mut body = div()
            .debug_selector(|| "source-form-body".into())
            .min_w(px(0.))
            .flex_shrink_0()
            .w_full()
            .max_w(px(720.))
            .flex()
            .flex_col()
            .gap(px(10.));
        match self.active_tab {
            0 => {
                body = body
                    .child(self.row(
                        "Driver",
                        self.combo(
                            "source-driver",
                            if self.engine == DbEngine::MySql {
                                "MySQL ▾"
                            } else {
                                "MariaDB ▾"
                            },
                            self.driver_open,
                            &[
                                (
                                    "source-engine-mysql",
                                    "MySQL",
                                    self.engine == DbEngine::MySql,
                                ),
                                (
                                    "source-engine-mariadb",
                                    "MariaDB",
                                    self.engine == DbEngine::MariaDb,
                                ),
                            ],
                            cx,
                        ),
                        cx,
                    ))
                    .child(
                        self.row(
                            "Connection type",
                            div()
                                .flex()
                                .gap(px(8.))
                                .child(self.button(
                                    "source-mode-default",
                                    "Default",
                                    self.endpoint_mode == 0,
                                    0,
                                    cx,
                                ))
                                .child(self.button(
                                    "source-mode-socket",
                                    "Unix Socket",
                                    self.endpoint_mode == 1,
                                    0,
                                    cx,
                                ))
                                .child(self.button(
                                    "source-mode-url",
                                    "URL-only",
                                    self.endpoint_mode == 2,
                                    0,
                                    cx,
                                )),
                            cx,
                        ),
                    );
                if self.endpoint_mode == 0 {
                    body = body.child(self.host_port_row("Host", "source-host", "source-port", cx));
                } else if self.endpoint_mode == 1 {
                    body = body.child(self.path_field("source-socket", "Socket path", "source-socket-browse", cx))
                        .child(div().text_color(palette.warning).child("Unix socket connections use local socket permissions; TLS unavailable."));
                }
                if self.endpoint_mode != 1 {
                    body = body.child(self.field("source-url", "Connection URL", 0, cx))
                        .child(self.row("", div().text_color(palette.muted).text_size(px(12.)).child(if self.endpoint_mode == 2 {
                            "URL controls host, port and database. Credentials are configured below."
                        } else { "Edit URL switches to URL-only. Generated URLs contain no credentials." }), cx));
                }
                body = body.child(self.row(
                    "Authentication",
                    self.combo(
                        "source-authentication",
                        if self.authentication == Authentication::UserPassword {
                            "User & Password ▾"
                        } else {
                            "No Auth ▾"
                        },
                        self.authentication_open,
                        &[
                            (
                                "source-auth-user-password",
                                "User & Password",
                                self.authentication == Authentication::UserPassword,
                            ),
                            (
                                "source-auth-none",
                                "No Auth",
                                self.authentication == Authentication::NoAuth,
                            ),
                        ],
                        cx,
                    ),
                    cx,
                ));
                if self.authentication == Authentication::UserPassword {
                    body = body.child(self.field("source-user", "User", 0, cx)).child(
                        self.row(
                            "Password",
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .min_w(px(0.))
                                .child(
                                    div()
                                        .id("source-password")
                                        .debug_selector(|| "source-password".into())
                                        .flex_1()
                                        .min_w(px(0.))
                                        .child(self.inputs["source-password"].clone()),
                                )
                                .child(self.checkbox(cx)),
                            cx,
                        ),
                    );
                }
                if self.endpoint_mode != 2 {
                    body = body.child(self.field("source-database", "Database (optional)", 0, cx));
                }
            }
            1 => {
                body = body
                    .child(self.field("source-connect-timeout", "Connect timeout (s)", 0, cx))
                    .child(self.field("source-query-timeout", "Query timeout (s)", 0, cx))
                    .child(self.field("source-page-size", "Page size", 0, cx))
                    .child(div().text_color(palette.muted).child(
                        "Connect: 1–60 seconds · Query: 1–120 seconds · Page size: 1–200 rows",
                    ));
            }
            2 => {
                if self.endpoint_mode == 1 {
                    body = body.child(div().text_color(palette.warning).child("Unix socket connections use local socket permissions; TLS unavailable. SSH and proxies are unavailable for local sockets."));
                } else {
                    body = body.child(
                        self.row(
                            "Transport",
                            div()
                                .flex()
                                .flex_wrap()
                                .gap(px(8.))
                                .child(self.button(
                                    "source-direct",
                                    "Direct",
                                    self.transport == 0,
                                    0,
                                    cx,
                                ))
                                .child(self.button(
                                    "source-http",
                                    "HTTP CONNECT",
                                    self.transport == 2,
                                    0,
                                    cx,
                                ))
                                .child(self.button(
                                    "source-https",
                                    "HTTPS CONNECT",
                                    self.transport == 3,
                                    0,
                                    cx,
                                )),
                            cx,
                        ),
                    );
                    body = body
                        .child(
                            self.row(
                                "",
                                Checkbox::new("source-ssh")
                                    .debug_selector(|| "source-ssh".into())
                                    .label("Enable SSH")
                                    .checked(self.transport == 1)
                                    .disabled(self.model.read(cx).saving)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.activate("source-ssh", cx)
                                    })),
                                cx,
                            ),
                        )
                        .child(self.ssh_profile_control(cx));
                    if self.transport == 1 {
                        body = body.child(
                            self.row(
                                "Local port",
                                div()
                                    .text_color(palette.muted)
                                    .child("Dynamic (assigned when connecting)"),
                                cx,
                            ),
                        );
                    } else if self.transport == 2 || self.transport == 3 {
                        body = body.child(self.host_port_row(
                            "Proxy host",
                            "source-proxy-host",
                            if self.transport == 3 {
                                "source-https-port"
                            } else {
                                "source-proxy-port"
                            },
                            cx,
                        ));
                    }
                    body = body.child(self.row(
                        "TLS",
                        self.combo(
                            "source-tls-mode",
                            match self.tls {
                                TlsMode::Disabled => "Disabled",
                                TlsMode::Required => "Required",
                                TlsMode::VerifyCa => "Verify CA",
                                TlsMode::VerifyIdentity => "Verify identity",
                            },
                            self.tls_open,
                            &[
                                (
                                    "source-tls-disabled",
                                    "Disabled",
                                    self.tls == TlsMode::Disabled,
                                ),
                                (
                                    "source-tls-required",
                                    "Required",
                                    self.tls == TlsMode::Required,
                                ),
                                (
                                    "source-tls-verify-ca",
                                    "Verify CA",
                                    self.tls == TlsMode::VerifyCa,
                                ),
                                (
                                    "source-tls-verify",
                                    "Verify identity",
                                    self.tls == TlsMode::VerifyIdentity,
                                ),
                            ],
                            cx,
                        ),
                        cx,
                    ));
                    if let Some(warning) = tls_warning(self.tls) {
                        body = body.child(
                            div()
                                .id("source-tls-warning")
                                .debug_selector(|| "source-tls-warning".into())
                                .text_color(palette.warning)
                                .child(warning),
                        );
                    }
                    if self.tls != TlsMode::Disabled {
                        body = body
                            .child(
                                self.row(
                                    "CA file (optional)",
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.))
                                        .child(
                                            div()
                                                .id("source-ca")
                                                .debug_selector(|| "source-ca".into())
                                                .flex_1()
                                                .min_w(px(0.))
                                                .child(self.inputs["source-ca"].clone()),
                                        )
                                        .child(self.ca_browse_button(cx)),
                                    cx,
                                ),
                            )
                            .child(self.path_field(
                                "source-client-cert",
                                "Client certificate",
                                "source-client-cert-browse",
                                cx,
                            ))
                            .child(self.path_field(
                                "source-client-key",
                                "Client key",
                                "source-client-key-browse",
                                cx,
                            ));
                    }
                }
            }
            _ => {
                body = body
                    .child(
                        self.row(
                            "Schemas",
                            div()
                                .flex()
                                .gap(px(8.))
                                .child(self.button(
                                    "source-schemas-all",
                                    "All schemas",
                                    self.schemas_all,
                                    0,
                                    cx,
                                ))
                                .child(self.button(
                                    "source-schemas-selected",
                                    "Selected schemas",
                                    !self.schemas_all,
                                    0,
                                    cx,
                                )),
                            cx,
                        ),
                    )
                    .child(
                        div().text_color(palette.muted).child(
                            "Display filtering only; this does not restrict database access.",
                        ),
                    );
                body = body
                    .child(self.field("source-schema-search", "Search", 0, cx))
                    .child(self.schema_list(cx))
                    .child(self.row(
                        "",
                        self.button("source-fetch-schemas", "Fetch schemas", false, 0, cx),
                        cx,
                    ));
                if !self.schemas_all {
                    body = body.child(self.field("source-schemas", "Additional schemas", 0, cx))
                        .child(div().text_color(palette.muted).child("Enter schema names separated by commas. An empty selection displays no schemas."));
                }
            }
        }
        body = body
            .when_some(feedback, |body, feedback| {
                let color = if feedback.starts_with("Connected:") {
                    palette.success
                } else if feedback.starts_with("Connection failed:")
                    || feedback.starts_with("Not saved:")
                {
                    palette.error
                } else {
                    palette.muted
                };
                body.child(
                    div()
                        .id("source-feedback")
                        .debug_selector(|| "source-feedback".into())
                        .text_color(color)
                        .child(feedback),
                )
            })
            .when(busy, |body| {
                body.child(div().text_color(palette.muted).child("Working…"))
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
            .bg(palette.panel)
            .text_color(palette.text)
            .text_size(px(13.))
            .on_action(cx.listener(|_, _: &NextFocus, window, cx| {
                cx.stop_propagation();
                window.focus_next(cx);
            }))
            .on_action(cx.listener(|_, _: &PreviousFocus, window, cx| {
                cx.stop_propagation();
                window.focus_prev(cx);
            }))
            .on_action(cx.listener(|this, _: &Dismiss, _, cx| {
                cx.stop_propagation();
                if this.driver_open
                    || this.authentication_open
                    || this.ssh_combo_open
                    || this.tls_open
                {
                    this.driver_open = false;
                    this.authentication_open = false;
                    this.ssh_combo_open = false;
                    this.tls_open = false;
                    cx.notify();
                } else {
                    this.cancel(cx);
                }
            }))
            .child(
                div()
                    .id("source-titlebar")
                    .debug_selector(|| "source-titlebar".into())
                    .h(px(34.))
                    .w_full()
                    .flex_shrink_0()
                    .bg(palette.header)
                    .window_control_area(gpui::WindowControlArea::Drag)
                    .on_mouse_down(gpui::MouseButton::Left, |_, window, _| {
                        window.start_window_move()
                    }),
            )
            .child(
                div()
                    .id("source-identity-header")
                    .debug_selector(|| "source-identity-header".into())
                    .w_full()
                    .flex_shrink_0()
                    .px(px(16.))
                    .py(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_color(palette.muted)
                            .child("Name"),
                    )
                    .child(
                        div()
                            .id("source-name")
                            .debug_selector(|| "source-name".into())
                            .flex_1()
                            .min_w(px(0.))
                            .child(self.inputs["source-name"].clone()),
                    )
                    .child(self.color_row(cx)),
            )
            .child(
                div()
                    .id("source-tab-bar")
                    .debug_selector(|| "source-tab-bar".into())
                    .h(px(34.))
                    .flex_shrink_0()
                    .w_full()
                    .flex()
                    .items_center()
                    .bg(palette.header)
                    .px(px(16.))
                    .child(
                        TabBar::new("source-form-tabs")
                            .small()
                            .selected_index(self.active_tab as usize)
                            .children(
                                [
                                    ("source-tab-general", "General"),
                                    ("source-tab-options", "Options"),
                                    ("source-tab-ssh", "SSH/SSL"),
                                    ("source-tab-schemas", "Schemas"),
                                ]
                                .into_iter()
                                .map(|(id, label)| {
                                    Tab::new()
                                        .aria_label(label)
                                        .prefix(
                                            div().debug_selector(move || id.into()).child(label),
                                        )
                                        .disabled(self.model.read(cx).saving)
                                        .on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                this.activate(id, cx)
                                            }),
                                        )
                                }),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .window_control_area(gpui::WindowControlArea::Drag)
                            .on_mouse_down(gpui::MouseButton::Left, |_, window, _| {
                                window.start_window_move()
                            }),
                    ),
            )
            .child(
                div()
                    .id("source-form-scroll")
                    .debug_selector(|| "source-form-scroll".into())
                    .w_full()
                    .min_w(px(0.))
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .p(px(16.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(body),
            )
            .child(
                div()
                    .debug_selector(|| "source-form-footer".into())
                    .flex_shrink_0()
                    .py(px(8.))
                    .px(px(16.))
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

fn tls_warning(mode: TlsMode) -> Option<&'static str> {
    match mode {
        TlsMode::Disabled => {
            Some("Database TLS is disabled. Traffic is not protected by database TLS.")
        }
        TlsMode::Required => {
            Some("TLS encrypts traffic but does not verify certificate trust or server identity.")
        }
        TlsMode::VerifyCa => {
            Some("The certificate chain is verified, but the server hostname is not verified.")
        }
        TlsMode::VerifyIdentity => None,
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
    use gpui::{Modifiers, TestAppContext, VisualTestContext};

    fn fixture(
        cx: &mut TestAppContext,
    ) -> (
        Entity<SourceForm>,
        Entity<SourceModel>,
        &mut VisualTestContext,
    ) {
        // Kit globals must exist before constructing inputs or popup controls.
        cx.update(gpui::init);
        cx.update(crate::desktop::bind_keys);
        let model = cx.new(|_| SourceModel::for_tests(vec![]));
        model.update(cx, |model, cx| model.new_source(cx));
        let profile = model.read_with(cx, |model, _| model.form_profile.clone().unwrap());
        let form = cx.new(|cx| SourceForm::new(profile, model.clone(), cx));
        let (_, visual) =
            cx.add_window_view(|window, cx| gpui::base::Root::new(form.clone(), window, cx));
        visual.simulate_resize(gpui::size(px(850.), px(600.)));
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

    #[test]
    fn empty_database_is_nullable_and_ports_are_checked() {
        assert_eq!(optional(String::new()), None);
        assert_eq!(optional("db".into()), Some("db".into()));
        assert_eq!(parse_port(" 3306 ", "Database").unwrap(), 3306);
        for value in ["", "zero", "0", "65536", "-1", "22.5"] {
            assert!(parse_port(value, "Database").is_err());
        }
    }

    #[test]
    fn form_and_fields_use_kit_components_and_semantic_theme_roles() {
        let form = include_str!("source_form.rs");
        let input = include_str!("input.rs");
        let about = include_str!("about.rs");
        assert!(form.contains("let palette = colors(cx)"));
        assert!(form.contains("Checkbox::new"));
        assert!(form.contains("TabBar::new"));
        assert!(form.contains(".dropdown_menu("));
        assert!(input.contains("InputState::new"));
        assert!(input.contains("Input::new"));
        for source in [form, input, about] {
            assert!(!source.contains(&["rgb(", "0x"].concat()));
            for legacy in [
                "ERROR",
                "MUTED",
                "PANEL",
                "HEADER",
                "TEXT",
                "INPUT_BG",
                "INPUT_BORDER",
            ] {
                assert!(!source.contains(&format!("rgb({legacy})")));
            }
        }
    }

    #[gpui::test]
    fn tabs_are_compact_and_keep_drafts_and_footer(cx: &mut TestAppContext) {
        let (form, _, cx) = fixture(cx);
        set(&form, cx, "source-name", "Development");
        set(&form, cx, "source-color", "#123ABC");
        let identity = cx.debug_bounds("source-identity-header").unwrap();
        let name = cx.debug_bounds("source-name").unwrap();
        let color = cx.debug_bounds("source-color-menu").unwrap();
        let titlebar = cx.debug_bounds("source-titlebar").unwrap();
        assert_eq!(titlebar.size.height, px(34.));
        assert!(identity.top() >= titlebar.bottom());
        assert!(name.right() < color.left());
        for id in [
            "source-tab-options",
            "source-tab-ssh",
            "source-tab-schemas",
            "source-tab-general",
        ] {
            click(cx, id);
            assert_eq!(
                cx.debug_bounds("source-tab-bar").unwrap().size.height,
                px(34.)
            );
            let tab_label = cx.debug_bounds(id).unwrap();
            let tab_bar = cx.debug_bounds("source-tab-bar").unwrap();
            assert!(tab_label.size.height > px(0.));
            assert!(tab_label.top() >= tab_bar.top());
            assert!(tab_label.bottom() <= tab_bar.bottom());
            assert_eq!(cx.debug_bounds("source-identity-header").unwrap(), identity);
            assert_eq!(cx.debug_bounds("source-name").unwrap(), name);
            assert_eq!(cx.debug_bounds("source-color-menu").unwrap(), color);
            assert!(identity.bottom() <= tab_bar.top());
            let scroll = cx.debug_bounds("source-form-scroll").unwrap();
            assert!(tab_bar.bottom() <= scroll.top());
            assert!(name.bottom() <= scroll.top());
            cx.simulate_event(gpui::ScrollWheelEvent {
                position: scroll.center(),
                delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(-400.))),
                ..Default::default()
            });
            cx.run_until_parked();
            assert_eq!(cx.debug_bounds("source-identity-header").unwrap(), identity);
            assert_eq!(cx.debug_bounds("source-name").unwrap(), name);
            assert_eq!(cx.debug_bounds("source-color-menu").unwrap(), color);
            let footer = cx.debug_bounds("source-form-footer").unwrap();
            // Small Kit buttons are 24px, plus the footer's 8px vertical padding.
            assert_eq!(footer.size.height, px(24. + 16.));
            assert!(footer.bottom() <= px(600.));
            assert_eq!(
                form.read_with(cx, |form, app| form.value("source-name", app)),
                "Development"
            );
            assert_eq!(
                form.read_with(cx, |form, app| form.value("source-color", app)),
                "#123ABC"
            );
        }
        assert_eq!(form.read_with(cx, |form, _| form.active_tab), 0);
        assert!(cx.debug_bounds("source-tunnel-host").is_none());
    }

    #[gpui::test]
    fn persistent_color_menu_presets_and_custom_hex_share_the_draft(cx: &mut TestAppContext) {
        let (form, _, cx) = fixture(cx);
        click(cx, "source-tab-options");
        assert!(cx.debug_bounds("source-color").is_none());
        click(cx, "source-color-menu");
        assert!(cx.debug_bounds("source-color").is_some());
        click(cx, "source-color-green");
        assert_eq!(
            form.read_with(cx, |form, app| form.value("source-color", app)),
            "#8CD4A4"
        );
        click(cx, "source-color-menu");
        click(cx, "source-color");
        cx.simulate_keystrokes("cmd-a");
        cx.simulate_input("#123ABC");
        cx.run_until_parked();
        assert!(cx.debug_bounds("source-color").is_some());
        assert_eq!(
            form.read_with(cx, |form, app| form.value("source-color", app)),
            "#123ABC"
        );
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        click(cx, "source-tab-general");
        assert_eq!(
            form.read_with(cx, |form, app| form.value("source-color", app)),
            "#123ABC"
        );
    }

    #[gpui::test]
    fn driver_and_authentication_are_keyboard_operable_popovers(cx: &mut TestAppContext) {
        let (form, _, cx) = fixture(cx);
        assert!(cx.debug_bounds("source-engine-mariadb").is_none());
        click(cx, "source-driver");
        click(cx, "source-engine-mariadb");
        assert_eq!(form.read_with(cx, |form, _| form.engine), DbEngine::MariaDb);
        assert!(!form.read_with(cx, |form, _| form.driver_open));
        click(cx, "source-driver");
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        // Kit restores a context handle after dismissal. Traverse the native
        // focus tree again rather than assuming that handle is the button.
        for _ in 0..30 {
            cx.update(|window, cx| window.focus_next(cx));
            cx.run_until_parked();
            let keystroke = gpui::Keystroke::parse("space").unwrap();
            cx.simulate_event(gpui::KeyDownEvent {
                keystroke: keystroke.clone(),
                is_held: false,
                prefer_character_input: false,
            });
            cx.simulate_event(gpui::KeyUpEvent { keystroke });
            cx.run_until_parked();
            if cx.debug_bounds("source-engine-mysql").is_some() {
                break;
            }
        }
        assert!(cx.debug_bounds("source-engine-mysql").is_some());
        click(cx, "source-engine-mysql");
        set(&form, cx, "source-password", "fixture-secret");
        click(cx, "source-save-password");
        click(cx, "source-authentication");
        click(cx, "source-auth-none");
        assert_eq!(
            form.read_with(cx, |form, _| form.authentication),
            Authentication::NoAuth
        );
        let draft = cx.update(|_, app| form.read(app).profile(app).unwrap());
        assert_eq!(draft.authentication, Authentication::NoAuth);
        assert_eq!(draft.username, "");
        assert!(!draft.save_password);
        assert_eq!(cx.update(|_, app| form.read(app).password(app)), "");
    }

    #[gpui::test]
    fn generated_url_and_manual_override_use_backend_canonicalization(cx: &mut TestAppContext) {
        let (form, _, cx) = fixture(cx);
        set(&form, cx, "source-host", "::1");
        set(&form, cx, "source-database", "my database");
        assert_eq!(form.read_with(cx, |form, _| form.endpoint_mode), 0);
        let generated = form.read_with(cx, |form, app| form.value("source-url", app));
        assert!(generated.contains("[::1]:3306"));
        assert!(generated.contains("my%20database"));
        set(&form, cx, "source-url", "mysql://db.example:3307/inventory");
        assert_eq!(form.read_with(cx, |form, _| form.endpoint_mode), 2);

        set(&form, cx, "source-port", "not active");
        let draft = cx.update(|_, app| form.read(app).profile(app).unwrap());
        assert_eq!(
            draft.endpoint,
            ConnectionMode::UrlOnly {
                url: "mysql://db.example:3307/inventory".into()
            }
        );
    }

    #[gpui::test]
    fn unix_socket_is_explicitly_local_and_validated(cx: &mut TestAppContext) {
        let (form, _, cx) = fixture(cx);
        click(cx, "source-tab-ssh");
        click(cx, "source-http");
        click(cx, "source-tab-general");
        click(cx, "source-mode-socket");
        assert!(cx.debug_bounds("source-socket").is_some());

        set(&form, cx, "source-socket", "/tmp/mysql.sock");
        let draft = cx.update(|_, app| form.read(app).profile(app).unwrap());
        assert_eq!(draft.transport, Transport::Direct);
        assert_eq!(draft.tls, TlsMode::Disabled);
        assert_eq!(
            draft.endpoint,
            ConnectionMode::UnixSocket {
                path: "/tmp/mysql.sock".into()
            }
        );
        click(cx, "source-tab-ssh");
        assert_eq!(form.read_with(cx, |form, _| form.endpoint_mode), 1);
    }

    #[gpui::test]
    fn options_and_schema_selections_round_trip_and_validate(cx: &mut TestAppContext) {
        let (form, _, cx) = fixture(cx);
        click(cx, "source-tab-options");
        set(&form, cx, "source-connect-timeout", "60");
        set(&form, cx, "source-query-timeout", "120");
        set(&form, cx, "source-page-size", "200");
        click(cx, "source-tab-schemas");
        click(cx, "source-schemas-selected");
        set(&form, cx, "source-schemas", "inventory, analytics");
        let draft = cx.update(|_, app| form.read(app).profile(app).unwrap());
        assert_eq!(draft.options.page_size, 200);
        assert_eq!(draft.options.connect_timeout_seconds, 60);
        assert_eq!(
            draft.schemas,
            SchemaSelection::Selected(vec!["analytics".into(), "inventory".into()])
        );
        set(&form, cx, "source-page-size", "201");
        assert!(cx.update(|_, app| form.read(app).profile(app)).is_err());
    }

    #[gpui::test]
    fn constructor_preserves_appended_fields_and_ssh_config(cx: &mut TestAppContext) {
        let profile = SourceProfile {
            transport: Transport::Ssh {
                host: "bastion.internal".into(),
                port: 2222,
                user: "tunnel".into(),
                identity_file: None,
                known_hosts_file: None,
                parse_config: true,
            },
            ssl_client_cert: Some("/fixture/cert.pem".into()),
            ssl_client_key: Some("/fixture/key.pem".into()),
            ssh_configuration_id: Some("9e0cb661-3f23-4c1f-945a-cda90e876e6b".into()),
            schemas: SchemaSelection::Selected(vec!["inventory".into()]),
            ..SourceProfile::default()
        };
        let model = cx.new(|_| SourceModel::for_tests(vec![profile.clone()]));
        // Kit globals must exist before constructing inputs or popup controls.
        cx.update(gpui::init);
        cx.update(crate::desktop::bind_keys);
        let form = cx.new(|cx| SourceForm::new(profile.clone(), model.clone(), cx));
        let session = SshProfile {
            id: profile.ssh_configuration_id.clone().unwrap(),
            host: "bastion.internal".into(),
            port: 2222,
            user: "tunnel".into(),
            auth: dalan_app::ssh_config_store::SshAuthentication::Agent,
            identity_file: None,
            parse_config: true,
            ..SshProfile::default()
        };
        form.update(cx, |form, cx| {
            form.refresh_ssh_configurations(vec![session], cx)
        });
        let (_, visual) =
            cx.add_window_view(|window, cx| gpui::base::Root::new(form.clone(), window, cx));
        assert_eq!(
            visual.update(|_, app| form.read(app).profile(app).unwrap()),
            profile
        );
    }

    #[gpui::test]
    fn inline_only_ssh_profile_requires_session_instead_of_fallback(cx: &mut TestAppContext) {
        cx.update(gpui::init);
        cx.update(crate::desktop::bind_keys);
        let profile = SourceProfile {
            transport: SshProfile::default().transport(),
            ..SourceProfile::default()
        };
        let model = cx.new(|_| SourceModel::for_tests(vec![profile.clone()]));
        let form = cx.new(|cx| SourceForm::new(profile, model, cx));
        let (_, cx) =
            cx.add_window_view(|window, cx| gpui::base::Root::new(form.clone(), window, cx));
        click(cx, "source-tab-ssh");
        assert!(
            cx.update(|_, app| form.read(app).profile(app))
                .unwrap_err()
                .to_string()
                .contains("Select an SSH session")
        );
        assert!(cx.debug_bounds("source-tunnel-host").is_none());
        click(cx, "source-ssh-profile");
        assert!(cx.debug_bounds("source-ssh-custom").is_none());
        cx.simulate_keystrokes("escape");
        let session = SshProfile::default();
        form.update(cx, |f, cx| f.set_ssh_configuration(session.clone(), cx));
        assert_eq!(
            cx.update(|_, app| form.read(app).profile(app).unwrap())
                .transport,
            session.transport()
        );
    }

    #[gpui::test]
    fn native_picker_rejects_stale_paths_and_preserves_manual_edits(cx: &mut TestAppContext) {
        let (form, model, cx) = fixture(cx);
        click(cx, "source-tab-ssh");
        set(&form, cx, "source-ca", "/tmp/newer.pem");
        form.update_in(cx, |form, window, cx| {
            form.finish_ca_pick(
                Ok(Some(vec!["/tmp/selected.pem".into()])),
                "/tmp/older.pem".into(),
                window,
                cx,
            )
        });
        assert_eq!(
            form.read_with(cx, |form, app| form.value("source-ca", app)),
            "/tmp/newer.pem"
        );
        assert!(
            model
                .read_with(cx, |model, _| model.form_feedback.clone().unwrap())
                .contains("edited")
        );
        form.update_in(cx, |form, window, cx| {
            form.finish_path_pick(
                "source-client-cert",
                Ok(Some(vec!["/tmp/client.pem".into()])),
                String::new(),
                window,
                cx,
            )
        });
        assert_eq!(
            form.read_with(cx, |form, app| form.value("source-client-cert", app)),
            "/tmp/client.pem"
        );
    }

    #[gpui::test]
    fn ssh_manager_callback_updates_only_draft_and_clears_feedback(cx: &mut TestAppContext) {
        let (form, model, cx) = fixture(cx);
        let profile = SshProfile {
            name: "Production bastion".into(),
            host: "jump.example".into(),
            port: 2222,
            user: "operator".into(),
            parse_config: true,
            ..SshProfile::default()
        };
        let before = model.read_with(cx, |model, _| model.profiles.clone());
        model.update(cx, |model, _| {
            model.form_feedback = Some("obsolete result".into())
        });
        form.update(cx, |form, cx| {
            form.set_ssh_configuration(profile.clone(), cx)
        });
        cx.run_until_parked();
        let draft = cx.update(|_, app| form.read(app).profile(app).unwrap());
        assert_eq!(draft.transport, profile.transport());
        assert_eq!(draft.ssh_configuration_id, Some(profile.id));
        assert!(model.read_with(cx, |model, _| model.form_feedback.is_none()));
        assert_eq!(
            model.read_with(cx, |model, _| model.profiles.clone()),
            before
        );
        assert_eq!(form.read_with(cx, |form, _| form.ssh_profiles.len()), 1);
    }

    #[gpui::test]
    fn ssh_profile_dropdown_selects_exact_session_metadata_without_inline_fallback(
        cx: &mut TestAppContext,
    ) {
        let (form, _, cx) = fixture(cx);
        let profile = SshProfile {
            name: "Fixture SSH".into(),
            host: "jump.example".into(),
            user: "operator".into(),
            port: 2200,
            parse_config: true,
            ..SshProfile::default()
        };
        form.update(cx, |form, cx| {
            form.ssh_profiles = vec![profile.clone()];
            cx.notify();
        });
        click(cx, "source-tab-ssh");
        click(cx, "source-ssh");
        click(cx, "source-ssh-profile");
        click(cx, "source-ssh-profile-0");
        let draft = cx.update(|_, app| form.read(app).profile(app).unwrap());
        assert_eq!(draft.transport, profile.transport());
        assert_eq!(draft.ssh_configuration_id, Some(profile.id.clone()));
        assert!(form.read_with(cx, |form, _| form.ssh_selected.is_some()));
        assert!(cx.debug_bounds("source-tunnel-host").is_none());
        assert!(cx.debug_bounds("source-tunnel-key").is_none());
        click(cx, "source-ssh-profile");
        assert!(cx.debug_bounds("source-ssh-custom").is_none());
    }

    #[gpui::test]
    fn enable_ssh_requires_explicit_session_and_toggle_retains_choice(cx: &mut TestAppContext) {
        let (form, model, cx) = fixture(cx);
        click(cx, "source-tab-ssh");
        assert!(cx.debug_bounds("source-manage-ssh").is_some());
        assert!(cx.debug_bounds("source-tunnel-host").is_none());
        click(cx, "source-ssh-profile");
        assert!(cx.debug_bounds("source-ssh-custom").is_none()); // Disabled until enabled.
        click(cx, "source-ssh");
        assert!(cx.debug_bounds("source-tunnel-host").is_none());
        assert!(
            cx.update(|_, app| form.read(app).profile(app))
                .unwrap_err()
                .to_string()
                .contains("Manage SSH Sessions")
        );
        click(cx, "source-test");
        assert!(model.read_with(cx, |m, _| {
            !m.form_busy
                && m.form_feedback
                    .as_ref()
                    .unwrap()
                    .contains("Select an SSH session")
        }));
        let session = SshProfile {
            host: "jump.example".into(),
            ..SshProfile::default()
        };
        form.update(cx, |form, cx| {
            form.set_ssh_configuration(session.clone(), cx)
        });
        click(cx, "source-ssh");
        let disabled = cx.update(|_, app| form.read(app).profile(app).unwrap());
        assert_eq!(disabled.transport, Transport::Direct);
        assert!(disabled.ssh_configuration_id.is_none());
        click(cx, "source-ssh");
        let enabled = cx.update(|_, app| form.read(app).profile(app).unwrap());
        assert_eq!(enabled.transport, session.transport());
        assert_eq!(enabled.ssh_configuration_id, Some(session.id));
    }

    #[gpui::test]
    fn session_refresh_uses_latest_settings_and_missing_reference_fails_closed(
        cx: &mut TestAppContext,
    ) {
        let (form, model, cx) = fixture(cx);
        let mut session = SshProfile::default();
        form.update(cx, |form, cx| {
            form.set_ssh_configuration(session.clone(), cx)
        });
        model.update(cx, |m, _| {
            m.form_feedback = Some("Old test succeeded".into())
        });
        session.host = "new-jump.example".into();
        form.update(cx, |form, cx| {
            form.refresh_ssh_configurations(vec![session.clone()], cx)
        });
        assert!(model.read_with(cx, |m, _| m.form_feedback.is_none()));
        let current = cx.update(|_, app| form.read(app).profile(app).unwrap());
        assert_eq!(current.transport, session.transport());
        assert_eq!(current.ssh_configuration_id, Some(session.id.clone()));
        form.update(cx, |form, cx| form.refresh_ssh_configurations(vec![], cx));
        assert!(
            cx.update(|_, app| form.read(app).profile(app))
                .unwrap_err()
                .to_string()
                .contains("unavailable")
        );
        click(cx, "source-tab-ssh");
        click(cx, "source-ssh");
        assert_eq!(
            cx.update(|_, app| form.read(app).profile(app).unwrap())
                .transport,
            Transport::Direct
        );
    }

    #[gpui::test]
    fn ssh_checkbox_keyboard_and_compact_session_controls_are_bounded(cx: &mut TestAppContext) {
        let (form, model, cx) = fixture(cx);
        click(cx, "source-tab-ssh");
        cx.simulate_resize(gpui::size(px(780.), px(560.)));
        cx.run_until_parked();
        click(cx, "source-ssh");
        assert_eq!(form.read_with(cx, |f, _| f.transport), 1);
        cx.update(|window, app| {
            window.blur(app);
            for _ in 0..5 {
                window.focus_next(app);
            }
        });
        let keystroke = gpui::Keystroke::parse("space").unwrap();
        cx.simulate_event(gpui::KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        });
        cx.simulate_event(gpui::KeyUpEvent { keystroke });
        cx.run_until_parked();
        assert_eq!(form.read_with(cx, |f, _| f.transport), 0);
        for id in ["source-ssh", "source-ssh-profile", "source-manage-ssh"] {
            let bounds = cx.debug_bounds(id).unwrap();
            assert!(bounds.left() >= px(0.) && bounds.right() <= px(780.));
            assert!(bounds.top() >= px(34.) && bounds.bottom() <= px(560.));
        }
        model.update(cx, |m, cx| {
            m.saving = true;
            cx.notify();
        });
        click(cx, "source-ssh");
        assert_eq!(form.read_with(cx, |f, _| f.transport), 0);
    }

    #[gpui::test]
    fn cached_schema_checkboxes_preserve_comma_names_search_and_empty_selection(
        cx: &mut TestAppContext,
    ) {
        let (form, model, cx) = fixture(cx);
        let id = form.read_with(cx, |form, _| form.original.id.clone());
        model.update(cx, |model, cx| {
            model
                .tree
                .databases
                .insert(id, vec!["analytics".into(), "name,with,commas".into()]);
            cx.notify();
        });
        click(cx, "source-tab-schemas");
        set(&form, cx, "source-schema-search", "NAME,WITH");
        assert_eq!(
            cx.update(|_, app| form.read(app).filtered_schemas(app)),
            vec!["name,with,commas"]
        );
        click(cx, "source-schemas-selected");
        click(cx, "source-schema-0");
        assert_eq!(
            cx.update(|_, app| form.read(app).profile(app).unwrap().schemas),
            SchemaSelection::Selected(vec!["name,with,commas".into()])
        );
        // The row is tab-focusable and shares the same Enter/Space callback.
        click(cx, "source-schema-0");
        assert_eq!(
            cx.update(|_, app| form.read(app).profile(app).unwrap().schemas),
            SchemaSelection::Selected(vec![])
        );
        click(cx, "source-schemas-all");
        assert_eq!(
            cx.update(|_, app| form.read(app).profile(app).unwrap().schemas),
            SchemaSelection::All
        );
    }
    #[test]
    fn tls_mode_warnings_explain_verification_boundaries() {
        assert!(
            tls_warning(TlsMode::Required)
                .unwrap()
                .contains("does not verify")
        );
        assert!(
            tls_warning(TlsMode::VerifyCa)
                .unwrap()
                .contains("hostname is not verified")
        );
        assert!(tls_warning(TlsMode::Disabled).unwrap().contains("disabled"));
        assert!(tls_warning(TlsMode::VerifyIdentity).is_none());
    }
}

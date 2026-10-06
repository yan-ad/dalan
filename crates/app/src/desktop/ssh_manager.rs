//! Standalone or embedded SSH session editor. Only saved metadata is published; Use selects a session.
use std::{
    collections::HashMap,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result, ensure};
use dalan_app::{
    source_store::SourceRepository,
    ssh_config_store::{SshAuthentication, SshProfile, SshRepository},
};
use gpui::{
    AnyWindowHandle, App, Bounds, Context, Div, Entity, EventEmitter, Global, KeyBinding,
    Subscription, TitlebarOptions, WeakEntity, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, size,
};

use gpui::component::{
    ActiveTheme, Disableable, Sizable,
    button::Button as KitButton,
    checkbox::Checkbox as KitCheckbox,
    menu::{DropdownMenu as KitDropdown, PopupMenuItem},
};
struct ManagerSlot {
    window: AnyWindowHandle,
    view: WeakEntity<SshManager>,
}
impl Global for ManagerSlot {}
pub(super) fn current_view(cx: &App) -> Option<Entity<SshManager>> {
    cx.try_global::<ManagerSlot>()?.view.upgrade()
}
pub(super) fn current_window(cx: &App) -> Option<AnyWindowHandle> {
    let slot = cx.try_global::<ManagerSlot>()?;
    slot.view.upgrade()?;
    cx.windows()
        .into_iter()
        .find(|window| *window == slot.window)
}
use super::{
    CloseWindow, Dismiss, NextFocus, PreviousFocus, icons::Icon, input::TextInput,
    source_form::SourceForm,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum AuthChoice {
    Agent,
    KeyPair,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EmbeddedSshEvent {
    Cancel,
    Used,
    Saved,
}

impl EventEmitter<EmbeddedSshEvent> for SshManager {}

pub(super) struct SshManager {
    embedded: bool,
    native_close_registered: bool,
    saved_profiles: Vec<SshProfile>,
    profiles: Vec<SshProfile>,
    inputs: HashMap<&'static str, Entity<TextInput>>,
    last_values: HashMap<&'static str, String>,
    auth: AuthChoice,
    parse_config: bool,
    selected: Option<String>,
    error: Option<String>,
    feedback: Option<String>,
    busy: bool,
    saving: bool,
    loaded: bool,
    repo: Option<SshRepository>,
    source_repository: Option<SourceRepository>,
    test_cancel: Option<Arc<AtomicBool>>,
    owner: WeakEntity<SourceForm>,
    delete_confirmation: bool,
    picker_open: bool,
    revision: u64,
    _subscriptions: Vec<Subscription>,
}

impl SshManager {
    pub(super) fn embedded(
        owner: Entity<SourceForm>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        #[cfg(not(feature = "ui-tests"))]
        let mut this = Self::new(owner, None, window, cx);
        #[cfg(feature = "ui-tests")]
        let mut this = {
            let mut manager = Self::with_repository(
                owner,
                None,
                Err(anyhow::anyhow!(
                    "Synthetic SSH editor: no local repository access"
                )),
                window,
                cx,
            );
            manager.loaded = true;
            manager
        };
        this.embedded = true;
        this
    }

    /// Rebind only outside a save, and publish only the last persisted snapshot.
    pub(super) fn set_owner(&mut self, owner: Entity<SourceForm>, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        self.owner = owner.downgrade();
        if self.loaded {
            owner.update(cx, |owner, cx| {
                owner.refresh_ssh_configurations(self.saved_profiles.clone(), cx);
            });
        }
        cx.notify();
    }

    pub(super) fn apply_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.save(false, window, cx);
    }

    pub(super) fn is_saving(&self) -> bool {
        self.saving
    }

    pub(super) fn has_unsaved_changes(&self, cx: &App) -> bool {
        if !self.loaded {
            return false;
        }
        if self.profiles != self.saved_profiles {
            return true;
        }
        self.selected.is_some()
            && self.collect_current(cx).map_or(true, |profile| {
                !self.saved_profiles.iter().any(|saved| saved == &profile)
            })
    }

    fn new(
        owner: Entity<SourceForm>,
        selected: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let repository = SshRepository::default_path().map(SshRepository::new);
        let mut this = Self::with_repository(owner, selected, repository, window, cx);
        match SourceRepository::default_path() {
            Ok(path) => this.source_repository = Some(SourceRepository::new(path)),
            Err(error) => this.error = Some(error.to_string()),
        }
        this
    }

    fn with_repository(
        owner: Entity<SourceForm>,
        selected: Option<String>,
        repository: Result<SshRepository>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut this = Self {
            embedded: false,
            native_close_registered: false,
            saved_profiles: Vec::new(),
            profiles: Vec::new(),
            inputs: HashMap::new(),
            last_values: HashMap::new(),
            auth: AuthChoice::Agent,
            parse_config: false,
            selected,
            error: repository.as_ref().err().map(ToString::to_string),
            feedback: None,
            busy: false,
            saving: false,
            loaded: false,
            repo: repository.ok(),
            source_repository: None,
            test_cancel: None,
            owner: owner.downgrade(),
            delete_confirmation: false,
            picker_open: false,
            revision: 0,
            _subscriptions: Vec::new(),
        };
        for (id, placeholder) in [
            ("ssh-name", "Session name"),
            ("ssh-host", "localhost"),
            ("ssh-port", "22"),
            ("ssh-user", "SSH user"),
            ("ssh-key", "Absolute path to private key"),
            ("ssh-known-hosts", "Default OpenSSH known_hosts"),
        ] {
            let input = cx.new(|cx| TextInput::new(String::new(), placeholder, false, cx));
            this.last_values.insert(id, String::new());
            this._subscriptions
                .push(cx.observe(&input, move |this, input, cx| {
                    let value = input.read(cx).value();
                    if this.last_values.get(id) == Some(&value) {
                        return;
                    }
                    if this.saving {
                        let previous = this.last_values.get(id).cloned().unwrap_or_default();
                        input.update(cx, |input, cx| input.set_value(previous, cx));
                        return;
                    }
                    this.last_values.insert(id, value);
                    this.revision += 1;
                    this.cancel_test();
                    this.feedback = None;
                    cx.notify();
                }));
            this.inputs.insert(id, input);
        }
        this.load(window, cx);
        this
    }

    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        self.busy = true;
        let task = cx.background_executor().spawn(async move { repo.load() });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.busy = false;
                match result {
                    Ok(profiles) => {
                        this.saved_profiles = profiles.clone();
                        this.profiles = profiles;
                        this.loaded = true;
                        if this.embedded
                            && let Some(owner) = this.owner.upgrade()
                        {
                            owner.update(cx, |owner, cx| {
                                owner.refresh_ssh_configurations(this.saved_profiles.clone(), cx);
                            });
                        }
                        let id = this
                            .selected
                            .clone()
                            .filter(|id| this.profiles.iter().any(|p| &p.id == id))
                            .or_else(|| this.profiles.first().map(|p| p.id.clone()));
                        if let Some(id) = id {
                            this.select(id, cx);
                        } else {
                            this.add(false, cx);
                        }
                        let focus = this.inputs["ssh-name"].read(cx).focus_handle();
                        focus.focus(window, cx);
                    }
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn editable(&self) -> bool {
        self.loaded && !self.saving && !self.busy && !self.picker_open
    }

    fn collect_current(&self, cx: &App) -> Result<SshProfile> {
        let id = self
            .selected
            .clone()
            .context("Select an SSH session first")?;
        let value = |key| self.inputs[key].read(cx).value().trim().to_string();
        let optional = |key| {
            let v = value(key);
            (!v.is_empty()).then_some(v)
        };
        let profile = SshProfile {
            id,
            name: value("ssh-name"),
            host: value("ssh-host"),
            port: value("ssh-port")
                .parse::<u16>()
                .context("SSH port must be a number from 1 to 65535")?,
            user: value("ssh-user"),
            auth: if self.auth == AuthChoice::Agent {
                SshAuthentication::Agent
            } else {
                SshAuthentication::KeyPair
            },
            identity_file: if self.auth == AuthChoice::KeyPair {
                optional("ssh-key")
            } else {
                None
            },
            known_hosts_file: optional("ssh-known-hosts"),
            parse_config: self.parse_config,
        };
        profile.validate()?;
        Ok(profile)
    }

    fn commit_current(&mut self, cx: &App) -> bool {
        if self.selected.is_none() {
            return true;
        }
        match self.collect_current(cx) {
            Ok(profile) => {
                if let Some(current) = self.profiles.iter_mut().find(|p| p.id == profile.id) {
                    *current = profile;
                }
                self.error = None;
                true
            }
            Err(error) => {
                self.error = Some(error.to_string());
                false
            }
        }
    }

    fn select(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(profile) = self.profiles.iter().find(|p| p.id == id).cloned() else {
            return;
        };
        self.selected = Some(id);
        self.auth = if profile.auth == SshAuthentication::Agent {
            AuthChoice::Agent
        } else {
            AuthChoice::KeyPair
        };
        self.parse_config = profile.parse_config;
        for (key, value) in [
            ("ssh-name", profile.name),
            ("ssh-host", profile.host),
            ("ssh-port", profile.port.to_string()),
            ("ssh-user", profile.user),
            ("ssh-key", profile.identity_file.unwrap_or_default()),
            (
                "ssh-known-hosts",
                profile.known_hosts_file.unwrap_or_default(),
            ),
        ] {
            self.last_values.insert(key, value.clone());
            self.inputs[key].update(cx, |input, cx| input.set_value(value, cx));
        }
        self.revision += 1;
        self.error = None;
        self.feedback = None;
        self.delete_confirmation = false;
        cx.notify();
    }

    fn add(&mut self, duplicate: bool, cx: &mut Context<Self>) {
        if !self.editable() || !self.commit_current(cx) {
            cx.notify();
            return;
        }
        if self.profiles.len() >= 100 {
            self.error = Some("At most 100 SSH sessions are supported.".into());
            cx.notify();
            return;
        }
        let mut profile = SshProfile::default();
        if duplicate
            && let Some(source) = self
                .profiles
                .iter()
                .find(|p| Some(&p.id) == self.selected.as_ref())
        {
            let id = profile.id;
            profile = source.clone();
            profile.id = id;
            profile.name = format!("{} copy", source.name.chars().take(250).collect::<String>());
        }
        let id = profile.id.clone();
        self.profiles.push(profile);
        self.select(id, cx);
    }

    fn remove(&mut self, cx: &mut Context<Self>) {
        if !self.editable() {
            return;
        }
        if !self.delete_confirmation {
            self.delete_confirmation = true;
            cx.notify();
            return;
        }
        // Reference protection happens again on the background save worker. Nothing is
        // deleted from disk here, so Cancel also undoes a confirmed draft deletion.
        self.profiles
            .retain(|p| Some(&p.id) != self.selected.as_ref());
        self.selected = None;
        self.delete_confirmation = false;
        if let Some(id) = self.profiles.first().map(|p| p.id.clone()) {
            self.select(id, cx);
        }
        cx.notify();
    }

    fn browse(&mut self, key: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editable() {
            return;
        }
        self.picker_open = true;
        let id = self.selected.clone();
        let original = self.inputs[key].read(cx).value();
        let revision = self.revision;
        let picker = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose SSH file".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = picker.await;
            let _ = this.update_in(cx, |this, _, cx| {
                this.picker_open = false;
                if this.revision != revision
                    || this.selected != id
                    || this.inputs[key].read(cx).value() != original
                {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(Ok(Some(paths))) if paths.len() == 1 => {
                        if let Some(path) = paths[0].to_str() {
                            this.inputs[key]
                                .update(cx, |input, cx| input.set_value(path.to_string(), cx));
                        } else {
                            this.error = Some("The file path must be valid UTF-8.".into());
                        }
                    }
                    Ok(Ok(None)) => {}
                    _ => {
                        this.error = Some(
                            "Could not choose a file. Enter its absolute path manually.".into(),
                        )
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn save(&mut self, use_profile: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editable() || !self.commit_current(cx) {
            cx.notify();
            return;
        }
        let chosen = if use_profile {
            let Some(profile) = self
                .profiles
                .iter()
                .find(|p| Some(&p.id) == self.selected.as_ref())
                .cloned()
            else {
                self.error = Some("Select a session to use.".into());
                cx.notify();
                return;
            };
            if self.owner.upgrade().is_none() {
                self.error = Some(
                    "The datasource form has closed. You can still Apply sessions or Cancel."
                        .into(),
                );
                cx.notify();
                return;
            }
            if self
                .owner
                .upgrade()
                .is_some_and(|owner| !owner.read(cx).can_use_ssh_session(cx))
            {
                self.error = Some("The datasource is saving or uses a local socket. Apply sessions without enabling SSH, or switch the datasource to a network endpoint.".into());
                cx.notify();
                return;
            }
            Some(profile)
        } else {
            None
        };
        let Some(repo) = self.repo.clone() else {
            return;
        };
        let Some(source_repository) = self.source_repository.clone() else {
            self.error = Some("Cannot verify saved datasource references.".into());
            cx.notify();
            return;
        };
        let profiles = self.profiles.clone();
        let saved_profiles = profiles.clone();
        self.saving = true;
        self.error = None;
        self.feedback = None;
        let task = cx
            .background_executor()
            .spawn(async move { save_profiles(&repo, &source_repository, &profiles) });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.saving = false;
                match result {
                    Ok(()) => {
                        this.saved_profiles = saved_profiles.clone();
                        this.feedback = Some("SSH sessions saved.".into());
                        if let Some(owner) = this.owner.upgrade() {
                            owner.update(cx, |owner, cx| {
                                owner.refresh_ssh_configurations(saved_profiles.clone(), cx);
                            });
                        }
                        if this.embedded { cx.emit(EmbeddedSshEvent::Saved); }
                        if let Some(profile) = chosen {
                            if let Some(owner) = this.owner.upgrade() {
                                if owner.read(cx).can_use_ssh_session(cx) {
                                    owner.update(cx, |owner, cx| {
                                        owner.set_ssh_configuration(profile, cx)
                                    });
                                    if this.embedded {
                                        cx.emit(EmbeddedSshEvent::Used);
                                    } else {
                                        window.remove_window();
                                    }
                                } else {
                                    this.error = Some("Sessions saved, but the datasource is saving or uses a local socket. Return to a network endpoint before using this session.".into());
                                }
                            } else {
                                this.error = Some(
                                    "Sessions saved, but the datasource form has closed.".into(),
                                );
                            }
                        }
                    }
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn test(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editable() {
            return;
        }
        let profile = match self.collect_current(cx) {
            Ok(p) => p,
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        self.busy = true;
        self.error = None;
        self.feedback = Some("Testing SSH authentication…".into());
        let revision = self.revision;
        let cancel = Arc::new(AtomicBool::new(false));
        self.test_cancel = Some(cancel.clone());
        let task = cx
            .background_executor()
            .spawn(async move { test_connection(&profile, &cancel) });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, _, cx| {
                this.busy = false;
                this.test_cancel = None;
                if this.revision == revision {
                    match result {
                        Ok(()) => this.feedback = Some("SSH authentication and remote command succeeded (database not tested).".into()),
                        Err(error) => { this.feedback = None; this.error = Some(error.to_string()); }
                    }
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }

    fn cancel_test(&self) {
        if let Some(cancel) = &self.test_cancel {
            cancel.store(true, Ordering::Release);
        }
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.saving {
            self.cancel_test();
            if self.embedded {
                // Discard in memory as well: the host may retain this entity.
                self.revision += 1;
                self.profiles = self.saved_profiles.clone();
                self.selected = None;
                self.error = None;
                self.feedback = None;
                self.delete_confirmation = false;
                if let Some(id) = self.profiles.first().map(|profile| profile.id.clone()) {
                    self.select(id, cx);
                }
                cx.emit(EmbeddedSshEvent::Cancel);
                cx.notify();
            } else {
                window.remove_window();
            }
        }
    }

    fn button(
        &self,
        id: &'static str,
        label: &'static str,
        symbol: Icon,
        enabled: bool,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> KitButton {
        let path = match symbol {
            Icon::Add => "icons/plus.svg",
            Icon::Remove => "icons/trash-2.svg",
            Icon::Folder => "icons/folder.svg",
            Icon::Check => "icons/check.svg",
            Icon::Close => "icons/x.svg",
            _ => "icons/settings-2.svg",
        };
        KitButton::new(id)
            .debug_selector(move || id.into())
            .small()
            .icon(gpui::component::Icon::default().path(path))
            .label(label)
            .disabled(!enabled)
            .on_click(cx.listener(move |this, _, window, cx| {
                if enabled {
                    action(this, window, cx);
                }
            }))
    }

    fn field(&self, id: &'static str, label: &'static str, cx: &App) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .w_full()
            .child(div().text_color(cx.theme().muted_foreground).child(label))
            .child(
                div()
                    .id(id)
                    .debug_selector(move || id.into())
                    .w_full()
                    .child(self.inputs[id].clone()),
            )
    }
}

/// Never remove configurations referenced by persisted sources. Fail closed if the
/// source repository cannot be read; no source JSON or credentials are modified.
fn save_profiles(
    repo: &SshRepository,
    source_repository: &SourceRepository,
    profiles: &[SshProfile],
) -> Result<()> {
    let sources = source_repository.load()?;
    for source in sources {
        if let Some(id) = source.ssh_configuration_id {
            ensure!(
                profiles.iter().any(|p| p.id == id),
                "Session is in use by a saved datasource. Restore it before applying sessions."
            );
        }
    }
    repo.save(profiles)
}

/// The explicit Test action executes only `true` remotely. No shell interpolation,
/// passwords, key contents, automatic host-key acceptance, or server output logging.
fn test_connection(profile: &SshProfile, cancel: &AtomicBool) -> Result<()> {
    ensure!(!cancel.load(Ordering::Acquire), "SSH test cancelled.");
    profile.validate()?;
    let mut command = Command::new("ssh");
    command.args([
        "-T",
        "-o",
        "BatchMode=yes",
        "-o",
        "StrictHostKeyChecking=yes",
        "-o",
        "ConnectTimeout=10",
        "-o",
        "ConnectionAttempts=1",
        "-o",
        "ClearAllForwardings=yes",
        "-o",
        "PermitLocalCommand=no",
        "-o",
        "RemoteCommand=none",
        "-o",
        "ControlMaster=no",
        "-o",
        "ControlPath=none",
        "-o",
        "PreferredAuthentications=publickey",
    ]);
    if !profile.parse_config {
        command
            .arg("-F")
            .arg(if cfg!(windows) { "NUL" } else { "/dev/null" });
    }
    command
        .arg("-p")
        .arg(profile.port.to_string())
        .arg("-l")
        .arg(&profile.user);
    if let Some(path) = &profile.identity_file {
        command.args(["-o", "IdentitiesOnly=yes", "-i"]).arg(path);
    }
    if let Some(path) = &profile.known_hosts_file {
        command.arg("-o").arg(format!("UserKnownHostsFile={path}"));
    }
    command
        .arg("--")
        .arg(&profile.host)
        .arg("true")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = command
        .spawn()
        .context("Could not start OpenSSH. Install ssh and ensure it is on PATH.")?;
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if cancel.load(Ordering::Acquire) {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("SSH test cancelled.");
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                ensure!(
                    status.success(),
                    "SSH test failed. Check host, user, trusted known_hosts and ssh-agent. Unlock encrypted keys with ssh-add. Servers that prohibit remote commands cannot pass this test."
                );
                return Ok(());
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            outcome => {
                let _ = child.kill();
                let _ = child.wait();
                if let Err(error) = outcome {
                    return Err(error).context("Could not wait for SSH test");
                }
                anyhow::bail!("SSH test timed out after 15 seconds.");
            }
        }
    }
}

impl Render for SshManager {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The embedded editor must not replace its host's native-close guard.
        if !self.embedded && !self.native_close_registered {
            self.native_close_registered = true;
            let weak = cx.entity().downgrade();
            window.on_window_should_close(cx, move |_, cx| {
                weak.upgrade().is_none_or(|entity| {
                    let manager = entity.read(cx);
                    if manager.saving {
                        return false;
                    }
                    manager.cancel_test();
                    true
                })
            });
        }
        let enabled = self.editable();
        let mut list = div()
            .id("ssh-profile-list")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll();
        for profile in &self.profiles {
            let id = profile.id.clone();
            let keyboard_id = id.clone();
            let selected = self.selected.as_ref() == Some(&id);
            list = list.child(
                div()
                    .id(gpui::SharedString::from(format!("ssh-profile-{id}")))
                    .tab_index(0)
                    .flex()
                    .flex_col()
                    .p(px(10.))
                    .gap(px(3.))
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(if selected {
                        cx.theme().list_active
                    } else {
                        cx.theme().background
                    })
                    .cursor_pointer()
                    .hover(|s| s.bg(cx.theme().list_hover))
                    .focus(|s| s.bg(cx.theme().list_active))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.editable()
                            && this.selected.as_ref() != Some(&id)
                            && this.commit_current(cx)
                        {
                            this.select(id.clone(), cx);
                        }
                        cx.notify();
                    }))
                    .on_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, _, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space")
                            && this.editable()
                        {
                            cx.stop_propagation();
                            if this.selected.as_ref() != Some(&keyboard_id)
                                && this.commit_current(cx)
                            {
                                this.select(keyboard_id.clone(), cx);
                            }
                            cx.notify();
                        }
                    }))
                    .child(profile.name.clone())
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "{}@{}:{}",
                                profile.user, profile.host, profile.port
                            )),
                    ),
            );
        }
        let mut editor = div()
            .id("ssh-manager-editor")
            .flex_1()
            .min_w(px(0.))
            .overflow_y_scroll()
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(12.));
        if self.selected.is_some() {
            editor = editor
                .child(self.field("ssh-name", "Name", cx))
                .child(
                    div()
                        .flex()
                        .gap(px(10.))
                        .child(self.field("ssh-host", "Host", cx))
                        .child(
                            div()
                                .w(px(90.))
                                .flex_shrink_0()
                                .child(self.field("ssh-port", "Port", cx)),
                        ),
                )
                .child(self.field("ssh-user", "User", cx));
            let auth_label = if self.auth == AuthChoice::Agent {
                "SSH agent"
            } else {
                "Key pair (file)"
            };
            let manager = cx.entity().downgrade();
            editor = editor.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child("Authentication"),
                    )
                    .child(
                        KitButton::new("ssh-auth")
                            .debug_selector(|| "ssh-auth".into())
                            .small()
                            .label(auth_label)
                            .disabled(!enabled)
                            .dropdown_menu(move |mut menu, _, _| {
                                for (label, choice) in [
                                    ("SSH agent", AuthChoice::Agent),
                                    ("Key pair (file)", AuthChoice::KeyPair),
                                ] {
                                    let manager = manager.clone();
                                    menu = menu.item(PopupMenuItem::new(label).on_click(
                                        move |_, _, cx| {
                                            if let Some(manager) = manager.upgrade() {
                                                manager.update(cx, |this, cx| {
                                                    if !this.editable() {
                                                        return;
                                                    }
                                                    this.auth = choice;
                                                    this.revision += 1;
                                                    this.cancel_test();
                                                    this.feedback = None;
                                                    cx.notify();
                                                });
                                            }
                                        },
                                    ));
                                }
                                menu
                            }),
                    ),
            );
            if self.auth == AuthChoice::KeyPair {
                editor = editor.child(
                    div()
                        .flex()
                        .items_end()
                        .gap(px(8.))
                        .child(self.field("ssh-key", "Private key file", cx))
                        .child(self.button(
                            "ssh-key-browse",
                            "Browse…",
                            Icon::Folder,
                            enabled,
                            |this, window, cx| this.browse("ssh-key", window, cx),
                            cx,
                        )),
                );
            }
            editor = editor.child(div().text_xs().text_color(cx.theme().muted_foreground)
                .child("Encrypted keys: unlock with ssh-add first. Password and passphrase storage are not supported."))
                .child(div().flex().items_end().gap(px(8.))
                    .child(self.field("ssh-known-hosts", "Known hosts file (optional)", cx))
                    .child(self.button("ssh-known-hosts-browse", "Browse…", Icon::Folder, enabled, |this, window, cx| this.browse("ssh-known-hosts", window, cx), cx)))
                .child(KitCheckbox::new("ssh-parse-config").debug_selector(|| "ssh-parse-config".into())
                    .label("Parse ~/.ssh/config").checked(self.parse_config).disabled(!enabled)
                    .on_change(cx.listener(|this, checked: &bool, _, cx| {
                        if !this.editable() { return; }
                        this.parse_config = *checked; this.revision += 1; this.cancel_test(); this.feedback = None; cx.notify();
                    })))
                .child(div().text_xs().text_color(cx.theme().warning)
                    .child("Opt-in: SSH config can invoke ProxyCommand / Match exec locally. Enable only for trusted configuration files."))
                .child(div().text_xs().text_color(cx.theme().muted_foreground)
                    .child("Host keys must already be trusted. Test executes the harmless command ‘true’ remotely; it does not test the database or forwarding."));
        } else {
            editor = editor.child("No SSH session selected. Add a session to get started.");
        }
        if self.delete_confirmation {
            editor = editor.child(div().text_color(cx.theme().warning).child("Remove this session from the draft? Click Confirm remove. Saved datasource references are checked on Apply / Use."))
                .child(self.button("ssh-delete-confirm", "Confirm remove", Icon::Remove, enabled, |this, _, cx| this.remove(cx), cx))
                .child(self.button("ssh-delete-cancel", "Keep session", Icon::Close, enabled, |this, _, cx| { this.delete_confirmation = false; cx.notify(); }, cx));
        }
        div()
            .id("ssh-manager")
            .debug_selector(|| "ssh-manager".into())
            .key_context("SshManager")
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .text_sm()
            .on_action(cx.listener(|_, _: &NextFocus, window, cx| {
                cx.stop_propagation();
                window.focus_next(cx);
            }))
            .on_action(cx.listener(|_, _: &PreviousFocus, window, cx| {
                cx.stop_propagation();
                window.focus_prev(cx);
            }))
            .on_action(cx.listener(|this, _: &CloseWindow, window, cx| {
                cx.stop_propagation();
                this.close(window, cx);
            }))
            .on_action(cx.listener(|this, _: &Dismiss, window, cx| {
                cx.stop_propagation();
                this.close(window, cx);
            }))
            .child(
                div()
                    .p(px(12.))
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        gpui::svg()
                            .path("icons/settings-2.svg")
                            .size(px(16.))
                            .text_color(cx.theme().foreground),
                    )
                    .child("Manage SSH Sessions")
                    .child(div().flex_1())
                    .child(self.button(
                        "ssh-add",
                        "Add",
                        Icon::Add,
                        enabled,
                        |this, _, cx| this.add(false, cx),
                        cx,
                    ))
                    .child(self.button(
                        "ssh-duplicate",
                        "Duplicate",
                        Icon::Add,
                        enabled && self.selected.is_some(),
                        |this, _, cx| this.add(true, cx),
                        cx,
                    ))
                    .child(self.button(
                        "ssh-remove",
                        "Remove",
                        Icon::Remove,
                        enabled && self.selected.is_some(),
                        |this, _, cx| this.remove(cx),
                        cx,
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .child(
                        div()
                            .w(px(240.))
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .border_r_1()
                            .border_color(cx.theme().border)
                            .child(list),
                    )
                    .child(editor),
            )
            .child(
                div()
                    .px(px(12.))
                    .py(px(6.))
                    .min_h(px(32.))
                    .when_some(self.error.clone(), |s, error| {
                        s.child(div().text_color(cx.theme().danger).child(error))
                    })
                    .when_some(self.feedback.clone(), |s, feedback| {
                        s.child(
                            div()
                                .text_color(cx.theme().muted_foreground)
                                .child(feedback),
                        )
                    })
                    .when(self.busy && !self.loaded, |s| {
                        s.child("Loading SSH sessions…")
                    })
                    .when(self.saving, |s| s.child("Saving SSH sessions…")),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .p(px(12.))
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(self.button(
                        "ssh-test",
                        "Test Connection",
                        Icon::Play,
                        enabled && self.selected.is_some(),
                        |this, window, cx| this.test(window, cx),
                        cx,
                    ))
                    .child(div().flex_1())
                    .child(self.button(
                        "ssh-cancel",
                        if self.embedded { "Back" } else { "Cancel" },
                        Icon::Close,
                        !self.saving,
                        |this, window, cx| this.close(window, cx),
                        cx,
                    ))
                    .child(self.button(
                        "ssh-apply",
                        "Apply",
                        Icon::Check,
                        enabled,
                        |this, window, cx| this.save(false, window, cx),
                        cx,
                    ))
                    .child(self.button(
                        "ssh-use",
                        "Use Session",
                        Icon::Check,
                        enabled && self.selected.is_some(),
                        |this, window, cx| this.save(true, window, cx),
                        cx,
                    )),
            )
    }
}

impl Drop for SshManager {
    fn drop(&mut self) {
        self.cancel_test();
    }
}

pub(super) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", NextFocus, Some("SshManager")),
        KeyBinding::new("shift-tab", PreviousFocus, Some("SshManager")),
        KeyBinding::new("escape", Dismiss, Some("SshManager")),
        KeyBinding::new("cmd-w", CloseWindow, Some("SshManager")),
    ]);
}

pub(super) fn show(owner: Entity<SourceForm>, selected: Option<String>, cx: &mut App) {
    if let Some(manager) = current_view(cx)
        && let Some(window) = current_window(cx)
        && cx
            .update_window(window, |_, window, cx| {
                manager.update(cx, |manager, cx| {
                    if manager.owner.entity_id() != owner.entity_id() && !manager.saving {
                        manager.set_owner(owner.clone(), cx);
                        if let Some(id) = selected.clone()
                            && manager.commit_current(cx)
                        {
                            manager.select(id, cx);
                        }
                    }
                    cx.notify();
                });
                window.activate_window();
            })
            .is_ok()
    {
        return;
    }
    let bounds = Bounds::centered(None, size(px(900.), px(760.)), cx);
    // Opening failure leaves the owning form and its draft untouched.
    if let Ok((window, view)) = gpui::open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(760.), px(620.))),
            window_background: gpui::WindowBackgroundAppearance::Opaque,
            titlebar: Some(TitlebarOptions {
                title: Some("Manage SSH Sessions".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        cx,
        |window, cx| cx.new(|cx| SshManager::new(owner, selected, window, cx)),
    ) {
        cx.set_global(ManagerSlot {
            window,
            view: view.downgrade(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_configuration_is_valid_and_has_independent_identity() {
        let first = SshProfile::default();
        let second = SshProfile::default();
        assert_ne!(first.id, second.id);
        assert_eq!(first.auth, SshAuthentication::Agent);
        assert!(first.identity_file.is_none());
        assert!(!first.parse_config);
        first.validate().unwrap();
    }

    #[test]
    fn cancelled_test_never_launches_ssh() {
        let error = test_connection(&SshProfile::default(), &AtomicBool::new(true)).unwrap_err();
        assert_eq!(error.to_string(), "SSH test cancelled.");
    }

    #[test]
    fn test_rejects_invalid_profile_before_launching_ssh() {
        let profile = SshProfile {
            port: 0,
            ..SshProfile::default()
        };
        assert!(test_connection(&profile, &AtomicBool::new(false)).is_err());
        let profile = SshProfile {
            auth: SshAuthentication::KeyPair,
            identity_file: None,
            ..SshProfile::default()
        };
        assert!(test_connection(&profile, &AtomicBool::new(false)).is_err());
    }
}

#[cfg(all(test, feature = "ui-tests"))]
mod ui_tests {
    use super::*;
    use crate::desktop::source_model::SourceModel;
    use gpui::{Modifiers, TestAppContext, VisualTestContext};
    use std::{fs, path::PathBuf};

    struct Files {
        root: PathBuf,
        ssh: SshRepository,
        sources: SourceRepository,
        profile: SshProfile,
    }

    impl Files {
        fn new(key_pair: bool) -> Self {
            let root = std::env::temp_dir().join(format!("dalan-ssh-ui-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&root).unwrap();
            let root = root.canonicalize().unwrap();
            let key = root.join("fixture-key");
            let known = root.join("fixture-known-hosts");
            fs::write(&key, "not a private key").unwrap();
            fs::write(&known, "not a host key").unwrap();
            let profile = SshProfile {
                name: "Fixture bastion".into(),
                host: "jump.example".into(),
                user: "operator".into(),
                port: 2222,
                auth: if key_pair {
                    SshAuthentication::KeyPair
                } else {
                    SshAuthentication::Agent
                },
                identity_file: key_pair.then(|| key.to_str().unwrap().to_owned()),
                known_hosts_file: Some(known.to_str().unwrap().to_owned()),
                parse_config: true,
                ..SshProfile::default()
            };
            let ssh = SshRepository::new(root.join("ssh-config.json"));
            let sources = SourceRepository::new(root.join("sources.json"));
            ssh.save(std::slice::from_ref(&profile)).unwrap();
            sources.save(&[]).unwrap();
            Self {
                root,
                ssh,
                sources,
                profile,
            }
        }
    }

    impl Drop for Files {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    struct EmbeddedHost(Entity<SshManager>);

    impl Render for EmbeddedHost {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(self.0.clone())
        }
    }

    fn embedded_fixture<'a>(
        cx: &'a mut TestAppContext,
        files: &Files,
    ) -> (
        Entity<SshManager>,
        Entity<SourceForm>,
        &'a mut VisualTestContext,
    ) {
        cx.update(bind_keys);
        let model = cx.new(|_| SourceModel::for_tests(vec![]));
        model.update(cx, |model, cx| model.new_source(cx));
        let profile = model.read_with(cx, |model, _| model.form_profile.clone().unwrap());
        let owner = cx.new(|cx| SourceForm::new(profile, model, cx));
        let (window, host) = cx.update(|cx| {
            gpui::init(cx);
            gpui::open_window(WindowOptions::default(), cx, |window, cx| {
                let manager = cx.new(|cx| {
                    let mut manager = SshManager::with_repository(
                        owner.clone(),
                        Some(files.profile.id.clone()),
                        Ok(files.ssh.clone()),
                        window,
                        cx,
                    );
                    manager.embedded = true;
                    manager.source_repository = Some(files.sources.clone());
                    manager
                });
                cx.new(|_| EmbeddedHost(manager))
            })
            .unwrap()
        });
        let manager = host.read_with(cx, |host, _| host.0.clone());
        let visual = VisualTestContext::from_window(window, cx).into_mut();
        visual.simulate_resize(size(px(900.), px(760.)));
        visual.refresh().unwrap();
        visual.run_until_parked();
        assert!(manager.read_with(visual, |manager, _| manager.loaded));
        (manager, owner, visual)
    }

    #[gpui::test]
    fn embedded_use_emits_used_and_preserves_host_window(cx: &mut TestAppContext) {
        let files = Files::new(false);
        let (manager, owner, cx) = embedded_fixture(cx, &files);
        let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let received = events.clone();
        let _subscription = cx.cx.update(|app| {
            app.subscribe(&manager, move |_, event: &EmbeddedSshEvent, _| {
                received.borrow_mut().push(*event);
            })
        });
        set(&manager, cx, "ssh-name", "Used embedded session");
        click(cx, "ssh-use");
        assert_eq!(
            *events.borrow(),
            vec![EmbeddedSshEvent::Saved, EmbeddedSshEvent::Used]
        );
        assert_eq!(cx.cx.read(|app| app.windows().len()), 1);
        assert!(cx.refresh().is_ok());
        assert!(!manager.read_with(cx, |manager, app| manager.has_unsaved_changes(app)));
        assert_eq!(files.ssh.load().unwrap()[0].name, "Used embedded session");
        assert_eq!(
            owner.read_with(cx, |form, app| form
                .profile(app)
                .unwrap()
                .ssh_configuration_id),
            Some(files.profile.id.clone())
        );
    }

    #[gpui::test]
    fn embedded_cancel_discards_drafts_and_keeps_unrelated_window_alive(cx: &mut TestAppContext) {
        let files = Files::new(false);
        let (manager, owner, cx) = embedded_fixture(cx, &files);
        let mut unrelated = owner_window(&owner, cx);
        let before = owner.read_with(cx, |form, app| form.profile(app).unwrap());
        let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let received = events.clone();
        let _subscription = cx.cx.update(|app| {
            app.subscribe(&manager, move |_, event: &EmbeddedSshEvent, _| {
                received.borrow_mut().push(*event);
            })
        });
        set(&manager, cx, "ssh-name", "Unapplied embedded draft");
        click(cx, "ssh-add");
        assert!(manager.read_with(cx, |manager, app| manager.has_unsaved_changes(app)));
        click(cx, "ssh-cancel");
        assert_eq!(*events.borrow(), vec![EmbeddedSshEvent::Cancel]);
        assert_eq!(cx.cx.read(|app| app.windows().len()), 2);
        assert!(unrelated.refresh().is_ok());
        assert!(cx.refresh().is_ok());
        assert_eq!(files.ssh.load().unwrap(), vec![files.profile.clone()]);
        assert!(!manager.read_with(cx, |manager, app| manager.has_unsaved_changes(app)));
        assert_eq!(
            owner.read_with(cx, |form, app| form.profile(app).unwrap()),
            before
        );
    }

    fn fixture<'a>(
        cx: &'a mut TestAppContext,
        files: &Files,
    ) -> (
        Entity<SshManager>,
        Entity<SourceForm>,
        &'a mut VisualTestContext,
    ) {
        cx.update(bind_keys);
        let model = cx.new(|_| SourceModel::for_tests(vec![]));
        model.update(cx, |model, cx| model.new_source(cx));
        let profile = model.read_with(cx, |model, _| model.form_profile.clone().unwrap());
        let owner = cx.new(|cx| SourceForm::new(profile, model, cx));
        let (window, manager) = cx.update(|cx| {
            gpui::init(cx);
            gpui::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| {
                    let mut manager = SshManager::with_repository(
                        owner.clone(),
                        Some(files.profile.id.clone()),
                        Ok(files.ssh.clone()),
                        window,
                        cx,
                    );
                    manager.source_repository = Some(files.sources.clone());
                    manager
                })
            })
            .unwrap()
        });
        let visual = VisualTestContext::from_window(window, cx).into_mut();
        visual.simulate_resize(size(px(900.), px(760.)));
        visual.refresh().unwrap();
        visual.run_until_parked();
        assert!(manager.read_with(visual, |manager, _| manager.loaded));
        (manager, owner, visual)
    }

    fn click(cx: &mut VisualTestContext, id: &'static str) {
        cx.run_until_parked();
        let bounds = cx
            .debug_bounds(id)
            .unwrap_or_else(|| panic!("missing {id}"));
        cx.simulate_click(bounds.center(), Modifiers::default());
        cx.run_until_parked();
    }

    fn set(
        manager: &Entity<SshManager>,
        cx: &mut VisualTestContext,
        id: &'static str,
        value: &str,
    ) {
        let input = manager.read_with(cx, |manager, _| manager.inputs[id].clone());
        input.update(cx, |input, cx| input.set_value(value.to_owned(), cx));
        cx.run_until_parked();
    }

    #[gpui::test]
    fn manager_load_add_duplicate_preserves_agent_metadata_and_drafts(cx: &mut TestAppContext) {
        let files = Files::new(false);
        let (manager, _, cx) = fixture(cx, &files);
        assert_eq!(
            manager.read_with(cx, |manager, app| manager.collect_current(app).unwrap()),
            files.profile
        );
        let name_focus = manager.read_with(cx, |manager, app| {
            manager.inputs["ssh-name"].read(app).focus_handle()
        });
        assert!(cx.update(|window, _| name_focus.is_focused(window)));
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        assert!(!cx.update(|window, _| name_focus.is_focused(window)));
        cx.simulate_keystrokes("shift-tab");
        cx.run_until_parked();
        assert!(cx.update(|window, _| name_focus.is_focused(window)));

        set(&manager, cx, "ssh-name", "Manual draft name");
        click(cx, "ssh-duplicate");
        let duplicate = manager.read_with(cx, |manager, app| manager.collect_current(app).unwrap());
        assert_ne!(duplicate.id, files.profile.id);
        assert_eq!(duplicate.name, "Manual draft name copy");
        assert_eq!(duplicate.transport(), files.profile.transport());
        set(&manager, cx, "ssh-name", "Second manual name");
        click(cx, "ssh-add");
        manager.read_with(cx, |manager, _| {
            assert_eq!(manager.profiles.len(), 3);
            assert_eq!(manager.profiles[0].name, "Manual draft name");
            assert_eq!(manager.profiles[1].name, "Second manual name");
            let ids: std::collections::HashSet<_> =
                manager.profiles.iter().map(|p| &p.id).collect();
            assert_eq!(ids.len(), 3);
            for profile in &manager.profiles {
                uuid::Uuid::parse_str(&profile.id).unwrap();
            }
        });
        assert_eq!(files.ssh.load().unwrap(), vec![files.profile.clone()]);
    }

    #[gpui::test]
    fn confirmed_delete_apply_rejects_saved_reference_cancel_leaves_disk_untouched(
        cx: &mut TestAppContext,
    ) {
        let files = Files::new(false);
        let (manager, owner, cx) = fixture(cx, &files);
        let mut source = owner.read_with(cx, |form, app| form.profile(app).unwrap());
        source.ssh_configuration_id = Some(files.profile.id.clone());
        source.transport = files.profile.transport();
        files.sources.save(&[source.clone()]).unwrap();
        click(cx, "ssh-remove");
        assert_eq!(
            manager.read_with(cx, |manager, _| manager.profiles.len()),
            1
        );
        assert_eq!(files.ssh.load().unwrap(), vec![files.profile.clone()]);
        click(cx, "ssh-delete-confirm");
        assert!(manager.read_with(cx, |manager, _| manager.profiles.is_empty()));
        click(cx, "ssh-apply");
        assert!(manager.read_with(cx, |manager, _| {
            manager.error.as_ref().unwrap().contains("in use")
        }));
        assert_eq!(files.sources.load().unwrap(), vec![source]);
        assert_eq!(files.ssh.load().unwrap(), vec![files.profile.clone()]);
        click(cx, "ssh-cancel");
        assert!(cx.cx.read(|app| app.windows().is_empty()));
        assert!(owner.read_with(cx, |form, app| {
            form.profile(app).unwrap().ssh_configuration_id.is_none()
        }));
    }

    #[gpui::test]
    fn apply_stays_open_and_saving_guards_cancel_native_close_and_edits(cx: &mut TestAppContext) {
        let files = Files::new(false);
        let (manager, owner, cx) = fixture(cx, &files);
        set(&manager, cx, "ssh-name", "Applied name");
        click(cx, "ssh-apply");
        assert!(!cx.cx.read(|app| app.windows().is_empty()));
        assert_eq!(files.ssh.load().unwrap()[0].name, "Applied name");
        assert!(owner.read_with(cx, |form, app| {
            form.profile(app).unwrap().ssh_configuration_id.is_none()
        }));
        manager.update(cx, |manager, cx| {
            manager.saving = true;
            cx.notify();
        });
        cx.run_until_parked();
        set(&manager, cx, "ssh-name", "Forbidden edit");
        assert_eq!(
            manager.read_with(cx, |manager, app| manager.inputs["ssh-name"]
                .read(app)
                .value()),
            "Applied name"
        );
        click(cx, "ssh-cancel");
        assert!(!cx.simulate_close());
        manager.update_in(cx, |manager, window, cx| manager.close(window, cx));
        assert!(!cx.cx.read(|app| app.windows().is_empty()));
        manager.update(cx, |manager, cx| {
            manager.saving = false;
            cx.notify();
        });
        cx.run_until_parked();
        click(cx, "ssh-cancel");
        assert!(cx.cx.read(|app| app.windows().is_empty()));
    }

    // Render the owner separately so regressions exercise its public picker rather
    // than reaching into SourceForm's private cache or selection state.
    fn owner_window(owner: &Entity<SourceForm>, cx: &mut VisualTestContext) -> VisualTestContext {
        let window = cx.cx.update(|app| {
            gpui::open_window(WindowOptions::default(), app, |_, _| owner.clone())
                .unwrap()
                .0
        });
        let mut visual = VisualTestContext::from_window(window, &cx.cx);
        visual.simulate_resize(size(px(900.), px(760.)));
        visual.refresh().unwrap();
        visual.run_until_parked();
        visual
    }

    #[gpui::test]
    fn apply_publishes_added_edited_deleted_sessions_without_enabling_or_selecting(
        cx: &mut TestAppContext,
    ) {
        let files = Files::new(false);
        let removed = SshProfile {
            name: "Remove me".into(),
            ..SshProfile::default()
        };
        files
            .ssh
            .save(&[files.profile.clone(), removed.clone()])
            .unwrap();
        let (manager, owner, cx) = fixture(cx, &files);
        let before = owner.read_with(cx, |form, app| form.profile(app).unwrap());
        set(&manager, cx, "ssh-name", "Edited session");
        set(&manager, cx, "ssh-host", "edited.example");
        click(cx, "ssh-add");
        set(&manager, cx, "ssh-name", "Added session");
        set(&manager, cx, "ssh-host", "added.example");
        set(&manager, cx, "ssh-user", "added-user");
        let added = manager.read_with(cx, |manager, app| manager.collect_current(app).unwrap());
        manager.update(cx, |manager, cx| {
            assert!(manager.commit_current(cx));
            manager.select(removed.id.clone(), cx);
        });
        click(cx, "ssh-remove");
        click(cx, "ssh-delete-confirm");
        manager.update(cx, |manager, cx| manager.select(added.id.clone(), cx));
        click(cx, "ssh-apply");
        assert!(!cx.cx.read(|app| app.windows().is_empty()));
        assert_eq!(
            owner.read_with(cx, |form, app| form.profile(app).unwrap()),
            before
        );
        let saved = files.ssh.load().unwrap();
        assert_eq!(saved.len(), 2);
        assert_eq!(saved[0].name, "Edited session");
        assert_eq!(saved[1], added);

        let mut picker = owner_window(&owner, cx);
        click(&mut picker, "source-tab-ssh");
        click(&mut picker, "source-ssh");
        click(&mut picker, "source-ssh-profile");
        assert!(picker.debug_bounds("source-ssh-profile-0").is_some());
        assert!(picker.debug_bounds("source-ssh-profile-1").is_some());
        assert!(picker.debug_bounds("source-ssh-profile-2").is_none());
        click(&mut picker, "source-ssh-profile-0");
        let selected = owner.read_with(&picker, |form, app| form.profile(app).unwrap());
        assert_eq!(selected.ssh_configuration_id, Some(saved[0].id.clone()));
        assert_eq!(selected.transport, saved[0].transport());
        click(&mut picker, "source-ssh-profile");
        click(&mut picker, "source-ssh-profile-1");
        let selected = owner.read_with(&picker, |form, app| form.profile(app).unwrap());
        assert_eq!(selected.ssh_configuration_id, Some(added.id.clone()));
        assert_eq!(selected.transport, added.transport());

        // Apply with the manager on another session must retain the owner's
        // existing selection and transport as well as keeping the window open.
        manager.update(cx, |manager, cx| manager.select(saved[0].id.clone(), cx));
        click(cx, "ssh-apply");
        assert_eq!(
            owner.read_with(cx, |form, app| form.profile(app).unwrap()),
            selected
        );
    }

    #[gpui::test]
    fn cancel_does_not_publish_added_edited_or_deleted_draft_sessions(cx: &mut TestAppContext) {
        let files = Files::new(false);
        let (manager, owner, cx) = fixture(cx, &files);
        owner.update(cx, |owner, cx| {
            owner.refresh_ssh_configurations(vec![files.profile.clone()], cx);
            owner.set_ssh_configuration(files.profile.clone(), cx);
        });
        let before = owner.read_with(cx, |form, app| form.profile(app).unwrap());
        set(&manager, cx, "ssh-host", "unsaved.example");
        click(cx, "ssh-add");
        set(&manager, cx, "ssh-name", "Unsaved new session");
        manager.update(cx, |manager, cx| {
            assert!(manager.commit_current(cx));
            manager.select(files.profile.id.clone(), cx);
        });
        click(cx, "ssh-remove");
        click(cx, "ssh-delete-confirm");
        click(cx, "ssh-cancel");
        assert_eq!(files.ssh.load().unwrap(), vec![files.profile.clone()]);
        assert_eq!(
            owner.read_with(cx, |form, app| form.profile(app).unwrap()),
            before
        );
        let mut picker = owner_window(&owner, cx);
        click(&mut picker, "source-tab-ssh");
        click(&mut picker, "source-ssh-profile");
        assert!(picker.debug_bounds("source-ssh-profile-0").is_some());
        assert!(picker.debug_bounds("source-ssh-profile-1").is_none());
        click(&mut picker, "source-ssh-profile-0");
        let selected = owner.read_with(&picker, |form, app| form.profile(app).unwrap());
        assert_eq!(
            selected.ssh_configuration_id,
            Some(files.profile.id.clone())
        );
        assert_eq!(selected.transport, files.profile.transport());
    }

    #[gpui::test]
    fn use_session_rejects_local_socket_without_publishing_or_closing(cx: &mut TestAppContext) {
        let files = Files::new(false);
        let (manager, owner, cx) = fixture(cx, &files);
        // Keep the owner alive, but disallow SSH by choosing a local socket.
        owner.update(cx, |owner, cx| {
            owner.refresh_ssh_configurations(vec![files.profile.clone()], cx);
        });
        // Exercise the actual public source controls to choose a local socket.
        let mut source = owner_window(&owner, cx);
        click(&mut source, "source-mode-socket");
        set(&manager, cx, "ssh-name", "Unapplied name");
        click(cx, "ssh-use");
        assert!(manager.read_with(cx, |m, _| {
            m.error.as_ref().is_some_and(|e| e.contains("local socket"))
        }));
        assert_eq!(files.ssh.load().unwrap(), vec![files.profile.clone()]);
        assert!(cx.cx.read(|app| app.windows().len() == 2));
    }

    #[gpui::test]
    fn use_commits_exact_key_metadata_to_owner_without_secrets_and_closes(cx: &mut TestAppContext) {
        let files = Files::new(true);
        let other = SshProfile {
            name: "Other saved session".into(),
            host: "other.example".into(),
            ..SshProfile::default()
        };
        files
            .ssh
            .save(&[files.profile.clone(), other.clone()])
            .unwrap();
        let (_, owner, cx) = fixture(cx, &files);
        click(cx, "ssh-use");
        assert!(cx.cx.read(|app| app.windows().is_empty()));
        let profile = owner.read_with(cx, |form, app| form.profile(app).unwrap());
        assert_eq!(profile.ssh_configuration_id, Some(files.profile.id.clone()));
        assert_eq!(profile.transport, files.profile.transport());
        let json = serde_json::to_string(&profile).unwrap();
        assert!(!json.contains("not a private key"));
        assert!(!json.contains("passphrase"));
        assert!(!json.contains("private_key"));
        assert_eq!(
            files.ssh.load().unwrap(),
            vec![files.profile.clone(), other.clone()]
        );
        let mut picker = owner_window(&owner, cx);
        click(&mut picker, "source-tab-ssh");
        click(&mut picker, "source-ssh-profile");
        click(&mut picker, "source-ssh-profile-1");
        let selected = owner.read_with(&picker, |form, app| form.profile(app).unwrap());
        assert_eq!(selected.ssh_configuration_id, Some(other.id.clone()));
        assert_eq!(selected.transport, other.transport());
    }
}

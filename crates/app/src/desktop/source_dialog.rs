use super::{
    CloseWindow, Dismiss,
    icons::{Icon, provider_icon},
    source_form::{SourceForm, SourceFormEvent},
    source_model::SourceModel,
    ssh_manager::{EmbeddedSshEvent, SshManager},
};
use dalan_drivers::sources::{DbEngine, SourceProfile};
use gpui::component::{
    ActiveTheme, Disableable, Selectable, Sizable,
    button::{Button, ButtonVariants},
    list::ListItem,
};
use gpui::{
    AnyWindowHandle, App, Bounds, Context, Entity, Global, Subscription, TitlebarOptions,
    WeakEntity, Window, WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use std::collections::HashMap;

#[derive(Clone)]
struct DialogSlot {
    window: AnyWindowHandle,
    view: WeakEntity<SourceDialog>,
}
impl Global for DialogSlot {}
#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsPage {
    Sources,
    Ssh,
    Drivers,
}
struct SourceDraft {
    form: Entity<SourceForm>,
    feedback: Option<String>,
    databases: Vec<String>,
}

pub(super) struct SourceDialog {
    model: Entity<SourceModel>,
    form: Entity<SourceForm>,
    generation: u64,
    active: String,
    drafts: HashMap<String, SourceDraft>,
    order: Vec<String>,
    page: SettingsPage,
    ssh: Option<Entity<SshManager>>,
    driver: DbEngine,
    jdbc_catalog: Vec<dalan_app::jdbc_catalog::CatalogEntry>,
    jdbc_installed: Vec<dalan_app::jdbc_catalog::InstalledDriver>,
    jdbc_selected: Option<String>,
    jdbc_version: Option<String>,
    jdbc_busy: bool,
    jdbc_loaded: bool,
    jdbc_confirm: bool,
    jdbc_filter: u8,
    jdbc_search: Entity<super::input::TextInput>,
    jdbc_java: Entity<super::input::TextInput>,
    jdbc_runtime: Option<String>,
    driver_choices: std::collections::BTreeMap<String, String>,
    driver_choices_saved: std::collections::BTreeMap<String, String>,
    confirm_close: bool,
    removing: Option<String>,
    ok_pending: bool,
    confirm_remove: bool,
    notice: Option<String>,
    _subscriptions: Vec<Subscription>,
}
impl SourceDialog {
    pub(super) fn current_window(cx: &App) -> Option<AnyWindowHandle> {
        let slot = cx.try_global::<DialogSlot>()?;
        slot.view.upgrade()?;
        cx.windows()
            .into_iter()
            .find(|window| *window == slot.window)
    }
    #[cfg(all(test, feature = "ui-tests"))]
    pub(super) fn current_view(cx: &App) -> Option<Entity<Self>> {
        Self::current_window(cx)?;
        cx.try_global::<DialogSlot>()?.view.upgrade()
    }
    fn attach_form(
        &mut self,
        form: &Entity<SourceForm>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self._subscriptions
            .push(cx.subscribe_in(form, window, |this, _, event, window, cx| {
                match event {
                    SourceFormEvent::ManageSsh => {
                        this.page = SettingsPage::Ssh;
                    }
                    SourceFormEvent::Cancel => {
                        this.close(window, cx);
                    }
                }
                cx.notify();
            }));
        self._subscriptions
            .push(cx.observe(form, |_, _, cx| cx.notify()));
    }
    fn make_form(
        profile: SourceProfile,
        model: Entity<SourceModel>,
        cx: &mut Context<Self>,
    ) -> Entity<SourceForm> {
        cx.new(|cx| {
            let mut form = SourceForm::new(profile, model, cx);
            form.set_embedded();
            form
        })
    }
    fn new(model: Entity<SourceModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Do not carry completed transfer notices into a newly opened settings window.
        model.update(cx, |m, _| {
            if !m.connector_busy && m.imported_profiles.is_empty() {
                m.connector_feedback = None;
            }
        });
        let profile = model
            .read(cx)
            .form_profile
            .clone()
            .expect("source dialog requires a draft");
        let generation = model.read(cx).form_generation;
        let form = Self::make_form(profile.clone(), model.clone(), cx);
        let active = profile.id.clone();
        let driver_choices = cx
            .try_global::<super::AppearanceSettings>()
            .map(|s| s.config.driver_versions.clone())
            .unwrap_or_default();
        let mut this = Self {
            model: model.clone(),
            form: form.clone(),
            generation,
            active: active.clone(),
            drafts: HashMap::new(),
            order: vec![active.clone()],
            page: SettingsPage::Sources,
            ssh: None,
            driver: if profile.engine == DbEngine::Jdbc {
                DbEngine::MySql
            } else {
                profile.engine
            },
            jdbc_catalog: dalan_app::jdbc_catalog::curated_catalog(),
            jdbc_installed: vec![],
            jdbc_selected: None,
            jdbc_version: None,
            jdbc_busy: false,
            jdbc_loaded: false,
            jdbc_confirm: false,
            jdbc_filter: 0,
            jdbc_search: cx.new(|cx| {
                super::input::TextInput::new("", "Search built-in and JDBC drivers", false, cx)
            }),
            jdbc_java: cx.new(|cx| {
                super::input::TextInput::new(
                    profile
                        .jdbc
                        .as_ref()
                        .map(|j| j.java_path.as_str())
                        .unwrap_or(""),
                    "Absolute path to Java 17+ executable",
                    false,
                    cx,
                )
            }),
            jdbc_runtime: None,
            driver_choices_saved: driver_choices.clone(),
            driver_choices,
            confirm_close: false,
            removing: None,
            ok_pending: false,
            confirm_remove: false,
            notice: None,
            _subscriptions: vec![],
        };
        this.drafts.insert(
            active,
            SourceDraft {
                form: form.clone(),
                feedback: None,
                databases: model.read(cx).form_databases.clone(),
            },
        );
        this._subscriptions
            .push(cx.observe(&this.jdbc_search, |_, _, cx| cx.notify()));
        this.attach_form(&form, window, cx);
        model.update(cx, |m, _| m.form_keep_open = true);
        form.read(cx).preferred_first_focus(cx).focus(window, cx);
        if !model.read(cx).imported_profiles.is_empty() {
            let profiles = model.update(cx, |m, _| std::mem::take(&mut m.imported_profiles));
            this.receive_import(profiles, window, cx);
        }
        this._subscriptions
            .push(cx.observe_in(&model, window, |this, _, window, cx| {
                if !this.model.read(cx).form_open {
                    window.remove_window();
                    return;
                }
                if let Some(id) = this.removing.clone()
                    && !this.model.read(cx).saving
                {
                    if !this.model.read(cx).profiles.iter().any(|p| p.id == id) {
                        this.removing = None;
                        this.notice = None;
                        this.discard_active(window, cx);
                    } else if this.model.read(cx).error.is_some() {
                        this.removing = None;
                        this.notice = this.model.read(cx).error.clone();
                    }
                }
                if !this.model.read(cx).imported_profiles.is_empty() && !this.blocked(cx) {
                    let profiles = this
                        .model
                        .update(cx, |m, _| std::mem::take(&mut m.imported_profiles));
                    this.receive_import(profiles, window, cx);
                }
                let generation = this.model.read(cx).form_generation;
                if generation != this.generation {
                    this.generation = generation;
                    let profile = this.model.read(cx).form_profile.clone().expect("open form");
                    if profile.id == this.active {
                        let saved = this.model.read(cx).form_saved_generation == generation;
                        this.form.update(cx, |f, cx| {
                            if saved {
                                f.mark_saved(profile.clone());
                            } else {
                                f.load_session_password(cx);
                            }
                        });
                    } else {
                        this.install(profile, window, cx);
                    }
                    if this.ok_pending && !this.model.read(cx).saving {
                        this.ok_pending = false;
                        if this.form.read(cx).is_dirty() {
                            cx.notify();
                        } else {
                            this.close(window, cx);
                        }
                    }
                }
                if this.ok_pending
                    && !this.model.read(cx).saving
                    && this
                        .model
                        .read(cx)
                        .form_feedback
                        .as_ref()
                        .is_some_and(|f| f.starts_with("Not saved:") || f.contains("unavailable"))
                {
                    this.ok_pending = false;
                }
                cx.notify();
            }));
        let weak = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, app| {
            weak.update(app, |this, cx| {
                this.close(window, cx);
                false
            })
            .unwrap_or(true)
        });
        this
    }
    fn request_import(
        &mut self,
        format: dalan_app::connector_transfer::ImportFormat,
        cx: &mut Context<Self>,
    ) {
        if self.blocked(cx) {
            return;
        }
        self.model
            .update(cx, |m, cx| m.import_connectors(format, cx));
    }
    fn receive_import(
        &mut self,
        profiles: Vec<SourceProfile>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let saved = self.model.read(cx).profiles.len();
        let pending = self
            .drafts
            .keys()
            .filter(|id| !self.model.read(cx).profiles.iter().any(|p| &p.id == *id))
            .count();
        let added = profiles
            .iter()
            .filter(|p| !self.drafts.contains_key(&p.id))
            .count();
        if saved + pending + added > 100 {
            self.notice = Some(
                "Import not added: retained drafts plus saved sources would exceed 100.".into(),
            );
            cx.notify();
            return;
        }
        self.snapshot_active(cx);
        let first = profiles.first().map(|p| p.id.clone());
        for profile in profiles {
            if let Some(draft) = self.drafts.get(&profile.id) {
                draft.form.update(cx, |f, _| f.mark_new());
                continue;
            }

            let form = Self::make_form(profile.clone(), self.model.clone(), cx);
            form.update(cx, |f, _| f.mark_new());
            self.attach_form(&form, window, cx);
            self.order.push(profile.id.clone());
            self.drafts.insert(
                profile.id,
                SourceDraft {
                    form,
                    feedback: None,
                    databases: vec![],
                },
            );
        }
        if let Some(first) = first {
            self.select(first, window, cx);
        }
        cx.notify();
    }
    fn blocked(&self, cx: &App) -> bool {
        self.jdbc_busy
            || self.model.read(cx).saving
            || self.model.read(cx).connector_busy
            || self
                .ssh
                .as_ref()
                .is_some_and(|ssh| ssh.read(cx).is_saving())
    }
    fn snapshot_active(&mut self, cx: &mut Context<Self>) {
        self.form.update(cx, |f, cx| f.hide_password(cx));
        if let Some(draft) = self.drafts.get_mut(&self.active) {
            draft.feedback = if self.model.read(cx).form_busy {
                None
            } else {
                self.model.read(cx).form_feedback.clone()
            };
            draft.databases = self.model.read(cx).form_databases.clone();
        }
    }
    fn install(&mut self, profile: SourceProfile, window: &mut Window, cx: &mut Context<Self>) {
        self.active = profile.id.clone();
        if !self.drafts.contains_key(&profile.id) {
            let form = Self::make_form(profile.clone(), self.model.clone(), cx);
            self.attach_form(&form, window, cx);
            self.order.push(profile.id.clone());
            self.drafts.insert(
                profile.id.clone(),
                SourceDraft {
                    form,
                    feedback: None,
                    databases: self.model.read(cx).form_databases.clone(),
                },
            );
        }
        self.form = self.drafts[&profile.id].form.clone();
        if self.jdbc_loaded && !self.jdbc_busy {
            self.form.update(cx, |f, cx| {
                f.set_installed_jdbc(self.jdbc_installed.clone(), cx)
            });
        }
        if let Some(ssh) = &self.ssh {
            ssh.update(cx, |ssh, cx| ssh.set_owner(self.form.clone(), cx));
        }
        self.form
            .read(cx)
            .preferred_first_focus(cx)
            .focus(window, cx);
    }
    fn select(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.blocked(cx) {
            return;
        }
        self.page = SettingsPage::Sources;
        if id == self.active {
            cx.notify();
            return;
        }
        self.snapshot_active(cx);
        let profile = self
            .drafts
            .get(&id)
            .map(|d| d.form.read(cx).base_profile())
            .or_else(|| {
                self.model
                    .read(cx)
                    .profiles
                    .iter()
                    .find(|p| p.id == id)
                    .cloned()
            });
        let Some(profile) = profile else {
            return;
        };
        self.model
            .update(cx, |m, cx| m.navigate_form(profile.clone(), cx));
        self.generation = self.model.read(cx).form_generation;
        self.install(profile, window, cx);
        if let Some(draft) = self.drafts.get(&id) {
            let feedback = draft.feedback.clone();
            let databases = draft.databases.clone();
            self.model.update(cx, |m, cx| {
                m.form_feedback = feedback;
                m.form_databases = databases;
                cx.notify();
            });
        }
        self.confirm_remove = false;
        self.notice = None;
        cx.notify();
    }
    fn add(&mut self, copy: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.blocked(cx) {
            return;
        }
        let saved = self.model.read(cx).profiles.len();
        let unsaved = self
            .drafts
            .keys()
            .filter(|id| !self.model.read(cx).profiles.iter().any(|p| &p.id == *id))
            .count();
        if saved + unsaved >= 100 {
            self.notice = Some("The limit of 100 sources has been reached.".into());
            cx.notify();
            return;
        }
        let mut profile = if copy {
            match self.form.read(cx).profile(cx) {
                Ok(p) => p,
                Err(e) => {
                    self.notice = Some(format!("Fix the draft before duplicating: {e}"));
                    cx.notify();
                    return;
                }
            }
        } else {
            SourceProfile::default()
        };
        if copy {
            profile.id = uuid::Uuid::new_v4().to_string();
            profile.name = format!("{} copy", profile.name);
            while profile.name.len() > 256 {
                profile.name.pop();
            }
            profile.save_password = false;
        }
        self.snapshot_active(cx);
        self.model
            .update(cx, |m, cx| m.navigate_form(profile.clone(), cx));
        self.generation = self.model.read(cx).form_generation;
        self.install(profile, window, cx);
        self.form.update(cx, |f, _| f.mark_new());
        self.page = SettingsPage::Sources;
        self.notice = None;
        cx.notify();
    }
    fn remove(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.blocked(cx) {
            return;
        }
        if !self.confirm_remove {
            self.confirm_remove = true;
            cx.notify();
            return;
        }
        let id = self.active.clone();
        if self.model.read(cx).profiles.iter().any(|p| p.id == id) {
            self.model.update(cx, |m, cx| {
                m.delete_confirm = false;
                m.request_delete_source(id.clone(), cx);
                m.confirm_delete(cx);
            });
            if !self.model.read(cx).saving {
                self.notice = Some("Source storage is unavailable; nothing was removed.".into());
                self.confirm_remove = false;
                cx.notify();
                return;
            }
            self.removing = Some(id);
            self.notice = Some("Removing source settings and credentials…".into());
            self.confirm_remove = false;
            cx.notify();
            return;
        }
        self.discard_active(window, cx);
    }
    fn discard_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.active.clone();
        self.drafts.remove(&id);
        self.order.retain(|x| x != &id);
        self.confirm_remove = false;
        let next = self
            .model
            .read(cx)
            .profiles
            .first()
            .map(|p| p.id.clone())
            .or_else(|| self.order.first().cloned());
        if let Some(next) = next {
            self.active.clear();
            self.select(next, window, cx);
        } else {
            self.add(false, window, cx);
        }
    }
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.blocked(cx) {
            return;
        }
        let dirty = self.driver_choices != self.driver_choices_saved
            || self.drafts.values().any(|d| d.form.read(cx).is_dirty())
            || self
                .ssh
                .as_ref()
                .is_some_and(|s| s.read(cx).has_unsaved_changes(cx));
        if dirty {
            self.confirm_close = true;
            cx.notify();
            return;
        }
        self.finish_close(window, cx);
    }
    fn finish_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.blocked(cx) {
            return;
        }
        self.model.update(cx, |m, cx| {
            m.form_keep_open = false;
            m.close_form(cx);
        });
        window.remove_window();
    }
    fn ensure_ssh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ssh.is_some() {
            return;
        }
        let ssh = cx.new(|cx| SshManager::embedded(self.form.clone(), window, cx));
        self._subscriptions
            .push(cx.subscribe(&ssh, |this, ssh, event, cx| {
                if matches!(event, EmbeddedSshEvent::Saved) {
                    for draft in this.drafts.values() {
                        ssh.update(cx, |ssh, cx| ssh.set_owner(draft.form.clone(), cx));
                    }
                    ssh.update(cx, |ssh, cx| ssh.set_owner(this.form.clone(), cx));
                } else {
                    this.page = SettingsPage::Sources;
                    if matches!(event, EmbeddedSshEvent::Cancel) {
                        this.ssh = None;
                    }
                }
                cx.notify();
            }));
        self._subscriptions
            .push(cx.observe(&ssh, |_, _, cx| cx.notify()));
        self.ssh = Some(ssh);
    }
    fn page(&mut self, page: SettingsPage, cx: &mut Context<Self>) {
        if self.blocked(cx) {
            return;
        }
        self.form.update(cx, |f, cx| f.hide_password(cx));
        self.page = page;
        cx.notify();
    }
    fn apply_driver_preferences(&mut self, cx: &mut Context<Self>) -> bool {
        if self.blocked(cx) {
            return false;
        }
        if dalan_drivers::versions::validate_preferences(&self.driver_choices).is_err() {
            self.notice =
                Some("Unsupported driver choice; current backend remains unchanged.".into());
            cx.notify();
            return false;
        }
        if self.driver_choices == self.driver_choices_saved {
            return true;
        }
        let Some(settings) = cx.try_global::<super::AppearanceSettings>().cloned() else {
            self.notice = Some(
                "Configuration storage is unavailable; driver selection was not saved.".into(),
            );
            cx.notify();
            return false;
        };
        let Some(repo) = settings.repository.clone() else {
            self.notice = Some(
                "Configuration storage is unavailable; driver selection was not saved.".into(),
            );
            cx.notify();
            return false;
        };
        let mut config = settings.config;
        config.appearance = settings.preference;
        config.driver_versions = self.driver_choices.clone();
        if repo.save(&config).is_err() {
            self.notice = Some(
                "Could not save driver choices; current backend preferences remain unchanged."
                    .into(),
            );
            cx.notify();
            return false;
        }
        if dalan_drivers::versions::install_preferences(config.driver_versions.clone()).is_err() {
            self.notice = Some(
                "Driver choices saved but runtime preferences could not be applied; restart Dalan."
                    .into(),
            );
            cx.notify();
            return false;
        }
        cx.global_mut::<super::AppearanceSettings>().config = config;
        self.driver_choices_saved = self.driver_choices.clone();
        self.notice = None;
        cx.notify();
        true
    }
    fn load_jdbc(&mut self, _cx: &mut Context<Self>) {
        if self.jdbc_loaded || self.jdbc_busy {
            return;
        }
        self.jdbc_loaded = true;
        #[cfg(not(feature = "ui-tests"))]
        {
            self.jdbc_busy = true;
            let task = _cx.background_executor().spawn(async {
                let repo = dalan_app::jdbc_catalog::CatalogRepository::new(
                    dalan_app::jdbc_catalog::CatalogRepository::default_path()?,
                );
                repo.load()
            });
            _cx.spawn(async move |this, cx| {
                let result = task.await;
                let _ = this.update(cx, |d, cx| {
                    d.jdbc_busy = false;
                    match result {
                        Ok(installed) => d.publish_jdbc(installed, cx),
                        Err(_) => d.notice = Some(
                            "Could not verify local JDBC installations; catalog remains available."
                                .into(),
                        ),
                    }
                    cx.notify();
                });
            })
            .detach();
        }
    }
    fn publish_jdbc(
        &mut self,
        installed: Vec<dalan_app::jdbc_catalog::InstalledDriver>,
        cx: &mut Context<Self>,
    ) {
        self.jdbc_installed = installed;
        for draft in self.drafts.values() {
            draft.form.update(cx, |f, cx| {
                f.set_installed_jdbc(self.jdbc_installed.clone(), cx)
            });
        }
    }
    fn refresh_jdbc(&mut self, cx: &mut Context<Self>) {
        if self.blocked(cx) {
            return;
        }
        self.jdbc_busy = true;
        self.notice = Some("Fetching JetBrains discovery and Maven Central versions…".into());
        let task = cx
            .background_executor()
            .spawn(async { dalan_app::jdbc_catalog::fetch_catalog() });
        cx.spawn(async move|this,cx|{let result=task.await;let _=this.update(cx,|d,cx|{d.jdbc_busy=false;match result{Ok(catalog)=>{d.jdbc_catalog=catalog;d.notice=Some("JDBC versions refreshed. Installation executes no Java; connecting executes trusted third-party code.".into());},Err(_)=>d.notice=Some("Catalog refresh failed; cached/curated entries remain. Check connectivity or service availability.".into())}cx.notify();});}).detach();
        cx.notify();
    }
    fn install_jdbc(&mut self, cx: &mut Context<Self>) {
        if self.blocked(cx) {
            return;
        }
        if !self.jdbc_confirm {
            self.jdbc_confirm = true;
            cx.notify();
            return;
        }
        self.jdbc_confirm = false;
        let Some(id) = self.jdbc_selected.clone() else {
            return;
        };
        let Some(entry) = self.jdbc_catalog.iter().find(|e| e.id == id).cloned() else {
            return;
        };
        let Some(version) = self
            .jdbc_version
            .clone()
            .or_else(|| entry.versions.first().cloned())
        else {
            self.notice = Some("Refresh the catalog to discover installable versions.".into());
            cx.notify();
            return;
        };
        let root = match dalan_app::jdbc_catalog::CatalogRepository::default_path() {
            Ok(root) => root,
            Err(_) => {
                self.notice = Some("JDBC storage is unavailable.".into());
                cx.notify();
                return;
            }
        };
        self.jdbc_busy = true;
        self.notice = Some("Downloading and verifying approved JDBC artifacts…".into());
        let task = cx.background_executor().spawn(async move {
            let repo = dalan_app::jdbc_catalog::CatalogRepository::new(root);
            repo.install(&entry, &version)?;
            repo.load()
        });
        cx.spawn(async move|this,cx|{let result=task.await;let _=this.update(cx,|d,cx|{d.jdbc_busy=false;match result{Ok(installed)=>{d.publish_jdbc(installed,cx);d.notice=Some("JDBC installed and verified. Choose its installed version and configure Java to create a source.".into());},Err(_)=>d.notice=Some("JDBC installation failed; no installation published. SHA-256 sidecars and all declared dependencies must be available; check service/access or an already installed version.".into())}cx.notify();});}).detach();
        cx.notify();
    }
    fn check_java(&mut self, cx: &mut Context<Self>) {
        if self.blocked(cx) {
            return;
        }
        let path = self.jdbc_java.read(cx).value();
        self.jdbc_busy = true;
        let task = cx.background_executor().spawn(async move {
            super::source_model::runtime().block_on(dalan_drivers::jdbc::runtime_info(&path))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |d, cx| {
                d.jdbc_busy = false;
                d.jdbc_runtime = Some(match result {
                    Ok(r) => format!("Java {} · verified executable", r.major),
                    Err(_) => {
                        "Java unavailable or unsupported; choose an absolute Java 17+ executable."
                            .into()
                    }
                });
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn create_jdbc_source(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.blocked(cx) {
            return;
        }
        let Some(id) = self.jdbc_selected.clone() else {
            return;
        };
        let Some(installed) = self
            .jdbc_installed
            .iter()
            .find(|d| d.id == id && self.jdbc_version.as_ref().is_none_or(|v| v == &d.version))
            .cloned()
        else {
            self.notice = Some("Install and select the JDBC version first.".into());
            cx.notify();
            return;
        };
        let java = self.jdbc_java.read(cx).value();
        if !std::path::Path::new(&java).is_absolute() {
            self.notice = Some("Configure an absolute Java 17+ executable first.".into());
            cx.notify();
            return;
        }
        let pending = self
            .drafts
            .keys()
            .filter(|id| !self.model.read(cx).profiles.iter().any(|p| &p.id == *id))
            .count();
        if self.model.read(cx).profiles.len() + pending >= 100 {
            self.notice = Some("Source limit reached.".into());
            cx.notify();
            return;
        }
        let profile = SourceProfile {
            name: format!("{} JDBC", installed.name),
            engine: DbEngine::Jdbc,
            tls: dalan_drivers::TlsMode::Disabled,
            jdbc: Some(dalan_drivers::JdbcOptions {
                java_path: java,
                driver_id: installed.id.clone(),
                driver_class: installed.driver_class,
                jars: installed
                    .jars
                    .into_iter()
                    .zip(installed.sha256)
                    .map(|(path, sha256)| dalan_drivers::JdbcJar {
                        path: path.to_string_lossy().into_owned(),
                        sha256,
                    })
                    .collect(),
                url: installed.url_prefix,
            }),
            ..SourceProfile::default()
        };
        self.snapshot_active(cx);
        self.model
            .update(cx, |m, cx| m.navigate_form(profile.clone(), cx));
        self.generation = self.model.read(cx).form_generation;
        self.install(profile, window, cx);
        self.form.update(cx, |f, cx| {
            f.set_installed_jdbc(self.jdbc_installed.clone(), cx);
            f.mark_new();
        });
        self.page = SettingsPage::Sources;
        cx.notify();
    }
    fn jdbc_details(&self, cx: &mut Context<Self>) -> impl IntoElement {
        use gpui::component::{
            menu::{DropdownMenu, PopupMenuItem},
            table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow},
        };
        let Some(entry) = self
            .jdbc_selected
            .as_ref()
            .and_then(|id| self.jdbc_catalog.iter().find(|e| &e.id == id))
        else {
            return div()
                .p_4()
                .child("Select a JDBC catalog entry.")
                .into_any_element();
        };
        let entry = entry.clone();
        let installed = self
            .jdbc_installed
            .iter()
            .filter(|d| d.id == entry.id)
            .collect::<Vec<_>>();
        let mut versions = entry.versions.clone();
        for d in &installed {
            if !versions.contains(&d.version) {
                versions.push(d.version.clone());
            }
        }
        let chosen = self
            .jdbc_version
            .clone()
            .or_else(|| versions.first().cloned());
        let is_installed = chosen
            .as_ref()
            .is_some_and(|v| installed.iter().any(|d| &d.version == v));
        let owner = cx.entity().downgrade();
        let disabled = self.blocked(cx);
        let selector = Button::new("jdbc-version")
            .debug_selector(|| "jdbc-version".into())
            .small()
            .label(
                chosen
                    .clone()
                    .unwrap_or_else(|| "Refresh to load versions".into()),
            )
            .dropdown_caret(true)
            .disabled(disabled || versions.is_empty())
            .dropdown_menu(move |mut menu, _, _| {
                for version in &versions {
                    let owner = owner.clone();
                    let version = version.clone();
                    let label = version.clone();
                    menu = menu.item(PopupMenuItem::new(label).on_click(move |_, _, cx| {
                        let _ = owner.update(cx, |d, cx| {
                            if d.blocked(cx) {
                                return;
                            }
                            d.jdbc_version = Some(version.clone());
                            d.jdbc_confirm = false;
                            cx.notify();
                        });
                    }));
                }
                menu
            });
        let mut body = TableBody::new();
        for (name, value) in [
            (
                "Origin",
                "Curated coordinates; JetBrains discovery + Maven Central versions".to_owned(),
            ),
            ("Coordinates", format!("{}:{}", entry.group, entry.artifact)),
            ("Driver class", entry.driver_class.clone()),
            ("URL prefix", entry.url_prefix.clone()),
            (
                "Install state",
                if is_installed {
                    "Installed · verified"
                } else {
                    "Not installed"
                }
                .into(),
            ),
            (
                "Runtime",
                "Explicit local Java 17+ (no automatic Java download)".into(),
            ),
            (
                "Operations",
                "Restricted SELECT / metadata / browse; JDBC writes disabled".into(),
            ),
        ] {
            body = body.child(
                TableRow::new()
                    .child(TableCell::new().child(name))
                    .child(TableCell::new().child(value)),
            );
        }
        div().id("jdbc-driver-details").debug_selector(||"jdbc-driver-details".into()).flex_1().min_w_0().min_h_0().p_4().overflow_y_scroll().flex().flex_col().gap_3()
            .child(div().text_xl().child(entry.name)).child(entry.description)
            .child(div().flex().items_center().gap_2().child("Version").child(selector)
                .child(Button::new("jdbc-install").debug_selector(||"jdbc-install".into()).label(if is_installed{"Installed"}else{"Install"}).primary().small().disabled(disabled||is_installed||chosen.is_none()).on_click(cx.listener(|d,_,_,cx|d.install_jdbc(cx))))
                .child(Button::new("jdbc-new-source").debug_selector(||"jdbc-new-source".into()).label("Create connection").small().disabled(disabled||!is_installed).on_click(cx.listener(|d,_,window,cx|d.create_jdbc_source(window,cx)))))
            .child(Table::new().small().accessibility_label("JDBC driver package details").child(TableHeader::new().child(TableRow::new().child(TableHead::new().child("Field")).child(TableHead::new().child("Details")))).child(body))
            .child("Java executable (absolute path)").child(self.jdbc_java.clone())
            .child(Button::new("jdbc-check-java").debug_selector(||"jdbc-check-java".into()).label("Check Java runtime").small().disabled(disabled).on_click(cx.listener(|d,_,_,cx|d.check_java(cx))))
            .when_some(self.jdbc_runtime.clone(),|body,status|body.child(status))
            .children(dalan_app::jdbc_catalog::catalog_notices().iter().map(|notice|div().text_sm().text_color(cx.theme().muted_foreground).child(*notice)))
            .when(self.jdbc_confirm,|body|body.child(div().id("jdbc-trust-confirm").debug_selector(||"jdbc-trust-confirm".into()).p_3().border_1().border_color(cx.theme().border).child("Trust this publisher’s executable code? Connecting runs its JARs with your local user permissions. SHA-256 verifies transport integrity, not publisher safety.").child(Button::new("jdbc-confirm-install").debug_selector(||"jdbc-confirm-install".into()).label("Trust and install").small().disabled(disabled).on_click(cx.listener(|d,_,_,cx|d.install_jdbc(cx)))).child(Button::new("jdbc-cancel-install").label("Cancel").small().on_click(cx.listener(|d,_,_,cx|{d.jdbc_confirm=false;cx.notify();})))))
            .into_any_element()
    }

    fn drivers(&self, cx: &mut Context<Self>) -> impl IntoElement {
        use dalan_drivers::versions::{bundled, engine_id, resolve};
        use gpui::component::{
            menu::{DropdownMenu, PopupMenuItem},
            table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow},
        };
        let mut list = div()
            .id("driver-catalog-list")
            .min_h_0()
            .overflow_y_scroll()
            .w(px(240.))
            .flex_shrink_0()
            .p_3()
            .flex()
            .flex_col()
            .gap_1();
        let search = self.jdbc_search.read(cx).value().to_lowercase();
        list = list.child(self.jdbc_search.clone()).child(
            div().flex().gap_1().children(
                [(0, "All"), (1, "Installed"), (2, "Available")]
                    .into_iter()
                    .map(|(mode, label)| {
                        Button::new(gpui::SharedString::from(format!("jdbc-filter-{mode}")))
                            .debug_selector(move || format!("jdbc-filter-{mode}"))
                            .label(label)
                            .xsmall()
                            .selected(self.jdbc_filter == mode)
                            .on_click(cx.listener(move |d, _, _, cx| {
                                d.jdbc_filter = mode;
                                cx.notify();
                            }))
                    }),
            ),
        );
        list = list.child("Built-in");
        for (index, engine) in [
            DbEngine::MySql,
            DbEngine::MariaDb,
            DbEngine::PostgreSql,
            DbEngine::MongoDb,
            DbEngine::Redis,
        ]
        .into_iter()
        .enumerate()
        {
            if self.jdbc_filter == 2 || !engine.display_name().to_lowercase().contains(&search) {
                continue;
            }
            list = list.child(
                ListItem::new(index)
                    .debug_selector(move || format!("settings-driver-{index}"))
                    .selected(self.driver == engine)
                    .child(
                        div()
                            .h(px(28.))
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .id(gpui::SharedString::from(format!(
                                        "settings-driver-icon-{index}"
                                    )))
                                    .debug_selector(move || format!("settings-driver-icon-{index}"))
                                    .size(px(20.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(provider_icon(engine)),
                            )
                            .child(
                                div()
                                    .id(gpui::SharedString::from(format!(
                                        "settings-driver-label-{index}"
                                    )))
                                    .debug_selector(move || {
                                        format!("settings-driver-label-{index}")
                                    })
                                    .h_full()
                                    .flex()
                                    .items_center()
                                    .child(engine.display_name()),
                            ),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.driver = engine;
                        this.jdbc_selected = None;
                        this.jdbc_confirm = false;
                        cx.notify();
                    })),
            );
        }
        list = list.child("JDBC catalog");
        for (index, entry) in self.jdbc_catalog.iter().enumerate() {
            let installed = self.jdbc_installed.iter().any(|d| d.id == entry.id);
            if (self.jdbc_filter == 1 && !installed)
                || (self.jdbc_filter == 2 && installed)
                || !format!("{} {} {}", entry.name, entry.group, entry.artifact)
                    .to_lowercase()
                    .contains(&search)
            {
                continue;
            }
            let id = entry.id.clone();
            let version = entry.versions.first().cloned().or_else(|| {
                self.jdbc_installed
                    .iter()
                    .find(|d| d.id == id)
                    .map(|d| d.version.clone())
            });
            list = list.child(
                ListItem::new(index + 10)
                    .debug_selector(move || format!("jdbc-catalog-{index}"))
                    .selected(self.jdbc_selected.as_ref() == Some(&id))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                gpui::component::Icon::new(gpui::assets::IconName::Database)
                                    .size(px(16.)),
                            )
                            .child(div().flex_1().child(entry.name.clone()))
                            .child(if installed { "✓" } else { "" }),
                    )
                    .on_click(cx.listener(move |d, _, _, cx| {
                        d.jdbc_selected = Some(id.clone());
                        d.jdbc_version = version.clone();
                        d.jdbc_confirm = false;
                        cx.notify();
                    })),
            );
        }
        list = list.child(
            Button::new("jdbc-refresh-catalog")
                .debug_selector(|| "jdbc-refresh-catalog".into())
                .label("Refresh remote catalog")
                .small()
                .disabled(self.blocked(cx))
                .on_click(cx.listener(|d, _, _, cx| d.refresh_jdbc(cx))),
        );
        if self.jdbc_selected.is_some() {
            return div()
                .id("settings-drivers")
                .debug_selector(|| "settings-drivers".into())
                .size_full()
                .flex()
                .child(list)
                .child(self.jdbc_details(cx))
                .into_any_element();
        }
        let choice = self
            .driver_choices
            .get(engine_id(self.driver))
            .map(String::as_str)
            .unwrap_or("latest");
        let selected = resolve(self.driver, choice).expect("validated bundled choice");
        let choice_label = if choice == "latest" {
            format!("Latest bundled · {}", selected.version)
        } else {
            format!("Pinned · {}", selected.version)
        };
        let engine = self.driver;
        let active_choice = choice.to_owned();
        let owner = cx.entity().downgrade();
        let disabled = self.blocked(cx);
        let selector = Button::new("driver-version-selector")
            .debug_selector(|| "driver-version-selector".into())
            .small()
            .label(choice_label)
            .dropdown_caret(true)
            .disabled(disabled)
            .dropdown_menu(move |mut menu, _, _| {
                let owner_latest = owner.clone();
                menu = menu.item(
                    PopupMenuItem::element(|_, _| {
                        div()
                            .debug_selector(|| "driver-choice-latest".into())
                            .child("Latest bundled (recommended)")
                    })
                    .checked(active_choice == "latest")
                    .on_click(move |_, _, cx| {
                        let _ = owner_latest.update(cx, |this, cx| {
                            if this.blocked(cx) {
                                return;
                            }
                            this.driver_choices
                                .insert(engine_id(engine).into(), "latest".into());
                            cx.notify();
                        });
                    }),
                );
                for backend in bundled(engine) {
                    let owner = owner.clone();
                    let id = backend.id;
                    menu = menu.item(
                        PopupMenuItem::element(move |_, _| {
                            div()
                                .debug_selector(move || format!("driver-choice-{id}"))
                                .child(format!("{} {} (bundled)", backend.library, backend.version))
                        })
                        .checked(active_choice == id)
                        .on_click(move |_, _, cx| {
                            let _ = owner.update(cx, |this, cx| {
                                if this.blocked(cx) {
                                    return;
                                }
                                this.driver_choices
                                    .insert(engine_id(engine).into(), id.into());
                                cx.notify();
                            });
                        }),
                    );
                }
                menu
            });
        let header = TableHeader::new().child(
            TableRow::new()
                .child(TableHead::new().child("Library / backend"))
                .child(TableHead::new().child("Bundled version"))
                .child(TableHead::new().child("Status")),
        );
        let mut body = TableBody::new();
        for backend in bundled(engine) {
            body = body.child(
                TableRow::new()
                    .child(TableCell::new().child(backend.library))
                    .child(TableCell::new().child(backend.version))
                    .child(TableCell::new().child("Available · Experimental")),
            );
        }
        let library_table = Table::new()
            .small()
            .accessibility_label("Bundled driver versions")
            .child(header)
            .child(body);
        let (reads, writes, transport, server) = match engine {
            DbEngine::MySql => (
                "Restricted SELECT + table browsing",
                "Staged scalar edits on eligible InnoDB tables",
                "TCP, Unix socket, saved SSH / CONNECT",
                "MySQL wire protocol; server version range not yet qualified",
            ),
            DbEngine::MariaDb => (
                "Restricted SELECT + table browsing",
                "Staged scalar edits on eligible InnoDB tables",
                "TCP, Unix socket, saved SSH / CONNECT",
                "MariaDB/MySQL wire protocol; server version range not yet qualified",
            ),
            DbEngine::PostgreSql => (
                "Restricted PostgreSQL SELECT + schema/table browsing",
                "Staged scalar edits on eligible base tables",
                "TCP, Unix socket directory, saved SSH / CONNECT",
                "PostgreSQL v3 wire protocol; server version range not yet qualified",
            ),
            DbEngine::MongoDb => (
                "Restricted JSON find + Extended JSON collection browsing",
                "Unavailable",
                "Direct TCP; TLS Verify Identity / Required / Disabled",
                "MongoDB driver compatibility must be verified against actual servers",
            ),
            DbEngine::Jdbc => (
                "Restricted JDBC SELECT",
                "Unavailable",
                "Vendor JDBC URL",
                "Per-driver unqualified server compatibility",
            ),
            DbEngine::Redis => (
                "Read-only JSON command arrays + key browsing",
                "Unavailable",
                "Direct TCP; native TLS",
                "RESP2; Redis Cluster, RESP3 and streams unavailable",
            ),
        };
        let mut capability_body = TableBody::new();
        for (label, value) in [
            ("Read operations", reads),
            ("Write operations", writes),
            ("Transport", transport),
            ("TLS backend", selected.tls),
            ("Server compatibility", server),
            (
                "Verification",
                "Owned loopback fixtures; actual-server authentication/TLS matrix unrun",
            ),
        ] {
            capability_body = capability_body.child(
                TableRow::new()
                    .child(TableCell::new().child(label))
                    .child(TableCell::new().child(value)),
            );
        }
        let capability_table = Table::new()
            .small()
            .accessibility_label("Driver capabilities and server compatibility")
            .child(
                TableHeader::new().child(
                    TableRow::new()
                        .child(TableHead::new().child("Capability"))
                        .child(TableHead::new().child("Coverage")),
                ),
            )
            .child(capability_body);
        div().id("settings-drivers").debug_selector(||"settings-drivers".into()).size_full().flex().child(list)
            .child(div().id("driver-details").flex_1().min_w_0().min_h_0().overflow_y_scroll().p_5().flex().flex_col().gap_3()
                .child(div().flex().items_center().gap_2().child(provider_icon(engine)).child(format!("{} driver",engine.display_name())))
                .child(div().flex().items_center().gap_3().child("Driver version").child(selector))
                .child("One backend version is bundled for this engine. Latest bundled and its exact pin currently execute the same native backend; no other versions are downloadable in this build.")
                .child(div().id("driver-version-table").debug_selector(||"driver-version-table".into()).child(library_table))
                .child(div().id("driver-capability-table").debug_selector(||"driver-capability-table".into()).child(capability_table))
                .child("Library versions are not server versions. No certified server-version range is claimed. Choices persist in dalan.config only on Apply/OK; Cancel discards unapplied choices.")).into_any_element()
    }
}
impl Render for SourceDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.page == SettingsPage::Drivers {
            self.load_jdbc(cx);
        }
        if self.page == SettingsPage::Ssh {
            self.ensure_ssh(window, cx);
        }
        let blocked = self.blocked(cx);
        let mut rail = div()
            .id("settings-icon-rail")
            .debug_selector(|| "settings-icon-rail".into())
            .w(px(48.))
            .flex_shrink_0()
            .border_r_1()
            .border_color(cx.theme().border)
            .py_2()
            .flex()
            .flex_col()
            .items_center()
            .gap_2();
        for (id, label, page, icon) in [
            (
                "settings-sources",
                "Data Sources",
                SettingsPage::Sources,
                Icon::Database,
            ),
            (
                "settings-ssh",
                "SSH Sessions",
                SettingsPage::Ssh,
                Icon::ReadOnly,
            ),
            (
                "settings-driver-page",
                "Drivers",
                SettingsPage::Drivers,
                Icon::Manage,
            ),
        ] {
            rail = rail.child(
                Button::new(id)
                    .debug_selector(move || id.into())
                    .icon(icon.kit_name())
                    .tooltip(label)
                    .ghost()
                    .selected(self.page == page)
                    .disabled(blocked)
                    .on_click(cx.listener(move |this, _, _, cx| this.page(page, cx))),
            );
        }
        let mut sidebar = div()
            .id("settings-source-list")
            .debug_selector(|| "settings-source-list".into())
            .w(px(228.))
            .flex_shrink_0()
            .border_r_1()
            .border_color(cx.theme().border)
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(40.))
                    .px_3()
                    .flex()
                    .items_center()
                    .child("Data Sources"),
            );
        let mut toolbar = div().h(px(34.)).px_2().flex().gap_1();
        for (id, label, icon, action) in [
            ("settings-add-source", "Add source (Cmd-N)", Icon::Add, 0),
            ("settings-remove-source", "Remove source", Icon::Remove, 1),
            (
                "settings-copy-source",
                "Duplicate source (Cmd-D)",
                Icon::Cached,
                2,
            ),
        ] {
            toolbar = toolbar.child(
                Button::new(id)
                    .debug_selector(move || id.into())
                    .icon(icon.kit_name())
                    .tooltip(label)
                    .ghost()
                    .small()
                    .disabled(blocked)
                    .on_click(cx.listener(move |this, _, window, cx| match action {
                        0 => this.add(false, window, cx),
                        1 => this.remove(window, cx),
                        _ => this.add(true, window, cx),
                    })),
            );
        }
        sidebar = sidebar.child(toolbar).child(
            div()
                .px_3()
                .py_2()
                .text_color(cx.theme().muted_foreground)
                .child("Saved Sources"),
        );
        let mut rows = div()
            .id("settings-source-rows")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px_2()
            .flex()
            .flex_col()
            .gap_1();
        let mut ids = self
            .model
            .read(cx)
            .profiles
            .iter()
            .map(|p| p.id.clone())
            .collect::<Vec<_>>();
        for id in &self.order {
            if !ids.contains(id) {
                ids.push(id.clone());
            }
        }
        for (index, id) in ids.iter().enumerate() {
            let (name, engine, dirty) = if let Some(d) = self.drafts.get(id) {
                let (n, e) = d.form.read(cx).draft_identity(cx);
                (n, e, d.form.read(cx).is_dirty())
            } else {
                let p = self
                    .model
                    .read(cx)
                    .profiles
                    .iter()
                    .find(|p| &p.id == id)
                    .unwrap();
                (p.name.clone(), p.engine, false)
            };
            let unsaved = !self.model.read(cx).profiles.iter().any(|p| &p.id == id);
            let label = format!(
                "{}{}",
                if name.is_empty() { "New source" } else { &name },
                if dirty || unsaved { " •" } else { "" }
            );
            let id = id.clone();
            rows = rows.child(
                ListItem::new(index)
                    .debug_selector(move || format!("settings-source-{index}"))
                    .selected(self.active == id)
                    .disabled(blocked)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .min_w_0()
                            .child(provider_icon(engine))
                            .child(div().flex_1().min_w_0().text_ellipsis().child(label)),
                    )
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.select(id.clone(), window, cx)),
                    ),
            );
        }
        sidebar = sidebar.child(rows);
        let content = match self.page {
            SettingsPage::Sources => self.form.clone().into_any_element(),
            SettingsPage::Ssh => self.ssh.as_ref().unwrap().clone().into_any_element(),
            SettingsPage::Drivers => self.drivers(cx).into_any_element(),
        };
        let mut root =
            div()
                .id("source-dialog")
                .debug_selector(|| "source-dialog".into())
                .key_context("SourceDialog")
                .relative()
                .size_full()
                .flex()
                .flex_col()
                .bg(cx.theme().background)
                .text_color(cx.theme().foreground)
                .on_action(cx.listener(|this, _: &super::NewConnection, window, cx| {
                    this.add(false, window, cx)
                }))
                .on_action(cx.listener(|this, _: &super::ImportDbx, _, cx| {
                    this.request_import(dalan_app::connector_transfer::ImportFormat::Dbx, cx)
                }))
                .on_action(cx.listener(|this, _: &super::ImportNavicat, _, cx| {
                    this.request_import(dalan_app::connector_transfer::ImportFormat::Navicat, cx)
                }))
                .on_action(cx.listener(|this, _: &super::ImportDataGrip, _, cx| {
                    this.request_import(dalan_app::connector_transfer::ImportFormat::DataGrip, cx)
                }))
                .on_action(cx.listener(|this, _: &super::ImportConnectors, _, cx| {
                    this.request_import(
                        dalan_app::connector_transfer::ImportFormat::ConnectorsList,
                        cx,
                    )
                }))
                .on_action(cx.listener(|this, _: &super::ExportConnectors, _, cx| {
                    if !this.blocked(cx) {
                        this.model.update(cx, |m, cx| m.export_connectors(cx));
                    }
                }))
                .on_action(cx.listener(|this, _: &CloseWindow, window, cx| this.close(window, cx)))
                .on_action(cx.listener(|this, _: &Dismiss, window, cx| this.close(window, cx)))
                .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                    if event.keystroke.modifiers.platform && !event.is_held {
                        match event.keystroke.key.as_str() {
                            "n" => {
                                this.add(false, window, cx);
                                cx.stop_propagation();
                            }
                            "d" => {
                                this.add(true, window, cx);
                                cx.stop_propagation();
                            }
                            "s" => {
                                if this.page == SettingsPage::Sources && !this.blocked(cx) {
                                    this.form.update(cx, |f, cx| f.save_draft(cx));
                                }
                                cx.stop_propagation();
                            }
                            "w" => {
                                this.close(window, cx);
                                cx.stop_propagation();
                            }
                            _ => {}
                        }
                    }
                }))
                .child(
                    div()
                        .id("source-titlebar")
                        .debug_selector(|| "source-titlebar".into())
                        .h(px(34.))
                        .flex_shrink_0()
                        .pl(px(96.))
                        .flex()
                        .items_center()
                        .child("Data Sources and Drivers")
                        .on_mouse_down(gpui::MouseButton::Left, |_, window, _| {
                            window.start_window_move()
                        }),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .child(rail)
                        .when(self.page == SettingsPage::Sources, |body| {
                            body.child(sidebar)
                        })
                        .child(div().flex_1().min_w_0().min_h_0().child(content)),
                );
        root = root.child(
            div()
                .id("settings-footer")
                .debug_selector(|| "settings-footer".into())
                .h(px(48.))
                .flex_shrink_0()
                .border_t_1()
                .border_color(cx.theme().border)
                .px_4()
                .flex()
                .items_center()
                .justify_end()
                .gap_2()
                .child(
                    Button::new("source-cancel")
                        .debug_selector(|| "source-cancel".into())
                        .label("Cancel")
                        .disabled(blocked)
                        .on_click(cx.listener(|this, _, window, cx| this.close(window, cx))),
                )
                .child(
                    Button::new("source-save")
                        .debug_selector(|| "source-save".into())
                        .label("Apply")
                        .disabled(
                            blocked
                                || (self.page == SettingsPage::Drivers
                                    && self.driver_choices == self.driver_choices_saved),
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            if this.page == SettingsPage::Drivers {
                                this.apply_driver_preferences(cx);
                            } else if this.page == SettingsPage::Ssh {
                                if let Some(ssh) = &this.ssh {
                                    ssh.update(cx, |s, cx| s.apply_settings(window, cx));
                                }
                            } else {
                                this.form.update(cx, |f, cx| f.save_draft(cx));
                            }
                        })),
                )
                .child(
                    Button::new("settings-ok")
                        .debug_selector(|| "settings-ok".into())
                        .label("OK")
                        .primary()
                        .disabled(blocked)
                        .on_click(cx.listener(|this, _, window, cx| {
                            if this.page == SettingsPage::Drivers {
                                if this.apply_driver_preferences(cx) {
                                    this.close(window, cx);
                                }
                                return;
                            }
                            if this.page == SettingsPage::Sources
                                && (this.form.read(cx).is_dirty()
                                    || !this
                                        .model
                                        .read(cx)
                                        .profiles
                                        .iter()
                                        .any(|p| p.id == this.active))
                            {
                                this.ok_pending = true;
                                this.form.update(cx, |f, cx| f.save_draft(cx));
                                if !this.model.read(cx).saving {
                                    this.ok_pending = false;
                                }
                            } else {
                                this.close(window, cx);
                            }
                        })),
                ),
        );
        if let Some(message) = self
            .notice
            .clone()
            .or_else(|| self.model.read(cx).connector_feedback.clone())
        {
            // Notifications do not add a row beneath the fixed action footer.
            root = root.child(
                div()
                    .id("settings-notice")
                    .debug_selector(|| "settings-notice".into())
                    .absolute()
                    .top(px(40.))
                    .right(px(12.))
                    .max_w(px(620.))
                    .p_3()
                    .bg(cx.theme().popover)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(cx.theme().radius)
                    .shadow_md()
                    .flex()
                    .items_start()
                    .gap_2()
                    .child(div().flex_1().min_w_0().child(message))
                    .child(
                        Button::new("settings-dismiss-notice")
                            .debug_selector(|| "settings-dismiss-notice".into())
                            .icon(gpui::assets::IconName::X)
                            .small()
                            .ghost()
                            .tooltip("Dismiss notification")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.notice = None;
                                this.model.update(cx, |m, cx| {
                                    m.connector_feedback = None;
                                    cx.notify();
                                });
                                cx.notify();
                            })),
                    ),
            );
        }
        if self.confirm_close {
            let confirm = cx.entity().downgrade();
            let cancel = confirm.clone();
            super::confirm::open(
                window,
                cx,
                super::confirm::ConfirmAlert {
                    title: "Discard unapplied settings?".into(),
                    description: "Closing settings will discard unsaved source/SSH drafts and unapplied driver choices. Saved connections and database data are not changed.".into(),
                    confirm_id: "settings-discard-close",
                    confirm_label: "Discard and close",
                    cancel_id: "settings-keep-editing",
                    cancel_label: "Keep editing",
                },
                move |window, app| {
                    confirm
                        .update(app, |d, cx| {
                            if d.blocked(cx) {
                                return false;
                            }
                            d.confirm_close = false;
                            d.finish_close(window, cx);
                            true
                        })
                        .unwrap_or(true)
                },
                move |_, app| {
                    cancel
                        .update(app, |d, cx| {
                            if d.blocked(cx) {
                                return false;
                            }
                            d.confirm_close = false;
                            cx.notify();
                            true
                        })
                        .unwrap_or(true)
                },
            );
        } else if self.confirm_remove {
            let target = self.active.clone();
            let target_cancel = target.clone();
            let confirm = cx.entity().downgrade();
            let cancel = confirm.clone();
            let description = format!(
                "Remove {} and its saved credentials? Only local source settings are removed; remote databases/data are not changed.",
                self.form.read(cx).draft_identity(cx).0
            );
            super::confirm::open(
                window,
                cx,
                super::confirm::ConfirmAlert {
                    title: "Remove data source?".into(),
                    description: description.into(),
                    confirm_id: "settings-confirm-remove",
                    confirm_label: "Remove source",
                    cancel_id: "settings-keep-source",
                    cancel_label: "Keep source",
                },
                move |window, app| {
                    confirm
                        .update(app, |d, cx| {
                            if d.blocked(cx) || d.active != target || !d.confirm_remove {
                                return false;
                            }
                            d.remove(window, cx);
                            true
                        })
                        .unwrap_or(true)
                },
                move |_, app| {
                    cancel
                        .update(app, |d, cx| {
                            if d.blocked(cx) {
                                return false;
                            }
                            if d.active == target_cancel {
                                d.confirm_remove = false;
                            }
                            cx.notify();
                            true
                        })
                        .unwrap_or(true)
                },
            );
        }
        root
    }
}

pub(super) fn show(model: Entity<SourceModel>, cx: &mut App) {
    if !model.read(cx).form_open {
        return;
    }
    if let Some(handle) = SourceDialog::current_window(cx) {
        let _ = handle.update(cx, |_, window, _| window.activate_window());
        return;
    }
    let bounds = Bounds::centered(None, size(px(1160.0), px(760.0)), cx);
    match gpui::open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(1040.0), px(560.0))),
            window_background: gpui::WindowBackgroundAppearance::Opaque,
            titlebar: Some(TitlebarOptions {
                title: Some("".into()),
                appears_transparent: true,
                traffic_light_position: Some(gpui::point(px(12.0), px(12.0))),
            }),
            ..Default::default()
        },
        cx,
        |window, cx| cx.new(|cx| SourceDialog::new(model.clone(), window, cx)),
    ) {
        Ok((window, view)) => cx.set_global(DialogSlot {
            window,
            view: view.downgrade(),
        }),
        Err(error) => model.update(cx, |model, cx| {
            model.close_form(cx);
            model.error = Some(format!("Could not open source dialog: {error}"));
            cx.notify();
        }),
    }
}

#[cfg(all(test, feature = "ui-tests"))]
mod tests {
    use super::*;
    use dalan_drivers::sources::{DbEngine, SourceProfile, TlsMode};
    use gpui::{Modifiers, TestAppContext, VisualTestContext};

    struct DialogHandle {
        window: AnyWindowHandle,
        view: WeakEntity<SourceDialog>,
    }
    impl DialogHandle {
        fn update<R>(
            &self,
            cx: &mut TestAppContext,
            callback: impl FnOnce(&mut SourceDialog, &mut Window, &mut Context<SourceDialog>) -> R,
        ) -> gpui::Result<R> {
            let view = self.view.upgrade().expect("live dialog");
            self.window.update(cx, |_, window, app| {
                view.update(app, |dialog, cx| callback(dialog, window, cx))
            })
        }
    }

    fn open(
        model: &Entity<SourceModel>,
        cx: &mut TestAppContext,
    ) -> (DialogHandle, VisualTestContext) {
        cx.update(|cx| cx.set_reduce_motion(true));
        cx.update(|app| show(model.clone(), app));
        let handle = cx.update(|app| {
            let windows = app.windows();
            assert_eq!(windows.len(), 1);
            DialogHandle {
                window: SourceDialog::current_window(app).unwrap(),
                view: SourceDialog::current_view(app).unwrap().downgrade(),
            }
        });
        let mut visual = VisualTestContext::from_window(handle.window, cx);
        visual.refresh().unwrap();
        visual.run_until_parked();
        (handle, visual)
    }

    fn new_model(cx: &mut TestAppContext) -> Entity<SourceModel> {
        cx.update(gpui::init);
        cx.update(crate::desktop::bind_keys);
        cx.update(|cx| cx.set_reduce_motion(true));
        assert!(cx.update(|app| app.windows().is_empty()));
        let model = cx.new(|_| SourceModel::for_tests(vec![]));
        model.update(cx, |model, cx| model.new_source(cx));
        model
    }

    fn assert_closed(model: &Entity<SourceModel>, cx: &mut TestAppContext) {
        cx.run_until_parked();
        let windows = cx.update(|app| app.windows());
        assert!(
            windows.is_empty(),
            "windows not empty: len={}, windows={:?}",
            windows.len(),
            windows
        );
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
    fn list_navigation_retains_invalid_drafts_passwords_color_and_does_not_select_browser(
        cx: &mut TestAppContext,
    ) {
        let model = new_model(cx);
        let (handle, mut visual) = open(&model, cx);
        let original = handle.update(cx, |d, _, _| d.active.clone()).unwrap();
        edit(&mut visual, "source-name", "First draft");
        edit(&mut visual, "source-password", "fixture secret");
        edit(&mut visual, "source-port", "invalid");
        click(&mut visual, "source-color-menu");
        click(&mut visual, "source-color-blue");
        click(&mut visual, "settings-add-source");
        let second = handle.update(cx, |d, _, _| d.active.clone()).unwrap();
        assert_ne!(second, original);
        edit(&mut visual, "source-name", "Second draft");
        click(&mut visual, "settings-source-0");
        handle
            .update(cx, |d, _, app| {
                assert_eq!(d.active, original);
                assert_eq!(d.form.read(app).draft_identity(app).0, "First draft");
                assert_eq!(d.form.read(app).password(app), "fixture secret");
                assert!(d.form.read(app).profile(app).is_err());
            })
            .unwrap();
        edit(&mut visual, "source-port", "3306");
        handle
            .update(cx, |d, _, app| {
                assert_eq!(
                    d.form.read(app).profile(app).unwrap().color.as_deref(),
                    Some("#8AB4F8")
                )
            })
            .unwrap();
        click(&mut visual, "settings-source-1");
        handle
            .update(cx, |d, _, app| {
                assert_eq!(d.active, second);
                assert_eq!(d.form.read(app).draft_identity(app).0, "Second draft");
                assert!(d.form.read(app).password(app).is_empty());
            })
            .unwrap();
        model.read_with(cx, |m, _| {
            assert!(m.profiles.is_empty());
            assert!(m.selected_source.is_none());
        });
        visual.simulate_keystrokes("cmd-w");
        visual.run_until_parked();
        assert!(visual.debug_bounds("settings-discard-close").is_some());
        visual.simulate_keystrokes("cmd-w");
        visual.run_until_parked();
        assert!(model.read_with(cx, |m, _| m.form_open));
        click(&mut visual, "settings-discard-close");
        assert_closed(&model, cx);
    }

    #[gpui::test]
    fn source_shortcuts_duplicate_without_password_and_remove_requires_confirmation(
        cx: &mut TestAppContext,
    ) {
        let model = new_model(cx);
        let (handle, mut visual) = open(&model, cx);
        edit(&mut visual, "source-name", "Fixture");
        edit(&mut visual, "source-password", "fixture secret");
        visual.simulate_keystrokes("cmd-d");
        visual.run_until_parked();
        handle
            .update(cx, |d, _, app| {
                assert_eq!(d.drafts.len(), 2);
                assert_eq!(d.form.read(app).profile(app).unwrap().name, "Fixture copy");
                assert!(d.form.read(app).password(app).is_empty());
                assert!(!d.form.read(app).profile(app).unwrap().save_password);
            })
            .unwrap();
        click(&mut visual, "settings-remove-source");
        handle
            .update(cx, |d, _, _| assert_eq!(d.drafts.len(), 2))
            .unwrap();
        click(&mut visual, "settings-confirm-remove");
        handle
            .update(cx, |d, _, app| {
                assert_eq!(d.drafts.len(), 1);
                assert_eq!(d.form.read(app).draft_identity(app).0, "Fixture");
            })
            .unwrap();
        visual.simulate_keystrokes("cmd-n");
        visual.run_until_parked();
        handle
            .update(cx, |d, _, _| assert_eq!(d.drafts.len(), 2))
            .unwrap();
        edit(&mut visual, "source-port", "invalid");
        visual.simulate_keystrokes("cmd-s");
        visual.run_until_parked();
        assert!(model.read_with(cx, |m, _| !m.saving && m.form_feedback.is_some()));
        click(&mut visual, "source-cancel");
        click(&mut visual, "settings-discard-close");
        assert_closed(&model, cx);
    }

    #[gpui::test]
    fn side_rail_embeds_ssh_and_driver_pages_without_new_window(cx: &mut TestAppContext) {
        let model = new_model(cx);
        let (handle, mut visual) = open(&model, cx);
        edit(&mut visual, "source-name", "Retained source");
        let form_id = handle.update(cx, |d, _, _| d.form.entity_id()).unwrap();
        click(&mut visual, "settings-ssh");
        assert_eq!(cx.update(|app| app.windows().len()), 1);
        assert!(visual.debug_bounds("ssh-manager").is_some());
        assert!(visual.debug_bounds("settings-source-list").is_none());
        click(&mut visual, "settings-driver-page");
        assert!(visual.debug_bounds("settings-drivers").is_some());
        click(&mut visual, "settings-driver-1");
        handle
            .update(cx, |d, _, _| assert_eq!(d.driver, DbEngine::MariaDb))
            .unwrap();
        click(&mut visual, "settings-sources");
        handle
            .update(cx, |d, _, app| {
                assert_eq!(d.form.entity_id(), form_id);
                assert_eq!(d.form.read(app).draft_identity(app).0, "Retained source");
            })
            .unwrap();
        click(&mut visual, "source-tab-ssh");
        click(&mut visual, "source-manage-ssh");
        assert!(visual.debug_bounds("ssh-manager").is_some());
        assert_eq!(cx.update(|app| app.windows().len()), 1);
        click(&mut visual, "settings-sources");
        click(&mut visual, "source-cancel");
        click(&mut visual, "settings-discard-close");
        assert_closed(&model, cx);
    }

    #[gpui::test]
    fn password_load_generation_never_marks_pending_password_or_preset_as_saved(
        cx: &mut TestAppContext,
    ) {
        let model = new_model(cx);
        let (handle, mut visual) = open(&model, cx);
        edit(&mut visual, "source-password", "fixture secret");
        model.update(cx, |m, cx| {
            let profile = m.form_profile.clone().unwrap();
            m.profiles.push(profile);
            m.form_generation += 1;
            cx.notify();
        });
        visual.run_until_parked();
        handle
            .update(cx, |d, _, app| {
                assert!(d.form.read(app).is_dirty());
                assert_eq!(d.form.read(app).password(app), "fixture secret");
            })
            .unwrap();
        click(&mut visual, "source-color-menu");
        click(&mut visual, "source-color-green");
        handle
            .update(cx, |d, _, app| assert!(d.form.read(app).is_dirty()))
            .unwrap();
        click(&mut visual, "source-cancel");
        click(&mut visual, "settings-discard-close");
        assert_closed(&model, cx);
    }

    #[gpui::test]
    fn apply_acknowledgement_stays_open_and_navigation_is_guarded_while_saving(
        cx: &mut TestAppContext,
    ) {
        let model = new_model(cx);
        let (handle, mut visual) = open(&model, cx);
        edit(&mut visual, "source-name", "Applied fixture");
        let profile = handle
            .update(cx, |d, _, app| d.form.read(app).profile(app).unwrap())
            .unwrap();
        let form_id = handle.update(cx, |d, _, _| d.form.entity_id()).unwrap();
        model.update(cx, |m, cx| {
            m.saving = true;
            cx.notify();
        });
        visual.run_until_parked();
        click(&mut visual, "settings-add-source");
        click(&mut visual, "settings-ssh");
        handle
            .update(cx, |d, _, _| {
                assert_eq!(d.form.entity_id(), form_id);
                assert_eq!(d.drafts.len(), 1);
                assert!(d.page == SettingsPage::Sources);
            })
            .unwrap();
        model.update(cx, |m, cx| {
            m.saving = false;
            m.form_busy = false;
            m.profiles.push(profile.clone());
            m.form_profile = Some(profile);
            m.form_generation += 1;
            m.form_saved_generation = m.form_generation;
            cx.notify();
        });
        visual.run_until_parked();
        handle
            .update(cx, |d, _, app| {
                assert_eq!(d.form.entity_id(), form_id);
                assert!(!d.form.read(app).is_dirty());
            })
            .unwrap();
        assert_eq!(cx.update(|app| app.windows().len()), 1);
        click(&mut visual, "settings-ok");
        assert_closed(&model, cx);
    }

    #[gpui::test]
    fn file_menu_actions_work_in_settings_and_cancellation_preserves_active_draft(
        cx: &mut TestAppContext,
    ) {
        let model = new_model(cx);
        let (handle, mut visual) = open(&model, cx);
        edit(&mut visual, "source-name", "Retained fixture");
        visual.update(|window, app| {
            window.dispatch_action(Box::new(super::super::ImportConnectors), app)
        });
        visual.run_until_parked();
        assert!(visual.did_prompt_for_paths());
        visual.simulate_path_prompt_response(|_| None);
        visual.run_until_parked();
        assert!(model.read_with(cx, |m, _| m.connector_feedback.is_none()));
        assert!(visual.debug_bounds("settings-notice").is_none());
        assert_eq!(
            visual.debug_bounds("settings-footer").unwrap().bottom(),
            px(760.)
        );
        handle
            .update(cx, |d, _, app| {
                assert_eq!(d.form.read(app).draft_identity(app).0, "Retained fixture")
            })
            .unwrap();
        visual.update(|window, app| {
            window.dispatch_action(Box::new(super::super::NewConnection), app)
        });
        visual.run_until_parked();
        handle
            .update(cx, |d, _, _| assert_eq!(d.drafts.len(), 2))
            .unwrap();
        model.update(cx, |m, cx| {
            m.profiles.push(SourceProfile::default());
            cx.notify();
        });
        visual.update(|window, app| {
            window.dispatch_action(Box::new(super::super::ExportConnectors), app)
        });
        visual.run_until_parked();
        assert!(visual.did_prompt_for_new_path());
        visual.simulate_new_path_selection(|_| None);
        cx.run_until_parked();
        visual.run_until_parked();
        click(&mut visual, "source-cancel");
        click(&mut visual, "settings-discard-close");
        assert_closed(&model, cx);
    }

    #[gpui::test]
    fn transfer_notice_never_adds_bottom_labels_or_changes_footer_bounds(cx: &mut TestAppContext) {
        let model = new_model(cx);
        model.update(cx, |m, _| {
            m.connector_feedback = Some("Import cancelled; no sources changed.".into())
        });
        let (_, mut visual) = open(&model, cx);
        let footer = visual.debug_bounds("settings-footer").unwrap();
        assert!(visual.debug_bounds("connector-feedback").is_none());
        assert!(visual.debug_bounds("settings-notice").is_none());
        assert_eq!(footer.bottom(), px(760.));
        model.update(cx, |m, cx| {
            m.connector_feedback = Some("Import failed: invalid connector file.".into());
            cx.notify();
        });
        visual.run_until_parked();
        assert!(visual.debug_bounds("settings-notice").is_some());
        assert_eq!(visual.debug_bounds("settings-footer").unwrap(), footer);
        click(&mut visual, "settings-dismiss-notice");
        assert!(visual.debug_bounds("settings-notice").is_none());
        assert!(model.read_with(cx, |m, _| m.connector_feedback.is_none()));
        assert_eq!(visual.debug_bounds("settings-footer").unwrap(), footer);
        click(&mut visual, "source-cancel");
        assert_closed(&model, cx);
    }

    #[gpui::test]
    fn imported_connectors_are_unsaved_retained_drafts_with_independent_ids_and_no_password(
        cx: &mut TestAppContext,
    ) {
        let model = new_model(cx);
        let (handle, mut visual) = open(&model, cx);
        edit(&mut visual, "source-name", "Unrelated retained draft");
        let report=dalan_app::connector_transfer::import_bytes(dalan_app::connector_transfer::ImportFormat::Dbx,br#"{"connections":[{"db_type":"mysql","name":"Imported A","host":"db.example.invalid","password":"must-not-import"},{"db_type":"mariadb","name":"Imported B","host":"db.example.invalid"}]}"#).unwrap();
        model.update(cx, |m, cx| {
            m.imported_profiles = report.profiles;
            cx.notify();
        });
        visual.run_until_parked();
        handle
            .update(cx, |d, _, app| {
                assert_eq!(d.drafts.len(), 3);
                assert_eq!(d.form.read(app).draft_identity(app).0, "Imported A");
                assert!(d.form.read(app).password(app).is_empty());
                assert!(d.form.read(app).is_dirty());
                assert!(d.model.read(app).profiles.is_empty());
            })
            .unwrap();
        click(&mut visual, "settings-source-0");
        handle
            .update(cx, |d, _, app| {
                assert_eq!(
                    d.form.read(app).draft_identity(app).0,
                    "Unrelated retained draft"
                )
            })
            .unwrap();
        click(&mut visual, "source-cancel");
        click(&mut visual, "settings-discard-close");
        assert_closed(&model, cx);
    }

    #[gpui::test]
    fn jdbc_catalog_filters_and_install_trust_are_local_until_explicit_approval(
        cx: &mut TestAppContext,
    ) {
        let model = new_model(cx);
        let (handle, mut visual) = open(&model, cx);
        click(&mut visual, "settings-driver-page");
        click(&mut visual, "jdbc-catalog-0");
        assert!(visual.debug_bounds("jdbc-driver-details").is_some());
        assert!(visual.debug_bounds("jdbc-install").is_some());
        handle
            .update(cx, |d, _, cx| {
                d.jdbc_catalog[0].versions = vec!["2.3.232".into(), "2.2.224".into()];
                d.jdbc_version = Some("2.3.232".into());
                cx.notify();
            })
            .unwrap();
        click(&mut visual, "jdbc-install");
        assert!(visual.debug_bounds("jdbc-trust-confirm").is_some());
        handle
            .update(cx, |d, _, _| {
                assert!(!d.jdbc_busy && d.jdbc_installed.is_empty())
            })
            .unwrap();
        click(&mut visual, "jdbc-filter-1");
        assert!(visual.debug_bounds("jdbc-catalog-0").is_none());
        click(&mut visual, "jdbc-filter-2");
        assert!(visual.debug_bounds("jdbc-catalog-0").is_some());
        handle
            .update(cx, |d, _, cx| {
                d.jdbc_search.update(cx, |i, cx| i.set_value("SQLite", cx));
            })
            .unwrap();
        visual.run_until_parked();
        assert!(visual.debug_bounds("jdbc-catalog-0").is_none());
        assert!(visual.debug_bounds("jdbc-catalog-1").is_some());
        click(&mut visual, "source-cancel");
        assert_closed(&model, cx);
    }
    #[gpui::test]
    fn jdbc_create_connection_uses_selected_installed_metadata_without_credentials(
        cx: &mut TestAppContext,
    ) {
        let model = new_model(cx);
        let (handle, mut visual) = open(&model, cx);
        click(&mut visual, "settings-driver-page");
        handle
            .update(cx, |d, _, cx| {
                let e = &d.jdbc_catalog[0];
                d.jdbc_installed = vec![dalan_app::jdbc_catalog::InstalledDriver {
                    id: e.id.clone(),
                    name: e.name.clone(),
                    group: e.group.clone(),
                    artifact: e.artifact.clone(),
                    version: "2.3.232".into(),
                    driver_class: e.driver_class.clone(),
                    url_prefix: e.url_prefix.clone(),
                    jars: vec!["/fixture/h2.jar".into()],
                    sha256: vec!["a".repeat(64)],
                }];
                d.jdbc_java
                    .update(cx, |i, cx| i.set_value("/fixture/java", cx));
                d.jdbc_selected = Some(e.id.clone());
                d.jdbc_version = Some("2.3.232".into());
                cx.notify();
            })
            .unwrap();
        visual.run_until_parked();
        click(&mut visual, "jdbc-new-source");
        assert!(visual.debug_bounds("source-jdbc-url").is_some());
        assert!(visual.debug_bounds("source-host").is_none());
        handle
            .update(cx, |d, _, app| {
                let profile = d.form.read(app).profile(app).unwrap();
                assert_eq!(profile.engine, DbEngine::Jdbc);
                assert_eq!(profile.jdbc.unwrap().driver_class, "org.h2.Driver");
                assert!(d.form.read(app).password(app).is_empty());
            })
            .unwrap();
        click(&mut visual, "source-cancel");
        click(&mut visual, "settings-discard-close");
        assert_closed(&model, cx);
    }

    #[gpui::test]
    fn driver_page_has_centered_branded_rows_tables_and_only_available_versions(
        cx: &mut TestAppContext,
    ) {
        let model = new_model(cx);
        let (_, mut visual) = open(&model, cx);
        click(&mut visual, "settings-driver-page");
        for index in 0..5 {
            let icon = visual
                .debug_bounds(Box::leak(
                    format!("settings-driver-icon-{index}").into_boxed_str(),
                ))
                .unwrap();
            let label = visual
                .debug_bounds(Box::leak(
                    format!("settings-driver-label-{index}").into_boxed_str(),
                ))
                .unwrap();
            assert_eq!(icon.center().y, label.center().y);
            assert!(icon.right() < label.left());
        }
        assert!(visual.debug_bounds("driver-version-table").is_some());
        assert!(visual.debug_bounds("driver-capability-table").is_some());
        visual.simulate_resize(size(px(1040.), px(560.)));
        visual.run_until_parked();
        let selector = visual.debug_bounds("driver-version-selector").unwrap();
        assert!(selector.left() >= px(258.) && selector.right() <= px(1040.));
        assert_eq!(
            visual.debug_bounds("settings-footer").unwrap().bottom(),
            px(560.)
        );
        click(&mut visual, "driver-version-selector");
        assert!(visual.debug_bounds("driver-choice-latest").is_some());
        visual.simulate_keystrokes("escape");
        visual.run_until_parked();
        click(&mut visual, "source-cancel");
        assert_closed(&model, cx);
    }
    #[gpui::test]
    fn exact_bundled_driver_choice_applies_without_losing_theme_and_cancel_isolated(
        cx: &mut TestAppContext,
    ) {
        use dalan_app::app_config::{AppearancePreference, Config, ConfigRepository};
        let dir =
            std::env::temp_dir().join(format!("dalan-driver-choice-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let repo = ConfigRepository::new(std::fs::canonicalize(&dir).unwrap().join("dalan.config"));
        let config = Config {
            appearance: AppearancePreference::Dark,
            ..Config::default()
        };
        repo.save(&config).unwrap();
        cx.update(|app| {
            app.set_global(super::super::AppearanceSettings {
                config: config.clone(),
                preference: config.appearance,
                repository: Some(repo.clone()),
                error: None,
            })
        });
        let model = new_model(cx);
        let (handle, mut visual) = open(&model, cx);
        click(&mut visual, "settings-driver-page");
        click(&mut visual, "driver-version-selector");
        click(&mut visual, "driver-choice-mysql-async-0.37.1");
        click(&mut visual, "source-save");
        let saved = repo.load().unwrap();
        assert_eq!(saved.appearance, AppearancePreference::Dark);
        assert_eq!(
            saved.driver_versions["mysql"],
            dalan_drivers::versions::MYSQL.id
        );
        handle
            .update(cx, |d, _, cx| {
                d.driver_choices.insert("mysql".into(), "latest".into());
                cx.notify();
            })
            .unwrap();
        click(&mut visual, "source-cancel");
        assert!(visual.debug_bounds("settings-discard-close").is_some());
        click(&mut visual, "settings-discard-close");
        assert_eq!(repo.load().unwrap(), saved);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[gpui::test]
    fn native_titlebar_precedes_persistent_identity_and_tabs(cx: &mut TestAppContext) {
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
        assert!(show.contains("window_min_size: Some(size(px(1040.0), px(560.0)))"));
        assert!(show.contains("window_background: gpui::WindowBackgroundAppearance::Opaque"));
        assert!(!show.contains("JDBC"));
        assert!(!show.contains("Dalan"));

        let model = new_model(cx);
        let (_, mut visual) = open(&model, cx);
        visual.update(|window, _| {
            assert_eq!(window.bounds().size, size(px(1160.), px(760.)));
        });
        for viewport in [size(px(1160.), px(760.)), size(px(1040.), px(560.))] {
            visual.simulate_resize(viewport);
            visual.run_until_parked();
            let titlebar = visual.debug_bounds("source-titlebar").unwrap();
            assert_eq!(titlebar.origin, gpui::point(px(0.), px(0.)));
            assert_eq!(titlebar.size, size(viewport.width, px(34.)));
            let identity = visual.debug_bounds("source-identity-header").unwrap();
            let bar = visual.debug_bounds("source-tab-bar").unwrap();
            assert_eq!(identity.top(), titlebar.bottom());
            assert_eq!(bar.top(), identity.bottom());
            assert_eq!(bar.size.height, px(34.));
            assert_eq!(bar.left(), px(276.));
            assert!(visual.debug_bounds("source-name").unwrap().top() >= titlebar.bottom());
            let mut right = px(292.);
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
                assert!(tab.top() >= bar.top());
                assert!(tab.bottom() <= bar.bottom());
                assert!(tab.size.height > px(0.));
                assert!(tab.right() <= bar.right());
                right = tab.right();
            }
        }
        visual.simulate_keystrokes("cmd-w");
        visual.run_until_parked();
        if model.read_with(cx, |m, _| m.form_open)
            && visual.debug_bounds("settings-discard-close").is_some()
        {
            click(&mut visual, "settings-discard-close");
        }
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
        click(&mut visual, "source-color-menu");
        edit(&mut visual, "source-color", "#123456");
        visual.simulate_keystrokes("escape");
        visual.run_until_parked();
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
        visual.run_until_parked();
        if model.read_with(cx, |m, _| m.form_open)
            && visual.debug_bounds("settings-discard-close").is_some()
        {
            click(&mut visual, "settings-discard-close");
        }
        assert_closed(&model, cx);
    }

    #[gpui::test]
    fn minimum_dialog_keeps_aligned_editors_footer_and_local_validation(cx: &mut TestAppContext) {
        let model = new_model(cx);
        let (_, mut visual) = open(&model, cx);
        visual.simulate_resize(size(px(1040.), px(560.)));
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
            assert!(body.right() <= px(1024.));
            let footer = visual.debug_bounds("source-form-footer").unwrap();
            assert!(footer.bottom() <= px(560.));
            assert!(footer.top() >= px(34.));
            let button = visual.debug_bounds("source-test").unwrap();
            assert!(button.top() >= footer.top());
            assert!(button.bottom() <= footer.bottom());
            if tab == "source-tab-options" {
                let mut left = None;
                for id in [
                    "source-connect-timeout",
                    "source-query-timeout",
                    "source-page-size",
                ] {
                    let field = visual.debug_bounds(id).unwrap();
                    assert_eq!(field.size.height, px(24.));
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
        visual.run_until_parked();
        if model.read_with(cx, |m, _| m.form_open)
            && visual.debug_bounds("settings-discard-close").is_some()
        {
            click(&mut visual, "settings-discard-close");
        }
        assert_closed(&model, cx);
    }

    #[gpui::test]
    fn reuses_dialog_and_preserves_native_draft_edits(cx: &mut TestAppContext) {
        let model = new_model(cx);
        let (handle, mut visual) = open(&model, cx);
        visual.update(|window, _| {
            assert_eq!(window.bounds().size, size(px(1160.), px(760.)));
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
        visual.run_until_parked();
        if model.read_with(cx, |m, _| m.form_open)
            && visual.debug_bounds("settings-discard-close").is_some()
        {
            click(&mut visual, "settings-discard-close");
        }
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
        cx.update(gpui::init);
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
                assert_eq!(dialog.form.entity_id(), form.entity_id());
                assert_eq!(dialog.generation, model.read(app).form_generation);
                assert_eq!(
                    dialog.form.read(app).profile(app).unwrap().name,
                    "Edited draft"
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
        visual.run_until_parked();
        if model.read_with(cx, |m, _| m.form_open)
            && visual.debug_bounds("settings-discard-close").is_some()
        {
            click(&mut visual, "settings-discard-close");
        }
        assert_closed(&model, cx);
    }
}

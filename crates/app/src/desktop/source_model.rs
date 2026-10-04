use std::{collections::HashMap, future::Future, sync::OnceLock};

use anyhow::{Result, anyhow};
use dalan_app::source_store::{NativeSecretStore, SecretStore, SourceRepository};
use dalan_drivers::{
    BrowseRequest, SortDirection, SourceProfile, TableFilter, TableInfo, TablePage, TableSort,
};
use gpui::Context;
use tokio::{runtime::Runtime, task::AbortHandle};

fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("database runtime initialization")
    })
}

pub(super) struct SourceModel {
    pub profiles: Vec<SourceProfile>,
    pub form_open: bool,
    pub form_profile: Option<SourceProfile>,
    pub form_generation: u64,
    pub form_feedback: Option<String>,
    pub form_busy: bool,
    pub saving: bool,
    pub error: Option<String>,
    pub busy: bool,
    pub databases: Vec<String>,
    pub selected_source: Option<String>,
    pub selected_database: Option<String>,
    pub tables: Vec<TableInfo>,
    pub selected_table: Option<String>,
    pub page: Option<TablePage>,
    pub delete_confirm: bool,
    pub sort: Option<TableSort>,
    pub export_busy: bool,
    pub export_feedback: Option<String>,
    passwords: HashMap<String, String>,
    filter: Option<TableFilter>,
    generation: u64,
    operation: Option<AbortHandle>,
    repository: Option<SourceRepository>,
    storage_ready: bool,
    previous_offsets: Vec<u64>,
}

impl Drop for SourceModel {
    fn drop(&mut self) {
        if !self.saving
            && let Some(task) = &self.operation
        {
            task.abort();
        }
    }
}

impl SourceModel {
    pub fn cycle_sort(&mut self, column: String, cx: &mut Context<Self>) {
        if self.busy
            || self.saving
            || self
                .page
                .as_ref()
                .is_none_or(|page| !page.columns.iter().any(|item| item.name == column))
        {
            return;
        }
        self.sort = match &self.sort {
            Some(sort) if sort.column == column && sort.direction == SortDirection::Ascending => {
                Some(TableSort {
                    column,
                    direction: SortDirection::Descending,
                })
            }
            Some(sort) if sort.column == column && sort.direction == SortDirection::Descending => {
                None
            }
            _ => Some(TableSort {
                column,
                direction: SortDirection::Ascending,
            }),
        };
        self.previous_offsets.clear();
        self.load_page(0, cx);
        cx.notify();
    }

    pub fn request_export(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.saving || self.error.is_some() || self.export_busy {
            return;
        }
        let Some(page) = self.page.clone().filter(|page| !page.truncated) else {
            return;
        };
        let generation = self.generation;
        self.export_busy = true;
        self.export_feedback = Some(
            "Choose a new CSV file for the loaded page. Existing files are not overwritten.".into(),
        );
        let home = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let picker = cx.prompt_for_new_path(&home, Some("Dalan-loaded-page.csv"));
        cx.spawn(async move |this, cx| {
            let selection = picker.await.map_err(|_| anyhow!("Save dialog closed unexpectedly")).and_then(|result| result);
            let mut proceed = false;
            let _ = this.update(cx, |this, cx| {
                if generation != this.generation {
                    this.export_busy = false;
                    this.export_feedback = Some("Export cancelled because the selected table or page changed.".into());
                    cx.notify(); return;
                }
                if matches!(selection, Ok(Some(_))) { proceed = true; }
                else {
                    this.export_busy = false;
                    this.export_feedback = Some(match &selection { Ok(None) => "Export cancelled; no file written.".into(), Err(error) => format!("Export failed: {error}"), _ => unreachable!() });
                    cx.notify();
                }
            });
            if !proceed { return; }
            let path = selection.unwrap().unwrap();
            let count = page.rows.len();
            let result = runtime().spawn_blocking(move || dalan_app::table_export::export_loaded_page(&path, &page, &Default::default())).await
                .map_err(|_| anyhow!("Export worker stopped")).and_then(|result| result);
            let _ = this.update(cx, |this, cx| {
                this.export_busy = false;
                this.export_feedback = Some(match result {
                    Ok(()) => format!("Exported {count} loaded row(s). NULL is \\N; spreadsheet-safe text may be prefixed with an apostrophe."),
                    Err(error) => format!("Export failed: {error}"),
                });
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
}

impl SourceModel {
    fn empty(repository: Option<SourceRepository>) -> Self {
        Self {
            profiles: vec![],
            form_open: false,
            form_profile: None,
            form_generation: 0,
            form_feedback: None,
            form_busy: false,
            saving: false,
            error: None,
            busy: false,
            databases: vec![],
            selected_source: None,
            selected_database: None,
            tables: vec![],
            selected_table: None,
            page: None,
            delete_confirm: false,
            sort: None,
            export_busy: false,
            export_feedback: None,
            passwords: HashMap::new(),
            filter: None,
            generation: 0,
            operation: None,
            repository,
            storage_ready: false,
            previous_offsets: vec![],
        }
    }

    #[cfg(not(all(test, feature = "ui-tests")))]
    pub fn new(cx: &mut Context<Self>) -> Self {
        let repository = SourceRepository::default_path().map(SourceRepository::new);
        let mut model = Self::empty(repository.as_ref().ok().cloned());
        if let Err(error) = repository {
            model.error = Some(error.to_string());
            return model;
        }
        let repo = model.repository.clone().unwrap();
        model.busy = true;
        model.run(
            async move { tokio::task::spawn_blocking(move || repo.load()).await? },
            cx,
            |this, result, cx| {
                this.busy = false;
                match result {
                    Ok(profiles) => {
                        this.profiles = profiles;
                        this.storage_ready = true;
                    }
                    Err(error) => this.error = Some(error.to_string()),
                };
                cx.notify();
            },
        );
        model
    }

    #[cfg(all(test, feature = "ui-tests"))]
    pub fn for_tests(profiles: Vec<SourceProfile>) -> Self {
        let mut model = Self::empty(None);
        model.profiles = profiles;
        model.storage_ready = true;
        model
    }

    fn invalidate(&mut self) {
        self.generation += 1;
        if let Some(task) = self.operation.take() {
            task.abort();
        }
        self.busy = false;
        self.form_busy = false;
        self.export_feedback = None;
    }

    fn run<T: Send + 'static>(
        &mut self,
        future: impl Future<Output = Result<T>> + Send + 'static,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, Result<T>, &mut Context<Self>) + 'static,
    ) {
        let generation = self.generation;
        let job = runtime().spawn(future);
        self.operation = Some(job.abort_handle());
        cx.spawn(async move |this, cx| {
            let result = job
                .await
                .map_err(|_| anyhow!("Operation cancelled or worker stopped"))
                .and_then(|result| result);
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.operation = None;
                done(this, result, cx);
            });
        })
        .detach();
    }

    pub fn password(&self, id: &str) -> String {
        self.passwords.get(id).cloned().unwrap_or_default()
    }

    pub fn new_source(&mut self, cx: &mut Context<Self>) {
        if self.saving || (!self.storage_ready && self.busy) {
            return;
        }
        self.invalidate();
        self.form_generation += 1;
        self.form_open = true;
        self.form_profile = Some(SourceProfile::default());
        self.form_feedback = None;
        cx.notify();
    }

    pub fn edit_source(&mut self, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let Some(profile) = self.selected_profile() else {
            return;
        };
        if !self.storage_ready {
            self.form_feedback = Some(
                "Source settings did not load successfully; resolve that error before saving."
                    .into(),
            );
            cx.notify();
            return;
        }
        self.invalidate();
        self.form_generation += 1;
        self.form_open = true;
        self.form_profile = Some(profile.clone());
        self.form_feedback = None;
        if profile.save_password && !self.passwords.contains_key(&profile.id) {
            let id = profile.id.clone();
            self.form_busy = true;
            self.run(async move { tokio::task::spawn_blocking(move || NativeSecretStore.get(&id)).await? }, cx,
                |this, result, cx| { this.form_busy = false; match result {
                    Ok(Some(password)) => { if let Some(profile) = &this.form_profile { this.passwords.insert(profile.id.clone(), password); } this.form_generation += 1; },
                    Ok(None) => this.form_feedback = Some("Saved Keychain password is missing; enter it again.".into()),
                    Err(error) => this.form_feedback = Some(error.to_string()),
                }; cx.notify(); });
        }
        cx.notify();
    }

    pub fn edit_form(&mut self, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        self.invalidate();
        self.form_feedback = None;
        cx.notify();
    }

    pub fn close_form(&mut self, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        self.invalidate();
        self.form_open = false;
        self.form_profile = None;
        self.form_feedback = None;
        self.form_generation += 1;
        cx.notify();
    }

    pub fn test(&mut self, profile: SourceProfile, password: String, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        self.invalidate();
        self.form_busy = true;
        self.form_feedback = Some("Testing connection…".into());
        self.run(
            async move { dalan_drivers::test_connection(&profile, &password).await },
            cx,
            |this, result, cx| {
                this.form_busy = false;
                this.form_feedback = Some(match result {
                    Ok(report) => format!(
                        "Connected: {}. {} visible database(s).",
                        report.server_version,
                        report.databases.len()
                    ),
                    Err(error) => format!("Connection failed: {error}"),
                });
                cx.notify();
            },
        );
        cx.notify();
    }

    pub fn save(&mut self, profile: SourceProfile, password: String, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        if let Err(error) = profile.validate() {
            self.form_feedback = Some(error.to_string());
            cx.notify();
            return;
        }
        let Some(repo) = self.repository.clone() else {
            self.form_feedback = Some("Profile storage is unavailable; nothing was saved.".into());
            cx.notify();
            return;
        };
        self.invalidate();
        self.saving = true;
        self.form_busy = true;
        self.form_feedback = Some("Saving profile and credential choice…".into());
        let mut profiles = self.profiles.clone();
        let previously_saved = profiles
            .iter()
            .find(|item| item.id == profile.id)
            .is_some_and(|item| item.save_password);
        if let Some(index) = profiles.iter().position(|item| item.id == profile.id) {
            profiles[index] = profile.clone();
        } else {
            profiles.push(profile.clone());
        }
        let saved_profile = profile.clone();
        let saved_password = password.clone();
        self.run(async move { tokio::task::spawn_blocking(move || {
            let store = NativeSecretStore;
            // Validate settings without mutating disk before touching credentials.
            for item in &profiles { item.validate()?; }
            let credential_change = profile.save_password || previously_saved;
            let old = if credential_change { store.get(&profile.id)? } else { None };
            if profile.save_password { store.set(&profile.id, &password)?; }
            else if old.is_some() { store.delete(&profile.id)?; }
            if let Err(error) = repo.save(&profiles) {
                let restore = if credential_change { match old { Some(old) => store.set(&profile.id, &old), None => store.delete(&profile.id) } } else { Ok(()) };
                return Err(if restore.is_err() { anyhow!("Settings save failed; restoring the Keychain credential also failed. Review this source before connecting.") } else { error });
            }
            Ok(profiles)
        }).await? }, cx, move |this, result, cx| {
            this.saving = false; this.form_busy = false;
            match result {
                Ok(profiles) => { this.profiles = profiles; this.passwords.insert(saved_profile.id.clone(), saved_password);
                    this.selected_source = Some(saved_profile.id); this.form_open = false; this.form_profile = None;
                    this.form_generation += 1; this.form_feedback = None; this.clear_data(); this.error = None; },
                Err(error) => this.form_feedback = Some(format!("Not saved: {error}")),
            } cx.notify();
        });
        cx.notify();
    }

    fn selected_profile(&self) -> Option<SourceProfile> {
        self.profiles
            .iter()
            .find(|profile| Some(&profile.id) == self.selected_source.as_ref())
            .cloned()
    }
    fn clear_data(&mut self) {
        self.databases.clear();
        self.selected_database = None;
        self.tables.clear();
        self.selected_table = None;
        self.page = None;
        self.filter = None;
        self.sort = None;
        self.previous_offsets.clear();
    }

    async fn resolve_password(profile: &SourceProfile, session: Option<String>) -> Result<String> {
        if let Some(password) = session {
            return Ok(password);
        }
        if profile.save_password {
            let id = profile.id.clone();
            tokio::task::spawn_blocking(move || NativeSecretStore.get(&id))
                .await??
                .ok_or_else(|| {
                    anyhow!("Keychain password is missing. Edit this source to enter it again.")
                })
        } else {
            Ok(String::new())
        }
    }

    pub fn connect(&mut self, id: String, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let Some(profile) = self
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .cloned()
        else {
            return;
        };
        self.invalidate();
        self.clear_data();
        self.form_open = false;
        self.form_profile = None;
        self.form_generation += 1;
        self.selected_source = Some(id.clone());
        self.error = None;
        self.delete_confirm = false;
        self.busy = true;
        let session = self.passwords.get(&id).cloned();
        self.run(
            async move {
                let password = Self::resolve_password(&profile, session).await?;
                let report = dalan_drivers::test_connection(&profile, &password).await?;
                Ok((report, password))
            },
            cx,
            move |this, result, cx| {
                this.busy = false;
                match result {
                    Ok((report, password)) => {
                        this.passwords.insert(id, password);
                        this.databases = report.databases;
                    }
                    Err(error) => this.error = Some(error.to_string()),
                };
                cx.notify();
            },
        );
        cx.notify();
    }

    pub fn select_database(&mut self, database: String, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let Some(profile) = self.selected_profile() else {
            return;
        };
        self.invalidate();
        self.selected_database = Some(database.clone());
        self.sort = None;
        self.selected_table = None;
        self.tables.clear();
        self.page = None;
        self.filter = None;
        self.previous_offsets.clear();
        self.error = None;
        self.busy = true;
        let password = self.password(&profile.id);
        self.run(
            async move { dalan_drivers::tables(&profile, &password, &database).await },
            cx,
            |this, result, cx| {
                this.busy = false;
                match result {
                    Ok(tables) => this.tables = tables,
                    Err(error) => this.error = Some(error.to_string()),
                };
                cx.notify();
            },
        );
        cx.notify();
    }

    pub fn select_table(&mut self, table: String, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        self.selected_table = Some(table);
        self.sort = None;
        self.filter = None;
        self.page = None;
        self.previous_offsets.clear();
        self.load_page(0, cx);
    }
    fn load_page(&mut self, offset: u64, cx: &mut Context<Self>) {
        let Some(profile) = self.selected_profile() else {
            return;
        };
        let (Some(database), Some(table)) =
            (self.selected_database.clone(), self.selected_table.clone())
        else {
            return;
        };
        self.invalidate();
        self.busy = true;
        self.error = None;
        let password = self.password(&profile.id);
        let request = BrowseRequest {
            database,
            table,
            filter: self.filter.clone(),
            sort: self.sort.clone(),
            offset,
            limit: 100,
        };
        self.run(
            async move { dalan_drivers::browse(&profile, &password, &request).await },
            cx,
            |this, result, cx| {
                this.busy = false;
                match result {
                    Ok(page) => this.page = Some(page),
                    Err(error) => this.error = Some(error.to_string()),
                };
                cx.notify();
            },
        );
        cx.notify();
    }
    pub fn apply_filter(&mut self, filter: Option<TableFilter>, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        self.filter = filter;
        self.previous_offsets.clear();
        self.load_page(0, cx);
    }
    pub fn change_page(&mut self, forward: bool, cx: &mut Context<Self>) {
        if self.busy || self.saving || self.error.is_some() {
            return;
        }
        let Some(page) = &self.page else {
            return;
        };
        let offset = if forward {
            let Some(next) = page.next_offset else {
                return;
            };
            self.previous_offsets.push(page.offset);
            next
        } else {
            self.previous_offsets.pop().unwrap_or(0)
        };
        self.load_page(offset, cx);
    }
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        self.previous_offsets.clear();
        if self.selected_table.is_some() {
            self.load_page(0, cx);
        } else if let Some(database) = self.selected_database.clone() {
            self.select_database(database, cx);
        } else if let Some(id) = self.selected_source.clone() {
            self.connect(id, cx);
        }
    }
    pub fn request_delete(&mut self, cx: &mut Context<Self>) {
        if !self.saving {
            self.delete_confirm = !self.delete_confirm;
            cx.notify();
        }
    }
    pub fn confirm_delete(&mut self, cx: &mut Context<Self>) {
        if self.saving || !self.delete_confirm {
            return;
        }
        let (Some(id), Some(repo)) = (self.selected_source.clone(), self.repository.clone()) else {
            return;
        };
        self.invalidate();
        self.saving = true;
        self.busy = true;
        self.delete_confirm = false;
        let profiles: Vec<_> = self
            .profiles
            .iter()
            .filter(|profile| profile.id != id)
            .cloned()
            .collect();
        let removed_id = id.clone();
        let credential_saved = self
            .selected_profile()
            .is_some_and(|profile| profile.save_password);
        self.run(async move { tokio::task::spawn_blocking(move || {
            let store = NativeSecretStore; let old = if credential_saved { store.get(&id)? } else { None };
            if credential_saved { store.delete(&id)?; }
            if let Err(error) = repo.save(&profiles) { if let Some(password) = old { store.set(&id, &password).map_err(|_| anyhow!("Settings removal failed and Keychain restore failed; review saved source."))?; } return Err(error); }
            Ok(profiles)
        }).await? }, cx, move |this, result, cx| { this.saving = false; this.busy = false;
            match result { Ok(profiles) => { this.profiles = profiles; this.passwords.remove(&removed_id); this.selected_source = None; this.clear_data(); this.error = None; }, Err(error) => this.error = Some(format!("Not removed: {error}")) }; cx.notify(); });
        cx.notify();
    }
}

#[cfg(all(test, feature = "ui-tests"))]
impl SourceModel {
    pub fn test_filter(&self) -> Option<TableFilter> {
        self.filter.clone()
    }
}

#[cfg(all(test, feature = "ui-tests"))]
mod tests {
    use super::*;
    use dalan_drivers::{CellValue, FilterOperator, mysql::ColumnInfo};
    use gpui::{AppContext, TestAppContext};

    fn loaded_model() -> SourceModel {
        let mut model = SourceModel::for_tests(vec![]);
        model.selected_database = Some("inventory".into());
        model.selected_table = Some("items".into());
        model.page = Some(TablePage {
            columns: ["id", "name"]
                .into_iter()
                .map(|name| ColumnInfo {
                    name: name.into(),
                    data_type: "VARCHAR".into(),
                    nullable: true,
                    is_primary_key: name == "id",
                })
                .collect(),
            rows: vec![
                vec![
                    CellValue::Number("41".into()),
                    CellValue::Text("Alice".into()),
                ],
                vec![CellValue::Number("42".into()), CellValue::Null],
            ],
            offset: 40,
            next_offset: Some(42),
            has_more: true,
            truncated: false,
        });
        // No selected source: sorting must not contact a database.
        model
    }

    struct ExportSandbox(std::path::PathBuf);

    impl ExportSandbox {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .canonicalize()
                .unwrap()
                .join(format!("dalan-export-model-test-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir(&path).unwrap();
            Self(path.canonicalize().unwrap())
        }
    }

    impl Drop for ExportSandbox {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn wait_for_export(model: &gpui::Entity<SourceModel>, cx: &mut TestAppContext) {
        for _ in 0..100 {
            cx.run_until_parked();
            if !model.read_with(cx, |model, _| model.export_busy) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("export did not complete within one second");
    }

    #[gpui::test]
    fn sorting_cycles_preserves_filter_and_clears_paging_history(cx: &mut TestAppContext) {
        let model = cx.new(|_| loaded_model());
        model.update(cx, |model, cx| {
            let filter = TableFilter {
                column: "name".into(),
                operator: FilterOperator::Equals,
                value: "Alice".into(),
            };
            model.filter = Some(filter.clone());
            for direction in [
                Some(SortDirection::Ascending),
                Some(SortDirection::Descending),
                None,
            ] {
                model.previous_offsets = vec![0, 20];
                model.cycle_sort("id".into(), cx);
                assert_eq!(model.sort.as_ref().map(|sort| sort.direction), direction);
                let retained = model.filter.as_ref().unwrap();
                assert_eq!(retained.column, filter.column);
                assert_eq!(retained.operator, filter.operator);
                assert_eq!(retained.value, filter.value);
                assert!(model.previous_offsets.is_empty());
                assert!(model.operation.is_none());
                assert!(!model.busy);
            }
            model.cycle_sort("id".into(), cx);
            model.cycle_sort("name".into(), cx);
            assert_eq!(
                model.sort,
                Some(TableSort {
                    column: "name".into(),
                    direction: SortDirection::Ascending
                })
            );
            model.select_table("other_items".into(), cx);
            assert_eq!(model.selected_table.as_deref(), Some("other_items"));
            assert!(model.sort.is_none());
            assert!(model.filter.is_none());
            assert!(model.page.is_none());
        });
    }

    #[gpui::test]
    fn sorting_rejects_busy_saving_and_invalid_column_metadata(cx: &mut TestAppContext) {
        let model = cx.new(|_| loaded_model());
        model.update(cx, |model, cx| {
            model.previous_offsets = vec![0, 20];
            model.busy = true;
            model.cycle_sort("id".into(), cx);
            model.busy = false;
            model.saving = true;
            model.cycle_sort("id".into(), cx);
            model.saving = false;
            model.cycle_sort("not_a_column".into(), cx);
            model.page.as_mut().unwrap().columns.clear();
            model.cycle_sort("id".into(), cx);
            model.page = None;
            model.cycle_sort("id".into(), cx);
            assert!(model.sort.is_none());
            assert_eq!(model.previous_offsets, vec![0, 20]);
            assert!(model.operation.is_none());
        });
    }

    #[gpui::test]
    fn export_saves_only_loaded_rows_even_with_more_pages(cx: &mut TestAppContext) {
        let sandbox = ExportSandbox::new();
        let path = sandbox.0.join("loaded.csv");
        let model = cx.new(|_| loaded_model());
        model.update(cx, |model, cx| model.request_export(cx));
        assert!(cx.did_prompt_for_new_path());
        assert!(model.read_with(cx, |model, _| model.export_busy));
        cx.simulate_new_path_selection(|_| Some(path.clone()));
        wait_for_export(&model, cx);
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "\"id\",\"name\"\r\n\"41\",\"Alice\"\r\n\"42\",\\N\r\n"
        );
        model.read_with(cx, |model, _| {
            assert!(
                model
                    .export_feedback
                    .as_deref()
                    .unwrap()
                    .starts_with("Exported 2 loaded row(s).")
            );
            assert!(model.page.as_ref().unwrap().has_more);
            assert!(model.operation.is_none());
        });
    }

    #[gpui::test]
    fn export_changed_generation_cancels_before_writing(cx: &mut TestAppContext) {
        let sandbox = ExportSandbox::new();
        let path = sandbox.0.join("stale.csv");
        let model = cx.new(|_| loaded_model());
        model.update(cx, |model, cx| {
            model.request_export(cx);
            model.invalidate();
        });
        assert!(cx.did_prompt_for_new_path());
        cx.simulate_new_path_selection(|_| Some(path.clone()));
        wait_for_export(&model, cx);
        assert!(!path.exists());
        assert_eq!(
            model
                .read_with(cx, |model, _| model.export_feedback.clone())
                .as_deref(),
            Some("Export cancelled because the selected table or page changed.")
        );
    }

    #[gpui::test]
    fn export_cancel_clears_busy_and_double_request_opens_only_one_picker(cx: &mut TestAppContext) {
        let model = cx.new(|_| loaded_model());
        model.update(cx, |model, cx| {
            model.request_export(cx);
            model.request_export(cx);
            assert!(model.export_busy);
        });
        assert!(cx.did_prompt_for_new_path());
        cx.simulate_new_path_selection(|_| None);
        wait_for_export(&model, cx);
        assert!(!cx.did_prompt_for_new_path());
        assert_eq!(
            model
                .read_with(cx, |model, _| model.export_feedback.clone())
                .as_deref(),
            Some("Export cancelled; no file written.")
        );
    }

    #[gpui::test]
    fn export_refuses_overwrite_and_preserves_existing_file(cx: &mut TestAppContext) {
        let sandbox = ExportSandbox::new();
        let path = sandbox.0.join("existing.csv");
        std::fs::write(&path, "keep these bytes\n").unwrap();
        let model = cx.new(|_| loaded_model());
        model.update(cx, |model, cx| model.request_export(cx));
        cx.simulate_new_path_selection(|_| Some(path.clone()));
        wait_for_export(&model, cx);
        assert_eq!(std::fs::read_to_string(path).unwrap(), "keep these bytes\n");
        assert!(model.read_with(cx, |model, _| {
            model
                .export_feedback
                .as_deref()
                .unwrap()
                .starts_with("Export failed:")
        }));
        assert_eq!(std::fs::read_dir(&sandbox.0).unwrap().count(), 1);
    }

    #[gpui::test]
    fn export_unavailable_pages_and_busy_states_never_open_picker(cx: &mut TestAppContext) {
        for state in 0..5 {
            let model = cx.new(|_| {
                let mut model = loaded_model();
                match state {
                    0 => model.page.as_mut().unwrap().truncated = true,
                    1 => model.error = Some("stale page after failed reload".into()),
                    2 => model.busy = true,
                    3 => model.saving = true,
                    _ => model.page = None,
                }
                model
            });
            model.update(cx, |model, cx| model.request_export(cx));
            assert!(!cx.did_prompt_for_new_path(), "state {state}");
            model.read_with(cx, |model, _| {
                assert!(!model.export_busy);
                assert!(model.export_feedback.is_none());
            });
        }
    }

    #[gpui::test]
    fn ready_fixture_and_edit_preserve_saved_profile_and_session_secret(cx: &mut TestAppContext) {
        let profile = SourceProfile::default();
        let id = profile.id.clone();
        let model = cx.new(|_| SourceModel::for_tests(vec![profile.clone()]));
        model.update(cx, |model, cx| {
            assert!(model.storage_ready);
            assert!(model.repository.is_none());
            assert!(model.operation.is_none());
            model.selected_source = Some(id.clone());
            model.passwords.insert(id.clone(), "session-secret".into());
            model.edit_source(cx);
            assert!(model.form_open);
            assert_eq!(model.form_profile.as_ref().unwrap().id, id);
            assert_eq!(model.selected_profile().unwrap().id, id);
            assert_eq!(model.password(&id), "session-secret");
            let serialized = serde_json::to_string(&model.profiles).unwrap();
            assert!(!serialized.contains("session-secret"));
            assert!(!format!("{:?}", model.form_profile).contains("session-secret"));
            assert!(model.operation.is_none());
            let generation = model.generation;
            model.form_feedback = Some("old test error".into());
            model.edit_form(cx);
            assert_eq!(model.generation, generation + 1);
            assert!(model.form_feedback.is_none());
            assert_eq!(model.profiles[0].id, profile.id);
            assert_eq!(model.profiles[0].host, profile.host);
        });
    }

    #[gpui::test]
    fn saving_guard_refuses_close_edit_and_new_draft(cx: &mut TestAppContext) {
        let model = cx.new(|_| SourceModel::for_tests(vec![]));
        model.update(cx, |model, cx| {
            model.new_source(cx);
            let id = model.form_profile.as_ref().unwrap().id.clone();
            let generation = model.form_generation;
            model.saving = true;
            model.form_feedback = Some("Saving".into());
            model.close_form(cx);
            model.edit_form(cx);
            model.new_source(cx);
            assert!(model.form_open);
            assert_eq!(model.form_generation, generation);
            assert_eq!(model.form_profile.as_ref().unwrap().id, id);
            assert_eq!(model.form_feedback.as_deref(), Some("Saving"));
            model.saving = false;
            model.close_form(cx);
            assert!(!model.form_open);
            assert!(model.form_profile.is_none());
        });
    }

    #[gpui::test]
    async fn invalidation_cancels_stale_completion_but_current_run_completes(
        cx: &mut TestAppContext,
    ) {
        let model = cx.new(|_| SourceModel::for_tests(vec![]));
        let (sender, receiver) = tokio::sync::oneshot::channel::<String>();
        model.update(cx, |model, cx| {
            model.busy = true;
            model.form_busy = true;
            model.run(
                async move { Ok(receiver.await?) },
                cx,
                |model, result, _| {
                    model.error = Some(result.unwrap());
                },
            );
            let generation = model.generation;
            model.invalidate();
            assert_eq!(model.generation, generation + 1);
            assert!(!model.busy);
            assert!(!model.form_busy);
            assert!(model.operation.is_none());
        });
        let _ = sender.send("stale result".into());
        let (sender, receiver) = tokio::sync::oneshot::channel::<String>();
        model.update(cx, |model, cx| {
            model.run(
                async move { Ok(receiver.await?) },
                cx,
                |model, result, cx| {
                    model.form_feedback = Some(result.unwrap());
                    cx.notify();
                },
            );
        });
        sender.send("current result".into()).unwrap();
        for _ in 0..50 {
            cx.run_until_parked();
            if model.read_with(cx, |model, _| model.form_feedback.is_some()) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        model.read_with(cx, |model, _| {
            assert!(model.error.is_none());
            assert_eq!(model.form_feedback.as_deref(), Some("current result"));
            assert!(model.operation.is_none());
        });
    }

    #[gpui::test]
    fn session_only_save_and_confirmed_delete_persist_without_keychain(cx: &mut TestAppContext) {
        struct Sandbox(std::path::PathBuf);
        impl Drop for Sandbox {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let directory = Sandbox(
            std::env::temp_dir()
                .canonicalize()
                .unwrap()
                .join(format!("dalan-model-test-{}", uuid::Uuid::new_v4())),
        );
        let path = directory.0.join("sources.json");
        let repository = SourceRepository::new(path.clone());
        let model = cx.new(|_| {
            let mut model = SourceModel::for_tests(vec![]);
            model.repository = Some(repository.clone());
            model
        });
        let profile = SourceProfile::default();
        let id = profile.id.clone();
        model.update(cx, |model, cx| {
            model.save(profile, "ephemeral-fixture-secret".into(), cx)
        });
        for _ in 0..100 {
            cx.run_until_parked();
            if !model.read_with(cx, |model, _| model.saving) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        model.read_with(cx, |model, _| {
            assert!(!model.saving);
            assert!(model.form_feedback.is_none(), "{:?}", model.form_feedback);
            assert_eq!(model.profiles.len(), 1);
            assert_eq!(model.password(&id), "ephemeral-fixture-secret");
        });
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("ephemeral-fixture-secret"));
        assert_eq!(repository.load().unwrap().len(), 1);
        model.update(cx, |model, cx| {
            model.request_delete(cx);
            assert!(model.delete_confirm);
            model.confirm_delete(cx);
        });
        for _ in 0..100 {
            cx.run_until_parked();
            if !model.read_with(cx, |model, _| model.saving) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        model.read_with(cx, |model, _| {
            assert!(!model.saving);
            assert!(model.error.is_none(), "{:?}", model.error);
            assert!(model.profiles.is_empty());
            assert!(model.password(&id).is_empty());
        });
        assert!(repository.load().unwrap().is_empty());
    }
}

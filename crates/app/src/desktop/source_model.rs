use std::{
    collections::{HashMap, HashSet},
    future::Future,
    sync::{Arc, OnceLock},
};

use anyhow::{Result, anyhow};
use dalan_app::explorer_tree::{ExplorerTree, TreeKey};
use dalan_app::schema_cache::{CachedSchema, SchemaCache, connection_identity};
use dalan_app::source_store::{NativeSecretStore, SecretStore, SourceRepository};
use dalan_drivers::{
    BrowseRequest, CatalogSnapshot, DatabaseCatalog, SortDirection, SourceProfile, TableFilter,
    TableInfo, TablePage, TableSort, discover_catalog,
};
use gpui::Context;
use tokio::{runtime::Runtime, sync::Mutex, task::AbortHandle};

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

/// Serialize ticket allocation, including blocking work outliving a canceled refresh.
async fn admit_cache_work<T: Send + 'static>(
    gate: Arc<Mutex<()>>,
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    let guard = gate.lock_owned().await;
    tokio::task::spawn_blocking(move || {
        // The worker owns admission even while queued or after its waiter is aborted.
        let _guard = guard;
        work()
    })
    .await?
}

type StartupCatalog = (
    Vec<SourceProfile>,
    Vec<(String, CachedSchema)>,
    Option<String>,
);

/// Settings remain usable even when the optional metadata store cannot be opened.
fn load_profiles_and_cache(
    repo: &SourceRepository,
    cache: Option<&SchemaCache>,
) -> Result<StartupCatalog> {
    let profiles = repo.load()?;
    let mut snapshots = Vec::new();
    let mut warning = None;
    if let Some(cache) = cache {
        if cache.prune(&profiles).is_err() {
            warning = Some("Some local metadata could not be restored.".into());
        }
        for profile in &profiles {
            if cache.register(profile).is_err() {
                warning = Some("Some local metadata could not be restored.".into());
                continue;
            }
            match cache.load(profile) {
                Ok(Some(snapshot)) => snapshots.push((profile.id.clone(), snapshot)),
                Ok(None) => {}
                Err(_) => warning = Some("Some local metadata could not be restored.".into()),
            }
        }
    }
    Ok((profiles, snapshots, warning))
}

struct RefreshOutcome {
    snapshot: CatalogSnapshot,
    password: String,
    fetched_at: u64,
    warning: Option<String>,
}

pub(super) struct SourceModel {
    pub metadata_notice: Option<String>,
    pub cached_at: HashMap<String, u64>,
    pub cached_offline: HashSet<String>,
    schema_cache: Option<SchemaCache>,
    cache_admission: Arc<Mutex<()>>,
    automatic_discovery: bool,
    pub profiles: Vec<SourceProfile>,
    pub tree: ExplorerTree,
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
    pub explorer_source: Option<String>,
    pending_delete_source: Option<String>,
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
    tree_generation: u64,
    tree_operations: HashMap<TreeKey, (u64, AbortHandle)>,
    repository: Option<SourceRepository>,
    storage_ready: bool,
    previous_offsets: Vec<u64>,
}

impl Drop for SourceModel {
    fn drop(&mut self) {
        for (_, task) in self.tree_operations.values() {
            task.abort();
        }
        if !self.saving
            && let Some(task) = &self.operation
        {
            task.abort();
        }
    }
}

impl SourceModel {
    pub fn refreshing_source(&self, id: &str) -> bool {
        self.tree.loading.contains(&TreeKey::Source(id.into()))
    }

    fn install_snapshot(
        &mut self,
        id: &str,
        snapshot: &CatalogSnapshot,
        captured_at: u64,
        offline: bool,
    ) {
        let names: HashSet<_> = snapshot
            .databases
            .iter()
            .map(|db| db.name.as_str())
            .collect();
        self.tree
            .expanded_databases
            .retain(|(source, db)| source != id || names.contains(db.as_str()));
        self.tree.expanded_groups.retain(|(source, db, views)| {
            source != id
                || snapshot.databases.iter().any(|catalog| {
                    catalog.name == *db
                        && catalog
                            .tables
                            .iter()
                            .any(|table| (table.kind != "BASE TABLE") == *views)
                })
        });
        self.tree.tables.retain(|(source, _), _| source != id);
        self.tree.errors.retain(|key, _| key.source() != id);
        let databases: Vec<_> = snapshot
            .databases
            .iter()
            .map(|db| db.name.clone())
            .collect();
        self.tree.databases.insert(id.into(), databases.clone());
        for DatabaseCatalog { name, tables } in &snapshot.databases {
            self.tree
                .tables
                .insert((id.into(), name.clone()), tables.clone());
        }
        self.tree.expanded_sources.insert(id.into());
        self.cached_at.insert(id.into(), captured_at);
        if offline {
            self.cached_offline.insert(id.into());
        } else {
            self.cached_offline.remove(id);
        }
        if self.selected_source.as_deref() == Some(id) {
            self.databases = databases;
            if let Some(database) = &self.selected_database {
                self.tables = self
                    .tree
                    .tables
                    .get(&(id.into(), database.clone()))
                    .cloned()
                    .unwrap_or_default();
            }
        }
        // The displayed row data and selected table are deliberately untouched.
    }

    pub fn refresh_schema(&mut self, id: String, cx: &mut Context<Self>) {
        if self.saving || self.refreshing_source(&id) {
            return;
        }
        let Some(profile) = self.profiles.iter().find(|p| p.id == id).cloned() else {
            return;
        };
        let cache = self.schema_cache.clone();
        let cache_admission = self.cache_admission.clone();
        let session = self.passwords.get(&id).cloned();
        self.tree.expanded_sources.insert(id.clone());
        self.run_catalog(
            TreeKey::Source(id.clone()),
            async move {
                let mut warning = None;
                let ticket = if let Some(cache) = cache.clone() {
                    let profile = profile.clone();
                    match admit_cache_work(cache_admission, move || cache.begin_refresh(&profile))
                        .await
                    {
                        Ok(ticket) => Some(ticket),
                        _ => {
                            warning = Some(
                                "Local metadata cache unavailable; snapshot is in-memory only."
                                    .into(),
                            );
                            None
                        }
                    }
                } else {
                    warning = Some(
                        "Local metadata cache unavailable; snapshot is in-memory only.".into(),
                    );
                    None
                };
                let password = Self::resolve_password(&profile, session).await?;
                let snapshot = discover_catalog(&profile, &password).await?;
                let fetched_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs();
                if let (Some(cache), Some(ticket)) = (cache, ticket) {
                    let persisted = snapshot.clone();
                    match tokio::task::spawn_blocking(move || {
                        cache.replace(&ticket, &persisted, fetched_at)
                    })
                    .await
                    {
                        Ok(Ok(true)) => {}
                        Ok(Ok(false)) => {
                            return Err(anyhow!(
                                "Schema refresh superseded; old metadata retained."
                            ));
                        }
                        _ => {
                            warning = Some(
                                "Live metadata loaded, but the local snapshot could not be saved."
                                    .into(),
                            )
                        }
                    }
                }
                Ok(RefreshOutcome {
                    snapshot,
                    password,
                    fetched_at,
                    warning,
                })
            },
            cx,
            move |this, outcome| {
                this.passwords.insert(id.clone(), outcome.password);
                this.install_snapshot(&id, &outcome.snapshot, outcome.fetched_at, false);
                this.metadata_notice = outcome.warning;
            },
        );
    }

    fn cancel_catalogs(&mut self, matches: impl Fn(&TreeKey) -> bool) {
        let keys: Vec<_> = self
            .tree_operations
            .keys()
            .filter(|key| matches(key))
            .cloned()
            .collect();
        for key in keys {
            if let Some((_, task)) = self.tree_operations.remove(&key) {
                task.abort();
            }
            self.tree.loading.remove(&key);
            if let TreeKey::Source(id) = &key
                && self.cached_at.contains_key(id)
            {
                self.cached_offline.insert(id.clone());
            }
        }
    }

    fn remove_tree_source(&mut self, source: &str) {
        self.cancel_catalogs(|key| key.source() == source);
        self.tree.remove_source(source);
    }

    /// Catalog jobs have independent generations from table paging and form/save jobs.
    /// Keep at most two requests alive; newer user intent supersedes the oldest request.
    fn run_catalog<T: Send + 'static>(
        &mut self,
        key: TreeKey,
        future: impl Future<Output = Result<T>> + Send + 'static,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, T) + 'static,
    ) {
        self.cancel_catalogs(|item| item == &key);
        if self.tree_operations.len() >= 2
            && let Some(oldest) = self
                .tree_operations
                .iter()
                .min_by_key(|(_, (generation, _))| generation)
                .map(|(key, _)| key.clone())
        {
            self.cancel_catalogs(|item| item == &oldest);
        }
        self.tree_generation += 1;
        let generation = self.tree_generation;
        self.tree.loading.insert(key.clone());
        self.tree.errors.remove(&key);
        let job = runtime().spawn(future);
        self.tree_operations
            .insert(key.clone(), (generation, job.abort_handle()));
        cx.spawn(async move |this, cx| {
            let result = job
                .await
                .map_err(|_| anyhow!("Catalog worker stopped"))
                .and_then(|result| result);
            let _ = this.update(cx, |this, cx| {
                if !this
                    .tree_operations
                    .get(&key)
                    .is_some_and(|(active, _)| *active == generation)
                {
                    return;
                }
                this.tree_operations.remove(&key);
                this.tree.loading.remove(&key);
                if !this.profiles.iter().any(|p| p.id == key.source()) {
                    return;
                }
                match result {
                    Ok(value) => done(this, value),
                    Err(error) => {
                        if let TreeKey::Source(id) = &key
                            && this.cached_at.contains_key(id)
                        {
                            this.cached_offline.insert(id.clone());
                        }
                        this.tree.errors.insert(key, error.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn toggle_source(&mut self, id: String, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let Some(_) = self.profiles.iter().find(|p| p.id == id) else {
            return;
        };
        if self.tree.expanded_sources.remove(&id) {
            self.cancel_catalogs(|key| key.source() == id);
            cx.notify();
            return;
        }
        self.tree.expanded_sources.insert(id.clone());
        // Expansion is independent of the displayed table's identity.
        if self.selected_source.is_none() {
            self.selected_source = Some(id.clone());
        }
        if self.tree.databases.contains_key(&id) {
            cx.notify();
            return;
        }
        self.refresh_schema(id, cx);
    }

    pub fn toggle_database(&mut self, source: String, database: String, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let Some(profile) = self.profiles.iter().find(|p| p.id == source).cloned() else {
            return;
        };
        if !self
            .tree
            .databases
            .get(&source)
            .is_some_and(|dbs| dbs.contains(&database))
        {
            return;
        }
        let pair = (source.clone(), database.clone());
        let key = TreeKey::Database {
            source: source.clone(),
            database: database.clone(),
        };
        if self.tree.expanded_databases.remove(&pair) {
            self.cancel_catalogs(|item| item == &key);
            cx.notify();
            return;
        }
        self.tree.expanded_databases.insert(pair.clone());
        if self.tree.tables.contains_key(&pair) {
            cx.notify();
            return;
        }
        let session = self.passwords.get(&source).cloned();
        self.run_catalog(
            key,
            async move {
                let password = Self::resolve_password(&profile, session).await?;
                let tables = dalan_drivers::tables(&profile, &password, &database).await?;
                Ok((tables, password))
            },
            cx,
            move |this, (tables, password)| {
                this.passwords.insert(source, password);
                this.tree
                    .tables
                    .insert(pair.clone(), tables.into_iter().take(1000).collect());
                this.tree.expanded_groups.insert((pair.0, pair.1, false));
            },
        );
    }

    pub fn toggle_group(
        &mut self,
        source: String,
        database: String,
        views: bool,
        cx: &mut Context<Self>,
    ) {
        let pair = (source.clone(), database.clone());
        if !self.profiles.iter().any(|p| p.id == source) || !self.tree.tables.contains_key(&pair) {
            return;
        }
        let group = (source, database, views);
        if !self.tree.expanded_groups.remove(&group) {
            self.tree.expanded_groups.insert(group);
        }
        cx.notify();
    }

    pub fn collapse_tree(&mut self, cx: &mut Context<Self>) {
        self.cancel_catalogs(|_| true);
        self.tree.collapse_all();
        cx.notify();
    }

    pub fn expand_loaded_tree(&mut self, cx: &mut Context<Self>) {
        self.tree.retain_profiles(&self.profiles);
        self.tree.expand_loaded();
        cx.notify();
    }

    pub fn open_tree_table(
        &mut self,
        source: String,
        database: String,
        table: String,
        cx: &mut Context<Self>,
    ) {
        if self.saving || !self.profiles.iter().any(|p| p.id == source) {
            return;
        }
        let Some(tables) = self
            .tree
            .tables
            .get(&(source.clone(), database.clone()))
            .filter(|tables| {
                tables
                    .iter()
                    .any(|t| t.name == table && t.kind == "BASE TABLE")
            })
            .cloned()
        else {
            return;
        };
        self.selected_source = Some(source.clone());
        self.explorer_source = Some(source.clone());
        self.databases = self
            .tree
            .databases
            .get(&source)
            .cloned()
            .unwrap_or_default();
        self.selected_database = Some(database);
        self.tables = tables;
        self.select_table(table, cx);
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
            metadata_notice: None,
            cached_at: HashMap::new(),
            cached_offline: HashSet::new(),
            schema_cache: None,
            cache_admission: Arc::new(Mutex::new(())),
            automatic_discovery: false,
            profiles: vec![],
            tree: ExplorerTree::default(),
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
            explorer_source: None,
            pending_delete_source: None,
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
            tree_generation: 0,
            tree_operations: HashMap::new(),
            repository,
            storage_ready: false,
            previous_offsets: vec![],
        }
    }

    #[cfg(not(all(test, feature = "ui-tests")))]
    pub fn new(cx: &mut Context<Self>) -> Self {
        let repository = SourceRepository::default_path().map(SourceRepository::new);
        let mut model = Self::empty(repository.as_ref().ok().cloned());
        model.automatic_discovery = true;
        match SchemaCache::default_path() {
            Ok(path) => model.schema_cache = Some(SchemaCache::new(path)),
            Err(_) => model.metadata_notice = Some("Local metadata cache is unavailable.".into()),
        }
        if let Err(error) = repository {
            model.error = Some(error.to_string());
            return model;
        }
        let repo = model.repository.clone().unwrap();
        let cache = model.schema_cache.clone();
        model.busy = true;
        model.run(
            async move {
                tokio::task::spawn_blocking(move || load_profiles_and_cache(&repo, cache.as_ref()))
                    .await?
            },
            cx,
            |this, result, cx| {
                this.busy = false;
                match result {
                    Ok((profiles, snapshots, warning)) => {
                        this.profiles = profiles;
                        for (id, cached) in snapshots {
                            this.install_snapshot(&id, &cached.snapshot, cached.fetched_at, true);
                        }
                        if warning.is_some() {
                            this.metadata_notice = warning;
                        }
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
        if self.form_open {
            cx.notify();
            return;
        }
        self.invalidate();
        self.form_generation += 1;
        self.form_open = true;
        self.form_profile = Some(SourceProfile::default());
        self.form_feedback = None;
        cx.notify();
    }

    pub fn edit_explorer_source(&mut self, cx: &mut Context<Self>) {
        let target = self
            .explorer_source
            .as_ref()
            .or(self.selected_source.as_ref());
        let Some(profile) = self
            .profiles
            .iter()
            .find(|profile| Some(&profile.id) == target)
            .cloned()
        else {
            return;
        };
        self.edit_profile(profile, cx);
    }

    fn edit_profile(&mut self, profile: SourceProfile, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        if !self.storage_ready {
            self.form_feedback = Some(
                "Source settings did not load successfully; resolve that error before saving."
                    .into(),
            );
            cx.notify();
            return;
        }
        if self.form_open {
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
        let identity_changed = self
            .profiles
            .iter()
            .find(|old| old.id == profile.id)
            .is_none_or(|old| connection_identity(old).ok() != connection_identity(&profile).ok());
        self.cancel_catalogs(|key| key.source() == profile.id);
        let cache = self.schema_cache.clone();
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
            let warning = cache.as_ref().and_then(|cache| cache.register(&profile).err())
                .map(|_| "Profile saved, but local metadata registration failed.".to_string());
            Ok((profiles, warning))
        }).await? }, cx, move |this, result, cx| {
            this.saving = false; this.form_busy = false;
            match result {
                Ok((profiles, warning)) => {
                    if identity_changed {
                        this.remove_tree_source(&saved_profile.id);
                        this.cached_at.remove(&saved_profile.id);
                        this.cached_offline.remove(&saved_profile.id);
                    }
                    this.profiles = profiles;
                    this.passwords.insert(saved_profile.id.clone(), saved_password);
                    this.metadata_notice = warning;
                    if this.selected_source.as_ref() == Some(&saved_profile.id) || this.selected_source.is_none() {
                        this.selected_source = Some(saved_profile.id.clone());
                        if identity_changed { this.clear_data(); }
                    }
                    this.form_open = false; this.form_profile = None;
                    this.form_generation += 1; this.form_feedback = None; this.error = None;
                    if this.automatic_discovery { this.refresh_schema(saved_profile.id, cx); }
                },
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
    pub fn refresh_explorer(&mut self, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let Some(id) = self.explorer_source.clone() else {
            return;
        };
        self.refresh_schema(id, cx);
    }

    pub fn request_delete_explorer(&mut self, cx: &mut Context<Self>) {
        self.request_delete_for(
            self.explorer_source
                .clone()
                .or_else(|| self.selected_source.clone()),
            cx,
        );
    }

    fn request_delete_for(&mut self, target: Option<String>, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        if self.delete_confirm {
            self.delete_confirm = false;
            self.pending_delete_source = None;
        } else if let Some(id) =
            target.filter(|id| self.profiles.iter().any(|profile| &profile.id == id))
        {
            self.delete_confirm = true;
            self.pending_delete_source = Some(id);
        }
        cx.notify();
    }

    pub fn confirm_delete_explorer(&mut self, cx: &mut Context<Self>) {
        self.confirm_delete(cx);
    }

    pub fn removal_source_name(&self) -> Option<&str> {
        self.pending_delete_source.as_ref().and_then(|id| {
            self.profiles
                .iter()
                .find(|profile| &profile.id == id)
                .map(|profile| profile.name.as_str())
        })
    }

    pub fn confirm_delete(&mut self, cx: &mut Context<Self>) {
        if self.saving || !self.delete_confirm {
            return;
        }
        let (Some(id), Some(repo)) = (
            self.pending_delete_source
                .clone()
                .or_else(|| self.selected_source.clone()),
            self.repository.clone(),
        ) else {
            return;
        };
        self.cancel_catalogs(|key| key.source() == id);
        let cache = self.schema_cache.clone();
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
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .is_some_and(|profile| profile.save_password);
        self.run(async move { tokio::task::spawn_blocking(move || {
            let store = NativeSecretStore; let old = if credential_saved { store.get(&id)? } else { None };
            if credential_saved { store.delete(&id)?; }
            if let Err(error) = repo.save(&profiles) { if let Some(password) = old { store.set(&id, &password).map_err(|_| anyhow!("Settings removal failed and Keychain restore failed; review saved source."))?; } return Err(error); }
            let warning = cache.as_ref().and_then(|cache| cache.remove(&id).err())
                .map(|_| "Profile removed, but local metadata cleanup failed.".to_string());
            Ok((profiles, warning))
        }).await? }, cx, move |this, result, cx| { this.saving = false; this.busy = false;
            match result { Ok((profiles, warning)) => { this.metadata_notice = warning; this.cached_at.remove(&removed_id); this.cached_offline.remove(&removed_id); this.remove_tree_source(&removed_id); this.profiles = profiles; this.passwords.remove(&removed_id); this.pending_delete_source = None;
                    if this.selected_source.as_ref() == Some(&removed_id) { this.selected_source = None; this.clear_data(); }
                    if this.explorer_source.as_ref() == Some(&removed_id) { this.explorer_source = None; }
                    this.error = None; }, Err(error) => this.error = Some(format!("Not removed: {error}")) }; cx.notify(); });
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

    fn snapshot(table: &str) -> CatalogSnapshot {
        CatalogSnapshot {
            databases: vec![
                DatabaseCatalog {
                    name: "inventory".into(),
                    tables: vec![TableInfo {
                        name: table.into(),
                        kind: "BASE TABLE".into(),
                    }],
                },
                DatabaseCatalog {
                    name: "empty".into(),
                    tables: vec![],
                },
            ],
        }
    }

    #[test]
    fn cache_admission_survives_canceled_running_worker() {
        let sandbox = ExportSandbox::new();
        let cache = SchemaCache::new(sandbox.0.join("metadata.sqlite3"));
        let profile = SourceProfile::default();
        cache.register(&profile).unwrap();
        runtime().block_on(async {
            let gate = Arc::new(Mutex::new(()));
            let (running_tx, running_rx) = tokio::sync::oneshot::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            let (old_tx, old_rx) = tokio::sync::oneshot::channel();
            let old_cache = cache.clone();
            let old_profile = profile.clone();
            let old_job = tokio::spawn(admit_cache_work(gate.clone(), move || {
                running_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                let ticket = old_cache.begin_refresh(&old_profile)?;
                old_tx.send(ticket.clone()).unwrap();
                Ok(ticket)
            }));
            running_rx.await.unwrap();
            old_job.abort();
            assert!(old_job.await.unwrap_err().is_cancelled());

            let (pending_tx, pending_rx) = tokio::sync::oneshot::channel();
            let (entered_tx, mut entered_rx) = tokio::sync::oneshot::channel();
            let new_cache = cache.clone();
            let new_profile = profile.clone();
            let new_job = tokio::spawn(async move {
                let mut admission = Box::pin(admit_cache_work(gate, move || {
                    entered_tx.send(()).unwrap();
                    new_cache.begin_refresh(&new_profile)
                }));
                std::future::poll_fn(|cx| {
                    assert!(admission.as_mut().poll(cx).is_pending());
                    std::task::Poll::Ready(())
                })
                .await;
                pending_tx.send(()).unwrap();
                admission.await
            });
            pending_rx.await.unwrap();
            let blocked =
                tokio::time::timeout(std::time::Duration::from_millis(20), &mut entered_rx)
                    .await
                    .is_err();
            // Always release the blocking worker before asserting, even on failure.
            release_tx.send(()).unwrap();
            let old = old_rx.await.unwrap();
            let new = new_job.await.unwrap().unwrap();
            assert!(
                blocked,
                "new worker entered before the old allocation completed"
            );
            assert!(new.generation > old.generation);
            assert!(cache.replace(&new, &snapshot("new"), 200).unwrap());
            assert!(!cache.replace(&old, &snapshot("old"), 100).unwrap());
            let loaded = cache.load(&profile).unwrap().unwrap();
            assert_eq!(loaded.fetched_at, 200);
            assert_eq!(
                serde_json::to_value(loaded.snapshot).unwrap(),
                serde_json::to_value(snapshot("new")).unwrap()
            );
        });
    }

    #[test]
    fn canceled_pending_cache_admission_does_not_advance_clock() {
        let sandbox = ExportSandbox::new();
        let cache = SchemaCache::new(sandbox.0.join("metadata.sqlite3"));
        let profile = SourceProfile::default();
        cache.register(&profile).unwrap();
        let before = cache.begin_refresh(&profile).unwrap();
        runtime().block_on(async {
            let gate = Arc::new(Mutex::new(()));
            let guard = gate.clone().lock_owned().await;
            let pending_gate = gate.clone();
            let pending_cache = cache.clone();
            let pending_profile = profile.clone();
            let (pending_tx, pending_rx) = tokio::sync::oneshot::channel();
            let job = tokio::spawn(async move {
                let mut admission = Box::pin(admit_cache_work(pending_gate, move || {
                    pending_cache.begin_refresh(&pending_profile)
                }));
                std::future::poll_fn(|cx| {
                    assert!(admission.as_mut().poll(cx).is_pending());
                    std::task::Poll::Ready(())
                })
                .await;
                pending_tx.send(()).unwrap();
                admission.await
            });
            pending_rx.await.unwrap();
            job.abort();
            assert!(job.await.unwrap_err().is_cancelled());
            drop(guard);
            let new_cache = cache.clone();
            let new_profile = profile.clone();
            let after = admit_cache_work(gate, move || new_cache.begin_refresh(&new_profile))
                .await
                .unwrap();
            assert_eq!(after.generation, before.generation + 1);
        });
    }

    #[gpui::test]
    fn startup_restores_complete_offline_catalog_without_passwords(cx: &mut TestAppContext) {
        let sandbox = ExportSandbox::new();
        let profile = SourceProfile::default();
        let repo = SourceRepository::new(sandbox.0.join("sources.json"));
        repo.save(std::slice::from_ref(&profile)).unwrap();
        let cache = SchemaCache::new(sandbox.0.join("metadata.sqlite3"));
        cache.register(&profile).unwrap();
        let ticket = cache.begin_refresh(&profile).unwrap();
        cache.replace(&ticket, &snapshot("items"), 123).unwrap();
        let (profiles, restored, warning) = load_profiles_and_cache(&repo, Some(&cache)).unwrap();
        assert!(warning.is_none());
        let model = cx.new(|_| SourceModel::for_tests(profiles));
        model.update(cx, |model, cx| {
            for (id, cached) in restored {
                model.install_snapshot(&id, &cached.snapshot, cached.fetched_at, true);
            }
            assert!(model.cached_offline.contains(&profile.id));
            assert_eq!(model.cached_at[&profile.id], 123);
            assert!(model.tree.expanded_databases.is_empty());
            assert!(model.passwords.is_empty());
            model.toggle_database(profile.id.clone(), "empty".into(), cx);
            assert!(model.tree_operations.is_empty());
            assert!(model.tree.tables[&(profile.id.clone(), "empty".into())].is_empty());
        });
    }

    #[test]
    fn startup_cache_failure_keeps_committed_profiles() {
        let sandbox = ExportSandbox::new();
        let repo = SourceRepository::new(sandbox.0.join("sources.json"));
        let profile = SourceProfile::default();
        repo.save(std::slice::from_ref(&profile)).unwrap();
        let path = sandbox.0.join("metadata.sqlite3");
        std::fs::write(&path, "not a SQLite database").unwrap();
        let (profiles, cached, warning) =
            load_profiles_and_cache(&repo, Some(&SchemaCache::new(path))).unwrap();
        assert_eq!(profiles[0].id, profile.id);
        assert!(cached.is_empty());
        assert!(warning.is_some());
    }

    #[gpui::test]
    fn failed_refresh_retains_offline_metadata_and_displayed_page(cx: &mut TestAppContext) {
        let profile = SourceProfile::default();
        let id = profile.id.clone();
        let model = cx.new(|_| {
            let mut model = loaded_model();
            model.profiles = vec![profile];
            model.selected_source = Some(id.clone());
            model.install_snapshot(&id, &snapshot("items"), 20, false);
            model
        });
        let page = model.read_with(cx, |model, _| serde_json::to_value(&model.page).unwrap());
        model.update(cx, |model, cx| {
            model.run_catalog::<()>(
                TreeKey::Source(id.clone()),
                async { Err(anyhow!("Offline")) },
                cx,
                |_, _| panic!("unexpected success"),
            )
        });
        for _ in 0..100 {
            cx.run_until_parked();
            if model.read_with(cx, |model, _| model.tree_operations.is_empty()) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        model.read_with(cx, |model, _| {
            assert!(model.tree_operations.is_empty());
            assert!(model.cached_offline.contains(&id));
            assert_eq!(model.cached_at[&id], 20);
            assert_eq!(
                model.tree.tables[&(id.clone(), "inventory".into())][0].name,
                "items"
            );
            assert_eq!(serde_json::to_value(&model.page).unwrap(), page);
        });
    }

    #[gpui::test]
    fn snapshot_replacement_preserves_other_source_page_and_valid_expansion(
        cx: &mut TestAppContext,
    ) {
        let sandbox = ExportSandbox::new();
        let profile = SourceProfile::default();
        let id = profile.id.clone();
        let cache = SchemaCache::new(sandbox.0.join("metadata.sqlite3"));
        cache.register(&profile).unwrap();
        let ticket = cache.begin_refresh(&profile).unwrap();
        cache.replace(&ticket, &snapshot("old"), 1).unwrap();
        let ticket = cache.begin_refresh(&profile).unwrap();
        cache.replace(&ticket, &snapshot("new"), 2).unwrap();
        let restored = cache.load(&profile).unwrap().unwrap();
        let model = cx.new(|_| two_source_model());
        model.update(cx, |model, _| {
            let other = model.profiles[0].id.clone();
            let page = model.page.clone().unwrap();
            model.install_snapshot(&id, &snapshot("old"), 1, true);
            model
                .tree
                .expanded_databases
                .insert((id.clone(), "inventory".into()));
            model
                .tree
                .expanded_databases
                .insert((id.clone(), "removed".into()));
            model.install_snapshot(&id, &restored.snapshot, restored.fetched_at, false);
            assert!(
                model
                    .tree
                    .expanded_databases
                    .contains(&(id.clone(), "inventory".into()))
            );
            assert!(
                !model
                    .tree
                    .expanded_databases
                    .contains(&(id.clone(), "removed".into()))
            );
            assert_eq!(
                model.tree.tables[&(id.clone(), "inventory".into())][0].name,
                "new"
            );
            assert!(!model.cached_offline.contains(&id));
            assert_displayed_a(model, &other, &page);
            assert!(model.tree.databases.contains_key(&other));
        });
    }

    #[gpui::test]
    fn refresh_without_explicit_explorer_selection_is_inert(cx: &mut TestAppContext) {
        let model = cx.new(|_| two_source_model());
        model.update(cx, |model, cx| {
            model.explorer_source = None;
            let selected = model.selected_source.clone();
            model.refresh_explorer(cx);
            assert!(model.tree_operations.is_empty());
            assert_eq!(model.selected_source, selected);
        });
    }

    #[test]
    fn connection_edit_invalidates_disk_but_presentation_edit_preserves_it() {
        let sandbox = ExportSandbox::new();
        let cache = SchemaCache::new(sandbox.0.join("metadata.sqlite3"));
        let mut profile = SourceProfile::default();
        cache.register(&profile).unwrap();
        let ticket = cache.begin_refresh(&profile).unwrap();
        cache.replace(&ticket, &snapshot("items"), 1).unwrap();
        let identity = connection_identity(&profile).unwrap();
        profile.name = "Renamed".into();
        profile.color = Some("#112233".into());
        assert_eq!(connection_identity(&profile).unwrap(), identity);
        cache.register(&profile).unwrap();
        assert!(cache.load(&profile).unwrap().is_some());
        profile.host = "different.example".into();
        cache.register(&profile).unwrap();
        assert!(cache.load(&profile).unwrap().is_none());
        assert!(!cache.replace(&ticket, &snapshot("stale"), 2).unwrap());
    }

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
    fn closing_catalog_branch_cancels_only_its_metadata(cx: &mut TestAppContext) {
        let model = cx.new(|_| loaded_model());
        model.update(cx, |model, cx| {
            model.profiles = vec![
                SourceProfile {
                    id: "a".into(),
                    ..Default::default()
                },
                SourceProfile {
                    id: "b".into(),
                    ..Default::default()
                },
            ];
            for source in ["a", "b"] {
                model.tree.expanded_sources.insert(source.into());
                model.run_catalog(
                    TreeKey::Source(source.into()),
                    std::future::pending::<Result<()>>(),
                    cx,
                    |_, _| panic!("cancelled catalog completed"),
                );
            }
            assert_eq!(model.tree_operations.len(), 2);
            model.toggle_source("a".into(), cx);
            assert_eq!(model.tree_operations.len(), 1);
            assert!(!model.tree.loading.contains(&TreeKey::Source("a".into())));
            assert!(model.tree.loading.contains(&TreeKey::Source("b".into())));
            assert_eq!(model.page.as_ref().unwrap().offset, 40);
            model.collapse_tree(cx);
            assert!(model.tree_operations.is_empty());
            assert!(model.tree.loading.is_empty());
            assert_eq!(model.page.as_ref().unwrap().offset, 40);
        });
        cx.run_until_parked();
        model.read_with(cx, |model, _| assert!(model.tree.errors.is_empty()));
    }

    #[gpui::test]
    fn cached_tree_toggles_preserve_page_and_never_start_operations(cx: &mut TestAppContext) {
        let model = cx.new(|_| {
            let mut model = loaded_model();
            model.profiles = vec![SourceProfile {
                id: "test-source".into(),
                ..Default::default()
            }];
            model
                .tree
                .databases
                .insert("test-source".into(), vec!["inventory".into()]);
            model.tree.tables.insert(
                ("test-source".into(), "inventory".into()),
                vec![TableInfo {
                    name: "items".into(),
                    kind: "BASE TABLE".into(),
                }],
            );
            model
        });
        model.update(cx, |model, cx| {
            model.toggle_source("test-source".into(), cx);
            model.toggle_database("test-source".into(), "inventory".into(), cx);
            model.toggle_group("test-source".into(), "inventory".into(), false, cx);
            assert_eq!(model.tree.flatten(&model.profiles).len(), 5);
            model.toggle_database("test-source".into(), "inventory".into(), cx);
            assert_eq!(model.tree.flatten(&model.profiles).len(), 2);
            model.toggle_source("test-source".into(), cx);
            assert_eq!(model.tree.flatten(&model.profiles).len(), 1);
            model.expand_loaded_tree(cx);
            model.collapse_tree(cx);
            assert_eq!(model.tree.tables.len(), 1);
            assert_eq!(model.page.as_ref().unwrap().offset, 40);
            assert_eq!(model.selected_table.as_deref(), Some("items"));
            assert!(model.operation.is_none());
            assert!(model.tree_operations.is_empty());
            assert!(model.tree.loading.is_empty());
            assert!(!model.busy);
        });
    }

    fn two_source_model() -> SourceModel {
        let mut model = loaded_model();
        model.profiles = ["Source A", "Source B"]
            .into_iter()
            .map(|name| SourceProfile {
                name: name.into(),
                save_password: false,
                ..Default::default()
            })
            .collect();
        model.selected_source = Some(model.profiles[0].id.clone());
        model.explorer_source = Some(model.profiles[1].id.clone());
        for profile in &model.profiles {
            model.passwords.insert(
                profile.id.clone(),
                format!("{}-session-secret", profile.name),
            );
            model
                .tree
                .databases
                .insert(profile.id.clone(), vec!["inventory".into()]);
            model.tree.tables.insert(
                (profile.id.clone(), "inventory".into()),
                vec![TableInfo {
                    name: "items".into(),
                    kind: "BASE TABLE".into(),
                }],
            );
        }
        model
    }

    fn assert_displayed_a(model: &SourceModel, id: &str, page: &TablePage) {
        assert_eq!(model.selected_source.as_deref(), Some(id));
        assert_eq!(model.selected_database.as_deref(), Some("inventory"));
        assert_eq!(model.selected_table.as_deref(), Some("items"));
        assert_eq!(
            serde_json::to_value(model.page.as_ref().unwrap()).unwrap(),
            serde_json::to_value(page).unwrap(),
        );
    }

    #[gpui::test]
    fn editing_explorer_b_preserves_displayed_a_and_session_passwords(cx: &mut TestAppContext) {
        let model = cx.new(|_| two_source_model());
        model.update(cx, |model, cx| {
            let a = model.profiles[0].id.clone();
            let b = model.profiles[1].id.clone();
            let page = model.page.clone().unwrap();
            let generation = model.generation;
            model.edit_explorer_source(cx);
            assert_eq!(model.form_profile.as_ref().unwrap().id, b);
            assert_eq!(model.password(&b), "Source B-session-secret");
            assert_eq!(model.password(&a), "Source A-session-secret");
            assert!(!model.form_profile.as_ref().unwrap().save_password);
            assert!(model.form_open);
            assert!(!model.form_busy);
            assert!(model.operation.is_none());
            assert_displayed_a(model, &a, &page);
            model.close_form(cx);
            assert!(!model.form_open);
            assert!(model.form_profile.is_none());
            assert!(model.generation > generation);
            assert_displayed_a(model, &a, &page);
            // Refresh must also respect the persistence guard without touching either cache.
            model.saving = true;
            model.refresh_explorer(cx);
            model.saving = false;
            assert!(model.tree.databases.contains_key(&a));
            assert!(model.tree.databases.contains_key(&b));
            assert!(model.tree_operations.is_empty());
            assert_displayed_a(model, &a, &page);
        });
    }

    #[gpui::test]
    fn deleting_captured_explorer_b_preserves_displayed_a(cx: &mut TestAppContext) {
        let sandbox = ExportSandbox::new();
        let path = sandbox.0.join("sources.json");
        let repository = SourceRepository::new(path.clone());
        let model = cx.new(|_| {
            let mut model = two_source_model();
            repository.save(&model.profiles).unwrap();
            model.repository = Some(repository.clone());
            model
        });
        let (a, b, page) = model.read_with(cx, |model, _| {
            (
                model.profiles[0].id.clone(),
                model.profiles[1].id.clone(),
                model.page.clone().unwrap(),
            )
        });
        model.update(cx, |model, cx| {
            model.request_delete_explorer(cx);
            assert!(model.delete_confirm);
            assert_eq!(model.pending_delete_source.as_deref(), Some(b.as_str()));
            model.explorer_source = Some(a.clone());
            assert_eq!(model.pending_delete_source.as_deref(), Some(b.as_str()));
            model.confirm_delete_explorer(cx);
            assert!(model.saving);
            assert!(!model.delete_confirm);
        });
        for _ in 0..100 {
            cx.run_until_parked();
            if !model.read_with(cx, |model, _| model.saving) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        model.read_with(cx, |model, _| {
            assert!(!model.saving, "delete did not complete within one second");
            assert!(model.error.is_none(), "{:?}", model.error);
            assert!(model.pending_delete_source.is_none());
            assert_eq!(model.profiles.len(), 1);
            assert_eq!(model.profiles[0].id, a);
            assert_eq!(model.explorer_source.as_deref(), Some(a.as_str()));
            assert_displayed_a(model, &a, &page);
            assert_eq!(model.password(&a), "Source A-session-secret");
            assert!(!model.passwords.contains_key(&b));
            assert!(model.tree.databases.contains_key(&a));
            assert!(!model.tree.databases.contains_key(&b));
            assert!(
                model
                    .tree
                    .tables
                    .contains_key(&(a.clone(), "inventory".into()))
            );
            assert!(
                !model
                    .tree
                    .tables
                    .contains_key(&(b.clone(), "inventory".into()))
            );
        });
        let saved = repository.load().unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].id, a);
        let json = std::fs::read_to_string(path).unwrap();
        assert!(!json.contains(&b));
        assert!(!json.contains("Source B"));
        assert!(!json.contains("Source A-session-secret"));
        assert!(!json.contains("Source B-session-secret"));
    }

    #[gpui::test]
    fn views_and_group_collapse_do_not_replace_or_cancel_displayed_table(cx: &mut TestAppContext) {
        let model = cx.new(|_| two_source_model());
        model.update(cx, |model, cx| {
            let a = model.profiles[0].id.clone();
            let b = model.profiles[1].id.clone();
            let page = model.page.clone().unwrap();
            model
                .tree
                .tables
                .get_mut(&(b.clone(), "inventory".into()))
                .unwrap()
                .push(TableInfo {
                    name: "items_view".into(),
                    kind: "VIEW".into(),
                });
            let generation = model.generation;
            model.open_tree_table(b.clone(), "inventory".into(), "items_view".into(), cx);
            assert_displayed_a(model, &a, &page);
            assert_eq!(model.explorer_source.as_deref(), Some(b.as_str()));
            assert_eq!(model.generation, generation);
            assert!(model.operation.is_none());
            assert!(!model.busy);
            model.busy = true;
            model.run(std::future::pending::<Result<()>>(), cx, |_, _, _| {
                panic!("pending table operation completed");
            });
            let operation_id = model.operation.as_ref().unwrap().id();
            model
                .tree
                .expanded_groups
                .insert((b.clone(), "inventory".into(), false));
            model.toggle_group(b.clone(), "inventory".into(), false, cx);
            assert!(
                !model
                    .tree
                    .expanded_groups
                    .contains(&(b, "inventory".into(), false))
            );
            assert!(model.busy);
            assert_eq!(model.generation, generation);
            assert_eq!(model.operation.as_ref().unwrap().id(), operation_id);
            assert!(!model.operation.as_ref().unwrap().is_finished());
            assert_displayed_a(model, &a, &page);
            model.invalidate();
        });
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
            model.edit_explorer_source(cx);
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
            model.request_delete_explorer(cx);
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
    #[gpui::test]
    fn save_automatically_discovers_and_failed_fetch_keeps_last_snapshot(cx: &mut TestAppContext) {
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
                .join(format!("dalan-auto-cache-{}", uuid::Uuid::new_v4())),
        );
        let repository = SourceRepository::new(directory.0.join("sources.json"));
        let cache = SchemaCache::new(directory.0.join("metadata.sqlite3"));
        let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        socket.set_nonblocking(true).unwrap();
        let port = socket.local_addr().unwrap().port();
        let profile = SourceProfile {
            host: "127.0.0.1".into(),
            port,
            tls: dalan_drivers::TlsMode::Disabled,
            ..Default::default()
        };
        cache.register(&profile).unwrap();
        let snapshot = CatalogSnapshot {
            databases: vec![DatabaseCatalog {
                name: "saved_database".into(),
                tables: vec![TableInfo {
                    name: "saved_table".into(),
                    kind: "BASE TABLE".into(),
                }],
            }],
        };
        let ticket = cache.begin_refresh(&profile).unwrap();
        assert!(cache.replace(&ticket, &snapshot, 123).unwrap());
        repository.save(std::slice::from_ref(&profile)).unwrap();
        let id = profile.id.clone();
        let model = cx.new(|_| {
            let mut model = SourceModel::for_tests(vec![profile.clone()]);
            model.repository = Some(repository.clone());
            model.schema_cache = Some(cache.clone());
            model.automatic_discovery = true;
            model.install_snapshot(&id, &snapshot, 123, true);
            model
        });
        model.update(cx, |model, cx| {
            model.save(profile.clone(), "session-only-fixture".into(), cx)
        });
        let mut accepted = false;
        for _ in 0..200 {
            cx.run_until_parked();
            if let Ok((peer, _)) = socket.accept() {
                accepted = true;
                drop(peer);
            }
            if model.read_with(cx, |model, _| {
                !model.saving && !model.refreshing_source(&id)
            }) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(
            accepted,
            "Save must initiate discovery without clicking Refresh"
        );
        model.read_with(cx, |model, _| {
            assert!(!model.saving && !model.refreshing_source(&id));
            assert!(!model.form_open);
            assert!(
                model.explorer_source.is_none(),
                "Saving must not implicitly enable Refresh"
            );
            assert!(model.tree.errors.contains_key(&TreeKey::Source(id.clone())));
            assert_eq!(model.tree.databases[&id], vec!["saved_database"]);
            assert!(model.cached_offline.contains(&id));
            assert_eq!(model.password(&id), "session-only-fixture");
        });
        assert_eq!(repository.load().unwrap().len(), 1);
        assert_eq!(
            cache.load(&profile).unwrap().unwrap().snapshot.databases[0].tables[0].name,
            "saved_table"
        );
        let json = std::fs::read_to_string(directory.0.join("sources.json")).unwrap();
        assert!(!json.contains("session-only-fixture"));
    }
}

//! Password-free, bounded SQLite metadata persistence. All methods are synchronous;
//! invoke them on a blocking worker, never on the UI thread. Profile settings and
//! credentials remain in their separate stores. Portable filesystem checks cannot
//! defend against a hostile concurrent replacement of a checked directory.
use anyhow::{Context, Result, bail, ensure};
use dalan_drivers::{CatalogSnapshot, DatabaseCatalog, SourceProfile, TableInfo};
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};
use serde::Serialize;
use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    time::Duration,
};
use uuid::Uuid;

const APPLICATION_ID: i64 = 0x44414c4e;
const MAX_FILE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_METADATA_BYTES: usize = 8 * 1024 * 1024;
// Tables and views share the driver's 50,000 cap; databases have a separate cap.
const TOTAL_TABLE_CAP: usize = 50_000;

#[derive(Clone, Debug)]
pub struct SchemaCache {
    path: PathBuf,
}
#[derive(Clone, Debug)]
pub struct RefreshTicket {
    pub source_id: String,
    pub identity: String,
    pub generation: u64,
}
#[derive(Clone, Debug)]
pub struct CachedSchema {
    pub snapshot: CatalogSnapshot,
    /// UTC seconds since the Unix epoch, captured by the refresh worker.
    pub fetched_at: u64,
}

/// Stable, explicit allowlist of connection settings, not a credential hash.
/// Presentation settings and the user's save-password choice are intentionally absent.
pub fn connection_identity(profile: &SourceProfile) -> Result<String> {
    let profile = profile.resolved()?;
    #[derive(Serialize)]
    struct Identity<'a> {
        engine: &'a dalan_drivers::DbEngine,
        host: &'a str,
        port: u16,
        username: &'a str,
        database: &'a Option<String>,
        transport: &'a dalan_drivers::Transport,
        tls: &'a dalan_drivers::TlsMode,
        ca_path: &'a Option<String>,
        endpoint: &'a dalan_drivers::ConnectionMode,
        authentication: &'a dalan_drivers::Authentication,
        ssl_client_cert: &'a Option<String>,
        ssl_client_key: &'a Option<String>,
        jdbc: &'a Option<dalan_drivers::JdbcOptions>,
        connect_timeout_seconds: u64,
        query_timeout_seconds: u64,
    }
    serde_json::to_string(&Identity {
        engine: &profile.engine,
        host: &profile.host,
        port: profile.port,
        username: &profile.username,
        database: &profile.database,
        transport: &profile.transport,
        tls: &profile.tls,
        ca_path: &profile.ca_path,
        endpoint: &profile.endpoint,
        authentication: &profile.authentication,
        ssl_client_cert: &profile.ssl_client_cert,
        ssl_client_key: &profile.ssl_client_key,
        jdbc: &profile.jdbc,
        connect_timeout_seconds: profile.options.connect_timeout_seconds,
        query_timeout_seconds: profile.options.query_timeout_seconds,
    })
    .context("Cannot encode metadata connection identity")
}

fn source_id(id: &str) -> Result<String> {
    Ok(Uuid::parse_str(id)
        .map_err(|_| anyhow::anyhow!("Invalid source UUID"))?
        .to_string())
}

impl SchemaCache {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
    pub fn default_path() -> Result<PathBuf> {
        let settings = crate::source_store::default_path()?;
        Ok(settings
            .parent()
            .context("Source settings require a parent directory")?
            .join("metadata.sqlite3"))
    }

    pub fn register(&self, profile: &SourceProfile) -> Result<()> {
        let identity = connection_identity(profile)?;
        let id = source_id(&profile.id)?;
        let mut connection = self.open()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let old: Option<String> = tx
            .query_row(
                "SELECT identity FROM source_state WHERE id=?1",
                [&id],
                |r| r.get(0),
            )
            .optional()?;
        if old.as_deref() != Some(&identity) {
            // Global generations prevent an old ticket matching a deleted and re-added source.
            let generation = next_generation(&tx)?;
            tx.execute("DELETE FROM cached_databases WHERE source_id=?1", [&id])?;
            tx.execute("INSERT INTO source_state(id,identity,generation) VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET identity=excluded.identity,generation=excluded.generation,fetched_at=NULL", params![id, identity, generation])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Only registered sources can be refreshed. Registration is an explicit lifecycle
    /// operation, so a late worker cannot resurrect a removed source.
    pub fn begin_refresh(&self, profile: &SourceProfile) -> Result<RefreshTicket> {
        let identity = connection_identity(profile)?;
        let source_id = source_id(&profile.id)?;
        let mut connection = self.open()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let generation = next_generation(&tx)?;
        ensure!(
            tx.execute(
                "UPDATE source_state SET generation=?3 WHERE id=?1 AND identity=?2",
                params![source_id, identity, generation]
            )? == 1,
            "Source is not registered with these connection settings"
        );
        tx.commit()?;
        Ok(RefreshTicket {
            source_id,
            identity,
            generation: generation as u64,
        })
    }

    pub fn replace(
        &self,
        ticket: &RefreshTicket,
        snapshot: &CatalogSnapshot,
        captured_at: u64,
    ) -> Result<bool> {
        let id = source_id(&ticket.source_id)?;
        let generation = i64::try_from(ticket.generation).context("Invalid metadata generation")?;
        let captured_at =
            i64::try_from(captured_at).context("Metadata timestamp is out of range")?;
        validate_snapshot(snapshot)?;
        let mut connection = self.open()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let matches: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM source_state WHERE id=?1 AND identity=?2 AND generation=?3)", params![id, ticket.identity, generation], |r| r.get(0))?;
        if !matches {
            return Ok(false);
        }
        tx.execute("DELETE FROM cached_databases WHERE source_id=?1", [&id])?;
        {
            let mut databases = tx.prepare(
                "INSERT INTO cached_databases(source_id,name,position) VALUES(?1,?2,?3)",
            )?;
            let mut tables = tx.prepare("INSERT INTO cached_tables(source_id,database_name,name,kind,position) VALUES(?1,?2,?3,?4,?5)")?;
            for (position, database) in snapshot.databases.iter().enumerate() {
                databases.execute(params![id, database.name, position as i64])?;
                for (position, table) in database.tables.iter().enumerate() {
                    tables.execute(params![
                        id,
                        database.name,
                        table.name,
                        table.kind,
                        position as i64
                    ])?;
                }
            }
        }
        tx.execute(
            "UPDATE source_state SET fetched_at=?2 WHERE id=?1",
            params![id, captured_at],
        )?;
        tx.commit()?;
        Ok(true)
    }

    pub fn load(&self, profile: &SourceProfile) -> Result<Option<CachedSchema>> {
        let identity = connection_identity(profile)?;
        let id = source_id(&profile.id)?;
        let mut connection = self.open()?;
        let tx = connection.transaction()?;
        let fetched_at: Option<i64> = tx
            .query_row(
                "SELECT fetched_at FROM source_state WHERE id=?1 AND identity=?2",
                params![id, identity],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        let Some(fetched_at) = fetched_at else {
            return Ok(None);
        };
        ensure!(fetched_at >= 0, "Invalid cached metadata timestamp");
        // Check counts and field sizes before decoding strings out of a corrupted file.
        let (db_count, table_count): (i64, i64) = tx.query_row("SELECT (SELECT COUNT(*) FROM cached_databases WHERE source_id=?1),(SELECT COUNT(*) FROM cached_tables WHERE source_id=?1)", [&id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        ensure!(
            db_count <= 1000 && table_count <= TOTAL_TABLE_CAP as i64,
            "Cached metadata exceeds database or table limits"
        );
        let malformed: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM cached_databases WHERE source_id=?1 AND length(CAST(name AS BLOB))>256) OR EXISTS(SELECT 1 FROM cached_tables WHERE source_id=?1 AND (length(CAST(database_name AS BLOB))>256 OR length(CAST(name AS BLOB))>256 OR length(CAST(kind AS BLOB))>64))", [&id], |r| r.get(0))?;
        ensure!(!malformed, "Cached metadata contains oversized fields");
        let bytes: i64 = tx.query_row(
            "SELECT (SELECT COALESCE(SUM(length(CAST(name AS BLOB))),0) FROM cached_databases WHERE source_id=?1) + (SELECT COALESCE(SUM(length(CAST(name AS BLOB))+length(CAST(kind AS BLOB))),0) FROM cached_tables WHERE source_id=?1)",
            [&id], |r| r.get(0),
        )?;
        ensure!(
            bytes <= MAX_METADATA_BYTES as i64,
            "Cached metadata exceeds byte limit"
        );
        let mut snapshot = CatalogSnapshot {
            databases: Vec::new(),
        };
        let mut db_statement = tx.prepare("SELECT name FROM cached_databases WHERE source_id=?1 ORDER BY position,name LIMIT 1001")?;
        let mut table_statement = tx.prepare("SELECT name,kind FROM cached_tables WHERE source_id=?1 AND database_name=?2 ORDER BY position,name LIMIT 1001")?;
        for name in db_statement.query_map([&id], |r| r.get::<_, String>(0))? {
            let name = name?;
            let tables = table_statement
                .query_map(params![id, name], |r| {
                    Ok(TableInfo {
                        name: r.get(0)?,
                        kind: r.get(1)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            snapshot.databases.push(DatabaseCatalog { name, tables });
        }
        validate_snapshot(&snapshot)?;
        ensure!(
            snapshot
                .databases
                .iter()
                .map(|d| d.tables.len())
                .sum::<usize>()
                == table_count as usize,
            "Cached metadata contains orphaned tables"
        );
        Ok(Some(CachedSchema {
            snapshot,
            fetched_at: fetched_at as u64,
        }))
    }

    pub fn remove(&self, id: &str) -> Result<()> {
        let id = source_id(id)?;
        let mut connection = self.open()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("DELETE FROM source_state WHERE id=?1", [&id])?;
        tx.commit()?;
        Ok(())
    }

    pub fn prune(&self, profiles: &[SourceProfile]) -> Result<()> {
        let mut keep = HashSet::new();
        for profile in profiles {
            profile.validate()?;
            ensure!(
                keep.insert(source_id(&profile.id)?),
                "Duplicate source UUID"
            );
        }
        let mut connection = self.open()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let ids = {
            let mut statement = tx.prepare("SELECT id FROM source_state")?;
            statement
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        for id in ids {
            if !keep.contains(&id) {
                tx.execute("DELETE FROM source_state WHERE id=?1", [&id])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    fn open(&self) -> Result<Connection> {
        prepare_path(&self.path)?;
        let mut connection = Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .context("Cannot open Dalan metadata cache")?;
        connection.busy_timeout(Duration::from_secs(2))?;
        connection.execute_batch("PRAGMA foreign_keys=ON; PRAGMA trusted_schema=OFF;")?;
        // Validate ownership/version before making any persistent changes.
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version: i64 = tx.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        let app: i64 = tx.query_row("PRAGMA application_id", [], |r| r.get(0))?;
        ensure!(
            version == 0 || version == 1,
            "Unsupported Dalan metadata cache version; cache was not reset"
        );
        ensure!(
            app == APPLICATION_ID || (app == 0 && version == 0),
            "Not a Dalan metadata cache; file was not reset"
        );
        if version == 0 {
            let count: i64 = tx.query_row(
                "SELECT COUNT(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
                [],
                |r| r.get(0),
            )?;
            ensure!(
                count == 0,
                "Unsupported metadata cache schema; file was not reset"
            );
            tx.execute_batch("CREATE TABLE source_state(id TEXT PRIMARY KEY NOT NULL,identity TEXT NOT NULL,generation INTEGER NOT NULL DEFAULT 0 CHECK(generation>=0),fetched_at INTEGER CHECK(fetched_at>=0));
                CREATE TABLE cache_clock(id INTEGER PRIMARY KEY CHECK(id=1),generation INTEGER NOT NULL CHECK(generation>=0));
                INSERT INTO cache_clock VALUES(1,0);
                CREATE TABLE cached_databases(source_id TEXT NOT NULL REFERENCES source_state(id) ON DELETE CASCADE,name TEXT NOT NULL,position INTEGER NOT NULL,PRIMARY KEY(source_id,name));
                CREATE TABLE cached_tables(source_id TEXT NOT NULL,database_name TEXT NOT NULL,name TEXT NOT NULL,kind TEXT NOT NULL,position INTEGER NOT NULL,PRIMARY KEY(source_id,database_name,name),FOREIGN KEY(source_id,database_name) REFERENCES cached_databases(source_id,name) ON DELETE CASCADE);
                PRAGMA application_id=1145130062; PRAGMA user_version=1;")?;
        }
        tx.commit()?;
        connection.execute_batch("PRAGMA journal_mode=DELETE;")?;
        let page_size: i64 = connection.query_row("PRAGMA page_size", [], |r| r.get(0))?;
        ensure!(page_size > 0, "Invalid metadata cache page size");
        connection.pragma_update(None, "max_page_count", MAX_FILE_BYTES as i64 / page_size)?;
        Ok(connection)
    }
}

fn next_generation(tx: &rusqlite::Transaction<'_>) -> Result<i64> {
    let generation: i64 =
        tx.query_row("SELECT generation FROM cache_clock WHERE id=1", [], |r| {
            r.get(0)
        })?;
    let next = generation
        .checked_add(1)
        .filter(|n| *n > 0)
        .context("Metadata generation exhausted or invalid")?;
    tx.execute("UPDATE cache_clock SET generation=?1 WHERE id=1", [next])?;
    Ok(next)
}

fn validate_snapshot(snapshot: &CatalogSnapshot) -> Result<()> {
    ensure!(
        snapshot.databases.len() <= 1000,
        "Metadata exceeds database limit"
    );
    let mut databases = HashSet::new();
    let mut total = 0;
    for database in &snapshot.databases {
        validate_name(&database.name)?;
        ensure!(
            databases.insert(&database.name),
            "Duplicate metadata database"
        );
        ensure!(
            database.tables.len() <= 1000,
            "Metadata exceeds per-database table limit"
        );
        total += database.tables.len();
        ensure!(
            total <= TOTAL_TABLE_CAP,
            "Metadata exceeds total table limit"
        );
        let mut tables = HashSet::new();
        for table in &database.tables {
            validate_name(&table.name)?;
            ensure!(tables.insert(&table.name), "Duplicate metadata table");
            ensure!(
                !table.kind.trim().is_empty()
                    && table.kind.len() <= 64
                    && !table.kind.chars().any(char::is_control),
                "Invalid metadata table kind"
            );
        }
    }
    ensure!(
        serde_json::to_vec(snapshot)
            .context("Cannot encode metadata snapshot")?
            .len()
            <= MAX_METADATA_BYTES,
        "Metadata exceeds 8 MiB limit"
    );
    Ok(())
}
fn validate_name(name: &str) -> Result<()> {
    ensure!(
        !name.trim().is_empty()
            && name.len() <= 256
            && name.chars().count() <= 64
            && !name.chars().any(char::is_control),
        "Invalid metadata identifier"
    );
    Ok(())
}

fn prepare_path(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .context("Metadata cache requires a parent directory")?;
    check_directories(parent)?;
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(parent)
        .context("Cannot create metadata cache directory")?;
    check_directories(parent)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            ensure!(
                metadata.is_file() && !metadata.file_type().is_symlink(),
                "Metadata cache must be a regular file, not a symlink"
            );
            ensure!(
                metadata.len() <= MAX_FILE_BYTES,
                "Metadata cache exceeds 128 MiB limit"
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(path, fs::Permissions::from_mode(0o600))
                    .context("Cannot restrict metadata cache permissions")?;
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(path) {
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    return prepare_path(path);
                }
                Err(error) => return Err(error).context("Cannot create metadata cache file"),
            }
        }
        Err(_) => bail!("Cannot inspect metadata cache file"),
    }
    // SQLite may access sidecars before journal-mode conversion; never follow links.
    for suffix in ["-journal", "-wal", "-shm"] {
        let mut sidecar = path.as_os_str().to_os_string();
        sidecar.push(suffix);
        match fs::symlink_metadata(PathBuf::from(sidecar)) {
            Ok(metadata) => ensure!(
                metadata.is_file() && !metadata.file_type().is_symlink(),
                "Metadata cache sidecar must be a regular file"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => bail!("Cannot inspect metadata cache sidecar"),
        }
    }
    Ok(())
}
fn check_directories(path: &Path) -> Result<()> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) => ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "Metadata cache path must not contain symlinked or non-directory ancestors"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => bail!("Cannot inspect metadata cache directory"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Sandbox(PathBuf);
    impl Sandbox {
        fn new() -> Self {
            Self(
                std::env::temp_dir()
                    .canonicalize()
                    .unwrap()
                    .join(format!("dalan-cache-test-{}", Uuid::new_v4())),
            )
        }
        fn cache(&self) -> SchemaCache {
            SchemaCache::new(self.0.join("metadata.sqlite3"))
        }
    }
    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn snapshot() -> CatalogSnapshot {
        CatalogSnapshot {
            databases: vec![
                DatabaseCatalog {
                    name: "空 ` db".into(),
                    tables: vec![],
                },
                DatabaseCatalog {
                    name: "app".into(),
                    tables: vec![
                        TableInfo {
                            name: "users".into(),
                            kind: "BASE TABLE".into(),
                        },
                        TableInfo {
                            name: "view".into(),
                            kind: "VIEW".into(),
                        },
                    ],
                },
            ],
        }
    }
    fn seed(cache: &SchemaCache, profile: &SourceProfile) {
        cache.register(profile).unwrap();
        let ticket = cache.begin_refresh(profile).unwrap();
        assert!(cache.replace(&ticket, &snapshot(), 123).unwrap());
    }
    #[test]
    fn jdbc_artifact_url_and_runtime_change_metadata_identity() {
        let profile = SourceProfile {
            engine: dalan_drivers::DbEngine::Jdbc,
            tls: dalan_drivers::TlsMode::Disabled,
            jdbc: Some(dalan_drivers::JdbcOptions {
                java_path: "/fixture/java".into(),
                driver_id: "h2".into(),
                driver_class: "org.h2.Driver".into(),
                jars: vec![dalan_drivers::JdbcJar {
                    path: "/fixture/h2.jar".into(),
                    sha256: "a".repeat(64),
                }],
                url: "jdbc:h2:mem:fixture".into(),
            }),
            ..SourceProfile::default()
        };
        let original = connection_identity(&profile).unwrap();
        let mut other = profile.clone();
        other.jdbc.as_mut().unwrap().url = "jdbc:h2:mem:other".into();
        assert_ne!(original, connection_identity(&other).unwrap());
        let mut other = profile.clone();
        other.jdbc.as_mut().unwrap().jars[0].sha256 = "b".repeat(64);
        assert_ne!(original, connection_identity(&other).unwrap());
        let mut other = profile;
        other.jdbc.as_mut().unwrap().java_path = "/fixture/other-java".into();
        assert_ne!(original, connection_identity(&other).unwrap());
    }

    #[test]
    fn restart_roundtrip_and_private_files() {
        let sandbox = Sandbox::new();
        let profile = SourceProfile::default();
        seed(&sandbox.cache(), &profile);
        let loaded = sandbox.cache().load(&profile).unwrap().unwrap();
        assert_eq!(loaded.fetched_at, 123);
        assert_eq!(
            serde_json::to_value(loaded.snapshot).unwrap(),
            serde_json::to_value(snapshot()).unwrap()
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&sandbox.0).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(sandbox.cache().path)
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        let connection = sandbox.cache().open().unwrap();
        let names: Vec<String> = connection
            .prepare("SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(
            names,
            vec![
                "cache_clock",
                "cached_databases",
                "cached_tables",
                "source_state"
            ]
        );
        let identity = connection_identity(&profile).unwrap();
        for key in ["password", "save_password", "color", "name"] {
            assert!(
                !serde_json::from_str::<serde_json::Value>(&identity)
                    .unwrap()
                    .as_object()
                    .unwrap()
                    .contains_key(key)
            );
        }
    }
    #[test]
    fn identity_edits_invalidate_but_presentation_does_not() {
        let sandbox = Sandbox::new();
        let cache = sandbox.cache();
        let mut profile = SourceProfile::default();
        seed(&cache, &profile);
        let old = connection_identity(&profile).unwrap();
        profile.name = "Renamed".into();
        profile.color = Some("#112233".into());
        profile.save_password = true;
        assert_eq!(old, connection_identity(&profile).unwrap());
        cache.register(&profile).unwrap();
        assert_eq!(cache.load(&profile).unwrap().unwrap().fetched_at, 123);
        profile.host = "example.com".into();
        assert_ne!(old, connection_identity(&profile).unwrap());
        assert!(cache.load(&profile).unwrap().is_none());
        cache.register(&profile).unwrap();
        assert!(cache.load(&profile).unwrap().is_none());
    }
    #[test]
    fn effective_url_identity_and_display_selection_are_stable() {
        let mut profile = SourceProfile {
            endpoint: dalan_drivers::ConnectionMode::UrlOnly {
                url: "mysql://localhost:3306/url_database".into(),
            },
            ..SourceProfile::default()
        };
        let identity = connection_identity(&profile).unwrap();
        assert_eq!(
            identity,
            connection_identity(&profile.resolved().unwrap()).unwrap()
        );
        profile.database = Some("ignored_form_database".into());
        profile.schemas = dalan_drivers::SchemaSelection::Selected(vec!["url_database".into()]);
        profile.options.page_size = 200;
        assert_eq!(identity, connection_identity(&profile).unwrap());
        profile.authentication = dalan_drivers::Authentication::NoAuth;
        assert_ne!(identity, connection_identity(&profile).unwrap());
        let no_auth_identity = connection_identity(&profile).unwrap();
        profile.username = "unused_account".into();
        assert_eq!(no_auth_identity, connection_identity(&profile).unwrap());
    }
    #[test]
    fn invalid_snapshot_preserves_previous_commit() {
        let sandbox = Sandbox::new();
        let cache = sandbox.cache();
        let profile = SourceProfile::default();
        seed(&cache, &profile);
        let ticket = cache.begin_refresh(&profile).unwrap();
        let mut bad = snapshot();
        bad.databases[1].tables[0].name = "private\nsecret".into();
        let error = cache.replace(&ticket, &bad, 200).unwrap_err();
        assert!(!error.to_string().contains("private"));
        bad = snapshot();
        bad.databases.push(bad.databases[0].clone());
        assert!(cache.replace(&ticket, &bad, 200).is_err());
        bad = snapshot();
        bad.databases[1].tables = (0..1001)
            .map(|i| TableInfo {
                name: format!("t{i}"),
                kind: "BASE TABLE".into(),
            })
            .collect();
        assert!(cache.replace(&ticket, &bad, 200).is_err());
        assert!(cache.replace(&ticket, &snapshot(), u64::MAX).is_err());
        assert_eq!(cache.load(&profile).unwrap().unwrap().fetched_at, 123);
        // Force a database-side failure after deletion; rollback retains old data.
        cache.open().unwrap().execute_batch("CREATE TRIGGER fail_insert BEFORE INSERT ON cached_tables BEGIN SELECT RAISE(ABORT,'test failure'); END;").unwrap();
        assert!(cache.replace(&ticket, &snapshot(), 200).is_err());
        assert_eq!(cache.load(&profile).unwrap().unwrap().fetched_at, 123);
    }
    #[test]
    fn table_cap_excludes_databases_and_preserves_previous_commit() {
        let sandbox = Sandbox::new();
        let cache = sandbox.cache();
        let profile = SourceProfile::default();
        cache.register(&profile).unwrap();
        let ticket = cache.begin_refresh(&profile).unwrap();
        let mut boundary = CatalogSnapshot {
            databases: (0..50)
                .map(|database| DatabaseCatalog {
                    name: format!("db{database}"),
                    tables: (0..1000)
                        .map(|table| TableInfo {
                            name: format!("t{table}"),
                            kind: "BASE TABLE".into(),
                        })
                        .collect(),
                })
                .collect(),
        };
        assert!(cache.replace(&ticket, &boundary, 100).unwrap());
        assert_eq!(
            serde_json::to_value(cache.load(&profile).unwrap().unwrap().snapshot).unwrap(),
            serde_json::to_value(&boundary).unwrap()
        );
        boundary.databases.push(DatabaseCatalog {
            name: "overflow".into(),
            tables: vec![TableInfo {
                name: "one_more".into(),
                kind: "VIEW".into(),
            }],
        });
        assert!(cache.replace(&ticket, &boundary, 200).is_err());
        boundary.databases.pop();
        let loaded = cache.load(&profile).unwrap().unwrap();
        assert_eq!(loaded.fetched_at, 100);
        assert_eq!(
            serde_json::to_value(loaded.snapshot).unwrap(),
            serde_json::to_value(&boundary).unwrap()
        );
        let empty = CatalogSnapshot {
            databases: (0..1000)
                .map(|database| DatabaseCatalog {
                    name: format!("empty{database}"),
                    tables: vec![],
                })
                .collect(),
        };
        assert!(cache.replace(&ticket, &empty, 300).unwrap());
        assert_eq!(
            serde_json::to_value(cache.load(&profile).unwrap().unwrap().snapshot).unwrap(),
            serde_json::to_value(empty).unwrap()
        );
    }

    #[test]
    fn stale_tickets_cannot_replace_newer_removed_or_readded_sources() {
        let sandbox = Sandbox::new();
        let cache = sandbox.cache();
        let profile = SourceProfile::default();
        seed(&cache, &profile);
        let old = cache.begin_refresh(&profile).unwrap();
        let newer = cache.begin_refresh(&profile).unwrap();
        assert!(cache.replace(&newer, &snapshot(), 300).unwrap());
        assert!(!cache.replace(&old, &snapshot(), 200).unwrap());
        assert_eq!(cache.load(&profile).unwrap().unwrap().fetched_at, 300);
        cache.remove(&profile.id).unwrap();
        assert!(!cache.replace(&newer, &snapshot(), 400).unwrap());
        assert!(cache.begin_refresh(&profile).is_err());
        cache.register(&profile).unwrap();
        assert!(!cache.replace(&newer, &snapshot(), 400).unwrap());
        assert!(cache.load(&profile).unwrap().is_none());
        seed(&cache, &profile);
        let before_edit = cache.begin_refresh(&profile).unwrap();
        let mut edited = profile.clone();
        edited.username = "other".into();
        cache.register(&edited).unwrap();
        cache.register(&profile).unwrap();
        assert!(!cache.replace(&before_edit, &snapshot(), 400).unwrap());
    }
    #[test]
    fn pruning_is_atomic_and_keeps_only_active_sources() {
        let sandbox = Sandbox::new();
        let cache = sandbox.cache();
        let a = SourceProfile::default();
        let b = SourceProfile::default();
        seed(&cache, &a);
        seed(&cache, &b);
        let ticket = cache.begin_refresh(&b).unwrap();
        let mut bad = b.clone();
        bad.id = "not-a-uuid".into();
        assert!(cache.prune(&[bad]).is_err());
        assert!(cache.load(&a).unwrap().is_some());
        cache.prune(std::slice::from_ref(&a)).unwrap();
        assert!(cache.load(&a).unwrap().is_some());
        assert!(cache.load(&b).unwrap().is_none());
        assert!(!cache.replace(&ticket, &snapshot(), 200).unwrap());
    }
    #[test]
    fn unsupported_and_corrupt_files_are_not_reset() {
        let sandbox = Sandbox::new();
        let cache = sandbox.cache();
        let profile = SourceProfile::default();
        seed(&cache, &profile);
        cache
            .open()
            .unwrap()
            .execute_batch("PRAGMA user_version=2")
            .unwrap();
        let before = fs::read(&cache.path).unwrap();
        assert!(cache.load(&profile).is_err());
        assert_eq!(fs::read(&cache.path).unwrap(), before);
        fs::write(&cache.path, b"private-secret corrupt header").unwrap();
        let before = fs::read(&cache.path).unwrap();
        let error = cache.register(&profile).unwrap_err();
        assert!(!format!("{error:#}").contains("private-secret"));
        assert_eq!(fs::read(&cache.path).unwrap(), before);
        fs::remove_file(&cache.path).unwrap();
        Connection::open(&cache.path)
            .unwrap()
            .execute_batch("CREATE TABLE unrelated(value TEXT)")
            .unwrap();
        let before = fs::read(&cache.path).unwrap();
        assert!(cache.register(&profile).is_err());
        assert_eq!(fs::read(&cache.path).unwrap(), before);
    }
    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_file_directory_and_sidecar() {
        use std::os::unix::fs::symlink;
        let sandbox = Sandbox::new();
        let cache = sandbox.cache();
        let profile = SourceProfile::default();
        seed(&cache, &profile);
        let target = sandbox.0.join("target");
        fs::rename(&cache.path, &target).unwrap();
        symlink(&target, &cache.path).unwrap();
        assert!(cache.load(&profile).is_err());
        fs::remove_file(&cache.path).unwrap();
        fs::rename(&target, &cache.path).unwrap();
        let link = sandbox.0.join("link");
        symlink(&sandbox.0, &link).unwrap();
        assert!(
            SchemaCache::new(link.join("other.sqlite3"))
                .register(&profile)
                .is_err()
        );
        symlink(&cache.path, sandbox.0.join("metadata.sqlite3-journal")).unwrap();
        assert!(cache.load(&profile).is_err());
    }
}

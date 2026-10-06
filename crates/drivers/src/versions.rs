//! Only compiled backends are selectable. "latest" means latest bundled, never
//! the newest upstream release or an unverified server compatibility promise.
use crate::DbEngine;
use anyhow::{Result, ensure};
use std::{
    collections::BTreeMap,
    sync::{OnceLock, RwLock},
};
#[derive(Debug, Clone, Copy)]
pub struct BundledDriver {
    pub id: &'static str,
    pub library: &'static str,
    pub version: &'static str,
    pub tls: &'static str,
    pub protocol: &'static str,
}
pub const MYSQL: BundledDriver = BundledDriver {
    id: "mysql-async-0.37.1",
    library: "mysql_async",
    version: "0.37.1",
    tls: "rustls",
    protocol: "MySQL wire",
};
pub const POSTGRES: BundledDriver = BundledDriver {
    id: "tokio-postgres-0.7.18",
    library: "tokio-postgres",
    version: "0.7.18",
    tls: "postgres-native-tls 0.5.3",
    protocol: "PostgreSQL wire",
};
pub const MONGO: BundledDriver = BundledDriver {
    id: "mongodb-3.7.0",
    library: "mongodb",
    version: "3.7.0",
    tls: "rustls",
    protocol: "MongoDB wire / BSON",
};
pub const REDIS: BundledDriver = BundledDriver {
    id: "dalan-resp2-1",
    library: "Dalan bounded RESP2",
    version: "1",
    tls: "native-tls",
    protocol: "RESP2",
};
pub fn engine_id(engine: DbEngine) -> &'static str {
    match engine {
        DbEngine::MySql => "mysql",
        DbEngine::MariaDb => "mariadb",
        DbEngine::PostgreSql => "postgresql",
        DbEngine::MongoDb => "mongodb",
        DbEngine::Redis => "redis",
    }
}
pub fn bundled(engine: DbEngine) -> &'static [BundledDriver] {
    match engine {
        DbEngine::MySql | DbEngine::MariaDb => std::slice::from_ref(&MYSQL),
        DbEngine::PostgreSql => std::slice::from_ref(&POSTGRES),
        DbEngine::MongoDb => std::slice::from_ref(&MONGO),
        DbEngine::Redis => std::slice::from_ref(&REDIS),
    }
}
pub fn resolve(engine: DbEngine, choice: &str) -> Result<BundledDriver> {
    let available = bundled(engine);
    if choice == "latest" {
        return Ok(available[0]);
    }
    available
        .iter()
        .find(|b| b.id == choice)
        .copied()
        .ok_or_else(|| {
            anyhow::anyhow!("Requested native driver version is not bundled for this engine")
        })
}
pub fn validate_preferences(choices: &BTreeMap<String, String>) -> Result<()> {
    ensure!(choices.len() <= 5, "Invalid driver preferences");
    for (id, choice) in choices {
        let engine = match id.as_str() {
            "mysql" => DbEngine::MySql,
            "mariadb" => DbEngine::MariaDb,
            "postgresql" => DbEngine::PostgreSql,
            "mongodb" => DbEngine::MongoDb,
            "redis" => DbEngine::Redis,
            _ => return Err(anyhow::anyhow!("Unknown driver preference")),
        };
        resolve(engine, choice)?;
    }
    Ok(())
}
fn preferences() -> &'static RwLock<BTreeMap<String, String>> {
    static STATE: OnceLock<RwLock<BTreeMap<String, String>>> = OnceLock::new();
    STATE.get_or_init(|| RwLock::new(BTreeMap::new()))
}
pub fn install_preferences(choices: BTreeMap<String, String>) -> Result<()> {
    validate_preferences(&choices)?;
    *preferences()
        .write()
        .map_err(|_| anyhow::anyhow!("Driver preference lock unavailable"))? = choices;
    Ok(())
}
pub fn selected(engine: DbEngine) -> Result<BundledDriver> {
    let preferences = preferences()
        .read()
        .map_err(|_| anyhow::anyhow!("Driver preference lock unavailable"))?;
    resolve(
        engine,
        preferences
            .get(engine_id(engine))
            .map(String::as_str)
            .unwrap_or("latest"),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_choice_resolves_only_a_compiled_engine_backend() {
        for engine in [
            DbEngine::MySql,
            DbEngine::MariaDb,
            DbEngine::PostgreSql,
            DbEngine::MongoDb,
            DbEngine::Redis,
        ] {
            let b = bundled(engine)[0];
            assert_eq!(resolve(engine, "latest").unwrap().id, b.id);
            assert_eq!(resolve(engine, b.id).unwrap().id, b.id);
            assert!(resolve(engine, "upstream-latest").is_err());
        }
        assert!(resolve(DbEngine::Redis, MONGO.id).is_err());
    }
    #[test]
    fn malformed_preferences_never_replace_current_backend() {
        assert!(validate_preferences(&BTreeMap::from([("jdbc".into(), "latest".into())])).is_err());
        assert!(validate_preferences(&BTreeMap::from([("mysql".into(), "0.1".into())])).is_err());
    }
    #[test]
    fn exact_library_pins_match_version_catalog() {
        let manifest = include_str!("../Cargo.toml");
        for (library, version) in [
            ("mysql_async", "0.37.1"),
            ("mongodb", "3.7.0"),
            ("tokio-postgres", "0.7.18"),
            ("postgres-native-tls", "0.5.3"),
        ] {
            assert!(
                manifest
                    .lines()
                    .any(|l| l.starts_with(library) && l.contains(&format!("\"={version}\""))),
                "{library}"
            );
        }
    }
}

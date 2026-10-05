pub mod catalog;
pub use catalog::{
    CatalogError, ConnectionCapabilities, DriverRuntime, driver_by_id, driver_for_engine,
    validate_driver_catalog,
};
mod table_clauses;
pub use table_clauses::validate_table_clauses;
pub mod mysql;
pub mod query;
pub use query::{QueryRequest, QueryResult, execute_read_only, validate_read_only};
mod relay;
pub mod sources;
pub use mysql::{
    BrowseRequest, CatalogSnapshot, CellValue, ColumnInfo, ConnectionReport, DatabaseCatalog,
    FilterOperator, SortDirection, TableFilter, TableInfo, TablePage, TableSort, browse, columns,
    discover_catalog, tables, test_connection,
};
pub use sources::{
    Authentication, ConnectionMode, ConnectionTarget, DbEngine, SchemaSelection, SourceOptions,
    SourceProfile, TlsMode, Transport,
};

use dalan_core::Engine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverStatus {
    Planned,
    Experimental,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriverDescriptor {
    /// Stable registry identity, not a user-created source/profile ID.
    pub id: &'static str,
    pub engine: Engine,
    pub proposed_backend: &'static str,
    pub status: DriverStatus,
    /// Intended runtime; consult status before treating a backend as available.
    pub runtime: DriverRuntime,
    /// SQL dialect identity; does not claim query execution is implemented.
    pub dialect: Option<&'static str>,
    /// None means not implemented/unknown rather than unsupported.
    pub capabilities: Option<ConnectionCapabilities>,
}

pub const PLANNED_DRIVERS: [DriverDescriptor; 4] = [
    DriverDescriptor {
        id: "postgresql",
        engine: Engine::PostgreSql,
        proposed_backend: "sqlx::Postgres",
        status: DriverStatus::Planned,
        runtime: DriverRuntime::Native,
        dialect: Some("postgresql"),
        capabilities: None,
    },
    DriverDescriptor {
        id: "mysql",
        engine: Engine::MySql,
        proposed_backend: "mysql_async",
        status: DriverStatus::Experimental,
        runtime: DriverRuntime::Native,
        dialect: Some("mysql"),
        capabilities: Some(ConnectionCapabilities {
            optional_database: true,
            unix_socket: true,
        }),
    },
    DriverDescriptor {
        id: "mariadb",
        engine: Engine::MariaDb,
        proposed_backend: "mysql_async",
        status: DriverStatus::Experimental,
        runtime: DriverRuntime::Native,
        dialect: Some("mysql"),
        capabilities: Some(ConnectionCapabilities {
            optional_database: true,
            unix_socket: true,
        }),
    },
    DriverDescriptor {
        id: "redis",
        engine: Engine::Redis,
        proposed_backend: "redis",
        status: DriverStatus::Planned,
        runtime: DriverRuntime::Native,
        dialect: None,
        capabilities: None,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_unique_engines_and_experimental_mysql() {
        for (index, driver) in PLANNED_DRIVERS.iter().enumerate() {
            assert_eq!(
                driver.status,
                if matches!(driver.engine, Engine::MySql | Engine::MariaDb) {
                    DriverStatus::Experimental
                } else {
                    DriverStatus::Planned
                }
            );
            assert!(
                !PLANNED_DRIVERS[..index]
                    .iter()
                    .any(|other| other.engine == driver.engine)
            );
        }
    }

    #[test]
    fn mysql_and_mariadb_share_backend_not_identity() {
        let mysql = PLANNED_DRIVERS
            .iter()
            .find(|driver| driver.engine == Engine::MySql)
            .unwrap();
        let mariadb = PLANNED_DRIVERS
            .iter()
            .find(|driver| driver.engine == Engine::MariaDb)
            .unwrap();
        assert_eq!(mysql.proposed_backend, mariadb.proposed_backend);
        assert_ne!(mysql.engine, mariadb.engine);
    }
}

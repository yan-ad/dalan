//! Small, static driver catalog; metadata here does not change connection behavior.
//!
//! Adapted from DBX's descriptor/capability separation and manifest lookup/validation
//! patterns (Apache-2.0), commit 38ce7b5dd25db0ec058090ffbb4ab0b707a9bfad:
//! `crates/dbx-driver-agent/src/database_capabilities.rs` and
//! `crates/dbx-types/src/database_manifest.rs`.
//! Dalan keeps its five native engines, with connection capabilities based
//! on its own source/executor implementation. No DBX driver list, agent runtime,
//! pool policy, generated manifest, or plugin infrastructure is imported.

use crate::{DriverDescriptor, DriverStatus, PLANNED_DRIVERS};
use dalan_core::Engine;

/// Intended backend execution model, not a claim that a driver is implemented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverRuntime {
    Native,
}

/// Source configuration supported by an implemented Dalan driver.
///
/// These are descriptive only: callers must still validate a source and connect.
/// In particular, this does not describe pooling (executors open fresh connections),
/// TCP preflight probes, or an account's database permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionCapabilities {
    /// A source may connect without selecting an initial database.
    pub optional_database: bool,
    /// A direct source may use a Unix socket instead of a TCP endpoint.
    pub unix_socket: bool,
}

/// Case-sensitive, stable catalog ID lookup, separate from user-created source IDs.
pub fn driver_by_id(id: &str) -> Option<&'static DriverDescriptor> {
    PLANNED_DRIVERS.iter().find(|driver| driver.id == id)
}

pub fn driver_for_engine(engine: Engine) -> Option<&'static DriverDescriptor> {
    PLANNED_DRIVERS
        .iter()
        .find(|driver| driver.engine == engine)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogError {
    InvalidId(&'static str),
    DuplicateId(&'static str),
    DuplicateEngine(Engine),
    PlannedCapabilities(&'static str),
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidId(id) => write!(f, "invalid driver catalog ID: {id:?}"),
            Self::DuplicateId(id) => write!(f, "duplicate driver catalog ID: {id}"),
            Self::DuplicateEngine(engine) => write!(f, "duplicate driver engine: {engine:?}"),
            Self::PlannedCapabilities(id) => {
                write!(
                    f,
                    "planned driver {id} must not advertise implemented capabilities"
                )
            }
        }
    }
}
impl std::error::Error for CatalogError {}

/// Validate registry identity and prevent planned backends advertising support.
/// `None` capabilities means unknown/not implemented, not unsupported.
pub fn validate_driver_catalog(entries: &[DriverDescriptor]) -> Result<(), CatalogError> {
    for (index, driver) in entries.iter().enumerate() {
        if driver.id.is_empty()
            || !driver
                .id
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'_' | b'-'))
        {
            return Err(CatalogError::InvalidId(driver.id));
        }
        if entries[..index].iter().any(|other| other.id == driver.id) {
            return Err(CatalogError::DuplicateId(driver.id));
        }
        if entries[..index]
            .iter()
            .any(|other| other.engine == driver.engine)
        {
            return Err(CatalogError::DuplicateEngine(driver.engine));
        }
        if driver.status == DriverStatus::Planned && driver.capabilities.is_some() {
            return Err(CatalogError::PlannedCapabilities(driver.id));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_lookups_preserve_distinct_engine_identity() {
        assert_eq!(validate_driver_catalog(&PLANNED_DRIVERS), Ok(()));
        for driver in &PLANNED_DRIVERS {
            assert_eq!(driver_by_id(driver.id), Some(driver));
            assert_eq!(driver_for_engine(driver.engine), Some(driver));
        }
        assert_eq!(driver_by_id("MYSQL"), None);
        assert_eq!(driver_by_id("source-id"), None);
        let mysql = driver_by_id("mysql").unwrap();
        let maria = driver_by_id("mariadb").unwrap();
        assert_eq!(mysql.proposed_backend, maria.proposed_backend);
        assert_ne!(mysql.id, maria.id);
    }

    #[test]
    fn only_experimental_drivers_advertise_connection_capabilities() {
        for driver in &PLANNED_DRIVERS {
            assert_eq!(driver.runtime, DriverRuntime::Native);
            match driver.status {
                DriverStatus::Planned => assert_eq!(driver.capabilities, None),
                DriverStatus::Experimental => {
                    assert_eq!(
                        driver.dialect,
                        match driver.engine {
                            Engine::PostgreSql => Some("postgresql"),
                            Engine::MySql | Engine::MariaDb => Some("mysql"),
                            _ => None,
                        }
                    );
                    assert_eq!(
                        driver.capabilities,
                        Some(ConnectionCapabilities {
                            optional_database: true,
                            unix_socket: !matches!(driver.engine, Engine::Redis | Engine::MongoDb),
                        })
                    );
                }
            }
        }
    }

    #[test]
    fn duplicate_ids_are_rejected() {
        let mut entries = PLANNED_DRIVERS;
        entries[1].id = entries[0].id;
        assert_eq!(
            validate_driver_catalog(&entries),
            Err(CatalogError::DuplicateId("postgresql"))
        );
    }

    #[test]
    fn duplicate_engines_are_rejected_even_with_distinct_ids() {
        let mut entries = PLANNED_DRIVERS;
        entries[1].engine = entries[0].engine;
        assert_eq!(
            validate_driver_catalog(&entries),
            Err(CatalogError::DuplicateEngine(Engine::PostgreSql))
        );
    }

    #[test]
    fn invalid_ids_and_planned_capability_claims_are_rejected() {
        for id in ["", "MySQL", "my sql", "mysql/", "café"] {
            let mut entries = PLANNED_DRIVERS;
            entries[0].id = id;
            assert_eq!(
                validate_driver_catalog(&entries),
                Err(CatalogError::InvalidId(id))
            );
        }
        let mut entries = PLANNED_DRIVERS;
        entries[0].capabilities = entries[1].capabilities;
        entries[0].status = DriverStatus::Planned;
        assert_eq!(
            validate_driver_catalog(&entries),
            Err(CatalogError::PlannedCapabilities("postgresql"))
        );
    }
}

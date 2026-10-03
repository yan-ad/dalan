use dalan_core::Engine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverStatus {
    Planned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriverDescriptor {
    pub engine: Engine,
    pub proposed_backend: &'static str,
    pub status: DriverStatus,
}

pub const PLANNED_DRIVERS: [DriverDescriptor; 4] = [
    DriverDescriptor {
        engine: Engine::PostgreSql,
        proposed_backend: "sqlx::Postgres",
        status: DriverStatus::Planned,
    },
    DriverDescriptor {
        engine: Engine::MySql,
        proposed_backend: "sqlx::MySql",
        status: DriverStatus::Planned,
    },
    DriverDescriptor {
        engine: Engine::MariaDb,
        proposed_backend: "sqlx::MySql",
        status: DriverStatus::Planned,
    },
    DriverDescriptor {
        engine: Engine::Redis,
        proposed_backend: "redis",
        status: DriverStatus::Planned,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_release_catalog_has_unique_engines_and_no_implemented_claims() {
        for (index, driver) in PLANNED_DRIVERS.iter().enumerate() {
            assert_eq!(driver.status, DriverStatus::Planned);
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

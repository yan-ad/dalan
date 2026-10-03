use std::num::NonZeroU32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    PostgreSql,
    MySql,
    MariaDb,
    Redis,
}

impl Engine {
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::PostgreSql => "PostgreSQL",
            Self::MySql => "MySQL",
            Self::MariaDb => "MariaDB",
            Self::Redis => "Redis",
        }
    }

    pub const fn default_port(self) -> u16 {
        match self {
            Self::PostgreSql => 5432,
            Self::MySql | Self::MariaDb => 3306,
            Self::Redis => 6379,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionLimits {
    pub max_rows: NonZeroU32,
    pub max_bytes: NonZeroU32,
    pub timeout_seconds: NonZeroU32,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            max_rows: NonZeroU32::new(1_000).expect("nonzero constant"),
            max_bytes: NonZeroU32::new(8 * 1024 * 1024).expect("nonzero constant"),
            timeout_seconds: NonZeroU32::new(30).expect("nonzero constant"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationRisk {
    Read,
    Write,
    Destructive,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessMode {
    ReadOnly,
    ReadWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionDecision {
    Allow,
    RequireConfirmation,
    Deny,
}

/// Evaluates declared risk, not SQL text. Adapters must classify conservatively
/// and enforce server-side restrictions; a SELECT can still have side effects.
pub const fn execution_decision(mode: AccessMode, risk: OperationRisk) -> ExecutionDecision {
    match (mode, risk) {
        (_, OperationRisk::Read) => ExecutionDecision::Allow,
        (AccessMode::ReadOnly, _) => ExecutionDecision::Deny,
        (AccessMode::ReadWrite, _) => ExecutionDecision::RequireConfirmation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_names_and_ports_are_distinct_from_driver_backends() {
        assert_eq!(Engine::PostgreSql.default_port(), 5432);
        assert_eq!(Engine::MySql.default_port(), 3306);
        assert_eq!(Engine::MariaDb.default_port(), 3306);
        assert_eq!(Engine::Redis.default_port(), 6379);
        assert_eq!(Engine::MariaDb.display_name(), "MariaDB");
    }

    #[test]
    fn readonly_denies_every_non_read_risk() {
        for risk in [
            OperationRisk::Write,
            OperationRisk::Destructive,
            OperationRisk::Unknown,
        ] {
            assert_eq!(
                execution_decision(AccessMode::ReadOnly, risk),
                ExecutionDecision::Deny
            );
        }
    }

    #[test]
    fn readwrite_requires_confirmation_for_every_non_read_risk() {
        for risk in [
            OperationRisk::Write,
            OperationRisk::Destructive,
            OperationRisk::Unknown,
        ] {
            assert_eq!(
                execution_decision(AccessMode::ReadWrite, risk),
                ExecutionDecision::RequireConfirmation
            );
        }
    }

    #[test]
    fn reads_are_allowed_in_both_modes() {
        for mode in [AccessMode::ReadOnly, AccessMode::ReadWrite] {
            assert_eq!(
                execution_decision(mode, OperationRisk::Read),
                ExecutionDecision::Allow
            );
        }
    }

    #[test]
    fn limits_are_bounded_by_default() {
        let limits = ExecutionLimits::default();
        assert_eq!(limits.max_rows.get(), 1_000);
        assert_eq!(limits.max_bytes.get(), 8 * 1024 * 1024);
        assert_eq!(limits.timeout_seconds.get(), 30);
    }
}

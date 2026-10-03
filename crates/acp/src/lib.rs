use std::path::PathBuf;

pub use agent_client_protocol::schema::v1 as protocol;

pub const SUPPORTED_PROTOCOL_VERSION: u16 = 1;

pub fn client_capabilities() -> protocol::ClientCapabilities {
    protocol::ClientCapabilities::default()
}

#[derive(Clone, PartialEq, Eq)]
pub struct AgentLaunchConfig {
    pub executable: PathBuf,
    pub arguments: Vec<String>,
    pub working_directory: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    ExecutableMustBeAbsolute,
    WorkingDirectoryMustBeAbsolute,
}

impl AgentLaunchConfig {
    /// This checks path shape only. Trust, existence, executable permissions,
    /// environment filtering, and process lifetime belong to the future launcher.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if !self.executable.is_absolute() {
            return Err(ConfigError::ExecutableMustBeAbsolute);
        }
        if !self.working_directory.is_absolute() {
            return Err(ConfigError::WorkingDirectoryMustBeAbsolute);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> AgentLaunchConfig {
        let root = std::env::current_dir().unwrap();
        AgentLaunchConfig {
            executable: root.join("example-agent"),
            arguments: vec![],
            working_directory: root,
        }
    }

    #[test]
    fn relative_executable_is_rejected() {
        let mut config = config();
        config.executable = "example-agent".into();
        assert_eq!(
            config.validate(),
            Err(ConfigError::ExecutableMustBeAbsolute)
        );
    }

    #[test]
    fn relative_working_directory_is_rejected() {
        let mut config = config();
        config.working_directory = ".".into();
        assert_eq!(
            config.validate(),
            Err(ConfigError::WorkingDirectoryMustBeAbsolute)
        );
    }

    #[test]
    fn absolute_paths_pass_shape_validation_without_claiming_trust() {
        assert_eq!(config().validate(), Ok(()));
    }

    #[test]
    fn no_filesystem_or_tool_terminal_capabilities_are_advertised() {
        let capabilities = client_capabilities();
        assert_eq!(capabilities, protocol::ClientCapabilities::default());
        assert!(!capabilities.fs.read_text_file);
        assert!(!capabilities.fs.write_text_file);
        assert!(!capabilities.terminal);
        assert!(!capabilities.auth.terminal);
        assert!(capabilities.session.is_none());
        assert!(capabilities.elicitation.is_none());
        assert_eq!(SUPPORTED_PROTOCOL_VERSION, 1);
    }
}

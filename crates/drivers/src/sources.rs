//! Persistable source settings. Credentials are deliberately not part of a profile.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DbEngine {
    MySql,
    MariaDb,
}
impl DbEngine {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::MySql => "MySQL",
            Self::MariaDb => "MariaDB",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Transport {
    Direct,
    Ssh {
        host: String,
        port: u16,
        user: String,
        identity_file: Option<String>,
        #[serde(default)]
        known_hosts_file: Option<String>,
    },
    HttpConnect {
        host: String,
        port: u16,
        https: bool,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TlsMode {
    VerifyIdentity,
    Disabled,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceProfile {
    pub id: String,
    pub name: String,
    pub engine: DbEngine,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub database: Option<String>,
    pub transport: Transport,
    pub tls: TlsMode,
    pub ca_path: Option<String>,
    pub save_password: bool,
}
impl Default for SourceProfile {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: "New source".into(),
            engine: DbEngine::MySql,
            host: "localhost".into(),
            port: 3306,
            username: "root".into(),
            database: None,
            transport: Transport::Direct,
            tls: TlsMode::VerifyIdentity,
            ca_path: None,
            save_password: false,
        }
    }
}
fn field(value: &str, max: usize, label: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.trim() == value
            && value.len() <= max
            && !value.chars().any(char::is_control),
        "Invalid {label}"
    );
    Ok(())
}
pub(crate) fn host(value: &str) -> Result<()> {
    field(value, 253, "host")?;
    ensure!(
        !value.starts_with('-')
            && !value
                .chars()
                .any(|c| c.is_whitespace() || matches!(c, '/' | '\\' | '@' | '?' | '#' | '[' | ']'))
            && !value.contains("://"),
        "Invalid host (use a bare hostname or IP address)"
    );
    // A colon is permitted only in an unbracketed IPv6 address.
    ensure!(
        !value.contains(':') || value.parse::<std::net::Ipv6Addr>().is_ok(),
        "Invalid host"
    );
    Ok(())
}
pub(crate) fn identifier(value: &str) -> Result<()> {
    field(value, 256, "database/table/column name")?;
    ensure!(
        value.chars().count() <= 64,
        "Identifier exceeds 64 characters"
    );
    Ok(())
}
impl SourceProfile {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            uuid::Uuid::parse_str(&self.id).is_ok(),
            "Invalid source UUID"
        );
        field(&self.name, 256, "source name")?;
        host(&self.host)?;
        ensure!(self.port != 0, "Port must be nonzero");
        field(&self.username, 128, "username")?;
        if let Some(db) = &self.database {
            identifier(db)?;
        }
        if let Some(path) = &self.ca_path {
            field(path, 4096, "CA path")?;
        }
        match &self.transport {
            Transport::Direct => {}
            Transport::Ssh {
                host: h,
                port,
                user,
                identity_file,
                known_hosts_file,
            } => {
                host(h)?;
                ensure!(*port != 0, "SSH port must be nonzero");
                field(user, 128, "SSH user")?;
                ensure!(
                    !user.starts_with('-')
                        && !user
                            .chars()
                            .any(|c| c.is_whitespace() || matches!(c, '@' | '/' | '\\')),
                    "Invalid SSH user"
                );
                if let Some(path) = identity_file {
                    field(path, 4096, "SSH identity path")?;
                }
                if let Some(path) = known_hosts_file {
                    field(path, 4096, "SSH known hosts path")?;
                    ensure!(
                        std::path::Path::new(path).is_absolute()
                            && std::path::Path::new(path).is_file()
                            && !path.contains('%')
                            && !path.contains("${"),
                        "SSH known hosts path must be an absolute regular file without expansion tokens"
                    );
                }
            }
            Transport::HttpConnect { host: h, port, .. } => {
                host(h)?;
                ensure!(*port != 0, "Proxy port must be nonzero");
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transport_rejects_unknown_nested_fields() {
        for json in [
            r#"{"Ssh":{"host":"bastion","port":22,"user":"alice","identity_file":null,"password":"secret"}}"#,
            r#"{"HttpConnect":{"host":"proxy","port":8080,"https":false,"password":"secret"}}"#,
        ] {
            assert!(serde_json::from_str::<Transport>(json).is_err());
        }
        assert!(
            serde_json::from_str::<Transport>(
                r#"{"Ssh":{"host":"bastion","port":22,"user":"alice","identity_file":null}}"#
            )
            .is_ok()
        );
        let legacy: Transport = serde_json::from_str(
            r#"{"Ssh":{"host":"bastion","port":22,"user":"alice","identity_file":null}}"#,
        )
        .unwrap();
        assert!(matches!(
            legacy,
            Transport::Ssh {
                known_hosts_file: None,
                ..
            }
        ));
    }
    #[test]
    fn selected_known_hosts_path_validation() {
        let directory = std::env::temp_dir().join(format!("dalan hosts {}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let file = directory.join("known hosts");
        std::fs::write(&file, "").unwrap();
        let mut p = SourceProfile::default();
        for (path, valid) in [
            (file.to_str().unwrap().to_owned(), true),
            (directory.to_str().unwrap().to_owned(), false),
            ("relative_hosts".into(), false),
            (directory.join("absent").to_str().unwrap().into(), false),
            ("/tmp/hosts\n-oProxyCommand=bad".into(), false),
        ] {
            p.transport = Transport::Ssh {
                host: "bastion".into(),
                port: 22,
                user: "alice".into(),
                identity_file: None,
                known_hosts_file: Some(path),
            };
            assert_eq!(p.validate().is_ok(), valid);
        }
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn defaults_and_validation() {
        let mut p = SourceProfile::default();
        assert!(p.validate().is_ok());
        assert_eq!(p.port, 3306);
        assert_eq!(p.tls, TlsMode::VerifyIdentity);
        assert_eq!(p.database, None);
        assert!(!p.save_password);
        p.port = 0;
        assert!(p.validate().is_err());
        p.port = 3306;
        for h in [
            "https://host",
            "host\r\nX: y",
            "host name",
            "-oProxyCommand=x",
            "host:3306",
        ] {
            p.host = h.into();
            assert!(p.validate().is_err());
        }
        p.host = "::1".into();
        assert!(p.validate().is_ok());
        p.database = Some("  db".into());
        assert!(p.validate().is_err());
        p.database = Some("".into());
        assert!(p.validate().is_err());
        p.database = None;
        p.id = "not uuid".into();
        assert!(p.validate().is_err());
    }
}

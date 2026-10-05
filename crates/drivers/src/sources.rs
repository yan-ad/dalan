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
        /// Opt in to local OpenSSH config, including any configured ProxyCommand.
        #[serde(default)]
        parse_config: bool,
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
    /// Encrypts traffic without verifying the certificate or server identity.
    Required,
    /// Verifies certificate trust, but not the server hostname.
    VerifyCa,
    Disabled,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ConnectionMode {
    #[default]
    Default,
    UnixSocket {
        path: String,
    },
    UrlOnly {
        url: String,
    },
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Authentication {
    #[default]
    UserPassword,
    NoAuth,
}
/// Display filtering only: never an account/database access restriction.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchemaSelection {
    #[default]
    All,
    Selected(Vec<String>),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SourceOptions {
    pub connect_timeout_seconds: u64,
    pub query_timeout_seconds: u64,
    pub page_size: u32,
}
impl Default for SourceOptions {
    fn default() -> Self {
        Self {
            connect_timeout_seconds: 10,
            query_timeout_seconds: 20,
            page_size: 100,
        }
    }
}
impl SourceOptions {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1..=60).contains(&self.connect_timeout_seconds),
            "Connect timeout must be 1 through 60 seconds"
        );
        ensure!(
            (1..=120).contains(&self.query_timeout_seconds),
            "Query timeout must be 1 through 120 seconds"
        );
        ensure!(
            (1..=200).contains(&self.page_size),
            "Page size must be 1 through 200"
        );
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionTarget {
    Tcp {
        host: String,
        port: u16,
        database: Option<String>,
    },
    UnixSocket {
        path: String,
        database: Option<String>,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceProfile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
    pub engine: DbEngine,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub database: Option<String>,
    pub transport: Transport,
    pub tls: TlsMode,
    pub ca_path: Option<String>,
    pub save_password: bool,
    #[serde(default)]
    pub endpoint: ConnectionMode,
    #[serde(default)]
    pub authentication: Authentication,
    #[serde(default)]
    pub schemas: SchemaSelection,
    #[serde(default)]
    pub options: SourceOptions,
    #[serde(default)]
    pub ssl_client_cert: Option<String>,
    #[serde(default)]
    pub ssl_client_key: Option<String>,
    /// Metadata reference only; callers must materialize Transport before connecting.
    #[serde(default)]
    pub ssh_configuration_id: Option<String>,
}
impl Default for SourceProfile {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: "New source".into(),
            color: None,
            engine: DbEngine::MySql,
            host: "localhost".into(),
            port: 3306,
            username: "root".into(),
            database: None,
            transport: Transport::Direct,
            tls: TlsMode::VerifyIdentity,
            ca_path: None,
            save_password: false,
            endpoint: ConnectionMode::default(),
            authentication: Authentication::default(),
            schemas: SchemaSelection::default(),
            options: SourceOptions::default(),
            ssl_client_cert: None,
            ssl_client_key: None,
            ssh_configuration_id: None,
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
    pub fn visible_schema(&self, database: &str) -> bool {
        match &self.schemas {
            SchemaSelection::All => true,
            SchemaSelection::Selected(names) => names.iter().any(|n| n == database),
        }
    }
    pub fn connection_target(&self) -> Result<ConnectionTarget> {
        let (h, port, db) = match &self.endpoint {
            ConnectionMode::Default => (self.host.clone(), self.port, self.database.clone()),
            ConnectionMode::UnixSocket { path } => {
                ensure!(cfg!(unix), "Unix sockets are unavailable on this platform");
                field(path, 4096, "Unix socket path")?;
                ensure!(
                    std::path::Path::new(path).is_absolute(),
                    "Unix socket path must be absolute"
                );
                ensure!(
                    matches!(self.transport, Transport::Direct),
                    "Unix sockets require direct transport"
                );
                // mysql_async does not upgrade Unix-domain streams to TLS.
                ensure!(
                    self.tls == TlsMode::Disabled,
                    "Unix sockets require TLS Disabled; TLS is not supported on this transport"
                );
                if let Some(db) = &self.database {
                    identifier(db)?;
                }
                return Ok(ConnectionTarget::UnixSocket {
                    path: path.clone(),
                    database: self.database.clone(),
                });
            }
            ConnectionMode::UrlOnly { url } => {
                field(url, 8192, "connection URL")?;
                let raw = url.strip_prefix("jdbc:").unwrap_or(url);
                ensure!(
                    raw.starts_with("mysql://") || raw.starts_with("mariadb://"),
                    "Connection URL must use mysql:// or mariadb://"
                );
                // Both schemes speak the same protocol, including mysql:// for MariaDB.
                let u =
                    url::Url::parse(raw).map_err(|_| anyhow::anyhow!("Invalid connection URL"))?;
                ensure!(
                    u.username().is_empty()
                        && u.password().is_none()
                        && !raw.split('/').nth(2).unwrap_or_default().contains('@'),
                    "Connection URL must not contain credentials; use authentication fields"
                );
                ensure!(
                    u.query().is_none() && u.fragment().is_none(),
                    "Connection URL options and fragments are unsupported; use source options"
                );
                let h = match u.host() {
                    Some(url::Host::Ipv6(ip)) => ip.to_string(),
                    Some(h) => h.to_string(),
                    None => return Err(anyhow::anyhow!("Connection URL requires a host")),
                };
                let path = u.path().strip_prefix('/').unwrap_or(u.path());
                ensure!(
                    !path.contains('/'),
                    "Connection URL accepts one database path segment"
                );
                // Reject malformed escapes explicitly; percent_decode otherwise preserves them.
                let bytes = path.as_bytes();
                for (i, b) in bytes.iter().enumerate() {
                    if *b == b'%' {
                        ensure!(
                            bytes.get(i + 1).is_some_and(u8::is_ascii_hexdigit)
                                && bytes.get(i + 2).is_some_and(u8::is_ascii_hexdigit),
                            "Invalid connection URL database encoding"
                        );
                    }
                }
                let db = percent_encoding::percent_decode_str(path)
                    .decode_utf8()
                    .map_err(|_| anyhow::anyhow!("Invalid connection URL database encoding"))?;
                (
                    h,
                    u.port().unwrap_or(3306),
                    if db.is_empty() {
                        None
                    } else {
                        Some(db.into_owned())
                    },
                )
            }
        };
        host(&h)?;
        ensure!(port != 0, "Port must be nonzero");
        if let Some(db) = &db {
            identifier(db)?;
        }
        Ok(ConnectionTarget::Tcp {
            host: h,
            port,
            database: db,
        })
    }
    pub fn effective_profile(&self) -> Result<Self> {
        self.resolved()
    }
    pub fn resolved(&self) -> Result<Self> {
        self.validate()?;
        let mut p = self.clone();
        match self.connection_target()? {
            ConnectionTarget::Tcp {
                host,
                port,
                database,
            } => {
                p.host = host;
                p.port = port;
                p.database = database;
                p.endpoint = ConnectionMode::Default;
            }
            ConnectionTarget::UnixSocket { path, database } => {
                p.endpoint = ConnectionMode::UnixSocket { path };
                p.database = database;
            }
        }
        if p.authentication == Authentication::NoAuth {
            p.username.clear();
        }
        Ok(p)
    }
    /// Credential-free URL. Socket endpoints have no equivalent TCP URL.
    pub fn canonical_url(&self) -> Result<String> {
        self.validate()?;
        if let ConnectionMode::UrlOnly { url } = &self.endpoint {
            return Ok(url.clone());
        }
        let ConnectionTarget::Tcp {
            host,
            port,
            database,
        } = self.connection_target()?
        else {
            return Err(anyhow::anyhow!("Unix sockets have no connection URL"));
        };
        let authority = if host.contains(':') {
            format!("[{host}]")
        } else {
            host
        };
        let mut u = url::Url::parse(&format!("mysql://{authority}:{port}/"))
            .map_err(|_| anyhow::anyhow!("Invalid connection URL"))?;
        if let Some(db) = database {
            u.path_segments_mut()
                .map_err(|_| anyhow::anyhow!("Invalid connection URL"))?
                .clear()
                .push(&db);
        }
        Ok(u.into())
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            uuid::Uuid::parse_str(&self.id).is_ok(),
            "Invalid source UUID"
        );
        field(&self.name, 256, "source name")?;
        if let Some(color) = &self.color {
            ensure!(
                color.len() == 7
                    && color.starts_with('#')
                    && color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit),
                "Invalid source color (use #RRGGBB)"
            );
        }
        self.connection_target()?;
        self.options.validate()?;
        if self.authentication == Authentication::UserPassword {
            field(&self.username, 128, "username")?;
        }
        if let SchemaSelection::Selected(names) = &self.schemas {
            ensure!(names.len() <= 1000, "Too many selected schemas");
            for name in names {
                identifier(name)?;
            }
        }
        if let Some(id) = &self.ssh_configuration_id {
            ensure!(
                uuid::Uuid::parse_str(id).is_ok(),
                "Invalid SSH configuration UUID"
            );
        }
        ensure!(
            self.ssl_client_cert.is_some() == self.ssl_client_key.is_some(),
            "TLS client certificate and key must both be supplied"
        );
        for path in [&self.ssl_client_cert, &self.ssl_client_key]
            .into_iter()
            .flatten()
        {
            field(path, 4096, "TLS client identity path")?;
            ensure!(
                std::path::Path::new(path).is_absolute(),
                "TLS client identity path must be absolute"
            );
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
                ..
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
    fn legacy_profile_defaults_and_round_trip() {
        let original = SourceProfile::default();
        let mut json = serde_json::to_value(&original).unwrap();
        for key in [
            "endpoint",
            "authentication",
            "schemas",
            "options",
            "ssl_client_cert",
            "ssl_client_key",
            "ssh_configuration_id",
        ] {
            json.as_object_mut().unwrap().remove(key);
        }
        let p: SourceProfile = serde_json::from_value(json).unwrap();
        assert_eq!(p, original);
        let ssh: Transport = serde_json::from_str(
            r#"{"Ssh":{"host":"host","port":22,"user":"user","identity_file":null}}"#,
        )
        .unwrap();
        assert!(matches!(
            ssh,
            Transport::Ssh {
                parse_config: false,
                ..
            }
        ));
        assert_eq!(
            serde_json::from_str::<SourceProfile>(&serde_json::to_string(&p).unwrap()).unwrap(),
            p
        );
    }
    #[test]
    fn urls_resolve_without_credentials_or_stale_fields() {
        let mut p = SourceProfile {
            host: "invalid://ignored".into(),
            port: 0,
            database: Some("".into()),
            ..SourceProfile::default()
        };
        for scheme in ["mysql", "mariadb", "jdbc:mysql", "jdbc:mariadb"] {
            let raw = format!("{scheme}://[::1]:3307/db%20name");
            p.endpoint = ConnectionMode::UrlOnly { url: raw.clone() };
            assert_eq!(p.canonical_url().unwrap(), raw);
            let effective = p.resolved().unwrap();
            assert_eq!(effective.host, "::1");
            assert_eq!(effective.port, 3307);
            assert_eq!(effective.database.as_deref(), Some("db name"));
            assert_eq!(effective.endpoint, ConnectionMode::Default);
            effective.validate().unwrap();
        }
        p.endpoint = ConnectionMode::UrlOnly {
            url: "mysql://localhost".into(),
        };
        assert_eq!(p.resolved().unwrap().database, None);
        p.endpoint = ConnectionMode::UrlOnly {
            url: "mysql://localhost/db%2Fname".into(),
        };
        assert_eq!(p.resolved().unwrap().database.as_deref(), Some("db/name"));
    }
    #[test]
    fn bad_url_errors_never_echo_inputs() {
        let mut p = SourceProfile::default();
        for raw in [
            "mysql://alice:private-secret@host/db",
            "mysql://private-secret@host",
            "mysql://@host",
            "mysql://host/db?password=private-secret",
            "mysql://host/db?ssl-mode=REQUIRED",
            "mysql://host/db#private-secret",
            "jdbc:postgresql://private-secret/db",
            "mysql://host:0/db",
            "mysql://host/a/b",
            "mysql://host/%FF",
            "mysql://host/%GG",
            "mysql://",
        ] {
            p.endpoint = ConnectionMode::UrlOnly { url: raw.into() };
            let error = p.validate().unwrap_err().to_string();
            assert!(!error.contains("private-secret"));
            assert!(!error.contains(raw));
        }
    }
    #[test]
    fn canonical_url_ipv6_and_encoded_database() {
        let p = SourceProfile {
            host: "::1".into(),
            database: Some("db/name?#".into()),
            ..SourceProfile::default()
        };
        let url = p.canonical_url().unwrap();
        assert!(url.starts_with("mysql://[::1]:3306/"));
        let from = SourceProfile {
            endpoint: ConnectionMode::UrlOnly { url },
            ..SourceProfile::default()
        };
        assert_eq!(
            p.connection_target().unwrap(),
            from.connection_target().unwrap()
        );
    }
    #[test]
    fn no_auth_and_schema_filter_are_not_access_restrictions() {
        let mut p = SourceProfile {
            authentication: Authentication::NoAuth,
            username: String::new(),
            schemas: SchemaSelection::Selected(vec![]),
            ..SourceProfile::default()
        };
        p.validate().unwrap();
        assert!(!p.visible_schema("anything"));
        p.schemas = SchemaSelection::Selected(vec!["selected".into()]);
        assert!(p.visible_schema("selected"));
        assert!(!p.visible_schema("other"));
        assert_eq!(p.resolved().unwrap().database, None);
        p.username = "ignored".into();
        assert!(p.resolved().unwrap().username.is_empty());
        p.authentication = Authentication::UserPassword;
        p.username.clear();
        assert!(p.validate().is_err());
    }
    #[test]
    fn options_are_strict_and_partial_json_uses_defaults() {
        assert_eq!(
            serde_json::from_str::<SourceOptions>(r#"{"page_size":200}"#).unwrap(),
            SourceOptions {
                page_size: 200,
                ..SourceOptions::default()
            }
        );
        for value in [0, 61, u64::MAX] {
            assert!(
                SourceOptions {
                    connect_timeout_seconds: value,
                    ..SourceOptions::default()
                }
                .validate()
                .is_err()
            );
        }
        for value in [0, 121, u64::MAX] {
            assert!(
                SourceOptions {
                    query_timeout_seconds: value,
                    ..SourceOptions::default()
                }
                .validate()
                .is_err()
            );
        }
        for value in [0, 201, u32::MAX] {
            assert!(
                SourceOptions {
                    page_size: value,
                    ..SourceOptions::default()
                }
                .validate()
                .is_err()
            );
        }
        SourceOptions {
            connect_timeout_seconds: 60,
            query_timeout_seconds: 120,
            page_size: 200,
        }
        .validate()
        .unwrap();
        SourceOptions {
            connect_timeout_seconds: 1,
            query_timeout_seconds: 1,
            page_size: 1,
        }
        .validate()
        .unwrap();
    }
    #[test]
    fn client_identity_validation_is_metadata_only() {
        let directory =
            std::env::temp_dir().join(format!("dalan-unread-identity-{}", uuid::Uuid::new_v4()));
        let mut p = SourceProfile {
            ssl_client_cert: Some(directory.join("client.pem").to_string_lossy().into_owned()),
            ..SourceProfile::default()
        };
        assert!(p.validate().is_err());
        p.ssl_client_key = Some(directory.join("client.key").to_string_lossy().into_owned());
        p.validate().unwrap(); // no reads, not even required to exist yet
        p.ssl_client_key = Some("relative-key".into());
        assert!(p.validate().is_err());
        p.ssl_client_key = Some("/tmp/key\nsecret".into());
        assert!(p.validate().is_err());
        p.ssl_client_key = None;
        p.ssl_client_cert = None;
        p.ssh_configuration_id = Some(uuid::Uuid::new_v4().to_string());
        p.validate().unwrap(); // reference alone does not start an SSH transport
    }
    #[cfg(unix)]
    #[test]
    fn socket_requires_direct_local_transport_and_explicit_tls_disabled() {
        let mut p = SourceProfile {
            endpoint: ConnectionMode::UnixSocket {
                path: "/tmp/mysql socket.sock".into(),
            },
            host: "ignored://host".into(),
            port: 0,
            ..SourceProfile::default()
        };
        assert!(p.validate().is_err());
        p.tls = TlsMode::Disabled;
        p.validate().unwrap();
        assert!(matches!(
            p.resolved().unwrap().endpoint,
            ConnectionMode::UnixSocket { .. }
        ));
        p.transport = Transport::HttpConnect {
            host: "proxy".into(),
            port: 8080,
            https: false,
        };
        assert!(p.validate().is_err());
        p.transport = Transport::Direct;
        p.endpoint = ConnectionMode::UnixSocket {
            path: "relative.sock".into(),
        };
        assert!(p.validate().is_err());
    }
    #[test]
    fn color_validation() {
        let mut profile = SourceProfile::default();
        assert_eq!(profile.color, None);
        assert!(profile.validate().is_ok());
        for color in ["#ff8800", "#AB12cd"] {
            profile.color = Some(color.into());
            assert!(profile.validate().is_ok());
        }
        for color in [
            "",
            "ff8800",
            "#fff",
            "#ff88000",
            "#gg8800",
            " #ff8800",
            "#ff8800 ",
            "#ff88\n0",
            "#ff88\0",
            "#ＡB12",
            "private-secret",
        ] {
            profile.color = Some(color.into());
            let error = profile.validate().unwrap_err().to_string();
            assert_eq!(error, "Invalid source color (use #RRGGBB)");
        }
    }
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
                parse_config: false,
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
        assert_eq!(p.color, None);
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

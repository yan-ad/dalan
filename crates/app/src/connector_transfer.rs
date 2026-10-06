//! Password-free, bounded connector interchange. Foreign transports and read-only
//! policies which cannot be reproduced are skipped, never silently made direct/writable.
//! No keychain, SSH repository, database, or network access occurs here. Filesystem
//! checks reject symlinks, but portable std operations cannot prevent a hostile
//! concurrent replacement of a parent directory.
use anyhow::{Result, bail, ensure};
use dalan_drivers::{ConnectionMode, DbEngine, SourceProfile, TlsMode};
use serde_json::{Value, json};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path},
};
use uuid::Uuid;

const MAX_BYTES: usize = 1024 * 1024;
const MAX_SOURCES: usize = 100;
const FORMAT: &str = "dalan-connectors";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFormat {
    Dbx,
    Navicat,
    DataGrip,
    ConnectorsList,
}
impl ImportFormat {
    pub fn label(self) -> &'static str {
        match self {
            Self::Dbx => "DBX",
            Self::Navicat => "Navicat",
            Self::DataGrip => "DataGrip",
            Self::ConnectorsList => "Dalan connectors list",
        }
    }
    pub fn default_extension(self) -> &'static str {
        match self {
            Self::Dbx | Self::ConnectorsList => "json",
            Self::Navicat | Self::DataGrip => "xml",
        }
    }
}
#[derive(Debug)]
pub struct ImportReport {
    pub profiles: Vec<SourceProfile>,
    pub warnings: Vec<String>,
}
#[derive(Default)]
struct Counts {
    unsupported: usize,
    transport: usize,
    invalid: usize,
    readonly: usize,
}
impl Counts {
    fn finish(self, report: &mut ImportReport) {
        for (count, reason) in [
            (self.unsupported, "unsupported database drivers"),
            (
                self.transport,
                "SSH/proxy transports or missing SSH sessions; configure transport separately",
            ),
            (self.invalid, "invalid or unsafe connection settings"),
            (
                self.readonly,
                "read-only policies that Dalan cannot enforce",
            ),
        ] {
            if count > 0 {
                report
                    .warnings
                    .push(format!("Skipped {count} source(s): {reason}."));
            }
        }
    }
}
fn warning(report: &mut ImportReport, text: &str) {
    if !report.warnings.iter().any(|s| s == text) {
        report.warnings.push(text.into());
    }
}
fn engine(value: &str) -> Option<DbEngine> {
    match value.to_ascii_lowercase().as_str() {
        "mysql" => Some(DbEngine::MySql),
        "mariadb" => Some(DbEngine::MariaDb),
        _ => None,
    }
}
fn false_flag(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "no" | "disabled" | "off" | "none" | ""
    )
}
fn transport_key(s: &str) -> bool {
    let s = s.to_ascii_lowercase();
    s.contains("ssh") || s.contains("proxy") || s.contains("tunnel") || s.starts_with("http")
}
// Conservative: foreign transport configuration must not be converted to direct.
fn foreign_transport(value: &Value) -> bool {
    match value {
        Value::Object(fields) => fields.iter().any(|(k, v)| {
            (transport_key(k)
                && match v {
                    Value::Null => false,
                    Value::Bool(b) => *b,
                    Value::String(s) => !false_flag(s),
                    Value::Number(n) => n.as_u64() != Some(0),
                    Value::Object(_) | Value::Array(_) => true,
                })
                || foreign_transport(v)
        }),
        Value::Array(a) => a.iter().any(foreign_transport),
        _ => false,
    }
}
fn text<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}
fn path_text(v: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|k| text(v, k))
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}
fn json_port(v: &Value) -> Option<u16> {
    match v.get("port") {
        None | Some(Value::Null) => Some(3306),
        Some(Value::Number(n)) => n
            .as_u64()
            .and_then(|n| u16::try_from(n).ok())
            .filter(|n| *n != 0),
        Some(Value::String(s)) => s.parse().ok().filter(|n| *n != 0),
        _ => None,
    }
}
fn accept(mut p: SourceProfile, report: &mut ImportReport, counts: &mut Counts) {
    p.id = Uuid::new_v4().to_string();
    p.save_password = false;
    if p.validate().is_err() {
        counts.invalid += 1;
        return;
    }
    if p.ca_path.is_some() || p.ssl_client_cert.is_some() || p.ssl_client_key.is_some() {
        warning(
            report,
            "Certificate/key paths refer to local files; review them on this machine. No file contents were imported.",
        );
    }
    report.profiles.push(p);
}
pub fn import_bytes(format: ImportFormat, bytes: &[u8]) -> Result<ImportReport> {
    ensure!(
        bytes.len() <= MAX_BYTES,
        "Connector import exceeds the 1 MiB limit"
    );
    let mut report = ImportReport {
        profiles: Vec::new(),
        warnings: vec!["Passwords are never imported. Review settings before connecting.".into()],
    };
    let mut counts = Counts::default();
    match format {
        ImportFormat::Dbx | ImportFormat::ConnectorsList => {
            import_json(format, bytes, &mut report, &mut counts)?
        }
        ImportFormat::Navicat | ImportFormat::DataGrip => {
            import_xml(format, bytes, &mut report, &mut counts)?
        }
    }
    counts.finish(&mut report);
    ensure!(
        !report.profiles.is_empty(),
        "No supported safe MySQL/MariaDB sources were found; unsupported drivers, transports, read-only policies, or unsafe settings must be configured separately"
    );
    Ok(report)
}
fn import_json(
    format: ImportFormat,
    bytes: &[u8],
    report: &mut ImportReport,
    counts: &mut Counts,
) -> Result<()> {
    let root: Value =
        serde_json::from_slice(bytes).map_err(|_| anyhow::anyhow!("Invalid connector JSON"))?;
    ensure!(
        text(&root, "format") != Some("dbx-encrypted"),
        "Encrypted DBX exports are unsupported; export an unencrypted connector list without passwords from DBX"
    );
    let sources = if format == ImportFormat::ConnectorsList {
        ensure!(
            text(&root, "format") == Some(FORMAT)
                && root.get("version").and_then(Value::as_u64) == Some(1),
            "Unsupported Dalan connectors format or version"
        );
        root.get("profiles").and_then(Value::as_array)
    } else {
        root.as_array()
            .or_else(|| root.get("connections").and_then(Value::as_array))
    }
    .ok_or_else(|| anyhow::anyhow!("Invalid connector list structure"))?;
    ensure!(
        sources.len() <= MAX_SOURCES,
        "Connector import exceeds the 100 source limit"
    );
    for v in sources {
        if format == ImportFormat::ConnectorsList {
            let Some(e) = text(v, "engine").and_then(engine) else {
                counts.unsupported += 1;
                continue;
            };
            if v.get("read_only")
                .is_some_and(|v| !v.is_null() && v != false && v != "false")
            {
                counts.readonly += 1;
                continue;
            }
            // Deserialize only the password-free model; unknown credential fields are ignored.
            let Ok(mut p) = serde_json::from_value::<SourceProfile>(v.clone()) else {
                counts.invalid += 1;
                continue;
            };
            p.engine = e;
            if matches!(p.transport, dalan_drivers::Transport::Ssh { .. }) {
                if p.ssh_configuration_id
                    .as_ref()
                    .is_none_or(|id| Uuid::parse_str(id).is_err())
                {
                    counts.transport += 1;
                    continue;
                }
                warning(
                    report,
                    "SSH session references are retained, but session definitions are not transferred. Select a local saved SSH session before connecting; missing references fail closed.",
                );
            }
            if !safe_endpoint(&p) {
                counts.invalid += 1;
                continue;
            }
            accept(p, report, counts);
            continue;
        }
        let Some(e) = text(v, "db_type").and_then(engine) else {
            counts.unsupported += 1;
            continue;
        };
        if foreign_transport(v)
            || v.get("transport").is_some_and(|t| {
                !t.is_null()
                    && !t
                        .as_str()
                        .is_some_and(|s| s.eq_ignore_ascii_case("direct") || false_flag(s))
            })
        {
            counts.transport += 1;
            continue;
        }
        if v.get("read_only").is_some_and(|v| match v {
            Value::Bool(b) => *b,
            Value::String(s) => !false_flag(s),
            Value::Null => false,
            _ => true,
        }) {
            counts.readonly += 1;
            continue;
        }
        let Some(port) = json_port(v) else {
            counts.invalid += 1;
            continue;
        };
        let Some(host) = text(v, "host") else {
            counts.invalid += 1;
            continue;
        };
        let tls = if v.get("ssl") == Some(&Value::Bool(false)) {
            warning(
                report,
                "DBX explicitly disabled TLS for a source; review before connecting.",
            );
            TlsMode::Disabled
        } else {
            if v.get("ssl") != Some(&Value::Bool(true)) {
                warning(
                    report,
                    "Unknown DBX TLS settings were mapped to VerifyIdentity; review before connecting.",
                );
            }
            TlsMode::VerifyIdentity
        };
        let p = SourceProfile {
            name: text(v, "name").unwrap_or("Imported source").into(),
            engine: e,
            host: host.into(),
            port,
            username: text(v, "username").unwrap_or("root").into(),
            database: path_text(v, &["database"]),
            tls,
            ca_path: path_text(v, &["ssl_ca", "ssl_ca_path", "ca_path"]),
            ssl_client_cert: path_text(v, &["ssl_cert", "ssl_cert_path"]),
            ssl_client_key: path_text(v, &["ssl_key", "ssl_key_path"]),
            ..SourceProfile::default()
        };
        // URL/socket options cannot silently override the imported TCP/security settings.
        if ["url", "connection_url", "socket", "socket_path"]
            .iter()
            .any(|k| v.get(k).is_some_and(|v| !v.is_null() && v != ""))
        {
            counts.invalid += 1;
            continue;
        }
        accept(p, report, counts);
    }
    Ok(())
}
fn decode_xml(bytes: &[u8]) -> Result<String> {
    let text = if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        ensure!((bytes.len() - 2).is_multiple_of(2), "Invalid XML encoding");
        let little = bytes[0] == 0xff;
        let units: Vec<u16> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| {
                if little {
                    u16::from_le_bytes([b[0], b[1]])
                } else {
                    u16::from_be_bytes([b[0], b[1]])
                }
            })
            .collect();
        String::from_utf16(&units).map_err(|_| anyhow::anyhow!("Invalid XML encoding"))?
    } else {
        std::str::from_utf8(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes))
            .map_err(|_| anyhow::anyhow!("Invalid XML encoding"))?
            .to_owned()
    };
    ensure!(
        text.len() <= MAX_BYTES,
        "Decoded XML exceeds the 1 MiB limit"
    );
    ensure!(
        !text.contains("<!DOCTYPE") && !text.contains("<!ENTITY"),
        "XML DTDs and entities are unsupported"
    );
    Ok(text)
}
fn child_text<'a>(n: roxmltree::Node<'a, 'a>, tag: &str) -> Option<&'a str> {
    n.children()
        .find(|n| n.has_tag_name(tag))
        .and_then(|n| n.text())
}
fn xml_transport(n: roxmltree::Node<'_, '_>) -> bool {
    n.descendants().filter(|n| n.is_element()).any(|n| {
        n.attributes().any(|a| {
            (transport_key(a.name()) && !false_flag(a.value()))
                || (a.name() == "name"
                    && transport_key(a.value())
                    && !false_flag(n.attribute("value").or_else(|| n.text()).unwrap_or("true")))
        }) || (transport_key(n.tag_name().name())
            && !false_flag(n.text().unwrap_or("true"))
            && n.attribute("enabled").is_none_or(|s| !false_flag(s)))
    })
}
fn import_xml(
    format: ImportFormat,
    bytes: &[u8],
    report: &mut ImportReport,
    counts: &mut Counts,
) -> Result<()> {
    let xml = decode_xml(bytes)?;
    let doc = roxmltree::Document::parse_with_options(
        &xml,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 20_000,
            ..Default::default()
        },
    )
    .map_err(|_| anyhow::anyhow!("Invalid connector XML"))?;
    let tag = if format == ImportFormat::Navicat {
        "Connection"
    } else {
        "data-source"
    };
    let sources: Vec<_> = doc.descendants().filter(|n| n.has_tag_name(tag)).collect();
    ensure!(
        sources.len() <= MAX_SOURCES,
        "Connector import exceeds the 100 source limit"
    );
    for n in sources {
        let driver = if format == ImportFormat::Navicat {
            n.attribute("ConnType")
        } else {
            child_text(n, "driver-ref")
        };
        let Some(e) = driver.and_then(engine) else {
            counts.unsupported += 1;
            continue;
        };
        if xml_transport(n) {
            counts.transport += 1;
            continue;
        }
        if n.descendants().filter(|n| n.is_element()).any(|n| {
            n.attributes()
                .any(|a| a.name().to_ascii_lowercase().contains("socket") && !false_flag(a.value()))
                || (n.tag_name().name().to_ascii_lowercase().contains("socket")
                    && !false_flag(n.text().unwrap_or("true")))
        }) {
            counts.invalid += 1;
            continue;
        }
        if n.attributes()
            .any(|a| a.name().to_ascii_lowercase().contains("readonly") && !false_flag(a.value()))
            || n.descendants().any(|n| {
                (n.attribute("name").is_some_and(|s| {
                    s.replace(['-', '_'], "")
                        .to_ascii_lowercase()
                        .contains("readonly")
                }) || n
                    .tag_name()
                    .name()
                    .replace(['-', '_'], "")
                    .to_ascii_lowercase()
                    .contains("readonly"))
                    && !false_flag(n.attribute("value").or_else(|| n.text()).unwrap_or("true"))
            })
        {
            counts.readonly += 1;
            continue;
        }
        warning(
            report,
            "Foreign XML TLS settings were conservatively mapped to VerifyIdentity; review TLS and certificate settings before connecting.",
        );
        let mut p = SourceProfile {
            engine: e,
            ..SourceProfile::default()
        };
        if format == ImportFormat::Navicat {
            p.name = n
                .attribute("ConnectionName")
                .unwrap_or("Imported source")
                .into();
            let Some(host) = n.attribute("Host") else {
                counts.invalid += 1;
                continue;
            };
            p.host = host.into();
            let Ok(port) = n.attribute("Port").unwrap_or("3306").parse::<u16>() else {
                counts.invalid += 1;
                continue;
            };
            p.port = port;
            p.username = n.attribute("UserName").unwrap_or("root").into();
            p.database = n
                .attribute("Database")
                .filter(|s| !s.is_empty())
                .map(str::to_owned);
            p.ca_path = n
                .attribute("SSLCA")
                .filter(|s| !s.is_empty())
                .map(str::to_owned);
            p.ssl_client_cert = n
                .attribute("SSLCert")
                .filter(|s| !s.is_empty())
                .map(str::to_owned);
            p.ssl_client_key = n
                .attribute("SSLKey")
                .filter(|s| !s.is_empty())
                .map(str::to_owned);
        } else {
            p.name = n.attribute("name").unwrap_or("Imported source").into();
            p.username = child_text(n, "user-name").unwrap_or("root").into();
            let Some(url) = child_text(n, "jdbc-url") else {
                counts.invalid += 1;
                continue;
            };
            let Some((host, port, database)) = jdbc_target(url) else {
                counts.invalid += 1;
                continue;
            };
            p.host = host;
            p.port = port;
            p.database = database;
        }
        accept(p, report, counts);
    }
    Ok(())
}
// Strict subset only: do not discard URL parameters (including TLS policy), userinfo,
// percent escapes, multi-host targets, fragments, or driver-specific URL syntax.
fn jdbc_target(url: &str) -> Option<(String, u16, Option<String>)> {
    let rest = url
        .strip_prefix("jdbc:mysql://")
        .or_else(|| url.strip_prefix("jdbc:mariadb://"))?;
    if rest.chars().any(|c| {
        c.is_whitespace() || c.is_control() || matches!(c, '@' | '?' | '#' | '%' | ';' | ',' | '\\')
    }) {
        return None;
    }
    let (authority, db) = rest.split_once('/').unwrap_or((rest, ""));
    if db.contains('/') || authority.is_empty() {
        return None;
    }
    let (host, port) = if let Some(ip) = authority.strip_prefix('[') {
        let (host, tail) = ip.split_once(']')?;
        host.parse::<std::net::Ipv6Addr>().ok()?;
        let port = if tail.is_empty() {
            3306
        } else {
            tail.strip_prefix(':')?.parse().ok()?
        };
        (host, port)
    } else if let Some((host, port)) = authority.split_once(':') {
        (host, port.parse().ok()?)
    } else {
        (authority, 3306)
    };
    if port == 0 {
        return None;
    }
    Some((
        host.into(),
        port,
        if db.is_empty() { None } else { Some(db.into()) },
    ))
}
fn safe_endpoint(p: &SourceProfile) -> bool {
    match &p.endpoint {
        ConnectionMode::UrlOnly { url } => {
            let raw = url.strip_prefix("jdbc:").unwrap_or(url);
            !raw.chars()
                .any(|c| matches!(c, '@' | '?' | '#' | '%' | ';'))
                && p.connection_target().is_ok()
        }
        _ => true,
    }
}
pub fn export_bytes(profiles: &[SourceProfile]) -> Result<Vec<u8>> {
    ensure!(
        profiles.len() <= MAX_SOURCES,
        "Connector export exceeds the 100 source limit"
    );
    let mut sanitized = Vec::with_capacity(profiles.len());
    for profile in profiles {
        ensure!(
            matches!(profile.engine, DbEngine::MySql | DbEngine::MariaDb),
            "Only MySQL/MariaDB connectors can be exported"
        );
        ensure!(
            safe_endpoint(profile),
            "Cannot export a connection URL containing credentials or options; use separate connection fields"
        );
        ensure!(
            profile.validate().is_ok(),
            "Cannot export invalid connector settings"
        );
        let mut p = profile.clone();
        p.save_password = false;
        sanitized.push(p);
    }
    let bytes =
        serde_json::to_vec_pretty(&json!({"format": FORMAT, "version": 1, "profiles": sanitized}))
            .map_err(|_| anyhow::anyhow!("Cannot serialize connector export"))?;
    ensure!(
        bytes.len() <= MAX_BYTES,
        "Connector export exceeds the 1 MiB limit"
    );
    Ok(bytes)
}
fn check_path(path: &Path, new: bool) -> Result<()> {
    ensure!(
        !path.components().any(|c| matches!(c, Component::ParentDir)),
        "Connector paths must not contain parent-directory traversal"
    );
    for ancestor in path.ancestors().filter(|p| !p.as_os_str().is_empty()) {
        match fs::symlink_metadata(ancestor) {
            Ok(m) => {
                ensure!(
                    !m.file_type().is_symlink(),
                    "Connector paths must not contain symlinks"
                );
                if ancestor == path {
                    ensure!(
                        m.is_file() && !new,
                        "Connector export destination must be a new file; existing files are never overwritten"
                    );
                } else {
                    ensure!(m.is_dir(), "Connector parent path must be a directory");
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && ancestor == path && new => {}
            Err(_) => bail!("Cannot inspect connector path; check directory and file permissions"),
        }
    }
    Ok(())
}
pub fn read_import(path: &Path, format: ImportFormat) -> Result<ImportReport> {
    check_path(path, false)?;
    let file = File::open(path).map_err(|_| anyhow::anyhow!("Cannot open connector import"))?;
    ensure!(
        file.metadata()
            .map_err(|_| anyhow::anyhow!("Cannot inspect connector import"))?
            .len()
            <= MAX_BYTES as u64,
        "Connector import exceeds the 1 MiB limit"
    );
    let mut bytes = Vec::new();
    file.take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| anyhow::anyhow!("Cannot read connector import"))?;
    import_bytes(format, &bytes)
}
pub fn write_export(path: &Path, profiles: &[SourceProfile]) -> Result<()> {
    let bytes = export_bytes(profiles)?;
    check_path(path, true)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temporary = parent.join(format!(".dalan-connectors-{}.tmp", Uuid::new_v4()));
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|_| anyhow::anyhow!("Cannot create connector export temporary file"))?;
    let _cleanup = Cleanup(temporary.clone());
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| anyhow::anyhow!("Cannot write connector export"))?;
    drop(file);
    check_path(path, true)?;
    // Hard-link publishes the fully written file atomically and fails if the target
    // exists; unlike rename it cannot overwrite a concurrently created destination.
    fs::hard_link(&temporary, path).map_err(|_| anyhow::anyhow!("Cannot publish connector export; destination must be new and filesystem must support hard links"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dbx() -> &'static [u8] {
        br#"{"connections":[{"name":"Example","db_type":"mysql","host":"db.example.invalid","username":"demo","database":"sample","ssl":true,"password":"never-transfer"}]}"#
    }
    #[test]
    fn dbx_password_free_fresh_ids() {
        let a = import_bytes(ImportFormat::Dbx, dbx()).unwrap();
        let b = import_bytes(ImportFormat::Dbx, dbx()).unwrap();
        assert_ne!(a.profiles[0].id, b.profiles[0].id);
        assert!(!a.profiles[0].save_password);
        assert_eq!(a.profiles[0].tls, TlsMode::VerifyIdentity);
        assert!(
            !String::from_utf8(export_bytes(&a.profiles).unwrap())
                .unwrap()
                .contains("never-transfer")
        );
    }
    #[test]
    fn native_roundtrip() {
        let p = SourceProfile {
            save_password: true,
            ..SourceProfile::default()
        };
        let r = import_bytes(
            ImportFormat::ConnectorsList,
            &export_bytes(std::slice::from_ref(&p)).unwrap(),
        )
        .unwrap();
        assert_ne!(r.profiles[0].id, p.id);
        assert!(!r.profiles[0].save_password);
    }
    #[test]
    fn dbx_mixed_drivers_and_tls() {
        let r = import_bytes(ImportFormat::Dbx, br#"[{"db_type":"postgres"},{"db_type":"mariadb","host":"db.example.invalid","ssl":false}]"#).unwrap();
        assert_eq!(r.profiles.len(), 1);
        assert_eq!(r.profiles[0].tls, TlsMode::Disabled);
        assert!(r.warnings.iter().any(|s| s.contains("unsupported")));
    }
    #[test]
    fn encrypted_rejected() {
        assert!(
            import_bytes(ImportFormat::Dbx, br#"{"format":"dbx-encrypted"}"#)
                .unwrap_err()
                .to_string()
                .contains("Encrypted")
        );
    }
    #[test]
    fn navicat_fixture() {
        let r = import_bytes(ImportFormat::Navicat, br#"<Connections><Connection ConnType="MYSQL" ConnectionName="Example" Host="db.example.invalid" Port="3306" UserName="demo" Database="sample" Password="never-transfer" SSL="false"/></Connections>"#).unwrap();
        assert_eq!(r.profiles[0].tls, TlsMode::VerifyIdentity);
        assert_eq!(r.profiles[0].database.as_deref(), Some("sample"));
    }
    #[test]
    fn datagrip_fixture() {
        let r = import_bytes(ImportFormat::DataGrip, br#"<project><data-source name="Example"><driver-ref>mariadb</driver-ref><jdbc-url>jdbc:mysql://db.example.invalid:3307/sample</jdbc-url><user-name>demo</user-name></data-source></project>"#).unwrap();
        assert_eq!(r.profiles[0].port, 3307);
        assert_eq!(r.profiles[0].engine, DbEngine::MariaDb);
    }
    #[test]
    fn unsafe_jdbc_rejected() {
        for u in [
            "jdbc:mysql://demo:secret@db.invalid/sample",
            "jdbc:mysql://db.invalid/sample?useSSL=false",
            "jdbc:mysql://db.invalid/sample?password=secret",
            "jdbc:mysql://db.invalid/sample#secret",
            "jdbc:mysql://db.invalid/%73ample",
        ] {
            assert!(jdbc_target(u).is_none());
        }
        assert_eq!(
            jdbc_target("jdbc:mysql://[::1]:3306/sample").unwrap().0,
            "::1"
        );
    }
    #[test]
    fn foreign_transports_and_readonly_fail_closed() {
        for field in [
            r#""ssh":true"#,
            r#""ssh_config":{"enabled":true}"#,
            r#""proxy_host":"proxy.invalid""#,
            r#""read_only":true"#,
        ] {
            let input = format!(r#"[{{"db_type":"mysql","host":"db.invalid",{field}}}]"#);
            assert!(import_bytes(ImportFormat::Dbx, input.as_bytes()).is_err());
        }
        assert!(import_bytes(ImportFormat::Navicat, br#"<Connections><Connection ConnType="MYSQL" Host="db.invalid" SSH="true"/></Connections>"#).is_err());
        assert!(import_bytes(ImportFormat::Navicat,br#"<Connections><Connection ConnType="MYSQL" Host="db.invalid" HTTP="true" HTTPURL="https://tunnel.invalid"/></Connections>"#).is_err());
        assert!(import_bytes(ImportFormat::DataGrip,br#"<project><data-source><driver-ref>mysql</driver-ref><jdbc-url>jdbc:mysql://db.invalid/</jdbc-url><read-only>true</read-only></data-source></project>"#).is_err());
        assert!(import_bytes(ImportFormat::DataGrip, br#"<project><data-source><driver-ref>mysql</driver-ref><jdbc-url>jdbc:mysql://db.invalid/</jdbc-url><ssh-settings enabled="true"/></data-source></project>"#).is_err());
    }
    #[test]
    fn native_ssh_not_downgraded() {
        let p = SourceProfile {
            ssh_configuration_id: Some(Uuid::new_v4().to_string()),
            transport: dalan_drivers::Transport::Ssh {
                host: "jump.example.invalid".into(),
                port: 22,
                user: "reader".into(),
                identity_file: None,
                known_hosts_file: None,
                parse_config: false,
            },
            ..SourceProfile::default()
        };
        let report = import_bytes(
            ImportFormat::ConnectorsList,
            &export_bytes(std::slice::from_ref(&p)).unwrap(),
        )
        .unwrap();
        assert_eq!(report.profiles[0].transport, p.transport);
        assert_eq!(
            report.profiles[0].ssh_configuration_id,
            p.ssh_configuration_id
        );
        assert!(
            report
                .warnings
                .iter()
                .any(|w| w.contains("session definitions"))
        );
        let inline = SourceProfile {
            ssh_configuration_id: None,
            ..p
        };
        assert!(
            import_bytes(
                ImportFormat::ConnectorsList,
                &export_bytes(&[inline]).unwrap()
            )
            .is_err()
        );
    }
    #[test]
    fn xml_dtd_and_malformed_sanitized() {
        for input in [
            br#"<!DOCTYPE Connections [<!ENTITY x SYSTEM "file:///secret">]><Connections/>"#
                .as_slice(),
            b"<secret-password".as_slice(),
        ] {
            let err = import_bytes(ImportFormat::Navicat, input)
                .unwrap_err()
                .to_string();
            assert!(!err.contains("secret"));
        }
    }
    #[test]
    fn utf16_both_endians() {
        let xml = "<Connections><Connection ConnType=\"MYSQL\" Host=\"db.invalid\" ConnectionName=\"Example\"/></Connections>";
        for little in [true, false] {
            let mut bytes = if little {
                vec![0xff, 0xfe]
            } else {
                vec![0xfe, 0xff]
            };
            for unit in xml.encode_utf16() {
                bytes.extend(if little {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                });
            }
            assert_eq!(
                import_bytes(ImportFormat::Navicat, &bytes)
                    .unwrap()
                    .profiles
                    .len(),
                1
            );
        }
    }
    #[test]
    fn bounds_and_version() {
        assert!(import_bytes(ImportFormat::Dbx, &vec![b' '; MAX_BYTES + 1]).is_err());
        let items = vec![json!({"db_type":"mysql","host":"db.invalid"}); 101];
        assert!(import_bytes(ImportFormat::Dbx, &serde_json::to_vec(&items).unwrap()).is_err());
        assert!(
            import_bytes(
                ImportFormat::ConnectorsList,
                br#"{"format":"dalan-connectors","version":2,"profiles":[]}"#
            )
            .is_err()
        );
    }
    #[test]
    fn export_url_secrets_refused() {
        for url in [
            "mysql://demo:secret@db.invalid/sample",
            "mysql://db.invalid/sample?password=secret",
        ] {
            let p = SourceProfile {
                endpoint: ConnectionMode::UrlOnly { url: url.into() },
                ..SourceProfile::default()
            };
            assert!(export_bytes(&[p]).is_err());
        }
    }
    #[test]
    fn io_no_overwrite_and_roundtrip() {
        let dir = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("dalan-transfer-test-{}", Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("connectors.json");
        let p = SourceProfile::default();
        write_export(&path, std::slice::from_ref(&p)).unwrap();
        assert!(write_export(&path, &[p]).is_err());
        assert_eq!(
            read_import(&path, ImportFormat::ConnectorsList)
                .unwrap()
                .profiles
                .len(),
            1
        );
        fs::remove_dir_all(dir).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn io_symlinks_rejected() {
        use std::os::unix::fs::symlink;
        let dir = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("dalan-transfer-test-{}", Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("real.json"), dbx()).unwrap();
        symlink(dir.join("real.json"), dir.join("link.json")).unwrap();
        symlink(&dir, dir.join("linked-dir")).unwrap();
        assert!(read_import(&dir.join("link.json"), ImportFormat::Dbx).is_err());
        assert!(
            write_export(
                &dir.join("linked-dir/out.json"),
                &[SourceProfile::default()]
            )
            .is_err()
        );
        fs::remove_dir_all(dir).unwrap();
    }
}

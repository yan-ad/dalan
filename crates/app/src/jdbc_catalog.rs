//! Explicit, blocking JDBC discovery and installation. Call network methods on a worker thread,
//! never on the UI thread or a Tokio async worker. Nothing here starts Java or executes a JAR.
//! Coordinates and driver classes are locally curated; remote metadata only supplies versions.
use anyhow::{Result, bail, ensure};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    time::Duration,
};

pub const JETBRAINS_DISCOVERY_URL: &str = "https://www.jetbrains.com/datagrip/jdbc-drivers/";
/// Public JSON endpoint referenced by the DataGrip page's JavaScript. Archive URLs in this
/// metadata are deliberately NOT followed: they are neither Maven coordinates nor checksums.
pub const JETBRAINS_METADATA_URL: &str =
    "https://frameworks.jetbrains.com/jdbc-drivers/jdbc-drivers.json";
pub const MAVEN_SEARCH_URL: &str = "https://search.maven.org/solrsearch/select";
const MAVEN_REPOSITORY: &str = "https://repo.maven.apache.org/maven2";
const METADATA_LIMIT: u64 = 2 * 1024 * 1024;
const JAR_LIMIT: u64 = 64 * 1024 * 1024;
const INSTALL_LIMIT: u64 = 128 * 1024 * 1024;
const MANIFEST_LIMIT: u64 = 64 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    pub group: String,
    pub artifact: String,
    pub driver_class: String,
    pub url_prefix: String,
    pub homepage: String,
    pub description: String,
    pub versions: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstalledDriver {
    pub id: String,
    pub name: String,
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub driver_class: String,
    pub url_prefix: String,
    /// Absolute validated paths returned to callers. Manifests store only relative filenames.
    pub jars: Vec<PathBuf>,
    pub sha256: Vec<String>,
}

struct Spec {
    id: &'static str,
    name: &'static str,
    group: &'static str,
    artifact: &'static str,
    class: &'static str,
    prefix: &'static str,
    homepage: &'static str,
    description: &'static str,
}
const SPECS: &[Spec] = &[
    Spec {
        id: "h2",
        name: "H2",
        group: "com.h2database",
        artifact: "h2",
        class: "org.h2.Driver",
        prefix: "jdbc:h2:",
        homepage: "https://www.h2database.com/",
        description: "Embedded/server SQL database; self-contained JDBC JAR.",
    },
    Spec {
        id: "sqlite",
        name: "SQLite JDBC",
        group: "org.xerial",
        artifact: "sqlite-jdbc",
        class: "org.sqlite.JDBC",
        prefix: "jdbc:sqlite:",
        homepage: "https://github.com/xerial/sqlite-jdbc",
        description: "Xerial SQLite; includes native libraries. Installs pinned SLF4J API 1.7.36 and no-op logger 1.7.36.",
    },
    Spec {
        id: "duckdb",
        name: "DuckDB",
        group: "org.duckdb",
        artifact: "duckdb_jdbc",
        class: "org.duckdb.DuckDBDriver",
        prefix: "jdbc:duckdb:",
        homepage: "https://duckdb.org/",
        description: "Analytical SQL database; JDBC JAR includes native libraries.",
    },
    Spec {
        id: "mssql",
        name: "Microsoft SQL Server",
        group: "com.microsoft.sqlserver",
        artifact: "mssql-jdbc",
        class: "com.microsoft.sqlserver.jdbc.SQLServerDriver",
        prefix: "jdbc:sqlserver:",
        homepage: "https://learn.microsoft.com/sql/connect/jdbc/",
        description: "SQL Server JDBC; basic password authentication only. Optional Azure/integrated authentication dependencies are not installed.",
    },
    Spec {
        id: "oracle",
        name: "Oracle",
        group: "com.oracle.database.jdbc",
        artifact: "ojdbc11",
        class: "oracle.jdbc.OracleDriver",
        prefix: "jdbc:oracle:thin:",
        homepage: "https://www.oracle.com/database/technologies/appdev/jdbc.html",
        description: "Oracle thin JDBC for Java 11+. Wallet and optional authentication libraries are not installed.",
    },
    Spec {
        id: "mariadb",
        name: "MariaDB",
        group: "org.mariadb.jdbc",
        artifact: "mariadb-java-client",
        class: "org.mariadb.jdbc.Driver",
        prefix: "jdbc:mariadb:",
        homepage: "https://mariadb.com/kb/en/about-mariadb-connector-j/",
        description: "MariaDB JDBC; basic connections without optional authentication plugins.",
    },
    Spec {
        id: "mysql",
        name: "MySQL (optional JDBC)",
        group: "com.mysql",
        artifact: "mysql-connector-j",
        class: "com.mysql.cj.jdbc.Driver",
        prefix: "jdbc:mysql:",
        homepage: "https://dev.mysql.com/doc/connector-j/en/",
        description: "Optional alternative to native MySQL. Classic JDBC only, not the dependency-requiring X DevAPI.",
    },
];
fn spec(id: &str) -> Result<&'static Spec> {
    SPECS
        .iter()
        .find(|s| s.id == id)
        .ok_or_else(|| anyhow::anyhow!("Unsupported JDBC catalog entry"))
}
fn entry(s: &Spec) -> CatalogEntry {
    CatalogEntry {
        id: s.id.into(),
        name: s.name.into(),
        group: s.group.into(),
        artifact: s.artifact.into(),
        driver_class: s.class.into(),
        url_prefix: s.prefix.into(),
        homepage: s.homepage.into(),
        description: s.description.into(),
        versions: Vec::new(),
    }
}

/// No network access. Useful to show supported engines and trust information before refresh.
pub fn curated_catalog() -> Vec<CatalogEntry> {
    SPECS.iter().map(entry).collect()
}
/// Display these notices alongside catalog/trust UI. A repository checksum detects corruption,
/// not a malicious publisher or a compromise of Maven Central. User approval is still required.
pub fn catalog_notices() -> &'static [&'static str] {
    &[
        "Installing third-party JDBC code requires explicit trust. Installation does not execute it; connecting will execute it in Java.",
        "Versions come from Maven Central search (up to 50 per engine). JetBrains public JSON is discovery-only; its ZIP archives are not installed.",
        "Only curated basic JDBC drivers and explicitly pinned SQLite logging dependencies are supported. General Maven dependency resolution is not supported.",
        "A SHA-256 sidecar must be available on Maven Central or installation fails closed. Checksums are not publisher signatures.",
    ]
}
fn client() -> Result<Client> {
    Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(45))
        .user_agent("Dalan-JDBC-Catalog/1")
        .build()
        .map_err(|_| anyhow::anyhow!("Unable to initialize JDBC HTTPS client"))
}
fn approved_url(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port_or_known_default() == Some(443)
        && url.fragment().is_none()
        && matches!(
            url.host_str(),
            Some("search.maven.org" | "repo.maven.apache.org" | "frameworks.jetbrains.com")
        )
}
fn download(client: &Client, url: reqwest::Url, limit: u64) -> Result<Vec<u8>> {
    ensure!(approved_url(&url), "Unapproved JDBC download endpoint");
    let response = client
        .get(url)
        .send()
        .map_err(|_| anyhow::anyhow!("JDBC HTTPS request failed"))?;
    ensure!(
        response.status().is_success(),
        "JDBC HTTPS endpoint returned status {}",
        response.status().as_u16()
    );
    ensure!(
        response.content_length().unwrap_or(0) <= limit,
        "JDBC download exceeds size limit"
    );
    read_bounded(response, limit)
}
fn read_bounded(reader: impl Read, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| anyhow::anyhow!("Unable to read JDBC data"))?;
    ensure!(bytes.len() as u64 <= limit, "JDBC data exceeds size limit");
    Ok(bytes)
}
#[derive(Deserialize)]
struct Search {
    response: SearchResponse,
}
#[derive(Deserialize)]
struct SearchResponse {
    docs: Vec<SearchDoc>,
}
#[derive(Deserialize)]
struct SearchDoc {
    g: String,
    a: String,
    v: String,
    p: String,
    #[serde(default)]
    ec: Vec<String>,
}
fn parse_versions(bytes: &[u8], s: &Spec) -> Result<Vec<String>> {
    ensure!(
        bytes.len() as u64 <= METADATA_LIMIT,
        "JDBC metadata exceeds size limit"
    );
    let data: Search = serde_json::from_slice(bytes)
        .map_err(|_| anyhow::anyhow!("Invalid Maven search metadata"))?;
    ensure!(data.response.docs.len() <= 50, "Too many Maven versions");
    let mut versions = Vec::new();
    for doc in data.response.docs {
        ensure!(
            doc.g == s.group && doc.a == s.artifact,
            "Unexpected Maven coordinates"
        );
        validate_component(&doc.v)?;
        if doc.p == "jar" && doc.ec.iter().any(|e| e == ".jar") && !versions.contains(&doc.v) {
            // Only Java 11+ builds of Microsoft's multi-target driver are advertised.
            if s.id != "mssql" || doc.v.ends_with(".jre11") {
                versions.push(doc.v);
            }
        }
    }
    ensure!(!versions.is_empty(), "No supported Maven versions returned");
    Ok(versions)
}
#[derive(Deserialize)]
struct Discovery {
    name: String,
}
/// Explicit blocking refresh; no constructor or load operation performs network access.
/// A Maven error fails refresh rather than inventing versions. JetBrains discovery is optional
/// and failure is reported in every entry's description; remote classes/URLs are never trusted.
pub fn fetch_catalog() -> Result<Vec<CatalogEntry>> {
    let client = client()?;
    let discovery = download(
        &client,
        reqwest::Url::parse(JETBRAINS_METADATA_URL)?,
        METADATA_LIMIT,
    )
    .and_then(|bytes| {
        serde_json::from_slice::<Vec<Discovery>>(&bytes)
            .map_err(|_| anyhow::anyhow!("Invalid JetBrains discovery metadata"))
    })
    .ok()
    .filter(|items| items.len() <= 1024);
    let mut catalog = curated_catalog();
    for (item, s) in catalog.iter_mut().zip(SPECS) {
        let mut url = reqwest::Url::parse(MAVEN_SEARCH_URL)?;
        url.query_pairs_mut()
            .append_pair("q", &format!("g:\"{}\" AND a:\"{}\"", s.group, s.artifact))
            .append_pair("core", "gav")
            .append_pair("rows", "50")
            .append_pair("wt", "json")
            .append_pair("sort", "timestamp desc");
        item.versions = parse_versions(&download(&client, url, METADATA_LIMIT)?, s)?;
        if discovery.is_none() {
            item.description.push_str(" JetBrains discovery metadata unavailable; using curated coordinates and Maven versions only.");
        } else if discovery
            .as_ref()
            .is_some_and(|items| items.iter().any(|d| d.name == s.name || d.name == s.id))
        {
            item.description
                .push_str(" Also listed in JetBrains JDBC discovery metadata.");
        }
    }
    Ok(catalog)
}

#[derive(Clone, Debug)]
pub struct CatalogRepository {
    root: PathBuf,
}
impl CatalogRepository {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
    pub fn default_path() -> Result<PathBuf> {
        Ok(crate::source_store::default_path()?.with_file_name("jdbc"))
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn refresh(&self) -> Result<Vec<CatalogEntry>> {
        fetch_catalog()
    }
    /// Fail closed on malformed/tampered manifests, paths, archives or hashes. Hidden staging
    /// directories left by interrupted installs are ignored and never returned as drivers.
    pub fn load(&self) -> Result<Vec<InstalledDriver>> {
        check_path(&self.root)?;
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        ensure!(self.root.is_dir(), "JDBC storage is not a directory");
        let root = fs::canonicalize(&self.root)
            .map_err(|_| anyhow::anyhow!("Unable to resolve JDBC storage"))?;
        let mut installed = Vec::new();
        for id_dir in children(&root)? {
            let id = filename(&id_dir)?;
            if id.starts_with('.') {
                continue;
            }
            spec(&id)?;
            ensure!(id_dir.is_dir(), "Invalid JDBC driver directory");
            for version_dir in children(&id_dir)? {
                let version = filename(&version_dir)?;
                if version.starts_with('.') {
                    continue;
                }
                validate_component(&version)?;
                ensure!(version_dir.is_dir(), "Invalid JDBC version directory");
                installed.push(load_driver(&version_dir, &id, &version)?);
                ensure!(installed.len() <= 1024, "Too many installed JDBC drivers");
            }
        }
        installed.sort_by(|a, b| (&a.id, &a.version).cmp(&(&b.id, &b.version)));
        Ok(installed)
    }
    /// Downloads to a private staging directory, verifies every sidecar, then atomically
    /// publishes a complete per-version manifest. Never overwrites an existing installation.
    pub fn install(&self, entry: &CatalogEntry, version: &str) -> Result<InstalledDriver> {
        let s = validate_entry(entry)?;
        validate_component(version)?;
        ensure!(
            entry.versions.len() <= 50 && entry.versions.iter().any(|v| v == version),
            "Select a discovered JDBC version"
        );
        if s.id == "mssql" {
            ensure!(
                version.ends_with(".jre11"),
                "Unsupported SQL Server Java target"
            );
        }
        prepare_dir(&self.root)?;
        let root = fs::canonicalize(&self.root)
            .map_err(|_| anyhow::anyhow!("Unable to resolve JDBC storage"))?;
        let id_dir = root.join(s.id);
        prepare_dir(&id_dir)?;
        let target = id_dir.join(version);
        let lock_path = id_dir.join(format!(".{version}.lock"));
        let _lock = CleanupFile::create(lock_path)?;
        ensure!(
            !target
                .try_exists()
                .map_err(|_| anyhow::anyhow!("Unable to check JDBC installation"))?,
            "JDBC version is already installed"
        );
        check_path(&target)?;
        let stage = id_dir.join(format!(".stage-{}", uuid::Uuid::new_v4()));
        create_private_dir(&stage)?;
        let cleanup = CleanupDir(stage.clone());
        let client = client()?;
        let mut driver = InstalledDriver {
            id: s.id.into(),
            name: s.name.into(),
            group: s.group.into(),
            artifact: s.artifact.into(),
            version: version.into(),
            driver_class: s.class.into(),
            url_prefix: s.prefix.into(),
            jars: Vec::new(),
            sha256: Vec::new(),
        };
        let mut total = 0;
        for (group, artifact, v) in coordinates(s, version) {
            let name = jar_name(artifact, v);
            let base = format!(
                "{MAVEN_REPOSITORY}/{}/{artifact}/{v}/{name}",
                group.replace('.', "/")
            );
            let expected = parse_checksum(&download(
                &client,
                reqwest::Url::parse(&format!("{base}.sha256"))?,
                256,
            )?)?;
            let jar = download(&client, reqwest::Url::parse(&base)?, JAR_LIMIT)?;
            total += jar.len() as u64;
            ensure!(
                total <= INSTALL_LIMIT,
                "JDBC installation exceeds total size limit"
            );
            verify_jar(&jar, &expected)?;
            private_write(&stage.join(&name), &jar)?;
            driver.jars.push(PathBuf::from(name));
            driver.sha256.push(expected);
        }
        let manifest = serde_json::to_vec_pretty(&driver)
            .map_err(|_| anyhow::anyhow!("Unable to encode JDBC manifest"))?;
        private_write(&stage.join("manifest.json"), &manifest)?;
        // The create_new lock serializes this repository's writers. The nonempty destination
        // check plus directory rename also prevents replacing an existing completed install.
        ensure!(
            !target
                .try_exists()
                .map_err(|_| anyhow::anyhow!("Unable to check JDBC installation"))?,
            "JDBC version is already installed"
        );
        fs::rename(&stage, &target)
            .map_err(|_| anyhow::anyhow!("Unable to publish JDBC installation"))?;
        drop(cleanup);
        load_driver(&target, s.id, version)
    }
}
fn coordinates<'a>(s: &'a Spec, version: &'a str) -> Vec<(&'a str, &'a str, &'a str)> {
    let mut result = vec![(s.group, s.artifact, version)];
    if s.id == "sqlite" {
        result.extend([
            ("org.slf4j", "slf4j-api", "1.7.36"),
            ("org.slf4j", "slf4j-nop", "1.7.36"),
        ]);
    }
    result
}
fn validate_entry(e: &CatalogEntry) -> Result<&'static Spec> {
    let s = spec(&e.id)?;
    ensure!(
        e.group == s.group
            && e.artifact == s.artifact
            && e.driver_class == s.class
            && e.url_prefix == s.prefix
            && e.name == s.name
            && e.homepage == s.homepage,
        "JDBC catalog entry does not match trusted coordinates"
    );
    Ok(s)
}
fn validate_component(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 128
            && value.as_bytes()[0].is_ascii_alphanumeric()
            && value
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'_'))
            && !value.contains("..")
            && !value.ends_with('.'),
        "Invalid JDBC coordinate or version"
    );
    Ok(())
}
fn jar_name(artifact: &str, version: &str) -> String {
    format!("{artifact}-{version}.jar")
}
fn parse_checksum(bytes: &[u8]) -> Result<String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| anyhow::anyhow!("Invalid JDBC SHA-256 sidecar"))?
        .trim();
    ensure!(
        text.len() == 64 && text.bytes().all(|c| c.is_ascii_hexdigit()),
        "Invalid JDBC SHA-256 sidecar"
    );
    Ok(text.to_ascii_lowercase())
}
fn verify_jar(bytes: &[u8], expected: &str) -> Result<()> {
    ensure!(
        bytes.len() as u64 <= JAR_LIMIT && bytes.starts_with(b"PK\x03\x04"),
        "Invalid JDBC JAR archive"
    );
    ensure!(
        format!("{:x}", Sha256::digest(bytes)) == expected,
        "JDBC JAR SHA-256 mismatch"
    );
    Ok(())
}
fn load_driver(dir: &Path, id: &str, version: &str) -> Result<InstalledDriver> {
    let bytes = read_file(&dir.join("manifest.json"), MANIFEST_LIMIT)?;
    let mut d: InstalledDriver = serde_json::from_slice(&bytes)
        .map_err(|_| anyhow::anyhow!("Invalid JDBC install manifest"))?;
    let s = spec(id)?;
    if id == "mssql" {
        ensure!(
            version.ends_with(".jre11"),
            "Unsupported SQL Server Java target"
        );
    }
    ensure!(
        d.id == id
            && d.version == version
            && d.name == s.name
            && d.group == s.group
            && d.artifact == s.artifact
            && d.driver_class == s.class
            && d.url_prefix == s.prefix,
        "JDBC manifest does not match trusted driver"
    );
    let coords = coordinates(s, version);
    ensure!(
        d.jars.len() == coords.len() && d.sha256.len() == coords.len(),
        "Invalid JDBC manifest artifact count"
    );
    let mut total = 0;
    for (i, (_, artifact, v)) in coords.iter().enumerate() {
        let name = jar_name(artifact, v);
        ensure!(
            d.jars[i] == Path::new(&name),
            "Invalid JDBC manifest JAR path"
        );
        let checksum = parse_checksum(d.sha256[i].as_bytes())?;
        let path = dir.join(&name);
        let bytes = read_file(&path, JAR_LIMIT)?;
        total += bytes.len() as u64;
        ensure!(
            total <= INSTALL_LIMIT,
            "JDBC installation exceeds total size limit"
        );
        verify_jar(&bytes, &checksum)?;
        d.jars[i] = path;
        d.sha256[i] = checksum;
    }
    Ok(d)
}
fn read_file(path: &Path, limit: u64) -> Result<Vec<u8>> {
    check_path(path)?;
    ensure!(
        fs::symlink_metadata(path)
            .map_err(|_| anyhow::anyhow!("Missing JDBC file"))?
            .is_file(),
        "Invalid JDBC file type"
    );
    read_bounded(
        File::open(path).map_err(|_| anyhow::anyhow!("Unable to open JDBC file"))?,
        limit,
    )
}
fn filename(path: &Path) -> Result<String> {
    path.file_name()
        .and_then(|s| s.to_str())
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("Invalid JDBC storage name"))
}
fn children(path: &Path) -> Result<Vec<PathBuf>> {
    check_path(path)?;
    let mut result = Vec::new();
    for item in fs::read_dir(path).map_err(|_| anyhow::anyhow!("Unable to list JDBC storage"))? {
        let path = item
            .map_err(|_| anyhow::anyhow!("Unable to list JDBC storage"))?
            .path();
        check_path(&path)?;
        result.push(path);
        ensure!(result.len() <= 1024, "Too many JDBC storage entries");
    }
    Ok(result)
}
/// Reject symlinks in every existing component, including the configured root's ancestors.
fn check_path(path: &Path) -> Result<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        ensure!(
            !matches!(component, Component::ParentDir),
            "JDBC storage traversal is not allowed"
        );
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(meta) => ensure!(
                !meta.file_type().is_symlink(),
                "JDBC storage symlinks are not allowed"
            ),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => (),
            Err(_) => bail!("Unable to inspect JDBC storage"),
        }
    }
    Ok(())
}
fn prepare_dir(path: &Path) -> Result<()> {
    ensure!(!path.as_os_str().is_empty(), "Empty JDBC storage path");
    check_path(path)?;
    if !path.exists() {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            prepare_parent(parent)?;
        }
        match create_private_dir(path) {
            Ok(()) => (),
            Err(err) if path.is_dir() => {
                check_path(path)?;
                drop(err);
            }
            Err(err) => return Err(err),
        }
    }
    ensure!(path.is_dir(), "JDBC storage is not a directory");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|_| anyhow::anyhow!("Unable to protect JDBC directory"))?;
    }
    Ok(())
}
fn prepare_parent(path: &Path) -> Result<()> {
    check_path(path)?;
    if path.exists() {
        ensure!(path.is_dir(), "Invalid JDBC storage parent");
        return Ok(());
    }
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        prepare_parent(parent)?;
    }
    if create_private_dir(path).is_err() {
        check_path(path)?;
        ensure!(path.is_dir(), "Unable to create JDBC storage parent");
    }
    Ok(())
}
fn create_private_dir(path: &Path) -> Result<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(path)
        .map_err(|_| anyhow::anyhow!("Unable to create private JDBC directory"))
}
fn private_file(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .map_err(|_| anyhow::anyhow!("JDBC file exists or cannot be created"))
}
fn private_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = private_file(path)?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| anyhow::anyhow!("Unable to persist JDBC file"))
}
struct CleanupDir(PathBuf);
impl Drop for CleanupDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
struct CleanupFile(PathBuf);
impl CleanupFile {
    fn create(path: PathBuf) -> Result<Self> {
        private_file(&path)?;
        Ok(Self(path))
    }
}
impl Drop for CleanupFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn temp() -> CleanupDir {
        // canonicalize avoids macOS's /var -> /private/var system symlink.
        CleanupDir(
            fs::canonicalize(std::env::temp_dir())
                .unwrap()
                .join(format!("dalan-jdbc-test-{}", uuid::Uuid::new_v4())),
        )
    }
    fn fixture(dir: &Path) -> InstalledDriver {
        prepare_dir(dir).unwrap();
        let s = spec("h2").unwrap();
        let jar = b"PK\x03\x04synthetic test archive; never execute";
        let d = InstalledDriver {
            id: s.id.into(),
            name: s.name.into(),
            group: s.group.into(),
            artifact: s.artifact.into(),
            version: "2.3.232".into(),
            driver_class: s.class.into(),
            url_prefix: s.prefix.into(),
            jars: vec!["h2-2.3.232.jar".into()],
            sha256: vec![format!("{:x}", Sha256::digest(jar))],
        };
        private_write(&dir.join(&d.jars[0]), jar).unwrap();
        private_write(&dir.join("manifest.json"), &serde_json::to_vec(&d).unwrap()).unwrap();
        d
    }
    #[test]
    fn parses_actual_maven_shape_and_rejects_coordinate_injection() {
        let valid = br#"{"response":{"docs":[{"g":"com.h2database","a":"h2","v":"2.3.232","p":"jar","ec":[".jar",".pom"]}]}}"#;
        assert_eq!(
            parse_versions(valid, spec("h2").unwrap()).unwrap(),
            ["2.3.232"]
        );
        assert!(parse_versions(valid, spec("sqlite").unwrap()).is_err());
        assert!(parse_versions(br#"{"response":{"docs":[]}}"#, spec("h2").unwrap()).is_err());
    }
    #[test]
    fn rejects_paths_urls_unknown_drivers_and_classes() {
        for value in [
            "",
            "..",
            "../1",
            "a/b",
            "a\\b",
            "https:x",
            "x?token=secret",
            "%2f",
            ".1",
            "a..b",
            "a.",
        ] {
            assert!(validate_component(value).is_err(), "{value}");
        }
        for value in ["1.2.3", "12.8.1.jre11", "1.0-rc_1"] {
            validate_component(value).unwrap();
        }
        for url in [
            "http://repo.maven.apache.org/x",
            "https://repo.maven.apache.org.evil/x",
            "https://user:secret@repo.maven.apache.org/x",
            "https://repo.maven.apache.org:444/x",
        ] {
            assert!(!approved_url(&reqwest::Url::parse(url).unwrap()));
        }
        let mut e = curated_catalog().remove(0);
        e.driver_class = "attacker.Driver".into();
        assert!(validate_entry(&e).is_err());
        assert!(spec("postgres").is_err());
    }
    #[test]
    fn hashes_archives_limits_and_sidecars_fail_closed() {
        let jar = b"PK\x03\x04fixture";
        let digest = format!("{:x}", Sha256::digest(jar));
        verify_jar(jar, &digest).unwrap();
        assert!(verify_jar(b"<html>error", &digest).is_err());
        assert!(verify_jar(jar, &"0".repeat(64)).is_err());
        assert!(parse_checksum(b"a checksum filename").is_err());
        assert!(read_bounded(&b"12345"[..], 4).is_err());
    }
    #[test]
    fn load_verifies_paths_hashes_and_private_no_overwrite_files() {
        let root = temp();
        let dir = root.0.join("h2/2.3.232");
        let mut d = fixture(&dir);
        let repo = CatalogRepository::new(root.0.clone());
        let result = repo.load().unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].jars[0].is_absolute());
        assert!(private_write(&dir.join(&d.jars[0]), b"replace").is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(dir.join("manifest.json"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        d.jars[0] = "../../outside.jar".into();
        fs::write(dir.join("manifest.json"), serde_json::to_vec(&d).unwrap()).unwrap();
        assert!(repo.load().is_err());
        d.jars[0] = "h2-2.3.232.jar".into();
        fs::write(dir.join("manifest.json"), serde_json::to_vec(&d).unwrap()).unwrap();
        fs::write(dir.join(&d.jars[0]), b"PK\x03\x04tampered").unwrap();
        assert!(repo.load().is_err());
    }
    #[test]
    fn lock_serializes_writers_and_install_rejects_existing_without_network() {
        let root = temp();
        let dir = root.0.join("h2/2.3.232");
        fixture(&dir);
        let path = root.0.join("h2/.test.lock");
        let lock = CleanupFile::create(path.clone()).unwrap();
        assert!(CleanupFile::create(path.clone()).is_err());
        drop(lock);
        assert!(!path.exists());
        let mut e = entry(spec("h2").unwrap());
        e.versions.push("2.3.232".into());
        assert!(
            CatalogRepository::new(root.0.clone())
                .install(&e, "2.3.232")
                .is_err()
        );
        assert_eq!(
            CatalogRepository::new(root.0.clone()).load().unwrap().len(),
            1
        );
    }
    #[cfg(unix)]
    #[test]
    fn rejects_symlink_root_and_jars() {
        use std::os::unix::fs::symlink;
        let root = temp();
        let dir = root.0.join("h2/2.3.232");
        let d = fixture(&dir);
        let link = root.0.join("link");
        symlink(&dir, &link).unwrap();
        assert!(CatalogRepository::new(link.clone()).load().is_err());
        fs::remove_file(link).unwrap();
        fs::remove_file(dir.join(&d.jars[0])).unwrap();
        symlink("manifest.json", dir.join(&d.jars[0])).unwrap();
        assert!(CatalogRepository::new(root.0.clone()).load().is_err());
    }
    #[test]
    fn missing_root_load_is_offline_and_does_not_create_storage() {
        let root = temp();
        assert!(
            CatalogRepository::new(root.0.clone())
                .load()
                .unwrap()
                .is_empty()
        );
        assert!(!root.0.exists());
        assert_eq!(
            CatalogRepository::default_path().unwrap(),
            crate::source_store::default_path()
                .unwrap()
                .with_file_name("jdbc")
        );
    }
}

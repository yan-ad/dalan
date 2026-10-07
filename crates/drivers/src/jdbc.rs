//! Opt-in Java 17 JDBC subprocess bridge. A driver JAR is executable third-party
//! code, NOT a sandbox. Timeouts kill our child only (not driver-created children).
//! JDBC/wire allocations inside drivers are not bounded by the preview budgets.
use crate::{
    BrowseRequest, CatalogSnapshot, ColumnInfo, ConnectionReport, QueryRequest, QueryResult,
    TableInfo, TablePage,
    sources::{Authentication, ConnectionMode, DbEngine, SourceProfile, TlsMode, Transport},
};
use anyhow::{Result, anyhow, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const REQUEST_CAP: usize = 1024 * 1024;
const RESPONSE_CAP: usize = 2 * 1024 * 1024;
const JAR_CAP: u64 = 64 * 1024 * 1024;
const TOTAL_JAR_CAP: u64 = 128 * 1024 * 1024;
const BRIDGE: &str = include_str!("../jvm/DalanJdbcBridge.java");
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeInfo {
    pub major: u32,
    pub version: String,
}

struct PrivateDir(PathBuf);
impl PrivateDir {
    fn new() -> Result<Self> {
        let base = std::env::temp_dir()
            .canonicalize()
            .map_err(|_| anyhow!("Cannot resolve JDBC runtime directory"))?;
        let path = base.join(format!("dalan-jdbc-v1-{}", uuid::Uuid::new_v4()));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(&path)
            .map_err(|_| anyhow!("Cannot create private JDBC runtime directory"))?;
        Ok(Self(path))
    }
}
impl Drop for PrivateDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn java_executable(value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    ensure!(
        path.is_absolute(),
        "Java executable must be an explicit absolute path"
    );
    let canonical = path
        .canonicalize()
        .map_err(|_| anyhow!("Java executable is unavailable"))?;
    ensure!(canonical.is_file(), "Java executable must be a file");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        ensure!(
            canonical.metadata()?.permissions().mode() & 0o111 != 0,
            "Java path is not executable"
        );
    }
    Ok(canonical)
}
async fn read_bounded(reader: impl tokio::io::AsyncRead + Unpin, cap: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(cap as u64 + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|_| anyhow!("JDBC runtime output failed"))?;
    ensure!(bytes.len() <= cap, "JDBC runtime response exceeds limit");
    Ok(bytes)
}
/// Checks only the explicitly supplied executable; never discovers or installs Java.
pub async fn runtime_info(java_path: &str) -> Result<RuntimeInfo> {
    let executable = java_executable(java_path)?;
    let run = async {
        let mut child = tokio::process::Command::new(executable)
            .arg("-version")
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| anyhow!("Cannot start Java runtime"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Missing runtime output"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("Missing runtime output"))?;
        let (out, err, status) = tokio::try_join!(
            read_bounded(stdout, 8192),
            read_bounded(stderr, 8192),
            async {
                child
                    .wait()
                    .await
                    .map_err(|_| anyhow!("Java runtime failed"))
            }
        )?;
        ensure!(status.success(), "Java runtime version check failed");
        let text = format!(
            "{}\n{}",
            String::from_utf8_lossy(&out),
            String::from_utf8_lossy(&err)
        );
        let version = text
            .lines()
            .find_map(|line| {
                if !(line.starts_with("java version ") || line.starts_with("openjdk version ")) {
                    return None;
                }
                line.split('"').nth(1)
            })
            .ok_or_else(|| anyhow!("Unsupported Java version output"))?;
        ensure!(
            version.len() <= 128
                && version
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || ".-_+".contains(c)),
            "Invalid Java version"
        );
        let major: u32 = version
            .split('.')
            .next()
            .unwrap_or("")
            .parse()
            .map_err(|_| anyhow!("Invalid Java version"))?;
        ensure!(major >= 17, "Java 17 or newer is required");
        Ok(RuntimeInfo {
            major,
            version: version.to_owned(),
        })
    };
    tokio::time::timeout(Duration::from_secs(5), run)
        .await
        .map_err(|_| anyhow!("Java runtime check timed out"))?
}
/// A discovered local runtime, not a managed/downloaded JVM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedJava {
    pub executable: String,
    pub runtime: RuntimeInfo,
}

#[cfg(any(target_os = "macos", test))]
fn executable_from_java_home(output: &[u8]) -> Result<PathBuf> {
    let text = std::str::from_utf8(output).map_err(|_| anyhow!("Invalid JDK locator output"))?;
    let home = text.trim();
    ensure!(
        !home.is_empty() && home.len() <= 4096 && !home.chars().any(char::is_control),
        "Invalid JDK locator output"
    );
    ensure!(
        Path::new(home).is_absolute(),
        "JDK locator requires an absolute home"
    );
    java_executable(&Path::new(home).join("bin/java").to_string_lossy())
}

/// macOS's system locator only. No shell, PATH/JAVA_HOME lookup, installation,
/// directory crawling or driver JAR execution. Both children have deadlines.
pub async fn detect_local_java() -> Result<DetectedJava> {
    #[cfg(target_os = "macos")]
    {
        detect_with_locator(Path::new("/usr/libexec/java_home")).await
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(anyhow!(
            "Automatic JDK detection is available on macOS only"
        ))
    }
}

#[cfg(any(target_os = "macos", test))]
async fn detect_with_locator(locator: &Path) -> Result<DetectedJava> {
    let locate = async {
        let mut child = tokio::process::Command::new(locator)
            .args(["-F", "-v", "17+"])
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| anyhow!("Cannot start system JDK locator"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Missing JDK locator output"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("Missing JDK locator output"))?;
        let (out, _, status) = tokio::try_join!(
            read_bounded(stdout, 4096),
            read_bounded(stderr, 4096),
            async {
                child
                    .wait()
                    .await
                    .map_err(|_| anyhow!("JDK locator failed"))
            }
        )?;
        ensure!(status.success(), "No matching installed JDK found");
        executable_from_java_home(&out)
    };
    let executable = tokio::time::timeout(Duration::from_secs(5), locate)
        .await
        .map_err(|_| anyhow!("JDK detection timed out"))??;
    let executable = executable
        .to_str()
        .ok_or_else(|| anyhow!("Invalid JDK executable path"))?
        .to_owned();
    let runtime = runtime_info(&executable).await?;
    Ok(DetectedJava {
        executable,
        runtime,
    })
}

fn checked_jar(path: &str, expected: &str, destination: &Path) -> Result<u64> {
    let path = Path::new(path);
    ensure!(path.is_absolute(), "JDBC JAR path must be absolute");
    let mut part = PathBuf::new();
    for component in path.components() {
        ensure!(
            !matches!(component, std::path::Component::ParentDir),
            "JDBC JAR path may not contain parent traversal"
        );
        part.push(component);
        ensure!(
            !std::fs::symlink_metadata(&part)
                .map_err(|_| anyhow!("JDBC JAR unavailable"))?
                .file_type()
                .is_symlink(),
            "JDBC JAR symlink paths are forbidden"
        );
    }
    let canonical = path
        .canonicalize()
        .map_err(|_| anyhow!("JDBC JAR unavailable"))?;
    let mut file = std::fs::File::open(canonical).map_err(|_| anyhow!("JDBC JAR unavailable"))?;
    let meta = file
        .metadata()
        .map_err(|_| anyhow!("JDBC JAR unavailable"))?;
    ensure!(
        meta.is_file() && meta.len() <= JAR_CAP,
        "JDBC JAR exceeds file limit"
    );
    let mut bytes = Vec::new();
    file.by_ref()
        .take(JAR_CAP + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| anyhow!("Cannot read JDBC JAR"))?;
    ensure!(bytes.len() as u64 <= JAR_CAP, "JDBC JAR exceeds file limit");
    ensure!(
        expected.len() == 64 && expected.bytes().all(|c| c.is_ascii_hexdigit()),
        "Invalid JDBC JAR digest"
    );
    let actual = format!("{:x}", Sha256::digest(&bytes));
    ensure!(
        actual.eq_ignore_ascii_case(expected),
        "JDBC JAR integrity check failed"
    );
    // Execute the verified snapshot, not a mutable original pathname.
    std::fs::write(destination, &bytes).map_err(|_| anyhow!("Cannot stage JDBC JAR"))?;
    Ok(bytes.len() as u64)
}
fn validate_profile(profile: &SourceProfile) -> Result<()> {
    ensure!(profile.engine == DbEngine::Jdbc, "JDBC engine is required");
    ensure!(
        matches!(profile.transport, Transport::Direct)
            && matches!(profile.endpoint, ConnectionMode::Default),
        "JDBC requires direct transport and its configured JDBC URL"
    );
    ensure!(
        profile.tls == TlsMode::Disabled
            && profile.ca_path.is_none()
            && profile.ssl_client_cert.is_none()
            && profile.ssl_client_key.is_none(),
        "JDBC TLS is owned by the vendor URL; source TLS options are unsupported"
    );
    profile.options.validate()?;
    let options = profile
        .jdbc
        .as_ref()
        .ok_or_else(|| anyhow!("Missing JDBC options"))?;
    ensure!(
        !options.driver_id.is_empty() && options.driver_id.len() <= 128,
        "Invalid JDBC driver identity"
    );
    ensure!(
        !options.jars.is_empty() && options.jars.len() <= 16,
        "JDBC requires 1 through 16 configured JARs"
    );
    ensure!(
        options.driver_class.len() <= 512
            && options.driver_class.split('.').all(|s| !s.is_empty()
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')),
        "Invalid JDBC driver class"
    );
    crate::sources::validate_jdbc_url(&options.url)?;
    ensure!(
        options.url.starts_with("jdbc:")
            && options.url.len() <= 65536
            && !options.url.chars().any(char::is_control),
        "Invalid JDBC URL"
    );
    Ok(())
}
#[derive(Deserialize)]
struct Response {
    ok: bool,
    #[serde(default)]
    errorcategory: String,
    report: Option<ConnectionReport>,
    catalog: Option<CatalogSnapshot>,
    tables: Option<Vec<TableInfo>>,
    columns: Option<Vec<ColumnInfo>>,
    result: Option<QueryResult>,
    page: Option<TablePage>,
}
async fn call(
    profile: &SourceProfile,
    password: &str,
    operation: &str,
    fields: Value,
) -> Result<Response> {
    validate_profile(profile)?;
    let options = profile
        .jdbc
        .as_ref()
        .ok_or_else(|| anyhow!("Missing JDBC options"))?;
    // Validation, hashes and request construction precede any subprocess execution.
    let directory = PrivateDir::new()?;
    let mut jars = Vec::new();
    let mut total = 0;
    for (index, jar) in options.jars.iter().enumerate() {
        let dest = directory.0.join(format!("driver-{index}.jar"));
        total += checked_jar(&jar.path, &jar.sha256, &dest)?;
        ensure!(total <= TOTAL_JAR_CAP, "JDBC JARs exceed total file limit");
        jars.push(dest);
    }
    let source = directory.0.join("DalanJdbcBridge.java");
    std::fs::write(&source, BRIDGE).map_err(|_| anyhow!("Cannot stage JDBC bridge"))?;
    let mut request = json!({"op": operation, "driver_class": options.driver_class, "jars": jars,
        "url": options.url, "database": profile.database, "timeout": profile.options.query_timeout_seconds,
        "connect_timeout": profile.options.connect_timeout_seconds});
    if profile.authentication == Authentication::UserPassword {
        request["user"] = json!(profile.username);
        request["password"] = json!(password);
    }
    for (key, value) in fields
        .as_object()
        .ok_or_else(|| anyhow!("Invalid bridge request"))?
    {
        request[key] = value.clone();
    }
    let bytes = serde_json::to_vec(&request).map_err(|_| anyhow!("Invalid bridge request"))?;
    ensure!(bytes.len() <= REQUEST_CAP, "JDBC request exceeds limit");
    runtime_info(&options.java_path).await?;
    let executable = java_executable(&options.java_path)?;
    let timeout =
        profile.options.connect_timeout_seconds + profile.options.query_timeout_seconds + 10;
    let run = async {
        let mut child = tokio::process::Command::new(executable)
            .arg("-Xmx256m")
            .arg(&source)
            .current_dir(&directory.0)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| anyhow!("Cannot start JDBC bridge"))?;
        let mut input = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Missing JDBC input"))?;
        let output = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Missing JDBC output"))?;
        let write = async {
            input
                .write_all(&bytes)
                .await
                .map_err(|_| anyhow!("JDBC input failed"))?;
            input
                .shutdown()
                .await
                .map_err(|_| anyhow!("JDBC input failed"))?;
            drop(input);
            Ok::<_, anyhow::Error>(())
        };
        let (_, output, status) =
            tokio::try_join!(write, read_bounded(output, RESPONSE_CAP), async {
                child
                    .wait()
                    .await
                    .map_err(|_| anyhow!("JDBC process failed"))
            })?;
        ensure!(status.success(), "JDBC bridge process failed");
        let response: Response =
            serde_json::from_slice(&output).map_err(|_| anyhow!("Invalid JDBC bridge response"))?;
        if !response.ok {
            // Never interpolate arbitrary driver/process messages or categories.
            let reason = match response.errorcategory.as_str() {
                "authentication" => "JDBC authentication denied",
                "unsupported" => "JDBC operation or read-only mode unsupported",
                "limit" => "JDBC preview or metadata exceeds limit",
                "timeout" => "JDBC operation timed out",
                "request" => "Invalid JDBC bridge request",
                _ => "JDBC connection or operation failed",
            };
            return Err(anyhow!(reason));
        }
        Ok(response)
    };
    tokio::time::timeout(Duration::from_secs(timeout), run)
        .await
        .map_err(|_| anyhow!("JDBC bridge timed out"))?
}
fn missing() -> anyhow::Error {
    anyhow!("Incomplete JDBC bridge response")
}
pub fn validate_read_only(sql: &str) -> Result<()> {
    crate::query::validate_jdbc_read_only(sql)
}
pub async fn test_connection(profile: &SourceProfile, password: &str) -> Result<ConnectionReport> {
    call(profile, password, "test", json!({}))
        .await?
        .report
        .ok_or_else(missing)
}
pub async fn discover_catalog(profile: &SourceProfile, password: &str) -> Result<CatalogSnapshot> {
    let mut catalog = call(profile, password, "catalog", json!({}))
        .await?
        .catalog
        .ok_or_else(missing)?;
    catalog
        .databases
        .retain(|d| profile.visible_schema(&d.name));
    Ok(catalog)
}
pub async fn tables(
    profile: &SourceProfile,
    password: &str,
    database: &str,
) -> Result<Vec<TableInfo>> {
    call(profile, password, "tables", json!({"database": database}))
        .await?
        .tables
        .ok_or_else(missing)
}
pub async fn columns(
    profile: &SourceProfile,
    password: &str,
    database: &str,
    table: &str,
) -> Result<Vec<ColumnInfo>> {
    call(
        profile,
        password,
        "columns",
        json!({"database": database,"table":table}),
    )
    .await?
    .columns
    .ok_or_else(missing)
}
pub async fn browse(
    profile: &SourceProfile,
    password: &str,
    request: &BrowseRequest,
) -> Result<TablePage> {
    ensure!(
        (1..=200).contains(&request.limit) && request.offset <= 10000,
        "JDBC browse limit or offset unsupported"
    );
    ensure!(
        request.filter.is_none()
            && request.sort.is_none()
            && request.where_clause.trim().is_empty()
            && request.order_by.trim().is_empty(),
        "JDBC browse filters and sorting are not supported"
    );
    call(profile, password, "browse", json!({"database":request.database,"table":request.table,"limit":request.limit,"offset":request.offset})).await?.page.ok_or_else(missing)
}
pub async fn execute_read_only(
    profile: &SourceProfile,
    password: &str,
    request: &QueryRequest,
) -> Result<QueryResult> {
    validate_read_only(&request.sql)?;
    ensure!(
        (1..=200).contains(&request.limit),
        "JDBC query limit must be 1 through 200"
    );
    call(
        profile,
        password,
        "query",
        json!({"sql":request.sql,"limit":request.limit,"database":profile.database}),
    )
    .await?
    .result
    .ok_or_else(missing)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn digest_mismatch_rejected() {
        let dir = PrivateDir::new().unwrap();
        let jar = dir.0.join("test.jar");
        std::fs::write(&jar, b"changed").unwrap();
        assert!(
            checked_jar(
                jar.to_str().unwrap(),
                &"0".repeat(64),
                &dir.0.join("copy.jar")
            )
            .is_err()
        );
    }
    #[test]
    fn relative_java_rejected() {
        assert!(java_executable("java").is_err());
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn jdk_locator_uses_fixed_args_and_validates_local_executable() {
        use std::os::unix::fs::PermissionsExt;
        let dir = PrivateDir::new().unwrap();
        let home = dir.0.join("JDK with spaces");
        std::fs::create_dir_all(home.join("bin")).unwrap();
        let java = home.join("bin/java");
        std::fs::write(&java, "#!/bin/sh\necho 'openjdk version \"21.0.2\"' >&2\n").unwrap();
        std::fs::set_permissions(&java, std::fs::Permissions::from_mode(0o700)).unwrap();
        let locator = dir.0.join("java_home");
        let args = dir.0.join("locator-args");
        std::fs::write(
            &locator,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nprintf '%s\\n' '{}'\n",
                args.display(),
                home.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&locator, std::fs::Permissions::from_mode(0o700)).unwrap();
        let detected = detect_with_locator(&locator).await.unwrap();
        assert_eq!(detected.runtime.major, 21);
        assert_eq!(
            Path::new(&detected.executable),
            java.canonicalize().unwrap()
        );
        assert_eq!(std::fs::read_to_string(args).unwrap(), "-F\n-v\n17+\n");
        // No launcher stub, older JDK, malformed output or nonzero locator fallback.
        std::fs::write(&java, "#!/bin/sh\necho 'openjdk version \"11.0.1\"' >&2\n").unwrap();
        assert!(detect_with_locator(&locator).await.is_err());
        std::fs::write(&locator, "#!/bin/sh\nexit 1\n").unwrap();
        assert!(detect_with_locator(&locator).await.is_err());
        for output in [
            b"relative/path".as_slice(),
            b"",
            b"/first\n/second",
            b"/bad\0path",
            b"/nonexistent/synthetic/jdk",
        ] {
            assert!(executable_from_java_home(output).is_err());
        }
        std::fs::write(
            &locator,
            "#!/bin/sh\ni=0; while [ $i -lt 5000 ]; do printf x; i=$((i+1)); done\n",
        )
        .unwrap();
        assert!(detect_with_locator(&locator).await.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_jar_rejected() {
        let dir = PrivateDir::new().unwrap();
        let jar = dir.0.join("test.jar");
        let link = dir.0.join("link.jar");
        std::fs::write(&jar, b"jar").unwrap();
        std::os::unix::fs::symlink(&jar, &link).unwrap();
        assert!(
            checked_jar(
                link.to_str().unwrap(),
                &format!("{:x}", Sha256::digest(b"jar")),
                &dir.0.join("copy.jar")
            )
            .is_err()
        );
    }
    #[tokio::test]
    async fn bounded_output_rejected() {
        assert!(read_bounded(&b"12345"[..], 4).await.is_err());
    }

    #[cfg(unix)]
    fn fixture(body: &str) -> (PrivateDir, SourceProfile, PathBuf, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let dir = PrivateDir::new().unwrap();
        let java = dir.0.join("java");
        let args = dir.0.join("args");
        let input = dir.0.join("input");
        let script = format!(
            "#!/bin/sh\nif [ \"$1\" = '-version' ]; then echo 'openjdk version \"17.0.1\"' >&2; exit 0; fi\nprintf '%s\\n' \"$@\" > '{}'\n/bin/cat > '{}'\n{}\n",
            args.display(),
            input.display(),
            body
        );
        std::fs::write(&java, script).unwrap();
        std::fs::set_permissions(&java, std::fs::Permissions::from_mode(0o700)).unwrap();
        let jar = dir.0.join("driver.jar");
        std::fs::write(&jar, b"fixture").unwrap();
        let profile = SourceProfile {
            engine: DbEngine::Jdbc,
            tls: TlsMode::Disabled,
            jdbc: Some(
                serde_json::from_value(json!({
                    "java_path":java, "driver_id":"fixture", "driver_class":"FixtureDriver",
                    "jars":[{"path":jar,"sha256":format!("{:x}", Sha256::digest(b"fixture"))}],
                    "url":"jdbc:fixture:local"
                }))
                .unwrap(),
            ),
            ..SourceProfile::default()
        };
        (dir, profile, args, input)
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn fake_java_credentials_only_on_stdin() {
        let (_dir, profile, args, input) = fixture(
            "printf '%s' '{\"ok\":true,\"report\":{\"server_version\":\"fixture\",\"databases\":[]}}'",
        );
        let report = test_connection(&profile, "secret-password").await.unwrap();
        assert_eq!(report.server_version, "fixture");
        let argv = std::fs::read_to_string(args).unwrap();
        assert!(argv.starts_with("-Xmx256m\n"));
        assert!(!argv.contains("secret-password"));
        assert!(
            std::fs::read_to_string(input)
                .unwrap()
                .contains("secret-password")
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn tampered_jar_never_starts_java() {
        let (_dir, profile, args, _input) = fixture("exit 1");
        let jar = &profile.jdbc.as_ref().unwrap().jars[0];
        std::fs::write(&jar.path, b"tampered").unwrap();
        assert!(test_connection(&profile, "secret-password").await.is_err());
        assert!(!args.exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cancellation_disposes_owned_child() {
        let (_dir, profile, args, _input) = fixture("while :; do :; done");
        let operation = test_connection(&profile, "secret-password");
        assert!(
            tokio::time::timeout(Duration::from_millis(500), operation)
                .await
                .is_err()
        );
        // The test does not assert termination of any grandchildren: only the
        // directly owned bridge child is covered by Tokio's kill_on_drop.
        assert!(args.exists());
    }
}

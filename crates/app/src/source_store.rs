//! Password-free source settings and separate local credential persistence.
//!
//! Credentials are stored unencrypted in a private `dalan.auth` file, not in an OS
//! keychain. Settings and credentials are separate commits, not a transaction.
//! Unix auth directories/files require private modes (0700/0600). On Windows,
//! privacy relies on inherited ACLs of the user's support directory; std does not
//! enforce or inspect ACLs. No keychain import or migration is performed.
//! Checks reject symlinks but portable std operations cannot defend against a
//! hostile concurrent directory replacement. Serialization protects only writers
//! in this process, not independent processes.

use anyhow::{Context, Result, bail, ensure};
use dalan_drivers::SourceProfile;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    env,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;

const MAX_PROFILES: usize = 100;
const MAX_BYTES: u64 = 1024 * 1024;
const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u32,
    profiles: Vec<SourceProfile>,
}

#[derive(Debug, Clone)]
pub struct SourceRepository {
    path: PathBuf,
}

impl SourceRepository {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn default_path() -> Result<PathBuf> {
        default_path()
    }

    /// Missing settings are an empty list; malformed or inaccessible settings are errors.
    pub fn load(&self) -> Result<Vec<SourceProfile>> {
        let parent = parent_of(&self.path)?;
        check_directory(parent)?;
        if !check_file(&self.path)? {
            return Ok(Vec::new());
        }
        let file = File::open(&self.path)
            .context("Cannot open Dalan source settings; check file access permissions")?;
        ensure!(
            file.metadata()?.len() <= MAX_BYTES,
            "Dalan source settings exceed the 1 MiB limit"
        );
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .context("Cannot read Dalan source settings; check file access permissions")?;
        ensure!(
            bytes.len() as u64 <= MAX_BYTES,
            "Dalan source settings exceed the 1 MiB limit"
        );
        // Reject unknown profile fields (including any accidental plaintext password).
        // SourceProfile itself intentionally remains a driver-owned type.
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| {
            anyhow::anyhow!(
                "Invalid Dalan source settings JSON; restore a valid version 1 settings file"
            )
        })?;
        if let Some(profiles) = value.get("profiles").and_then(|p| p.as_array()) {
            for profile in profiles {
                if let Some(fields) = profile.as_object() {
                    ensure!(
                        fields.keys().all(|key| matches!(
                            key.as_str(),
                            "id" | "name"
                                | "color"
                                | "engine"
                                | "host"
                                | "port"
                                | "username"
                                | "database"
                                | "transport"
                                | "tls"
                                | "ca_path"
                                | "save_password"
                                | "endpoint"
                                | "authentication"
                                | "schemas"
                                | "options"
                                | "ssl_client_cert"
                                | "ssl_client_key"
                                | "ssh_configuration_id"
                                | "jdbc"
                        )),
                        "Unknown field in Dalan source profile; remove unsupported fields from the settings file"
                    );
                }
            }
        }
        let document: Document = serde_json::from_value(value).map_err(|_| {
            anyhow::anyhow!(
                "Invalid Dalan source settings schema; restore a valid version 1 settings file"
            )
        })?;
        ensure!(
            document.version == VERSION,
            "Unsupported Dalan source settings version; use a version 1 settings file"
        );
        validate_profiles(&document.profiles)?;
        Ok(document.profiles)
    }

    /// Validate before touching disk, then commit using a same-directory atomic rename.
    /// No credentials are serialized. Only newly created directories receive mode 0700;
    /// existing parent directories are never chmod'ed. Unix settings files are mode 0600.
    pub fn save(&self, profiles: &[SourceProfile]) -> Result<()> {
        validate_profiles(profiles)?;
        let document = Document {
            version: VERSION,
            profiles: profiles.to_vec(),
        };
        let bytes = serde_json::to_vec_pretty(&document)
            .map_err(|_| anyhow::anyhow!("Cannot serialize Dalan source settings"))?;
        ensure!(
            bytes.len() as u64 <= MAX_BYTES,
            "Dalan source settings exceed the 1 MiB limit"
        );
        let parent = parent_of(&self.path)?;
        check_directory(parent)?;
        check_file(&self.path)?;
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(parent).context(
            "Cannot create Dalan settings directory; check parent directory permissions",
        )?;
        check_directory(parent)?;
        let temporary = parent.join(format!(".dalan-sources-{}.tmp", Uuid::new_v4()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .context("Cannot create temporary Dalan settings file; check directory permissions")?;
        let cleanup = TemporaryFile(temporary.clone());
        file.write_all(&bytes)
            .context("Cannot write Dalan source settings; check available disk space")?;
        file.flush().context("Cannot flush Dalan source settings")?;
        file.sync_all()
            .context("Cannot sync Dalan source settings to disk")?;
        drop(file);
        check_directory(parent)?;
        check_file(&self.path)?;
        fs::rename(&temporary, &self.path).context(
            "Cannot atomically replace Dalan source settings; previous settings were retained",
        )?;
        drop(cleanup);
        #[cfg(target_os = "linux")]
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .context("Dalan settings were replaced, but the directory could not be synced")?;
        Ok(())
    }
}

/// Resolve the user's support directory without depending on a platform directory crate.
pub fn default_path() -> Result<PathBuf> {
    fn nonempty_env(name: &str) -> Option<PathBuf> {
        env::var_os(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    }
    let home = || {
        nonempty_env("HOME")
            .or_else(|| nonempty_env("USERPROFILE"))
            .ok_or_else(|| {
                anyhow::anyhow!("Cannot locate your home directory; set HOME or USERPROFILE")
            })
    };
    #[cfg(target_os = "macos")]
    let base = home()?.join("Library/Application Support");
    #[cfg(target_os = "windows")]
    let base = match nonempty_env("APPDATA") {
        Some(path) => path,
        None => home()?.join("AppData/Roaming"),
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let base = match nonempty_env("XDG_CONFIG_HOME") {
        Some(path) => {
            ensure!(
                path.is_absolute(),
                "XDG_CONFIG_HOME must be an absolute directory path"
            );
            path
        }
        None => home()?.join(".config"),
    };
    ensure!(
        base.is_absolute(),
        "Dalan settings support directory must be an absolute path"
    );
    Ok(base.join("Dalan/sources.json"))
}

pub(crate) fn parent_of(path: &Path) -> Result<&Path> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!("Dalan source settings require a path with a parent directory")
        })
}

pub(crate) fn check_directory(path: &Path) -> Result<()> {
    for ancestor in path.ancestors() {
        if let Ok(metadata) = fs::symlink_metadata(ancestor) {
            ensure!(
                !metadata.file_type().is_symlink(),
                "Dalan settings path must not contain symlinked directories"
            );
        }
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) => ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "Dalan settings directory must be a real directory, not a symlink"
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => bail!("Cannot inspect Dalan settings directory; check access permissions"),
    }
    Ok(())
}

pub(crate) fn check_file(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            ensure!(
                metadata.is_file() && !metadata.file_type().is_symlink(),
                "Dalan source settings must be a regular file, not a symlink"
            );
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => bail!("Cannot inspect Dalan source settings; check access permissions"),
    }
}

fn validate_profiles(profiles: &[SourceProfile]) -> Result<()> {
    ensure!(
        profiles.len() <= MAX_PROFILES,
        "Dalan supports at most 100 saved sources"
    );
    let mut ids = HashSet::new();
    for (index, profile) in profiles.iter().enumerate() {
        profile.validate().with_context(|| {
            format!(
                "Invalid Dalan source profile at position {}; correct its settings",
                index + 1
            )
        })?;
        let id =
            Uuid::parse_str(&profile.id).map_err(|_| anyhow::anyhow!("Invalid source UUID"))?;
        ensure!(
            ids.insert(id),
            "Duplicate source UUID in Dalan settings; assign unique source IDs"
        );
    }
    Ok(())
}

struct TemporaryFile(PathBuf);
impl Drop for TemporaryFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub trait SecretStore: Send + Sync {
    fn get(&self, id: &str) -> Result<Option<String>>;
    fn set(&self, id: &str, password: &str) -> Result<()>;
    fn delete(&self, id: &str) -> Result<()>;
}

const MAX_PASSWORD_BYTES: usize = 64 * 1024;
static AUTH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthDocument {
    version: u32,
    #[serde(deserialize_with = "read_credentials")]
    credentials: std::collections::BTreeMap<Uuid, String>,
}

// A Value/map intermediate would silently accept duplicate JSON keys. Detect both
// identical keys and alternate spellings of the same UUID before inserting.
fn read_credentials<'de, D>(
    deserializer: D,
) -> std::result::Result<std::collections::BTreeMap<Uuid, String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Credentials;
    impl<'de> serde::de::Visitor<'de> for Credentials {
        type Value = std::collections::BTreeMap<Uuid, String>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a bounded UUID credential map")
        }
        fn visit_map<M>(self, mut map: M) -> std::result::Result<Self::Value, M::Error>
        where
            M: serde::de::MapAccess<'de>,
        {
            use serde::de::Error;
            let mut result = std::collections::BTreeMap::new();
            while let Some((key, password)) = map.next_entry::<String, String>()? {
                let id = Uuid::parse_str(&key).map_err(|_| M::Error::custom("invalid UUID"))?;
                if password.len() > MAX_PASSWORD_BYTES
                    || result.len() >= MAX_PROFILES
                    || result.contains_key(&id)
                {
                    return Err(M::Error::custom(
                        "invalid credential bounds or duplicate UUID",
                    ));
                }
                result.insert(id, password);
            }
            Ok(result)
        }
    }
    deserializer.deserialize_map(Credentials)
}

/// Explicit-path local storage, useful for isolated callers and tests. Does not
/// expose paths or passwords through Debug, or access any legacy secret backend.
#[derive(Clone)]
pub struct AuthRepository {
    path: PathBuf,
}
impl std::fmt::Debug for AuthRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AuthRepository")
    }
}
impl AuthRepository {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
    pub fn default_path() -> Result<PathBuf> {
        Ok(default_path()?.with_file_name("dalan.auth"))
    }

    fn parent(&self) -> Result<&Path> {
        ensure!(
            self.path.is_absolute()
                && !self
                    .path
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir)),
            "Invalid auth path"
        );
        let parent = parent_of(&self.path)?;
        for ancestor in parent.ancestors() {
            match fs::symlink_metadata(ancestor) {
                Ok(metadata) => ensure!(
                    metadata.is_dir() && !metadata.file_type().is_symlink(),
                    "Invalid auth directory"
                ),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => bail!("Cannot inspect auth directory"),
            }
        }
        #[cfg(unix)]
        if let Ok(metadata) = fs::symlink_metadata(parent) {
            use std::os::unix::fs::PermissionsExt;
            ensure!(
                metadata.permissions().mode() & 0o077 == 0,
                "Auth directory must be private"
            );
        }
        Ok(parent)
    }

    fn check_file(&self) -> Result<bool> {
        match fs::symlink_metadata(&self.path) {
            Ok(metadata) => {
                ensure!(
                    metadata.is_file() && !metadata.file_type().is_symlink(),
                    "Invalid auth file"
                );
                #[cfg(unix)]
                {
                    use std::os::unix::fs::{MetadataExt, PermissionsExt};
                    ensure!(
                        metadata.permissions().mode() & 0o077 == 0 && metadata.nlink() == 1,
                        "Auth file must be private and unshared"
                    );
                }
                ensure!(metadata.len() <= MAX_BYTES, "Auth file exceeds size limit");
                Ok(true)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(_) => bail!("Cannot inspect auth file"),
        }
    }

    fn load(&self) -> Result<AuthDocument> {
        self.parent()?;
        if !self.check_file()? {
            return Ok(AuthDocument {
                version: VERSION,
                credentials: Default::default(),
            });
        }
        let file = File::open(&self.path)?;
        ensure!(
            file.metadata()?.len() <= MAX_BYTES,
            "Auth file exceeds size limit"
        );
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() as u64 <= MAX_BYTES,
            "Auth file exceeds size limit"
        );
        let document: AuthDocument =
            serde_json::from_slice(&bytes).map_err(|_| anyhow::anyhow!("Invalid auth document"))?;
        ensure!(document.version == VERSION, "Unsupported auth version");
        Ok(document)
    }

    fn save(&self, document: &AuthDocument) -> Result<()> {
        let bytes = serde_json::to_vec(document)?;
        ensure!(
            bytes.len() as u64 <= MAX_BYTES,
            "Auth file exceeds size limit"
        );
        let parent = self.parent()?;
        self.check_file()?;
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(parent)?;
        self.parent()?;
        let temporary = parent.join(format!(".dalan-auth-{}.tmp", Uuid::new_v4()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        let cleanup = TemporaryFile(temporary.clone());
        file.write_all(&bytes)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        self.parent()?;
        self.check_file()?;
        fs::rename(&temporary, &self.path)?;
        drop(cleanup);
        #[cfg(target_os = "linux")]
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| anyhow::anyhow!("Auth file replaced but directory sync failed"))?;
        Ok(())
    }

    // Strip all underlying I/O/parser errors, including their source chains.
    fn operation<T>(&self, action: impl FnOnce(Uuid) -> Result<T>, id: &str) -> Result<T> {
        let result = (|| {
            let id = Uuid::parse_str(id).map_err(|_| anyhow::anyhow!("Invalid auth UUID"))?;
            let _guard = AUTH_LOCK
                .lock()
                .map_err(|_| anyhow::anyhow!("Auth lock unavailable"))?;
            action(id)
        })();
        result.map_err(|_| anyhow::anyhow!("Cannot access local Dalan credentials; check private directory/file permissions, valid version 1 data and storage limits (100 credentials, 64 KiB per password, 1 MiB file). A failed directory sync may follow a committed update."))
    }
}

impl SecretStore for AuthRepository {
    fn get(&self, id: &str) -> Result<Option<String>> {
        self.operation(|id| Ok(self.load()?.credentials.remove(&id)), id)
    }
    fn set(&self, id: &str, password: &str) -> Result<()> {
        self.operation(
            |id| {
                ensure!(
                    password.len() <= MAX_PASSWORD_BYTES,
                    "Password exceeds size limit"
                );
                let mut document = self.load()?;
                ensure!(
                    document.credentials.contains_key(&id)
                        || document.credentials.len() < MAX_PROFILES,
                    "Too many credentials"
                );
                document.credentials.insert(id, password.to_owned());
                self.save(&document)
            },
            id,
        )
    }
    fn delete(&self, id: &str) -> Result<()> {
        self.operation(
            |id| {
                let mut document = self.load()?;
                if document.credentials.remove(&id).is_some() {
                    self.save(&document)?;
                }
                Ok(())
            },
            id,
        )
    }
}

/// Compatibility facade: exclusively delegates to `dalan.auth` beside sources.json.
#[derive(Debug, Default, Clone, Copy)]
pub struct NativeSecretStore;
impl SecretStore for NativeSecretStore {
    fn get(&self, id: &str) -> Result<Option<String>> {
        AuthRepository::new(AuthRepository::default_path()?).get(id)
    }
    fn set(&self, id: &str, password: &str) -> Result<()> {
        AuthRepository::new(AuthRepository::default_path()?).set(id, password)
    }
    fn delete(&self, id: &str) -> Result<()> {
        AuthRepository::new(AuthRepository::default_path()?).delete(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Sandbox(PathBuf);
    impl Sandbox {
        fn new() -> Self {
            Self(
                env::temp_dir()
                    .canonicalize()
                    .unwrap()
                    .join(format!("dalan-source-test-{}", Uuid::new_v4())),
            )
        }
        fn repository(&self) -> SourceRepository {
            SourceRepository::new(self.0.join("sources.json"))
        }
    }
    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn missing_and_round_trip_without_credentials() {
        let sandbox = Sandbox::new();
        let repo = sandbox.repository();
        assert!(repo.load().unwrap().is_empty());
        let profile = SourceProfile::default();
        repo.save(std::slice::from_ref(&profile)).unwrap();
        assert_eq!(repo.load().unwrap(), vec![profile]);
        let json = fs::read_to_string(&repo.path).unwrap();
        assert!(json.contains("\"database\": null"));
        assert!(!json.contains("\"password\""));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&repo.path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(&sandbox.0).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        repo.save(&[]).unwrap();
        assert!(repo.load().unwrap().is_empty());
    }

    #[test]
    fn color_round_trip_legacy_and_invalid_colors() {
        let sandbox = Sandbox::new();
        let repo = sandbox.repository();
        let profile = SourceProfile {
            color: Some("#AB12cd".into()),
            ..SourceProfile::default()
        };
        repo.save(std::slice::from_ref(&profile)).unwrap();
        assert_eq!(repo.load().unwrap(), vec![profile.clone()]);
        let saved = fs::read(&repo.path).unwrap();
        let mut document: serde_json::Value = serde_json::from_slice(&saved).unwrap();
        assert_eq!(document["version"], 1);
        assert_eq!(document["profiles"][0]["color"], "#AB12cd");

        for color in ["#gg8800", "#fff", "#ff88\n0", "private-secret"] {
            let mut invalid = profile.clone();
            invalid.color = Some(color.into());
            let error = repo.save(&[invalid]).unwrap_err();
            assert!(!format!("{error:#}").contains(color));
            assert_eq!(fs::read(&repo.path).unwrap(), saved);

            document["profiles"][0]["color"] = color.into();
            let malformed = serde_json::to_vec(&document).unwrap();
            fs::write(&repo.path, &malformed).unwrap();
            let error = repo.load().unwrap_err();
            assert!(!format!("{error:#}").contains(color));
            assert_eq!(fs::read(&repo.path).unwrap(), malformed);
            fs::write(&repo.path, &saved).unwrap();
        }

        document["profiles"][0]
            .as_object_mut()
            .unwrap()
            .remove("color");
        fs::write(&repo.path, serde_json::to_vec(&document).unwrap()).unwrap();
        let mut legacy = profile;
        legacy.color = None;
        assert_eq!(repo.load().unwrap(), vec![legacy]);

        document["profiles"][0]["password"] = "private-secret".into();
        fs::write(&repo.path, serde_json::to_vec(&document).unwrap()).unwrap();
        let error = repo.load().unwrap_err();
        assert!(!format!("{error:#}").contains("private-secret"));
    }

    #[test]
    fn duplicate_invalid_and_excessive_profiles_do_not_replace_settings() {
        let sandbox = Sandbox::new();
        let repo = sandbox.repository();
        let profile = SourceProfile::default();
        repo.save(std::slice::from_ref(&profile)).unwrap();
        assert!(repo.save(&[profile.clone(), profile.clone()]).is_err());
        let mut invalid = profile.clone();
        invalid.port = 0;
        assert!(repo.save(&[invalid]).is_err());
        assert!(
            repo.save(
                &(0..101)
                    .map(|_| SourceProfile::default())
                    .collect::<Vec<_>>()
            )
            .is_err()
        );
        assert_eq!(repo.load().unwrap(), vec![profile]);
    }

    #[test]
    fn invalid_schema_version_and_size_are_errors_without_content_leaks() {
        let sandbox = Sandbox::new();
        let repo = sandbox.repository();
        repo.save(&[]).unwrap();
        for content in [
            "private-secret-invalid-json",
            "{\"version\":2,\"profiles\":[]}",
            "{\"version\":1}",
            "{\"version\":1,\"profiles\":[],\"password\":\"private-secret\"}",
        ] {
            fs::write(&repo.path, content).unwrap();
            let error = repo.load().unwrap_err().to_string();
            assert!(!error.contains("private-secret"));
        }
        fs::write(&repo.path, vec![b' '; MAX_BYTES as usize + 1]).unwrap();
        assert!(repo.load().is_err());
        let profile = SourceProfile::default();
        let json = serde_json::json!({"version":1,"profiles":[profile.clone(),profile]});
        fs::write(&repo.path, serde_json::to_vec(&json).unwrap()).unwrap();
        assert!(repo.load().is_err());
    }

    #[test]
    fn failed_commit_does_not_report_success() {
        let sandbox = Sandbox::new();
        fs::create_dir_all(sandbox.0.join("sources.json")).unwrap();
        assert!(sandbox.repository().save(&[]).is_err());
        assert!(sandbox.repository().load().is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_file_and_directory_symlinks() {
        use std::os::unix::fs::symlink;
        let sandbox = Sandbox::new();
        let repo = sandbox.repository();
        repo.save(&[]).unwrap();
        let target = sandbox.0.join("target.json");
        fs::rename(&repo.path, &target).unwrap();
        symlink(&target, &repo.path).unwrap();
        assert!(repo.load().is_err());
        assert!(repo.save(&[]).is_err());
        let link = sandbox.0.join("linked");
        symlink(&sandbox.0, &link).unwrap();
        let linked = SourceRepository::new(link.join("other.json"));
        assert!(linked.load().is_err());
        assert!(linked.save(&[]).is_err());
    }

    impl Sandbox {
        fn auth(&self) -> AuthRepository {
            AuthRepository::new(self.0.join("dalan.auth"))
        }
    }

    #[test]
    fn auth_round_trip_update_delete_and_private_modes() {
        let sandbox = Sandbox::new();
        let repo = sandbox.auth();
        let id = Uuid::new_v4().to_string();
        let other = Uuid::new_v4().to_string();
        assert_eq!(repo.get(&id).unwrap(), None);
        repo.delete(&id).unwrap();
        assert!(!sandbox.0.exists());
        repo.set(&id, "synthetic-secret-☃").unwrap();
        repo.set(&other, "").unwrap();
        assert_eq!(
            repo.get(&id).unwrap().as_deref(),
            Some("synthetic-secret-☃")
        );
        repo.set(&id, "updated").unwrap();
        assert_eq!(repo.get(&id).unwrap().as_deref(), Some("updated"));
        assert_eq!(repo.get(&other).unwrap().as_deref(), Some(""));
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(&repo.path).unwrap()).unwrap();
        assert_eq!(value["version"], 1);
        assert_eq!(value["credentials"][&id], "updated");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&repo.path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(&sandbox.0).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        repo.delete(&id).unwrap();
        repo.delete(&id).unwrap();
        assert_eq!(repo.get(&id).unwrap(), None);
        assert_eq!(repo.get(&other).unwrap().as_deref(), Some(""));
        repo.delete(&other).unwrap();
        assert_eq!(repo.get(&other).unwrap(), None);
        assert_eq!(fs::read_dir(&sandbox.0).unwrap().count(), 1);
        assert_eq!(format!("{repo:?}"), "AuthRepository");
    }

    #[test]
    fn auth_malformed_documents_are_retained_and_errors_redacted() {
        let sandbox = Sandbox::new();
        let repo = sandbox.auth();
        let id = Uuid::new_v4().to_string();
        repo.set(&id, "initial").unwrap();
        let duplicate =
            format!(r#"{{"version":1,"credentials":{{"{id}":"secret-marker","{id}":"other"}}}}"#);
        let alias = format!(
            r#"{{"version":1,"credentials":{{"{id}":"one","{}":"secret-marker"}}}}"#,
            id.replace('-', "")
        );
        for document in [
            "secret-marker invalid-json".to_string(),
            r#"{"version":2,"credentials":{}}"#.to_string(),
            r#"{"version":1,"credentials":{"secret-marker":"value"}}"#.to_string(),
            r#"{"version":1,"credentials":{},"unknown":"secret-marker"}"#.to_string(),
            r#"{"version":1,"version":1,"credentials":{}}"#.to_string(),
            r#"{"version":1}"#.to_string(),
            r#"{"version":1,"credentials":[]}"#.to_string(),
            duplicate,
            alias,
        ] {
            fs::write(&repo.path, &document).unwrap();
            for error in [
                repo.get(&id).unwrap_err(),
                repo.set(&id, "secret-marker").unwrap_err(),
                repo.delete(&id).unwrap_err(),
            ] {
                let text = format!("{error:#}");
                assert!(!text.contains("secret-marker"));
                assert!(!text.contains(repo.path.to_str().unwrap()));
                assert!(!text.contains("line "));
            }
            assert_eq!(fs::read_to_string(&repo.path).unwrap(), document);
        }
    }

    #[test]
    fn auth_limits_and_invalid_ids_preserve_previous_file() {
        let sandbox = Sandbox::new();
        let repo = sandbox.auth();
        let id = Uuid::new_v4().to_string();
        repo.set(&id, &"a".repeat(MAX_PASSWORD_BYTES)).unwrap();
        let initial = fs::read(&repo.path).unwrap();
        assert!(
            repo.set(&id, &"é".repeat(MAX_PASSWORD_BYTES / 2 + 1))
                .is_err()
        );
        assert!(repo.get("not-a-uuid-secret").is_err());
        assert!(repo.set("not-a-uuid-secret", "secret").is_err());
        assert!(repo.delete("not-a-uuid-secret").is_err());
        assert_eq!(fs::read(&repo.path).unwrap(), initial);
        repo.set(&id, "short").unwrap();
        for _ in 1..MAX_PROFILES {
            repo.set(&Uuid::new_v4().to_string(), "short").unwrap();
        }
        let full = fs::read(&repo.path).unwrap();
        assert!(repo.set(&Uuid::new_v4().to_string(), "extra").is_err());
        assert_eq!(fs::read(&repo.path).unwrap(), full);
        repo.set(&id, "updated").unwrap();
        repo.delete(&id).unwrap();
        repo.set(&Uuid::new_v4().to_string(), "replacement")
            .unwrap();
        let oversized_password = serde_json::json!({"version":1,"credentials":{id.clone(): "a".repeat(MAX_PASSWORD_BYTES + 1)}});
        fs::write(&repo.path, serde_json::to_vec(&oversized_password).unwrap()).unwrap();
        assert!(repo.get(&id).is_err());
        let credentials: std::collections::BTreeMap<_, _> = (0..101)
            .map(|_| (Uuid::new_v4().to_string(), "x"))
            .collect();
        fs::write(
            &repo.path,
            serde_json::to_vec(&serde_json::json!({"version":1,"credentials":credentials}))
                .unwrap(),
        )
        .unwrap();
        assert!(repo.get(&id).is_err());
        fs::write(&repo.path, vec![b' '; MAX_BYTES as usize + 1]).unwrap();
        assert!(repo.get(&id).is_err());
        assert!(repo.set(&id, "value").is_err());
        assert_eq!(fs::metadata(&repo.path).unwrap().len(), MAX_BYTES + 1);
    }

    #[test]
    fn auth_serialized_size_failure_retains_file_and_cleans_temporary() {
        let sandbox = Sandbox::new();
        let repo = sandbox.auth();
        // Escaping control bytes makes JSON much larger than the password itself.
        let password = "\0".repeat(MAX_PASSWORD_BYTES);
        repo.set(&Uuid::new_v4().to_string(), &password).unwrap();
        repo.set(&Uuid::new_v4().to_string(), &password).unwrap();
        let initial = fs::read(&repo.path).unwrap();
        assert!(repo.set(&Uuid::new_v4().to_string(), &password).is_err());
        assert_eq!(fs::read(&repo.path).unwrap(), initial);
        assert_eq!(fs::read_dir(&sandbox.0).unwrap().count(), 1);
    }

    #[test]
    fn auth_concurrent_independent_instances_do_not_lose_updates() {
        let sandbox = Sandbox::new();
        let path = sandbox.auth().path;
        let ids: Vec<_> = (0..24).map(|_| Uuid::new_v4().to_string()).collect();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(ids.len()));
        let workers: Vec<_> = ids
            .iter()
            .cloned()
            .map(|id| {
                let path = path.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    let repo = AuthRepository::new(path);
                    repo.set(&id, "first").unwrap();
                    repo.set(&id, "second").unwrap();
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
        let repo = AuthRepository::new(path);
        for id in ids {
            assert_eq!(repo.get(&id).unwrap().as_deref(), Some("second"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn auth_rejects_symlinks_hardlinks_and_permissive_files_and_directories() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let sandbox = Sandbox::new();
        let repo = sandbox.auth();
        let id = Uuid::new_v4().to_string();
        repo.set(&id, "retained").unwrap();
        let initial = fs::read(&repo.path).unwrap();
        fs::set_permissions(&repo.path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(repo.get(&id).is_err());
        assert!(repo.set(&id, "change").is_err());
        assert!(repo.delete(&id).is_err());
        assert_eq!(fs::read(&repo.path).unwrap(), initial);
        fs::set_permissions(&repo.path, fs::Permissions::from_mode(0o600)).unwrap();
        fs::set_permissions(&sandbox.0, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(repo.get(&id).is_err());
        assert!(repo.set(&id, "change").is_err());
        fs::set_permissions(&sandbox.0, fs::Permissions::from_mode(0o700)).unwrap();
        let target = sandbox.0.join("target");
        fs::hard_link(&repo.path, &target).unwrap();
        assert!(repo.get(&id).is_err());
        assert!(repo.set(&id, "change").is_err());
        fs::remove_file(&target).unwrap();
        fs::rename(&repo.path, &target).unwrap();
        symlink(&target, &repo.path).unwrap();
        assert!(repo.get(&id).is_err());
        assert!(repo.set(&id, "change").is_err());
        assert!(repo.delete(&id).is_err());
        let linked = sandbox.0.join("linked");
        symlink(&sandbox.0, &linked).unwrap();
        let nested = AuthRepository::new(linked.join("missing/sub/auth"));
        assert!(nested.get(&id).is_err());
        assert!(nested.set(&id, "change").is_err());
        assert_eq!(fs::read(&target).unwrap(), initial);
    }

    #[test]
    fn auth_rejects_non_regular_files_and_relative_paths() {
        let sandbox = Sandbox::new();
        sandbox.repository().save(&[]).unwrap();
        let repo = sandbox.auth();
        fs::create_dir(&repo.path).unwrap();
        let id = Uuid::new_v4().to_string();
        assert!(repo.get(&id).is_err());
        assert!(repo.set(&id, "secret").is_err());
        assert!(repo.delete(&id).is_err());
        let relative = AuthRepository::new(PathBuf::from("relative/dalan.auth"));
        assert!(relative.get(&id).is_err());
        assert!(relative.set(&id, "secret").is_err());
    }

    #[test]
    fn auth_default_path_is_beside_settings_without_access() {
        assert_eq!(
            AuthRepository::default_path().unwrap(),
            default_path().unwrap().with_file_name("dalan.auth")
        );
    }
}

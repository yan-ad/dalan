//! Versioned, password-free source settings and an explicit native credential store.
//!
//! JSON and Keychain are separate persistence operations, not a transaction. Callers
//! should validate/serialize settings before changing credentials, and report either
//! failure. A failed JSON commit after a credential change can leave the previous
//! profile with an updated credential. Never fall back to plaintext credentials.
//! Filesystem checks reject existing symlinks; as with other portable `std` path
//! operations, they do not protect against a hostile concurrent directory replacement.

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
                                | "engine"
                                | "host"
                                | "port"
                                | "username"
                                | "database"
                                | "transport"
                                | "tls"
                                | "ca_path"
                                | "save_password"
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

fn parent_of(path: &Path) -> Result<&Path> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!("Dalan source settings require a path with a parent directory")
        })
}

fn check_directory(path: &Path) -> Result<()> {
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

fn check_file(path: &Path) -> Result<bool> {
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

/// macOS Keychain only. Other platforms explicitly report unavailable; no mock or
/// plaintext fallback is ever installed. Accounts are canonical profile UUIDs.
#[derive(Debug, Default, Clone, Copy)]
pub struct NativeSecretStore;

#[cfg(target_os = "macos")]
fn entry(id: &str) -> Result<keyring::Entry> {
    let id = Uuid::parse_str(id).context("A valid source UUID is required for Keychain access")?;
    keyring::Entry::new("Dalan.database-sources", &id.to_string()).map_err(|_| {
        anyhow::anyhow!("Cannot access macOS Keychain; check Keychain access permissions")
    })
}

impl SecretStore for NativeSecretStore {
    fn get(&self, id: &str) -> Result<Option<String>> {
        #[cfg(target_os = "macos")]
        {
            match entry(id)?.get_password() {
                Ok(password) => Ok(Some(password)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(_) => bail!(
                    "Cannot read source password from macOS Keychain; check Keychain access permissions"
                ),
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = id;
            bail!("Native password storage is unavailable on this platform")
        }
    }

    fn set(&self, id: &str, password: &str) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            entry(id)?.set_password(password)
                .map_err(|_| anyhow::anyhow!("Cannot save source password in macOS Keychain; check Keychain access permissions"))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (id, password);
            bail!("Native password storage is unavailable on this platform")
        }
    }

    fn delete(&self, id: &str) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            match entry(id)?.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(_) => bail!(
                    "Cannot delete source password from macOS Keychain; check Keychain access permissions"
                ),
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = id;
            bail!("Native password storage is unavailable on this platform")
        }
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

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn native_secrets_are_explicitly_unavailable() {
        let id = Uuid::new_v4().to_string();
        assert!(NativeSecretStore.get(&id).is_err());
        assert!(NativeSecretStore.set(&id, "test").is_err());
        assert!(NativeSecretStore.delete(&id).is_err());
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "Requires an unlocked macOS Keychain and may prompt for permission"]
    fn generated_keychain_item_round_trip() {
        struct Cleanup(String);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = NativeSecretStore.delete(&self.0);
            }
        }
        let cleanup = Cleanup(Uuid::new_v4().to_string());
        let password = Uuid::new_v4().to_string();
        assert_eq!(NativeSecretStore.get(&cleanup.0).unwrap(), None);
        NativeSecretStore.set(&cleanup.0, &password).unwrap();
        assert!(NativeSecretStore.get(&cleanup.0).unwrap().as_deref() == Some(password.as_str()));
        NativeSecretStore.delete(&cleanup.0).unwrap();
        assert_eq!(NativeSecretStore.get(&cleanup.0).unwrap(), None);
    }
}

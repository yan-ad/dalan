//! Non-secret application preferences, separate from source settings and credentials.
//!
//! Newly created directories/files use 0700/0600 on Unix. Windows privacy relies
//! on inherited support-directory ACLs, as in `source_store`. Portable filesystem
//! checks reject symlinks but cannot prevent hostile concurrent path replacement.

use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};
use uuid::Uuid;

const VERSION: u32 = 1;
const MAX_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppearancePreference {
    #[default]
    System,
    Light,
    Dark,
}

impl AppearancePreference {
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::System => Self::Light,
            Self::Light => Self::Dark,
            Self::Dark => Self::System,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub appearance: AppearancePreference,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: VERSION,
            appearance: AppearancePreference::System,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfigRepository {
    path: PathBuf,
}

impl ConfigRepository {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn default_path() -> Result<PathBuf> {
        default_path()
    }

    /// Missing configuration is the default; invalid or inaccessible files are errors.
    /// Loading never creates directories or rewrites invalid configuration.
    pub fn load(&self) -> Result<Config> {
        let parent = checked_parent(&self.path)?;
        check_directories(parent)?;
        if !check_file(&self.path)? {
            return Ok(Config::default());
        }
        let file = File::open(&self.path)
            .map_err(|_| anyhow::anyhow!("Cannot open Dalan configuration"))?;
        let metadata = file
            .metadata()
            .map_err(|_| anyhow::anyhow!("Cannot inspect Dalan configuration"))?;
        ensure!(
            metadata.is_file(),
            "Dalan configuration must be a regular file"
        );
        ensure!(
            metadata.len() <= MAX_BYTES,
            "Dalan configuration exceeds the 64 KiB limit"
        );
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| anyhow::anyhow!("Cannot read Dalan configuration"))?;
        ensure!(
            bytes.len() as u64 <= MAX_BYTES,
            "Dalan configuration exceeds the 64 KiB limit"
        );
        // Do not surface serde errors: they can include untrusted file contents.
        let config: Config = serde_json::from_slice(&bytes).map_err(|_| {
            anyhow::anyhow!("Invalid Dalan configuration; expected version 1 configuration JSON")
        })?;
        validate(&config)?;
        Ok(config)
    }

    /// Validate before touching disk and replace atomically with a private file.
    /// Existing directories are never chmod'ed; failures before rename retain the
    /// previous configuration. This document contains no credential fields.
    pub fn save(&self, config: &Config) -> Result<()> {
        validate(config)?;
        let bytes = serde_json::to_vec_pretty(config)
            .map_err(|_| anyhow::anyhow!("Cannot serialize Dalan configuration"))?;
        ensure!(
            bytes.len() as u64 <= MAX_BYTES,
            "Dalan configuration exceeds the 64 KiB limit"
        );
        let parent = checked_parent(&self.path)?;
        check_directories(parent)?;
        check_file(&self.path)?;
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(parent)
            .map_err(|_| anyhow::anyhow!("Cannot create Dalan configuration directory"))?;
        check_directories(parent)?;
        let temporary = parent.join(format!(".dalan-config-{}.tmp", Uuid::new_v4()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|_| anyhow::anyhow!("Cannot create temporary Dalan configuration file"))?;
        let cleanup = TemporaryFile(temporary.clone());
        file.write_all(&bytes)
            .map_err(|_| anyhow::anyhow!("Cannot write Dalan configuration"))?;
        file.flush()
            .and_then(|()| file.sync_all())
            .map_err(|_| anyhow::anyhow!("Cannot sync Dalan configuration"))?;
        drop(file);
        check_directories(parent)?;
        check_file(&self.path)?;
        fs::rename(&temporary, &self.path).map_err(|_| {
            anyhow::anyhow!(
                "Cannot atomically replace Dalan configuration; previous configuration was retained"
            )
        })?;
        drop(cleanup);
        #[cfg(target_os = "linux")]
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| {
                anyhow::anyhow!(
                    "Dalan configuration was replaced, but its directory could not be synced"
                )
            })?;
        Ok(())
    }
}

/// Resolve alongside `sources.json` without reading or creating either file.
pub fn default_path() -> Result<PathBuf> {
    Ok(crate::source_store::default_path()?.with_file_name("dalan.config"))
}

fn validate(config: &Config) -> Result<()> {
    ensure!(
        config.version == VERSION,
        "Unsupported Dalan configuration version; expected version 1"
    );
    Ok(())
}

fn checked_parent(path: &Path) -> Result<&Path> {
    ensure!(
        !path
            .components()
            .any(|component| component == Component::ParentDir),
        "Dalan configuration path must not contain parent traversal"
    );
    ensure!(
        path.file_name().is_some(),
        "Dalan configuration requires a file name"
    );
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| anyhow::anyhow!("Dalan configuration requires a parent directory"))
}

fn check_directories(path: &Path) -> Result<()> {
    for ancestor in path.ancestors().filter(|path| !path.as_os_str().is_empty()) {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) => ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "Dalan configuration ancestors must be real directories, not symlinks"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => bail!("Cannot inspect Dalan configuration directory"),
        }
    }
    Ok(())
}

fn check_file(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            ensure!(
                metadata.is_file() && !metadata.file_type().is_symlink(),
                "Dalan configuration must be a regular file, not a symlink"
            );
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => bail!("Cannot inspect Dalan configuration file"),
    }
}

struct TemporaryFile(PathBuf);
impl Drop for TemporaryFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let root = std::env::temp_dir()
                .canonicalize()
                .unwrap()
                .join(format!("dalan-config-test-{}", Uuid::new_v4()));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
        fn repository(&self) -> ConfigRepository {
            ConfigRepository::new(self.0.join("support/dalan.config"))
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn missing_configuration_defaults_without_creating_files() {
        let scratch = Scratch::new();
        let repository = scratch.repository();
        assert_eq!(repository.load().unwrap(), Config::default());
        assert!(!repository.path.parent().unwrap().exists());
        assert_eq!(Config::default().version, 1);
        assert_eq!(
            AppearancePreference::default(),
            AppearancePreference::System
        );
    }

    #[test]
    fn appearance_names_labels_and_cycle() {
        for (appearance, name) in [
            (AppearancePreference::System, "system"),
            (AppearancePreference::Light, "light"),
            (AppearancePreference::Dark, "dark"),
        ] {
            assert_eq!(
                serde_json::to_string(&appearance).unwrap(),
                format!("\"{name}\"")
            );
            assert_eq!(
                serde_json::from_str::<AppearancePreference>(&format!("\"{name}\"")).unwrap(),
                appearance
            );
            assert_eq!(appearance.label().to_lowercase(), name);
            assert_eq!(appearance.next().next().next(), appearance);
        }
        assert_eq!(
            AppearancePreference::System.next(),
            AppearancePreference::Light
        );
        assert_eq!(
            AppearancePreference::Light.next(),
            AppearancePreference::Dark
        );
        assert_eq!(
            AppearancePreference::Dark.next(),
            AppearancePreference::System
        );
    }

    #[test]
    fn default_path_is_source_settings_sibling() {
        // Resolve names only; never load or save the actual user's configuration.
        let sources = crate::source_store::default_path().unwrap();
        let config = ConfigRepository::default_path().unwrap();
        assert_eq!(sources.parent(), config.parent());
        assert_eq!(config.file_name().unwrap(), "dalan.config");
    }

    #[test]
    fn saves_loads_and_replaces_without_temporary_files() {
        let scratch = Scratch::new();
        let repository = scratch.repository();
        for appearance in [
            AppearancePreference::System,
            AppearancePreference::Dark,
            AppearancePreference::Light,
        ] {
            let config = Config {
                appearance,
                ..Config::default()
            };
            repository.save(&config).unwrap();
            assert_eq!(repository.load().unwrap(), config);
            assert_eq!(
                fs::read_dir(repository.path.parent().unwrap())
                    .unwrap()
                    .count(),
                1
            );
        }
        let before = fs::read(&repository.path).unwrap();
        assert!(
            repository
                .save(&Config {
                    version: 2,
                    ..Config::default()
                })
                .is_err()
        );
        assert_eq!(fs::read(&repository.path).unwrap(), before);
    }

    #[test]
    fn invalid_and_oversized_documents_are_retained_and_errors_sanitized() {
        let scratch = Scratch::new();
        let repository = scratch.repository();
        fs::create_dir_all(repository.path.parent().unwrap()).unwrap();
        let marker = "sensitive-test-marker";
        for bytes in [
            format!(r#"{{"version":1,"appearance":"{marker}"}}"#).into_bytes(),
            format!(r#"{{"version":1,"appearance":"system","password":"{marker}"}}"#).into_bytes(),
            br#"{"version":2,"appearance":"system"}"#.to_vec(),
            br#"{"version":1}"#.to_vec(),
            br#"{"version":1,"version":1,"appearance":"system"}"#.to_vec(),
            marker.as_bytes().to_vec(),
            vec![b' '; MAX_BYTES as usize + 1],
        ] {
            fs::write(&repository.path, &bytes).unwrap();
            let error = repository.load().unwrap_err();
            assert!(!format!("{error:#}").contains(marker));
            assert_eq!(fs::read(&repository.path).unwrap(), bytes);
        }
    }

    #[test]
    fn rejects_parent_traversal_and_nonregular_paths() {
        let scratch = Scratch::new();
        let directory = scratch.0.join("directory");
        fs::create_dir(&directory).unwrap();
        let blocked = scratch.0.join("file");
        fs::write(&blocked, b"unchanged").unwrap();
        for path in [
            directory,
            blocked.join("child/dalan.config"),
            scratch.0.join("../dalan.config"),
        ] {
            let repository = ConfigRepository::new(path);
            assert!(repository.load().is_err());
            assert!(repository.save(&Config::default()).is_err());
        }
        assert_eq!(fs::read(blocked).unwrap(), b"unchanged");
    }

    #[cfg(unix)]
    #[test]
    fn private_permissions_and_existing_directory_preservation() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = Scratch::new();
        fs::set_permissions(&scratch.0, fs::Permissions::from_mode(0o755)).unwrap();
        let repository = scratch.repository();
        repository.save(&Config::default()).unwrap();
        assert_eq!(
            fs::metadata(&repository.path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(repository.path.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&scratch.0).unwrap().permissions().mode() & 0o777,
            0o755
        );
        fs::set_permissions(&repository.path, fs::Permissions::from_mode(0o644)).unwrap();
        repository.save(&Config::default()).unwrap();
        assert_eq!(
            fs::metadata(&repository.path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_file_and_ancestor_symlinks_without_changing_targets() {
        use std::os::unix::fs::symlink;
        let scratch = Scratch::new();
        let target = scratch.0.join("target");
        fs::write(&target, b"unchanged").unwrap();
        let link = scratch.0.join("link");
        symlink(&target, &link).unwrap();
        let linked_dir = scratch.0.join("linked-dir");
        symlink(&scratch.0, &linked_dir).unwrap();
        for path in [link, linked_dir.join("missing/nested/dalan.config")] {
            let repository = ConfigRepository::new(path);
            assert!(repository.load().is_err());
            assert!(repository.save(&Config::default()).is_err());
        }
        assert_eq!(fs::read(target).unwrap(), b"unchanged");
        assert!(!scratch.0.join("missing").exists());
    }
}

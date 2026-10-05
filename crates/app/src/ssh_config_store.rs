//! Reusable OpenSSH settings, never passwords, passphrases, or private key contents.
//! KeyPair references a local file; encrypted keys must be unlocked with ssh-add.
//! Existing symlinks in the repository path are rejected. Portable filesystem checks
//! cannot defend against hostile concurrent directory replacement.
use crate::source_store::{check_directory, check_file, parent_of};
use anyhow::{Context, Result, ensure};
use dalan_drivers::{SourceProfile, Transport};
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

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum SshAuthentication {
    #[default]
    Agent,
    KeyPair,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SshProfile {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    #[serde(default)]
    pub auth: SshAuthentication,
    #[serde(default)]
    pub identity_file: Option<String>,
    #[serde(default)]
    pub known_hosts_file: Option<String>,
    #[serde(default)]
    pub parse_config: bool,
}
impl Default for SshProfile {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: "New SSH configuration".into(),
            host: "localhost".into(),
            port: 22,
            user: env::var("USER")
                .ok()
                .filter(|u| !u.trim().is_empty())
                .unwrap_or_else(|| "root".into()),
            auth: SshAuthentication::Agent,
            identity_file: None,
            known_hosts_file: None,
            parse_config: false,
        }
    }
}
impl SshProfile {
    /// Materialize the exact backend transport. Validate before connecting or saving.
    pub fn transport(&self) -> Transport {
        Transport::Ssh {
            host: self.host.clone(),
            port: self.port,
            user: self.user.clone(),
            identity_file: self.identity_file.clone(),
            known_hosts_file: self.known_hosts_file.clone(),
            parse_config: self.parse_config,
        }
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            Uuid::parse_str(&self.id).is_ok(),
            "Invalid SSH configuration UUID"
        );
        ensure!(
            !self.name.is_empty()
                && self.name.trim() == self.name
                && self.name.len() <= 256
                && !self.name.chars().any(char::is_control),
            "Invalid SSH configuration name"
        );
        match self.auth {
            SshAuthentication::Agent => ensure!(
                self.identity_file.is_none(),
                "Agent authentication must not specify an identity file"
            ),
            SshAuthentication::KeyPair => {
                let path = self
                    .identity_file
                    .as_deref()
                    .context("KeyPair authentication requires an identity file")?;
                validate_local_file(path)?;
            }
        }
        if let Some(path) = &self.known_hosts_file {
            validate_local_file(path)?;
        }
        SourceProfile {
            transport: self.transport(),
            ..SourceProfile::default()
        }
        .validate()
    }
}
/// Resolve a reusable reference using validated, credential-free settings. Missing
/// references are errors, never an implicit fallback to obsolete inline transport.
pub fn materialize_ssh_profile(
    profile: &SourceProfile,
    configurations: &[SshProfile],
) -> Result<SourceProfile> {
    let Some(id) = &profile.ssh_configuration_id else {
        return Ok(profile.clone());
    };
    ensure!(
        matches!(profile.transport, Transport::Ssh { .. }),
        "SSH configuration requires SSH transport"
    );
    let configuration = configurations
        .iter()
        .find(|configuration| &configuration.id == id)
        .context("Referenced SSH configuration is unavailable; select an existing configuration")?;
    configuration.validate()?;
    let mut profile = profile.clone();
    profile.transport = configuration.transport();
    profile.validate()?;
    Ok(profile)
}

/// Read local metadata only: this function never opens a connection or keychain.
pub fn resolve_ssh_profile(profile: &SourceProfile) -> Result<SourceProfile> {
    if profile.ssh_configuration_id.is_none() {
        return Ok(profile.clone());
    }
    let repository = SshRepository::new(SshRepository::default_path()?);
    materialize_ssh_profile(profile, &repository.load()?)
}

fn validate_local_file(path: &str) -> Result<()> {
    ensure!(
        !path.is_empty()
            && path.trim() == path
            && path.len() <= 4096
            && !path.chars().any(char::is_control)
            && !path.contains('%')
            && !path.contains("${")
            && Path::new(path).is_absolute()
            && Path::new(path).is_file(),
        "SSH file path must be an absolute existing regular file without expansion tokens"
    );
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u32,
    profiles: Vec<SshProfile>,
}

#[derive(Debug, Clone)]
pub struct SshRepository {
    path: PathBuf,
}

impl SshRepository {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn default_path() -> Result<PathBuf> {
        default_path()
    }

    /// Missing settings are an empty list; malformed or inaccessible settings are errors.
    pub fn load(&self) -> Result<Vec<SshProfile>> {
        let parent = parent_of(&self.path)?;
        check_directory(parent)?;
        if !check_file(&self.path)? {
            return Ok(Vec::new());
        }
        let file = File::open(&self.path)
            .context("Cannot open Dalan SSH settings; check file access permissions")?;
        ensure!(
            file.metadata()?.len() <= MAX_BYTES,
            "Dalan SSH settings exceed the 1 MiB limit"
        );
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .context("Cannot read Dalan SSH settings; check file access permissions")?;
        ensure!(
            bytes.len() as u64 <= MAX_BYTES,
            "Dalan SSH settings exceed the 1 MiB limit"
        );
        let document: Document = serde_json::from_slice(&bytes).map_err(|_| {
            anyhow::anyhow!(
                "Invalid Dalan SSH settings schema; restore a valid version 1 settings file"
            )
        })?;
        ensure!(
            document.version == VERSION,
            "Unsupported Dalan SSH settings version; use a version 1 settings file"
        );
        validate_profiles(&document.profiles)?;
        Ok(document.profiles)
    }

    /// Validate before touching disk, then commit using a same-directory atomic rename.
    /// No credentials are serialized. Only newly created directories receive mode 0700;
    /// existing parent directories are never chmod'ed. Unix settings files are mode 0600.
    pub fn save(&self, profiles: &[SshProfile]) -> Result<()> {
        validate_profiles(profiles)?;
        let document = Document {
            version: VERSION,
            profiles: profiles.to_vec(),
        };
        let bytes = serde_json::to_vec_pretty(&document)
            .map_err(|_| anyhow::anyhow!("Cannot serialize Dalan SSH settings"))?;
        ensure!(
            bytes.len() as u64 <= MAX_BYTES,
            "Dalan SSH settings exceed the 1 MiB limit"
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
        let temporary = parent.join(format!(".dalan-ssh-{}.tmp", Uuid::new_v4()));
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
            .context("Cannot write Dalan SSH settings; check available disk space")?;
        file.flush().context("Cannot flush Dalan SSH settings")?;
        file.sync_all()
            .context("Cannot sync Dalan SSH settings to disk")?;
        drop(file);
        check_directory(parent)?;
        check_file(&self.path)?;
        fs::rename(&temporary, &self.path).context(
            "Cannot atomically replace Dalan SSH settings; previous settings were retained",
        )?;
        drop(cleanup);
        #[cfg(target_os = "linux")]
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .context("Dalan settings were replaced, but the directory could not be synced")?;
        Ok(())
    }
}

pub fn default_path() -> Result<PathBuf> {
    let sources = crate::source_store::default_path()?;
    Ok(parent_of(&sources)?.join("ssh-configurations.json"))
}
fn validate_profiles(profiles: &[SshProfile]) -> Result<()> {
    ensure!(
        profiles.len() <= MAX_PROFILES,
        "Dalan supports at most 100 saved SSH configurations"
    );
    let mut ids = HashSet::new();
    for (index, profile) in profiles.iter().enumerate() {
        profile.validate().with_context(|| {
            format!(
                "Invalid SSH configuration at position {}; correct its settings",
                index + 1
            )
        })?;
        let id = Uuid::parse_str(&profile.id)
            .map_err(|_| anyhow::anyhow!("Invalid SSH configuration UUID"))?;
        ensure!(ids.insert(id), "Duplicate SSH configuration UUID");
    }
    Ok(())
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

    #[test]
    fn references_use_latest_settings_and_missing_references_fail_closed() {
        let mut configuration = SshProfile::default();
        let source = SourceProfile {
            transport: configuration.transport(),
            ssh_configuration_id: Some(configuration.id.clone()),
            ..SourceProfile::default()
        };
        configuration.host = "new-bastion.example".into();
        let resolved =
            materialize_ssh_profile(&source, std::slice::from_ref(&configuration)).unwrap();
        assert_eq!(resolved.transport, configuration.transport());
        assert_eq!(resolved.ssh_configuration_id, source.ssh_configuration_id);
        assert!(materialize_ssh_profile(&source, &[]).is_err());
        let direct = SourceProfile {
            ssh_configuration_id: source.ssh_configuration_id,
            ..SourceProfile::default()
        };
        assert!(materialize_ssh_profile(&direct, &[configuration]).is_err());
    }

    #[test]
    fn repository_round_trip_contains_only_metadata() {
        let directory = env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("dalan-ssh-test-{}", Uuid::new_v4()));
        let repository = SshRepository::new(directory.join("ssh.json"));
        assert!(repository.load().unwrap().is_empty());
        let configuration = SshProfile::default();
        repository
            .save(std::slice::from_ref(&configuration))
            .unwrap();
        assert_eq!(repository.load().unwrap(), vec![configuration]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&repository.path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        fs::remove_dir_all(directory).unwrap();
    }
}

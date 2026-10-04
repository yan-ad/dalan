//! Metadata-only discovery of likely SSH identity files. Private key contents are never read.
use anyhow::{Context, Result, ensure};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshKeyCandidate {
    pub name: String,
    pub path: PathBuf,
}

pub fn user_ssh_directory() -> Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!("HOME is unavailable; enter an SSH identity path manually")
        })?;
    Ok(PathBuf::from(home).join(".ssh"))
}

/// Names are candidates, not proof of key format or support by OpenSSH.
pub fn discover(directory: &Path) -> Result<Vec<SshKeyCandidate>> {
    match fs::metadata(directory) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(_) => {
            return Err(anyhow::anyhow!(
                "Cannot access the SSH directory; enter an identity path manually"
            ));
        }
        Ok(metadata) => ensure!(
            metadata.is_dir(),
            "SSH identity directory is not a directory"
        ),
    }
    let mut files = Vec::new();
    let mut names = HashSet::new();
    for entry in fs::read_dir(directory)
        .context("Cannot list SSH identity filenames")?
        .take(513)
    {
        ensure!(
            files.len() < 512,
            "SSH directory exceeds 512 entries; enter an identity path manually"
        );
        let entry = entry.context("Cannot read an SSH directory entry")?;
        let name = entry.file_name().into_string().map_err(|_| {
            anyhow::anyhow!("An SSH filename is not UTF-8; use a manual identity path")
        })?;
        let metadata = entry.file_type().context("Cannot read SSH file metadata")?;
        // Skip symlinks and folders rather than follow files outside the requested directory.
        if metadata.is_file() {
            names.insert(name.clone());
            files.push((name, entry.path()));
        } else {
            // Count every entry, including directories and links, against the discovery limit.
            files.push((String::new(), PathBuf::new()));
        }
    }
    let mut candidates: Vec<_> = files
        .into_iter()
        .filter_map(|(name, path)| {
            if name.is_empty()
                || name.starts_with('.')
                || name.chars().any(char::is_control)
                || name.ends_with(".pub")
                || name.ends_with("-cert.pub")
                || matches!(
                    name.as_str(),
                    "config"
                        | "known_hosts"
                        | "known_hosts.old"
                        | "authorized_keys"
                        | "authorized_keys2"
                        | "environment"
                        | "rc"
                )
            {
                return None;
            }
            let candidate = name.starts_with("id_")
                || name.ends_with(".pem")
                || name.ends_with(".key")
                || names.contains(&format!("{name}.pub"));
            candidate.then_some(SshKeyCandidate { name, path })
        })
        .collect();
    candidates.sort_by(|left, right| left.name.cmp(&right.name));
    ensure!(
        candidates.len() <= 128,
        "More than 128 SSH identity candidates; enter an identity path manually"
    );
    Ok(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Sandbox(PathBuf);
    impl Sandbox {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .canonicalize()
                .unwrap()
                .join(format!("dalan-ssh-list-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn discovery_uses_filenames_and_metadata_not_key_contents() {
        let sandbox = Sandbox::new();
        for name in [
            "id_ed25519",
            "id_ed25519.pub",
            "production key.pem",
            "work",
            "work.pub",
            "config",
            "known_hosts",
            "authorized_keys",
            "README",
            ".hidden.key",
            "id_bad\nname",
        ] {
            fs::write(
                sandbox.0.join(name),
                "not a private key: discovery must not inspect or expose contents",
            )
            .unwrap();
        }
        fs::create_dir(sandbox.0.join("id_directory")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(sandbox.0.join("id_ed25519"), sandbox.0.join("id_symlink"))
            .unwrap();
        let keys = discover(&sandbox.0).unwrap();
        assert_eq!(
            keys.iter().map(|key| key.name.as_str()).collect::<Vec<_>>(),
            vec!["id_ed25519", "production key.pem", "work"]
        );
        assert!(keys.iter().all(|key| key.path.is_absolute()));
    }
    #[test]
    fn missing_directory_is_empty_and_limits_are_enforced() {
        let sandbox = Sandbox::new();
        assert!(discover(&sandbox.0.join("missing")).unwrap().is_empty());
        fs::write(sandbox.0.join("file"), "").unwrap();
        assert!(discover(&sandbox.0.join("file")).is_err());
        for index in 0..129 {
            fs::write(sandbox.0.join(format!("id_{index}")), "").unwrap();
        }
        assert!(discover(&sandbox.0).is_err());
    }
}

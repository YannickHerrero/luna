use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientProfile {
    pub server_url: String,
    pub device_id: Uuid,
    pub token: String,
}

#[derive(Debug, Clone)]
pub struct ProfileStore {
    directory: PathBuf,
}

impl ProfileStore {
    pub fn discover() -> Result<Self, ConfigError> {
        let base = BaseDirs::new().ok_or(ConfigError::MissingHome)?;
        Ok(Self::new(base.home_dir().join(".config/luna/tui")))
    }

    #[must_use]
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    pub fn load(&self, profile: &str) -> Result<ClientProfile, ConfigError> {
        let path = self.profile_path(profile)?;
        verify_private_file(&path)?;
        let profile: ClientProfile = serde_json::from_slice(&fs::read(&path)?)?;
        crate::api::ServerOrigin::parse(&profile.server_url)?;
        if profile.token.trim().is_empty() {
            return Err(ConfigError::InvalidCredential);
        }
        Ok(profile)
    }

    pub fn save(
        &self,
        profile_name: &str,
        profile: &ClientProfile,
        replace: bool,
    ) -> Result<PathBuf, ConfigError> {
        crate::api::ServerOrigin::parse(&profile.server_url)?;
        if profile.token.trim().is_empty() {
            return Err(ConfigError::InvalidCredential);
        }
        let path = self.profile_path(profile_name)?;
        if path.exists() && !replace {
            return Err(ConfigError::ProfileExists(profile_name.into()));
        }
        create_private_directory(&self.directory)?;
        let temporary = self
            .directory
            .join(format!(".{profile_name}.{}.tmp", Uuid::new_v4().simple()));
        let result = write_private_file(&temporary, &serde_json::to_vec_pretty(profile)?)
            .and_then(|()| fs::rename(&temporary, &path));
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result?;
        set_private_file_permissions(&path)?;
        Ok(path)
    }

    pub fn profile_path(&self, profile: &str) -> Result<PathBuf, ConfigError> {
        validate_profile_name(profile)?;
        Ok(self.directory.join(format!("{profile}.json")))
    }
}

pub fn validate_profile_name(profile: &str) -> Result<(), ConfigError> {
    if profile.is_empty()
        || profile.len() > 32
        || !profile
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(ConfigError::InvalidProfileName);
    }
    Ok(())
}

fn create_private_directory(path: &Path) -> Result<(), std::io::Error> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn write_private_file(path: &Path, contents: &[u8]) -> Result<(), std::io::Error> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(contents)?;
    file.sync_all()
}

fn set_private_file_permissions(path: &Path) -> Result<(), std::io::Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn verify_private_file(path: &Path) -> Result<(), ConfigError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() {
        return Err(ConfigError::NotRegularFile(path.into()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(ConfigError::InsecurePermissions(path.into()));
        }
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("home directory is unavailable")]
    MissingHome,
    #[error("profile names may contain only letters, numbers, '-' and '_', up to 32 characters")]
    InvalidProfileName,
    #[error("the profile credential is empty")]
    InvalidCredential,
    #[error("profile '{0}' already exists; pass --replace to overwrite it")]
    ProfileExists(String),
    #[error("profile path is not a regular file: {0}")]
    NotRegularFile(PathBuf),
    #[error("profile file must not be accessible by group or other users: {0}")]
    InsecurePermissions(PathBuf),
    #[error(transparent)]
    InvalidServer(#[from] crate::api::ServerOriginError),
    #[error("profile JSON is invalid: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("profile could not be read or written: {0}")]
    Io(#[from] std::io::Error),
}

impl ConfigError {
    #[must_use]
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::Io(error) if error.kind() == std::io::ErrorKind::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> ClientProfile {
        ClientProfile {
            server_url: "https://luna.example.ts.net:8447".into(),
            device_id: Uuid::nil(),
            token: "secret-token".into(),
        }
    }

    #[test]
    fn saves_and_loads_a_profile() {
        let directory = tempfile::tempdir().expect("temp directory");
        let store = ProfileStore::new(directory.path().join("profiles"));
        let path = store.save("default", &profile(), false).expect("save");

        assert_eq!(store.load("default").expect("load"), profile());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(path).expect("metadata").permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(directory.path().join("profiles"))
                    .expect("directory metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
    }

    #[test]
    fn rejects_unsafe_profile_names() {
        let store = ProfileStore::new(PathBuf::from("/tmp/profiles"));
        for name in ["", "../token", "contains space", "slash/name"] {
            assert!(matches!(
                store.profile_path(name),
                Err(ConfigError::InvalidProfileName)
            ));
        }
    }

    #[cfg(unix)]
    #[test]
    fn rejects_an_insecure_profile_file() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("temp directory");
        let store = ProfileStore::new(directory.path().into());
        let path = store.profile_path("default").expect("path");
        fs::write(&path, serde_json::to_vec(&profile()).expect("JSON")).expect("write");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("permissions");

        assert!(matches!(
            store.load("default"),
            Err(ConfigError::InsecurePermissions(_))
        ));
    }
}

use crate::error::CoreError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_TTL_SECS: u64 = 600;

#[derive(Debug, Serialize, Deserialize)]
struct CacheFile {
    expires_at_unix: u64,
    pub payload: Value,
}

pub struct Cache {
    dir: PathBuf,
    enabled: bool,
}

impl Cache {
    /// One cache directory per API base, workspace and acting member (`--as`), so one
    /// member's project list (which depends on its memberships) never serves another.
    pub fn new(
        base_url: &str,
        workspace: &str,
        identity: Option<&str>,
        enabled: bool,
    ) -> Result<Self, CoreError> {
        let dir = directories::ProjectDirs::from("dev", "plane", "plane-cli")
            .map(|d| d.cache_dir().to_path_buf())
            .ok_or_else(|| CoreError::Cache("could not resolve cache directory".into()))?;
        let mut key = Sha256::new();
        key.update(base_url.as_bytes());
        key.update(b"\0");
        key.update(workspace.as_bytes());
        if let Some(member) = identity {
            key.update(b"\0as:");
            key.update(member.as_bytes());
        }
        let digest = key.finalize();
        let hash: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        let dir = dir.join(&hash[..16.min(hash.len())]);
        if enabled {
            fs::create_dir_all(&dir)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
            }
        }
        Ok(Self { dir, enabled })
    }

    fn path_for(&self, kind: &str) -> PathBuf {
        self.dir.join(format!("{kind}.json"))
    }

    pub fn get(&self, kind: &str, bypass: bool) -> Result<Option<Value>, CoreError> {
        if !self.enabled || bypass {
            return Ok(None);
        }
        let p = self.path_for(kind);
        if !p.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(&p)?;
        let cf: CacheFile = serde_json::from_str(&raw)
            .map_err(|e| CoreError::Cache(format!("corrupt cache {kind}: {e}")))?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        if now >= cf.expires_at_unix {
            return Ok(None);
        }
        Ok(Some(cf.payload))
    }

    pub fn put(&self, kind: &str, payload: Value) -> Result<(), CoreError> {
        self.put_with_ttl(kind, payload, DEFAULT_TTL_SECS)
    }

    /// Drop one cached entry. A missing entry, or caching turned off, is not an error; any
    /// other I/O failure is returned (callers that only tidy up may ignore it).
    pub fn remove(&self, kind: &str) -> Result<(), CoreError> {
        if !self.enabled {
            return Ok(());
        }
        match fs::remove_file(self.path_for(kind)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn put_with_ttl(&self, kind: &str, payload: Value, ttl_secs: u64) -> Result<(), CoreError> {
        if !self.enabled {
            return Ok(());
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let cf = CacheFile {
            expires_at_unix: now + ttl_secs,
            payload,
        };
        let p = self.path_for(kind);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_vec_pretty(&cf)?;
        fs::write(&p, &data)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&p, fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }
}

//! Global application config — device name, creation timestamp, recent
//! workspaces. Persisted to `{app_data_dir}/config.json`.
//!
//! Host provides the app data directory (desktop: `~/.swarmnote/`; mobile:
//! sandbox documentDirectory). Core code never resolves it.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock as TokioRwLock;
use tracing::info;

use crate::error::{AppError, AppResult};

const MAX_RECENT_WORKSPACES: usize = 10;

/// Runtime wrapper with read/write locking, held by `AppCore`.
pub struct GlobalConfigState {
    inner: TokioRwLock<GlobalConfig>,
    path: PathBuf,
}

impl GlobalConfigState {
    pub fn new(config: GlobalConfig, path: PathBuf) -> Self {
        Self {
            inner: TokioRwLock::new(config),
            path,
        }
    }

    pub async fn read(&self) -> tokio::sync::RwLockReadGuard<'_, GlobalConfig> {
        self.inner.read().await
    }

    pub async fn write(&self) -> tokio::sync::RwLockWriteGuard<'_, GlobalConfig> {
        self.inner.write().await
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Persist the current in-memory state to disk.
    pub async fn save(&self) -> AppResult<()> {
        let guard = self.inner.read().await;
        save_config(&self.path, &guard)
    }
}

/// Persisted config shape. `#[serde(default)]` on newer fields preserves
/// compatibility with older `config.json` files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub device_name: String,
    pub created_at: String,
    #[serde(default)]
    pub last_workspace_path: Option<String>,
    #[serde(default)]
    pub recent_workspaces: Vec<RecentWorkspace>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentWorkspace {
    pub path: String,
    pub name: String,
    pub last_opened_at: String,
    /// Workspace UUID, used by the frontend to match live sync state.
    #[serde(default)]
    pub uuid: Option<String>,
}

/// Load existing config from `{app_data_dir}/config.json`, or create a new
/// default and persist it.
pub fn load_or_create_config(app_data_dir: &Path) -> AppResult<GlobalConfig> {
    let path = app_data_dir.join("config.json");

    if path.exists() {
        let content = fs::read_to_string(&path)?;
        let config: GlobalConfig = serde_json::from_str(&content)
            .map_err(|e| AppError::ConfigParse(format!("invalid config JSON: {e}")))?;
        info!("Loaded global config from {}", path.display());
        return Ok(config);
    }

    let default_name = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "SwarmNote Device".to_string());

    let config = GlobalConfig {
        device_name: default_name,
        created_at: chrono::Utc::now().to_rfc3339(),
        last_workspace_path: None,
        recent_workspaces: Vec::new(),
    };

    save_config(&path, &config)?;
    info!("Created default global config at {}", path.display());
    Ok(config)
}

/// Persist config to the given path, creating parent dirs as needed.
pub fn save_config(path: &Path, config: &GlobalConfig) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(config)
        .map_err(|e| AppError::ConfigParse(format!("serialize config: {e}")))?;

    fs::write(path, json)?;
    Ok(())
}

/// Update `last_workspace_path` and maintain `recent_workspaces` (dedup by
/// path, LRU order, capped at 10).
pub fn update_last_workspace(
    state: &GlobalConfigState,
    config: &mut GlobalConfig,
    path: &str,
    name: &str,
) -> AppResult<()> {
    apply_workspace_update(config, path, name, None);
    save_config(state.path(), config)
}

pub fn update_last_workspace_with_uuid(
    state: &GlobalConfigState,
    config: &mut GlobalConfig,
    path: &str,
    name: &str,
    uuid: &str,
) -> AppResult<()> {
    apply_workspace_update(config, path, name, Some(uuid));
    save_config(state.path(), config)
}

pub(crate) fn apply_workspace_update(
    config: &mut GlobalConfig,
    path: &str,
    name: &str,
    uuid: Option<&str>,
) {
    let now = chrono::Utc::now().to_rfc3339();

    config.last_workspace_path = Some(path.to_owned());
    config.recent_workspaces.retain(|w| w.path != path);
    config.recent_workspaces.insert(
        0,
        RecentWorkspace {
            path: path.to_owned(),
            name: name.to_owned(),
            last_opened_at: now,
            uuid: uuid.map(|s| s.to_owned()),
        },
    );
    config.recent_workspaces.truncate(MAX_RECENT_WORKSPACES);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_config() -> GlobalConfig {
        GlobalConfig {
            device_name: "Test".to_owned(),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            last_workspace_path: None,
            recent_workspaces: Vec::new(),
        }
    }

    #[test]
    fn deserialize_old_format_without_workspace_fields() {
        let json = r#"{"device_name":"PC","created_at":"2026-01-01T00:00:00Z"}"#;
        let config: GlobalConfig = serde_json::from_str(json).unwrap();

        assert_eq!(config.device_name, "PC");
        assert!(config.last_workspace_path.is_none());
        assert!(config.recent_workspaces.is_empty());
    }

    #[test]
    fn deserialize_full_format() {
        let json = r#"{
            "device_name": "PC",
            "created_at": "2026-01-01T00:00:00Z",
            "last_workspace_path": "/tmp/notes",
            "recent_workspaces": [
                {"path": "/tmp/notes", "name": "notes", "last_opened_at": "2026-01-02T00:00:00Z"}
            ]
        }"#;
        let config: GlobalConfig = serde_json::from_str(json).unwrap();

        assert_eq!(config.last_workspace_path.as_deref(), Some("/tmp/notes"));
        assert_eq!(config.recent_workspaces.len(), 1);
        assert_eq!(config.recent_workspaces[0].name, "notes");
    }

    #[test]
    fn serialize_roundtrip() {
        let mut config = empty_config();
        config.last_workspace_path = Some("/tmp/ws".to_owned());
        config.recent_workspaces.push(RecentWorkspace {
            path: "/tmp/ws".to_owned(),
            name: "ws".to_owned(),
            last_opened_at: "2026-01-01T00:00:00Z".to_owned(),
            uuid: None,
        });

        let json = serde_json::to_string(&config).unwrap();
        let restored: GlobalConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.last_workspace_path, config.last_workspace_path);
        assert_eq!(restored.recent_workspaces.len(), 1);
    }

    #[test]
    fn update_sets_last_workspace_path() {
        let mut config = empty_config();
        apply_workspace_update(&mut config, "/tmp/notes", "notes", None);
        assert_eq!(config.last_workspace_path.as_deref(), Some("/tmp/notes"));
    }

    #[test]
    fn update_adds_to_recent_list() {
        let mut config = empty_config();
        apply_workspace_update(&mut config, "/tmp/a", "a", None);
        apply_workspace_update(&mut config, "/tmp/b", "b", None);

        assert_eq!(config.recent_workspaces.len(), 2);
        assert_eq!(config.recent_workspaces[0].path, "/tmp/b");
        assert_eq!(config.recent_workspaces[1].path, "/tmp/a");
    }

    #[test]
    fn update_deduplicates_by_path() {
        let mut config = empty_config();
        apply_workspace_update(&mut config, "/tmp/a", "a", None);
        apply_workspace_update(&mut config, "/tmp/b", "b", None);
        apply_workspace_update(&mut config, "/tmp/a", "a-renamed", None);

        assert_eq!(config.recent_workspaces.len(), 2);
        assert_eq!(config.recent_workspaces[0].path, "/tmp/a");
        assert_eq!(config.recent_workspaces[0].name, "a-renamed");
        assert_eq!(config.recent_workspaces[1].path, "/tmp/b");
    }

    #[test]
    fn update_caps_at_10_entries() {
        let mut config = empty_config();
        for i in 0..15 {
            apply_workspace_update(&mut config, &format!("/tmp/{i}"), &format!("{i}"), None);
        }

        assert_eq!(config.recent_workspaces.len(), MAX_RECENT_WORKSPACES);
        assert_eq!(config.recent_workspaces[0].path, "/tmp/14");
        assert_eq!(config.recent_workspaces[9].path, "/tmp/5");
    }
}

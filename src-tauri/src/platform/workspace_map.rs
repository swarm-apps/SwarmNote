//! Desktop-side map of **window label → `Arc<WorkspaceCore>`** plus the
//! helper that spins up a fresh `WorkspaceCore` for a window.
//!
//! Keeps a strong reference alive for each window that has a workspace
//! bound to it. When the last window referencing a workspace UUID is
//! closed, [`crate::workspace::cleanup_window`] calls
//! [`AppCore::close_workspace`] to flush + unwatch + close-db.
//!
//! Legacy fs / document / yjs commands still go through the pre-extraction
//! Tauri State; [`WorkspaceMap::get`] exists for the command cut-over in
//! the follow-up change.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use swarmnote_core::{AppCore, WorkspaceCore};
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;
use uuid::Uuid;

pub struct WorkspaceMap(Mutex<HashMap<String, Arc<WorkspaceCore>>>);

impl WorkspaceMap {
    pub fn new() -> Self {
        Self(Mutex::new(HashMap::new()))
    }

    pub async fn bind(&self, label: &str, core: Arc<WorkspaceCore>) {
        self.0.lock().await.insert(label.to_owned(), core);
    }

    /// Remove the label binding. Returns the UUID of the workspace that was
    /// bound and a flag indicating whether this was the last label
    /// referring to that workspace (caller uses it to decide whether to
    /// close the workspace entirely).
    pub async fn unbind(&self, label: &str) -> Option<(Uuid, bool)> {
        let mut guard = self.0.lock().await;
        let core = guard.remove(label)?;
        let workspace_id = core.id();
        // Drop our strong reference, then check whether any other label
        // still holds a reference to the same workspace.
        drop(core);
        let still_bound = guard.values().any(|other| other.id() == workspace_id);
        Some((workspace_id, !still_bound))
    }

    /// Look up the `Arc<WorkspaceCore>` for a window label.
    pub async fn get(&self, label: &str) -> Option<Arc<WorkspaceCore>> {
        self.0.lock().await.get(label).cloned()
    }

    /// Full `(label, Arc<WorkspaceCore>)` snapshot. Used by commands that
    /// need to search across all bound workspaces (e.g.
    /// `open_workspace_window` matching on `path`).
    pub async fn snapshot(&self) -> Vec<(String, Arc<WorkspaceCore>)> {
        self.0
            .lock()
            .await
            .iter()
            .map(|(k, v)| (k.clone(), Arc::clone(v)))
            .collect()
    }
}

impl Default for WorkspaceMap {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a workspace runtime for `ws_path` via the registered factories on
/// `AppCore` (desktop: `LocalFs` + `NotifyFileWatcher`) and bind it to `label`.
///
/// Returns the `Arc` so callers can chain additional wiring; the map
/// binding has already happened when this returns `Ok`.
pub async fn start_core_workspace(
    app: &AppHandle,
    ws_path: &Path,
    label: &str,
) -> Result<Arc<WorkspaceCore>, swarmnote_core::AppError> {
    let app_core = app.try_state::<Arc<AppCore>>().ok_or_else(|| {
        swarmnote_core::AppError::ConfigParse("AppCore not registered — host setup missing".into())
    })?;

    let core = app_core.inner().clone().open_workspace(ws_path).await?;

    if let Some(map) = app.try_state::<WorkspaceMap>() {
        map.bind(label, Arc::clone(&core)).await;
    }

    Ok(core)
}

//! Tauri IPC commands for workspace / window management.
//!
//! These commands stay in the desktop shell because they drive `WebviewWindow`
//! lifecycles. Core state (DB, fs, Y.Doc) is reached through [`AppCore`] +
//! [`WorkspaceMap`]; this file only contains window plumbing and recent
//! workspaces bookkeeping (persisted via [`swarmnote_core::config`]).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use serde::Serialize;
use swarmnote_core::config::{save_config, RecentWorkspace};
use swarmnote_core::{AppCore, WorkspaceInfo};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::platform::{workspace_map::start_core_workspace, WorkspaceMap};

/// 应用平台相关的窗口装饰配置。
/// macOS: Overlay 标题栏 + 隐藏标题 + 红绿灯定位；其他平台: 无装饰（自定义标题栏）。
fn with_platform_decorations<'a, R: tauri::Runtime, M: tauri::Manager<R>>(
    builder: WebviewWindowBuilder<'a, R, M>,
) -> WebviewWindowBuilder<'a, R, M> {
    #[cfg(target_os = "macos")]
    let builder = {
        use tauri::TitleBarStyle;
        builder
            .decorations(true)
            .title_bar_style(TitleBarStyle::Overlay)
            .hidden_title(true)
            .traffic_light_position(tauri::LogicalPosition::new(15.0, 22.0))
    };

    #[cfg(not(target_os = "macos"))]
    let builder = builder.decorations(false);

    builder
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum OpenWorkspaceWindowResult {
    BoundToCaller { info: WorkspaceInfo },
    FocusedExisting,
    NewWindow,
}

fn workspace_window_label(path: &str) -> String {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    format!("ws-{:016x}", hasher.finish())
}

const MAX_RECENT: usize = 10;

/// Update `last_workspace_path` + maintain `recent_workspaces`, then persist.
async fn update_last_workspace(
    core: &Arc<AppCore>,
    path: &str,
    name: &str,
    uuid: Option<&str>,
) -> AppResult<()> {
    let mut cfg = core.config().write().await;
    cfg.last_workspace_path = Some(path.to_owned());
    cfg.recent_workspaces.retain(|w| w.path != path);
    cfg.recent_workspaces.insert(
        0,
        RecentWorkspace {
            path: path.to_owned(),
            name: name.to_owned(),
            last_opened_at: Utc::now().to_rfc3339(),
            uuid: uuid.map(|s| s.to_owned()),
        },
    );
    cfg.recent_workspaces.truncate(MAX_RECENT);
    save_config(core.config().path(), &cfg)?;
    Ok(())
}

fn workspace_name_from_path(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Workspace".to_owned())
}

/// Bind a workspace path to a window label and update recent list / watcher /
/// sync subscription.
async fn bind_workspace_to_window(
    app: &AppHandle,
    label: &str,
    path: &Path,
    core: &Arc<AppCore>,
) -> AppResult<WorkspaceInfo> {
    let ws_core = start_core_workspace(app, path, label).await?;
    let info = ws_core.info().clone();

    let dir_name = workspace_name_from_path(path);
    let path_str = info.path.clone();
    if let Err(e) = update_last_workspace(core, &path_str, &dir_name, None).await {
        log::warn!("Failed to update global config: {e}");
    }

    // Refresh tray menu (recent workspaces list changed).
    #[cfg(desktop)]
    {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            crate::tray::refresh_tray_menu(&app).await;
        });
    }

    Ok(info)
}

/// Register a window-close listener that tears down per-window core state.
fn bind_window_cleanup(window: &WebviewWindow, app: &AppHandle, label: &str) {
    let cleanup_label = label.to_owned();
    let cleanup_app = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            let lbl = cleanup_label.clone();
            let a = cleanup_app.clone();
            tauri::async_runtime::spawn(async move {
                cleanup_window(&a, &lbl).await;
            });
        }
    });
}

/// Release per-window resources when a window is destroyed.
pub async fn cleanup_window(app: &AppHandle, label: &str) {
    if let Some(ws_map) = app.try_state::<WorkspaceMap>() {
        if let Some((workspace_uuid, last)) = ws_map.unbind(label).await {
            if last {
                if let Some(core) = app.try_state::<Arc<AppCore>>() {
                    if let Err(e) = core.close_workspace(workspace_uuid).await {
                        tracing::warn!("close_workspace({workspace_uuid}) failed: {e}");
                    }
                }
            }
        }
    }
}

// ── Tauri commands ──

/// Idempotently open / create a workspace and bind it to the invoking window.
#[tauri::command]
pub async fn open_workspace(
    window: tauri::Window,
    path: String,
    core: State<'_, Arc<AppCore>>,
    app: AppHandle,
) -> AppResult<WorkspaceInfo> {
    let ws_path = PathBuf::from(&path);
    if !ws_path.is_dir() {
        return Err(AppError::InvalidPath(format!(
            "Directory does not exist: {path}"
        )));
    }
    bind_workspace_to_window(&app, window.label(), &ws_path, core.inner()).await
}

/// Return info for the workspace currently bound to this window.
#[tauri::command]
pub async fn get_workspace_info(
    window: tauri::Window,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<Option<WorkspaceInfo>> {
    Ok(ws_map.get(window.label()).await.map(|ws| ws.info().clone()))
}

#[tauri::command]
pub async fn get_recent_workspaces(
    core: State<'_, Arc<AppCore>>,
) -> AppResult<Vec<RecentWorkspace>> {
    let cfg = core.config().read().await;
    Ok(cfg.recent_workspaces.clone())
}

fn maybe_close_window(app: &AppHandle, label: &Option<String>) {
    if let Some(close_label) = label {
        if let Some(win) = app.get_webview_window(close_label) {
            #[cfg(debug_assertions)]
            if close_label == "main" {
                let _ = win.hide();
                return;
            }
            let _ = win.destroy();
        }
    }
}

/// Find an existing window already bound to `path`.
async fn find_label_for_path(ws_map: &WorkspaceMap, path: &str) -> Option<String> {
    let map = ws_map.snapshot().await;
    map.into_iter()
        .find_map(|(label, ws)| (ws.info().path == path).then_some(label))
}

/// Open a workspace window: reuse existing / bind to caller / create new.
#[tauri::command]
pub async fn open_workspace_window(
    app: AppHandle,
    path: String,
    bind_to_window: Option<String>,
    close_window: Option<String>,
    core: State<'_, Arc<AppCore>>,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<OpenWorkspaceWindowResult> {
    tracing::info!("open_workspace_window called: path={path}, bind_to_window={bind_to_window:?}");

    // Priority 1: path already open in some window → focus it.
    if let Some(existing_label) = find_label_for_path(&ws_map, &path).await {
        tracing::info!(
            "open_workspace_window: FocusedExisting via path match, label={existing_label}"
        );
        if let Some(win) = app.get_webview_window(&existing_label) {
            let _ = win.set_focus();
        }
        maybe_close_window(&app, &close_window);
        return Ok(OpenWorkspaceWindowResult::FocusedExisting);
    }

    let hashed_label = workspace_window_label(&path);
    if let Some(existing) = app.get_webview_window(&hashed_label) {
        tracing::info!("open_workspace_window: FocusedExisting via hashed_label={hashed_label}");
        existing
            .set_focus()
            .map_err(|e| AppError::InvalidPath(format!("failed to focus window: {e}")))?;
        maybe_close_window(&app, &close_window);
        return Ok(OpenWorkspaceWindowResult::FocusedExisting);
    }

    let ws_path = PathBuf::from(&path);
    if !ws_path.is_dir() {
        return Err(AppError::InvalidPath(format!(
            "Directory does not exist: {path}"
        )));
    }

    // Priority 2: caller requested binding to self and it's unbound → bind.
    if let Some(caller_label) = bind_to_window.as_deref() {
        let caller_has_ws = ws_map.get(caller_label).await.is_some();
        let caller_window = app.get_webview_window(caller_label);
        if !caller_has_ws && caller_window.is_some() {
            let info = bind_workspace_to_window(&app, caller_label, &ws_path, core.inner()).await?;

            if let Some(win) = caller_window {
                if let Err(e) = win.emit("workspace:ready", &info) {
                    log::warn!(
                        "Failed to emit workspace:ready to caller window '{caller_label}': {e}"
                    );
                }
                let _ = win.set_focus();
            }

            maybe_close_window(&app, &close_window);
            return Ok(OpenWorkspaceWindowResult::BoundToCaller { info });
        }
    }

    // Priority 3: create a new window with a stable hash-based label.
    let label = hashed_label;
    let info = bind_workspace_to_window(&app, &label, &ws_path, core.inner()).await?;

    let new_window = with_platform_decorations(
        WebviewWindowBuilder::new(&app, &label, WebviewUrl::App("index.html".into()))
            .title("SwarmNote")
            .inner_size(800.0, 600.0)
            .min_inner_size(800.0, 600.0),
    )
    .build()
    .map_err(|e| AppError::InvalidPath(format!("failed to create window: {e}")))?;

    if let Err(e) = new_window.emit("workspace:ready", &info) {
        log::warn!("Failed to emit workspace:ready to window '{label}': {e}");
    }

    bind_window_cleanup(&new_window, &app, &label);
    maybe_close_window(&app, &close_window);

    Ok(OpenWorkspaceWindowResult::NewWindow)
}

/// Create a workspace for sync (no window, pre-assigned UUID).
#[tauri::command]
pub async fn create_workspace_for_sync(
    uuid: String,
    name: String,
    base_path: String,
    core: State<'_, Arc<AppCore>>,
) -> AppResult<String> {
    let ws_uuid =
        Uuid::parse_str(&uuid).map_err(|e| AppError::InvalidPath(format!("Invalid UUID: {e}")))?;

    // Validate workspace name — prevent path traversal
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
        || name == "."
    {
        return Err(AppError::InvalidPath(format!(
            "Invalid workspace name: {name}"
        )));
    }

    let base = PathBuf::from(&base_path);
    if !base.is_dir() {
        tokio::fs::create_dir_all(&base).await.map_err(|e| {
            AppError::InvalidPath(format!("Failed to create base directory {base_path}: {e}"))
        })?;
    }
    let ws_path = base.join(&name);
    let ws_path_str = ws_path
        .to_str()
        .ok_or_else(|| AppError::InvalidPath("Workspace path is not valid UTF-8".to_owned()))?
        .to_owned();

    tokio::fs::create_dir_all(&ws_path)
        .await
        .map_err(AppError::Io)?;

    // Initialize workspace DB with pre-assigned UUID.
    let conn = match swarmnote_core::workspace::db::init_workspace_db(&ws_path).await {
        Ok(c) => c,
        Err(e) => {
            let _ = tokio::fs::remove_dir_all(&ws_path).await;
            return Err(e);
        }
    };

    let peer_id = match core.identity().peer_id() {
        Ok(p) => p,
        Err(e) => {
            let _ = tokio::fs::remove_dir_all(&ws_path).await;
            return Err(e);
        }
    };

    if let Err(e) = swarmnote_core::ensure_workspace_row(&conn, ws_uuid, &name, &peer_id).await {
        let _ = tokio::fs::remove_dir_all(&ws_path).await;
        return Err(e);
    }
    drop(conn); // release before open_workspace re-opens it

    // Register in AppCore by opening. This doesn't bind to any window —
    // caller drops the Arc and AppCore.workspaces keeps only the Weak.
    let _ws_core = core.inner().clone().open_workspace(ws_path.clone()).await?;

    // Record in recent_workspaces.
    if let Err(e) = update_last_workspace(
        core.inner(),
        &ws_path_str,
        &name,
        Some(&ws_uuid.to_string()),
    )
    .await
    {
        tracing::warn!("Failed to update global config for sync workspace: {e}");
    }

    tracing::info!("Created workspace for sync: {name} ({ws_uuid}) at {ws_path_str}");
    Ok(ws_path_str)
}

/// Create the onboarding window (called once at startup if needed).
pub fn create_onboarding_window(app: &AppHandle) -> AppResult<()> {
    with_platform_decorations(
        WebviewWindowBuilder::new(app, "onboarding", WebviewUrl::App("/onboarding".into()))
            .title("SwarmNote")
            .inner_size(600.0, 500.0)
            .min_inner_size(600.0, 500.0)
            .maximizable(false),
    )
    .build()
    .map_err(|e| AppError::Window(format!("Failed to create onboarding window: {e}")))?;
    Ok(())
}

#[tauri::command]
pub async fn finish_onboarding(app: AppHandle) -> AppResult<()> {
    open_workspace_manager_window(app.clone()).await?;
    if let Some(win) = app.get_webview_window("onboarding") {
        let _ = win.destroy();
    }
    Ok(())
}

#[tauri::command]
pub async fn remove_recent_workspace(
    app: AppHandle,
    path: String,
    core: State<'_, Arc<AppCore>>,
) -> AppResult<()> {
    let mut cfg = core.config().write().await;
    cfg.recent_workspaces.retain(|w| w.path != path);
    if cfg.last_workspace_path.as_deref() == Some(&path) {
        cfg.last_workspace_path = cfg.recent_workspaces.first().map(|w| w.path.clone());
    }
    save_config(core.config().path(), &cfg)?;
    drop(cfg);

    #[cfg(desktop)]
    crate::tray::refresh_tray_menu(&app).await;
    let _ = app;
    Ok(())
}

#[tauri::command]
pub async fn open_workspace_manager_window(app: AppHandle) -> AppResult<()> {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
        return Ok(());
    }

    with_platform_decorations(
        WebviewWindowBuilder::new(&app, "main", WebviewUrl::App("/workspace-manager".into()))
            .title("SwarmNote")
            .inner_size(780.0, 620.0)
            .min_inner_size(780.0, 620.0)
            .maximizable(false),
    )
    .build()
    .map_err(|e| AppError::Window(format!("Failed to create workspace manager window: {e}")))?;
    Ok(())
}

#[tauri::command]
pub async fn open_settings_window(app: AppHandle, route: Option<String>) -> AppResult<()> {
    let target_route = format!(
        "/settings/{}",
        route.unwrap_or_else(|| "general".to_string())
    );

    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.set_focus();
        let _ = win.emit("navigate", &target_route);
        return Ok(());
    }

    let _ = with_platform_decorations(
        WebviewWindowBuilder::new(&app, "settings", WebviewUrl::App(target_route.into()))
            .title("SwarmNote 设置")
            .inner_size(720.0, 520.0)
            .resizable(false),
    )
    .build()
    .map_err(|e| AppError::Window(format!("Failed to create settings window: {e}")))?;
    Ok(())
}

// ── Startup window dispatch ──

pub enum StartupWindow {
    Onboarding,
    WorkspaceManager,
    RestoreWorkspace(String),
}

/// Read a bool from a tauri-plugin-store settings.json Zustand-persisted
/// state: `{ "store-key": "{\"state\":{\"field\":true},\"version\":0}" }`.
fn read_store_bool(store: &serde_json::Value, store_key: &str, field: &str) -> Option<bool> {
    let inner_str = store.get(store_key)?.as_str()?;
    let inner: serde_json::Value = serde_json::from_str(inner_str).ok()?;
    inner.get("state")?.get(field)?.as_bool()
}

pub fn determine_startup_window(app: &AppHandle, core: &Arc<AppCore>) -> StartupWindow {
    let store = (|| -> Option<serde_json::Value> {
        let app_data = app.path().app_data_dir().ok()?;
        let content = std::fs::read_to_string(app_data.join("settings.json")).ok()?;
        serde_json::from_str(&content).ok()
    })()
    .unwrap_or_default();

    let onboarding_completed =
        read_store_bool(&store, "swarmnote-onboarding", "isCompleted").unwrap_or(false);

    if !onboarding_completed {
        return StartupWindow::Onboarding;
    }

    let restore_last =
        read_store_bool(&store, "swarmnote-preferences", "restoreLastWorkspace").unwrap_or(true);

    if restore_last {
        let last_path = tauri::async_runtime::block_on(async {
            core.config().read().await.last_workspace_path.clone()
        });
        if let Some(path) = last_path {
            if !path.is_empty() && PathBuf::from(&path).is_dir() {
                return StartupWindow::RestoreWorkspace(path);
            }
        }
    }

    StartupWindow::WorkspaceManager
}

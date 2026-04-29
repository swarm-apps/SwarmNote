mod commands;
pub mod error;
mod platform;
#[cfg(desktop)]
pub mod tray;

use std::path::PathBuf;
use std::sync::Arc;

use swarmnote_core::{AppCore, AppCoreBuilder};
use tauri::Manager;

/// Desktop config directory: `~/.swarmnote/`. Used both to bootstrap
/// [`AppCore`] and by the `config::*` helpers inside the commands module.
fn swarmnote_global_dir() -> Result<PathBuf, swarmnote_core::AppError> {
    let home = directories::BaseDirs::new().ok_or(swarmnote_core::AppError::NoAppDataDir)?;
    Ok(home.home_dir().join(".swarmnote"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "swarmnote_lib=info,swarm_p2p_core=info".into()),
        )
        .init();

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init());

    #[cfg(debug_assertions)]
    {
        builder = builder.plugin(tauri_plugin_mcp_bridge::init());
    }

    builder
        .invoke_handler(tauri::generate_handler![
            // 设备身份
            commands::identity::get_device_info,
            commands::identity::set_device_name,
            // 工作区管理
            commands::workspace::open_workspace,
            commands::workspace::get_workspace_info,
            commands::workspace::get_recent_workspaces,
            commands::workspace::open_workspace_window,
            commands::workspace::finish_onboarding,
            commands::workspace::remove_recent_workspace,
            commands::workspace::open_workspace_manager_window,
            commands::workspace::open_settings_window,
            commands::workspace::create_workspace_for_sync,
            // 文档 & 文件夹
            commands::document::db_get_documents,
            commands::document::db_upsert_document,
            commands::document::delete_document_by_rel_path,
            commands::document::delete_documents_by_prefix,
            commands::document::rename_document,
            commands::document::move_document,
            commands::document::db_get_folders,
            commands::document::db_create_folder,
            commands::document::db_delete_folder,
            // 文件系统
            commands::fs::scan_workspace_tree,
            commands::fs::fs_create_file,
            commands::fs::fs_create_dir,
            commands::fs::fs_delete_file,
            commands::fs::fs_delete_dir,
            commands::fs::fs_rename,
            commands::fs::load_document,
            commands::fs::save_document,
            commands::fs::save_media,
            // P2P 网络
            commands::network::start_p2p_node,
            commands::network::stop_p2p_node,
            commands::network::get_network_status,
            commands::network::get_connected_peers,
            // 配对管理
            commands::pairing::generate_pairing_code,
            commands::pairing::get_device_by_code,
            commands::pairing::request_pairing,
            commands::pairing::respond_pairing_request,
            commands::pairing::get_paired_devices,
            commands::pairing::unpair_device,
            commands::pairing::get_nearby_devices,
            commands::pairing::list_devices,
            commands::pairing::get_remote_workspaces,
            // Y.Doc 管理
            commands::yjs::open_ydoc,
            commands::yjs::apply_ydoc_update,
            commands::yjs::close_ydoc,
            commands::yjs::rename_ydoc,
            commands::yjs::reload_ydoc_confirmed,
            commands::yjs::hydrate_workspace,
            // 同步
            commands::sync::trigger_workspace_sync,
        ])
        .on_window_event(|window, event| {
            #[cfg(desktop)]
            {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    let label = window.label();
                    let is_workspace = label.starts_with("ws-");

                    if is_workspace {
                        let visible_ws_count = window
                            .app_handle()
                            .webview_windows()
                            .iter()
                            .filter(|(l, w)| {
                                l.starts_with("ws-") && w.is_visible().unwrap_or(false)
                            })
                            .count();

                        if visible_ws_count <= 1 {
                            // Last workspace window: hide to tray, remember label.
                            api.prevent_close();
                            let _ = window.hide();
                            if let Some(state) =
                                window.app_handle().try_state::<tray::TrayManagerState>()
                            {
                                if let Ok(mut mgr) = state.try_lock() {
                                    mgr.set_last_hidden(label);
                                }
                            }
                        }
                    }
                }
            }
        })
        .setup(|app| {
            // Bootstrap the platform-independent core.
            let app_data_dir = swarmnote_global_dir()?;
            let keychain = Arc::new(platform::DesktopKeychain::new());
            let event_bus = Arc::new(platform::TauriEventBus::new(app.handle().clone()));
            let app_core = tauri::async_runtime::block_on(
                AppCoreBuilder::new(keychain, event_bus, app_data_dir)
                    .with_watcher_factory(|p| Arc::new(platform::NotifyFileWatcher::new(p)))
                    .build(),
            )?;
            app.manage(app_core.clone());
            app.manage(platform::WorkspaceMap::new());

            // System tray (desktop only).
            #[cfg(desktop)]
            {
                tray::TrayManager::init(app.handle())?;
            }

            // Launch the appropriate startup window.
            let handle = app.handle().clone();
            match commands::workspace::determine_startup_window(&handle, &app_core) {
                commands::workspace::StartupWindow::Onboarding => {
                    commands::workspace::create_onboarding_window(&handle)?;
                }
                commands::workspace::StartupWindow::WorkspaceManager => {
                    tauri::async_runtime::block_on(async {
                        if let Err(e) =
                            commands::workspace::open_workspace_manager_window(handle.clone()).await
                        {
                            log::error!("Failed to create workspace manager window: {e}");
                        }
                    });
                }
                commands::workspace::StartupWindow::RestoreWorkspace(path) => {
                    let handle2 = handle.clone();
                    tauri::async_runtime::block_on(async {
                        let core = handle2.state::<Arc<AppCore>>();
                        let ws_map = handle2.state::<platform::WorkspaceMap>();
                        if let Err(e) = commands::workspace::open_workspace_window(
                            handle2.clone(),
                            path,
                            None,
                            None,
                            core,
                            ws_map,
                        )
                        .await
                        {
                            log::error!("Failed to restore workspace: {e}");
                            if let Err(e2) =
                                commands::workspace::open_workspace_manager_window(handle2).await
                            {
                                log::error!("Failed to create workspace manager window: {e2}");
                            }
                        }
                    });
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

mod commands;
pub mod error;
pub mod events;
mod platform;
mod setup;
#[cfg(desktop)]
pub mod tray;

pub use setup::specta_builder;

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

    // TS bindings 导出走专用测试 `cargo test --test specta_export`,避免每次
    // `pnpm tauri dev` 启动都重写 `src/lib/bindings.ts` 触发 Vite reload。
    let specta = setup::specta_builder();

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
        .invoke_handler(specta.invoke_handler())
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
        .setup(move |app| {
            // tauri-specta events —— 在 setup 内 mount,这样 Event 的 listen()
            // 才能接收到 emit。须在 manage app_core 之前调用避免漏接首批事件。
            specta.mount_events(app);

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
            app.manage(platform::SyncPendingMap::new());

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

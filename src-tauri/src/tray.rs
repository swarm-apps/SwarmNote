use tauri::{
    image::Image,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};
use tracing::warn;

// ── 枚举 ──

/// 托盘内部状态：节点是否运行 + 连接设备数
#[derive(Clone, Copy)]
pub enum TrayNodeStatus {
    Stopped,
    Running { peer_count: usize },
}

/// 托盘图标种类，编译期保证不会拼错
#[derive(Clone, Copy)]
enum IconKind {
    Gray,
    Normal,
    #[allow(dead_code)]
    Yellow,
}

impl IconKind {
    fn load(self) -> Image<'static> {
        let bytes: &[u8] = match self {
            Self::Normal => include_bytes!("../icons/tray/icon-normal.png"),
            Self::Yellow => include_bytes!("../icons/tray/icon-yellow.png"),
            Self::Gray => include_bytes!("../icons/tray/icon-gray.png"),
        };
        Image::from_bytes(bytes).expect("Failed to load tray icon")
    }
}

/// 菜单项 ID 前缀/常量
const RECENT_WS_PREFIX: &str = "recent-ws:";
const MAX_TRAY_RECENT: usize = 5;

/// 固定菜单项 ID
enum MenuAction {
    Open,
    WorkspaceManager,
    ToggleSync,
    Settings,
    Quit,
}

impl MenuAction {
    const STATUS_ID: &str = "status";

    fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::WorkspaceManager => "workspace-manager",
            Self::ToggleSync => "toggle-sync",
            Self::Settings => "settings",
            Self::Quit => "quit",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s {
            "open" => Some(Self::Open),
            "workspace-manager" => Some(Self::WorkspaceManager),
            "toggle-sync" => Some(Self::ToggleSync),
            "settings" => Some(Self::Settings),
            "quit" => Some(Self::Quit),
            _ => None,
        }
    }
}

// ── TrayManager：封装所有托盘状态和行为 ──

/// 系统托盘管理器，封装 TrayIcon 及其状态更新逻辑
pub struct TrayManager {
    tray: TrayIcon,
    app: AppHandle,
    status: TrayNodeStatus,
    /// 最后一个被隐藏到托盘的工作区窗口 label
    last_hidden_label: String,
}

impl TrayManager {
    /// 在 `setup` 中创建托盘并注册到 Tauri State
    pub fn init(app: &AppHandle) -> tauri::Result<()> {
        let status = TrayNodeStatus::Stopped;
        let recent = Self::load_recent_workspaces_sync(app);
        let menu = Self::build_menu(app, status, &recent)?;

        let tray = TrayIconBuilder::with_id("main-tray")
            .icon(IconKind::Gray.load())
            .tooltip("SwarmNote")
            .menu(&menu)
            .show_menu_on_left_click(false)
            .on_menu_event(Self::on_menu_event)
            .on_tray_icon_event(Self::on_tray_icon_event)
            .build(app)?;

        let manager = Self {
            tray,
            app: app.clone(),
            status,
            last_hidden_label: String::new(),
        };
        app.manage(tokio::sync::Mutex::new(manager));
        Ok(())
    }

    /// 记录最后被隐藏到托盘的工作区窗口
    pub fn set_last_hidden(&mut self, label: &str) {
        self.last_hidden_label = label.to_string();
    }

    /// 更新节点状态 — 唯一的公开状态变更入口
    pub async fn set_status(&mut self, status: TrayNodeStatus) {
        self.status = status;
        self.refresh_icon();
        self.refresh_menu().await;
    }

    /// 重建托盘菜单（在最近工作区列表变更后调用）
    pub async fn refresh_menu(&self) {
        let recent = Self::load_recent_workspaces(&self.app).await;
        if let Ok(menu) = Self::build_menu(&self.app, self.status, &recent) {
            let _ = self.tray.set_menu(Some(menu));
        }
    }

    fn refresh_icon(&self) {
        let kind = match self.status {
            TrayNodeStatus::Running { .. } => IconKind::Normal,
            TrayNodeStatus::Stopped => IconKind::Gray,
        };
        let _ = self.tray.set_icon(Some(kind.load()));
    }

    /// 恢复最后隐藏的工作区窗口，或打开工作区管理窗口。
    fn restore_window(&self) {
        let label = &self.last_hidden_label;
        if !label.is_empty() {
            if let Some(window) = self.app.get_webview_window(label) {
                let _ = window.show();
                let _ = window.set_focus();
                return;
            }
        }
        // 没有可恢复的窗口，打开工作区管理窗口
        let handle = self.app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = crate::commands::workspace::open_workspace_manager_window(handle).await
            {
                warn!("Failed to open workspace manager from tray: {e}");
            }
        });
    }

    /// 同步版本：init 阶段（setup 中，无 tokio context）使用。
    fn load_recent_workspaces_sync(
        app: &AppHandle,
    ) -> Vec<swarmnote_core::config::RecentWorkspace> {
        let Some(core) = app.try_state::<std::sync::Arc<swarmnote_core::AppCore>>() else {
            return Vec::new();
        };
        tauri::async_runtime::block_on(async {
            core.config()
                .read()
                .await
                .recent_workspaces
                .iter()
                .take(MAX_TRAY_RECENT)
                .cloned()
                .collect()
        })
    }

    /// 异步版本：运行时刷新（已在 tokio context 中）使用。
    async fn load_recent_workspaces(
        app: &AppHandle,
    ) -> Vec<swarmnote_core::config::RecentWorkspace> {
        let Some(core) = app.try_state::<std::sync::Arc<swarmnote_core::AppCore>>() else {
            return Vec::new();
        };
        let config = core.config().read().await;
        let result = config
            .recent_workspaces
            .iter()
            .take(MAX_TRAY_RECENT)
            .cloned()
            .collect();
        drop(config);
        result
    }

    fn build_menu(
        app: &AppHandle,
        status: TrayNodeStatus,
        recent: &[swarmnote_core::config::RecentWorkspace],
    ) -> tauri::Result<Menu<tauri::Wry>> {
        let menu = Menu::new(app)?;

        // 状态行（不可点击）
        let status_text = match status {
            TrayNodeStatus::Running { peer_count: 0 } => "P2P 已连接".to_string(),
            TrayNodeStatus::Running { peer_count } => format!("P2P 已连接 · {peer_count} 台设备"),
            TrayNodeStatus::Stopped => "P2P 未连接".to_string(),
        };
        menu.append(&MenuItem::with_id(
            app,
            MenuAction::STATUS_ID,
            &status_text,
            false,
            None::<&str>,
        )?)?;

        menu.append(&PredefinedMenuItem::separator(app)?)?;

        // 最近工作区列表
        if !recent.is_empty() {
            for ws in recent {
                let id = format!("{RECENT_WS_PREFIX}{}", ws.path);
                menu.append(&MenuItem::with_id(app, &id, &ws.name, true, None::<&str>)?)?;
            }
            menu.append(&PredefinedMenuItem::separator(app)?)?;
        }

        // 打开 SwarmNote（恢复隐藏窗口）
        menu.append(&MenuItem::with_id(
            app,
            MenuAction::Open.as_str(),
            "打开 SwarmNote",
            true,
            None::<&str>,
        )?)?;

        // 工作区管理
        menu.append(&MenuItem::with_id(
            app,
            MenuAction::WorkspaceManager.as_str(),
            "工作区管理",
            true,
            None::<&str>,
        )?)?;

        let toggle_text = match status {
            TrayNodeStatus::Running { .. } => "暂停同步",
            TrayNodeStatus::Stopped => "恢复同步",
        };
        menu.append(&MenuItem::with_id(
            app,
            MenuAction::ToggleSync.as_str(),
            toggle_text,
            true,
            None::<&str>,
        )?)?;

        menu.append(&PredefinedMenuItem::separator(app)?)?;

        menu.append(&MenuItem::with_id(
            app,
            MenuAction::Settings.as_str(),
            "设置",
            true,
            None::<&str>,
        )?)?;

        menu.append(&MenuItem::with_id(
            app,
            MenuAction::Quit.as_str(),
            "退出 SwarmNote",
            true,
            None::<&str>,
        )?)?;

        Ok(menu)
    }

    // ── 事件处理 ──

    fn on_menu_event(app: &AppHandle, event: MenuEvent) {
        let id = event.id().as_ref().to_string();

        // 检查是否为最近工作区点击
        if let Some(path) = id.strip_prefix(RECENT_WS_PREFIX) {
            let path = path.to_string();
            let handle = app.clone();
            tauri::async_runtime::spawn(async move {
                let Some(core) = handle.try_state::<std::sync::Arc<swarmnote_core::AppCore>>()
                else {
                    warn!("open_workspace_window: AppCore not registered");
                    return;
                };
                let ws_map = handle.state::<crate::platform::WorkspaceMap>();
                if let Err(e) = crate::commands::workspace::open_workspace_window(
                    handle.clone(),
                    path,
                    None,
                    None,
                    core,
                    ws_map,
                )
                .await
                {
                    warn!("Failed to open workspace from tray: {e}");
                }
            });
            return;
        }

        let Some(action) = MenuAction::from_str(&id) else {
            return;
        };
        match action {
            MenuAction::Open => {
                if let Some(state) = app.try_state::<TrayManagerState>() {
                    if let Ok(mgr) = state.try_lock() {
                        mgr.restore_window();
                    }
                }
            }
            MenuAction::WorkspaceManager => {
                let handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) =
                        crate::commands::workspace::open_workspace_manager_window(handle).await
                    {
                        warn!("Failed to open workspace manager: {e}");
                    }
                });
            }
            MenuAction::ToggleSync => {
                let handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    toggle_sync(&handle).await;
                });
            }
            MenuAction::Settings => {
                let handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = crate::commands::workspace::open_settings_window(
                        handle,
                        Some("network".into()),
                    )
                    .await
                    {
                        warn!("Failed to open settings window: {e}");
                    }
                });
            }
            MenuAction::Quit => {
                let handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Some(core) =
                        handle.try_state::<std::sync::Arc<swarmnote_core::AppCore>>()
                    {
                        let _ = core.stop_network().await;
                    }
                    handle.exit(0);
                });
            }
        }
    }

    fn on_tray_icon_event(tray: &TrayIcon, event: TrayIconEvent) {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            let app = tray.app_handle();
            if let Some(state) = app.try_state::<TrayManagerState>() {
                if let Ok(mgr) = state.try_lock() {
                    mgr.restore_window();
                }
            }
        }
    }
}

// ── Tauri State 类型别名 ──

pub type TrayManagerState = tokio::sync::Mutex<TrayManager>;

/// 刷新托盘菜单（在工作区列表变更后调用）。
pub async fn refresh_tray_menu(app: &AppHandle) {
    if let Some(state) = app.try_state::<TrayManagerState>() {
        let mgr = state.lock().await;
        mgr.refresh_menu().await;
    }
}

// ── 私有辅助 ──

async fn toggle_sync(app: &AppHandle) {
    let Some(core) = app.try_state::<std::sync::Arc<swarmnote_core::AppCore>>() else {
        warn!("toggle_sync: AppCore not registered");
        return;
    };

    if core.net().await.is_some() {
        if let Err(e) = core.stop_network().await {
            warn!("Failed to stop P2P node from tray: {e}");
            return;
        }
        if let Some(state) = app.try_state::<TrayManagerState>() {
            state.lock().await.set_status(TrayNodeStatus::Stopped).await;
        }
        tracing::info!("P2P node stopped via tray");
    } else {
        let core_arc = core.inner().clone();
        if let Err(e) = core_arc.start_network().await {
            warn!("Failed to start P2P node from tray: {e}");
        }
    }
}

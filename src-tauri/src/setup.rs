//! tauri-specta builder 装配
//!
//! 所有 IPC 命令在 [`specta_builder`] 中通过 `collect_commands![]` 集中注册,
//! 事件通过 `collect_events![]` 注册。TS bindings 导出走 `cargo test --test
//! specta_export`,写出到 `src/lib/bindings.ts` 供前端 typesafe 调用。

use tauri::Wry;
use tauri_specta::{collect_commands, collect_events, Builder as SpectaBuilder, ErrorHandlingMode};

use crate::{commands, events};

/// 构造 tauri-specta [`SpectaBuilder`]:集中注册全部 commands + events。
///
/// 启用 [`SpectaBuilder::dangerously_cast_bigints_to_number`]:本应用所有
/// `u64` / `i64` / `usize` 字段(文档大小、毫秒时间戳)都在 JS 安全整数范围
/// (< 2^53)内,统一映射成 TS `number` 比 `bigint` 对前端更友好。
///
/// `ErrorHandlingMode::Throw`:生成的 commands 直接 throw,前端代码维持现有
/// `try/catch` 习惯,不用改成 `if (r.status === "error") ...` 的 tuple 风格。
pub fn specta_builder() -> SpectaBuilder<Wry> {
    SpectaBuilder::<Wry>::new()
        .dangerously_cast_bigints_to_number()
        .error_handling(ErrorHandlingMode::Throw)
        .commands(collect_commands![
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
            commands::yjs::broadcast_awareness,
            commands::yjs::close_ydoc,
            commands::yjs::rename_ydoc,
            commands::yjs::reload_ydoc_confirmed,
            commands::yjs::hydrate_workspace,
            // 同步
            commands::sync::trigger_workspace_sync,
        ])
        .events(collect_events![
            // YDoc / 文档
            events::DocFlushed,
            events::ExternalUpdate,
            events::ExternalAwarenessUpdate,
            events::ExternalConflict,
            // 文件树
            events::FileTreeChanged,
            // 设备
            events::DevicesChanged,
            // 配对
            events::PairingRequestReceived,
            events::PairedDeviceAdded,
            events::PairedDeviceRemoved,
            // 网络
            events::NetworkStatusChanged,
            events::NodeStarted,
            events::NodeStopped,
            // sync
            events::SyncStarted,
            events::SyncProgress,
            events::SyncCompleted,
            // window 内部
            events::WorkspaceReady,
            events::Navigate,
        ])
}

//! Desktop implementations of the `swarmnote-core` platform traits + any
//! glue that only makes sense when a workspace is bound to an OS window.
//!
//! | Trait / Helper     | Impl (this module)              | Backing crate        |
//! |--------------------|---------------------------------|----------------------|
//! | `KeychainProvider` | [`keychain::DesktopKeychain`]   | `keyring`            |
//! | `EventBus`         | [`event_bus::TauriEventBus`]    | `tauri::AppHandle`   |
//! | `FileWatcher`      | [`file_watcher::NotifyFileWatcher`] | `notify` + debouncer |
//! | Window→workspace   | [`workspace_map::WorkspaceMap`] | `tokio::sync::Mutex` |

pub mod event_bus;
pub mod file_watcher;
pub mod keychain;
pub mod workspace_map;

pub use event_bus::TauriEventBus;
pub use file_watcher::NotifyFileWatcher;
pub use keychain::DesktopKeychain;
pub use workspace_map::WorkspaceMap;

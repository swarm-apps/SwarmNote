//! # swarmnote-core
//!
//! Platform-independent core for SwarmNote. Shared between the Tauri desktop
//! shell (`src-tauri/`) and the future mobile shell (`swarmnote-mobile`, via
//! uniffi-bindgen-react-native).
//!
//! ## Architecture (two-layer Core)
//!
//! * [`AppCore`] — device-level singleton (identity, P2P node, paired devices,
//!   pairing manager, ctrl-topic dispatcher).
//! * [`WorkspaceCore`] — workspace-level unit; desktop may hold many via
//!   `Arc`, mobile holds at most one.
//!
//! Platform-specific side-effects are injected through four traits, each
//! in its domain module:
//!
//! * [`fs::FileSystem`] + [`fs::FileWatcher`]
//! * [`events::EventBus`]
//! * [`keychain::KeychainProvider`]
//!
//! This crate is intentionally free of `tauri`, `keyring`, `notify`, or any
//! host-specific dependency — those live in the desktop `src-tauri/src/platform/`
//! and the mobile `mobile-core/` wrapper crate.
//!
//! ## Flat public surface
//!
//! The crate root re-exports every host-facing type via a single flat
//! `pub use` list below. Any host shell (`src-tauri`, `mobile-core`, future
//! CLI / server harnesses) imports core types via `use swarmnote_core::{...}`
//! — there is no `api` / `internal` split, and no host-specific subpath.
//!
//! A handful of categories stay behind their submodule as namespaces, because
//! the `xxx::yyy` path is more informative than flattening would be:
//!
//! * [`protocol`] — P2P wire types (`AppRequest`, `AppResponse`, `OsInfo`, …)
//! * [`config`] — global config I/O (`save_config`, `RecentWorkspace`, …)
//! * [`fs::ops`] — filesystem business operations (functions, not types)
//! * [`yjs::doc_state`] — low-level yjs doc-state helpers
//!
//! New pub items must be registered in the root `pub use` list below; that
//! list is the stable API contract of the crate.

pub mod app;
pub mod config;
pub mod device;
pub mod document;
pub mod error;
pub mod events;
pub mod fs;
pub mod identity;
pub mod keychain;
pub mod network;
pub mod pairing;
pub mod protocol;
pub mod workspace;
pub mod yjs;

// ── Host core ──────────────────────────────────────────────────────────────
pub use app::{AppCore, AppCoreBuilder, FsFactory, WatcherFactory};
pub use config::RecentWorkspace;
pub use workspace::{ensure_workspace_row, WorkspaceCore, WorkspaceInfo};

// ── Errors & events ────────────────────────────────────────────────────────
pub use error::{AppError, AppResult};
pub use events::{AppEvent, EventBus};

// ── Identity & keychain ────────────────────────────────────────────────────
pub use identity::{DeviceInfo, IdentityManager};
pub use keychain::KeychainProvider;

// ── Devices ────────────────────────────────────────────────────────────────
pub use device::{
    ConnectionType, Device, DeviceFilter, DeviceListResult, DeviceManager, DeviceStatus,
};

// ── Pairing ────────────────────────────────────────────────────────────────
pub use pairing::{PairedDeviceInfo, PairingCodeInfo, PairingManager, ShareCodeRecord};

// ── Network ────────────────────────────────────────────────────────────────
pub use network::{AppNetClient, NetManager, NodeStatus};

// ── Libp2p (re-exported whole module so wrap layers don't depend on swarm-p2p-core) ──
// Consumers: `use swarmnote_core::libp2p::{PeerId, Multiaddr, identity::Keypair};`
pub use swarm_p2p_core::libp2p;

// ── Documents ──────────────────────────────────────────────────────────────
pub use document::{title_from_rel_path, CreateFolderInput, DocumentCrud, UpsertDocumentInput};

// ── Filesystem ─────────────────────────────────────────────────────────────
pub use fs::{FileEvent, FileEventCallback, FileSystem, FileTreeNode, FileWatcher, LocalFs};

// ── Yjs (manager + hydrate entry points) ───────────────────────────────────
pub use yjs::doc_state::{hydrate_workspace, HydrateProgress, HydrateProgressFn, HydrateResult};
pub use yjs::manager::{OpenDocResult, ReloadStatus, YDocManager};

// ── Workspace sync ─────────────────────────────────────────────────────────
pub use workspace::sync::{AppSyncCoordinator, WorkspaceSync};

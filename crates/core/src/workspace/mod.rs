//! Workspace-level core: `WorkspaceCore` owns every per-workspace resource
//! — DB connection, filesystem, file watcher, YDocManager, document CRUD —
//! and is handed out by `AppCore::open_workspace`.
//!
//! Desktop may hold many `Arc<WorkspaceCore>` instances (one per workspace,
//! shared across windows of the same workspace). Mobile holds at most one.

pub mod db;
pub mod keys;
pub mod permissions;
pub mod sharing;
pub mod sync;

use std::path::Path;
use std::sync::{Arc, Weak};

use chrono::{DateTime, Utc};
use entity::workspace::{documents, workspaces, workspaces::Entity as WorkspacesEntity};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app::AppCore;
use crate::document::DocumentCrud;
use crate::error::{AppError, AppResult};
use crate::events::{AppEvent, EventBus};
use crate::fs::{FileEvent, FileEventCallback, FileSystem, FileWatcher};
use crate::workspace::sync::WorkspaceSync;
use crate::yjs::manager::YDocManager;

/// Runtime + DB record of an open workspace. Returned to the frontend by
/// `get_workspace_info`-style commands; held by [`WorkspaceCore`] as its
/// own metadata snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct WorkspaceInfo {
    pub id: Uuid,
    pub name: String,
    pub path: String,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Number of document rows in this workspace's DB. Populated at construction
    /// time (0) and refreshed on demand via [`WorkspaceCore::fresh_info`] — the
    /// `info()` getter returns the last-cached snapshot without hitting the DB.
    #[serde(default)]
    pub doc_count: u32,
}

/// Workspace-level unit. Constructed only by [`AppCore::open_workspace`] —
/// never directly.
///
/// **Lifecycle**: host MUST call [`WorkspaceCore::close`] before the last
/// `Arc<WorkspaceCore>` reference is dropped. `Drop` aborts background
/// tasks as a best-effort fallback but does NOT flush pending writes —
/// `close().await` is the only path that guarantees persistence.
pub struct WorkspaceCore {
    pub(crate) info: WorkspaceInfo,
    /// Shared DB connection. Wrapped in `Arc` so `DocumentCrud`,
    /// `YDocManager`, and future `WorkspaceSync` can hold it without
    /// cloning the underlying pool.
    db: Arc<DatabaseConnection>,
    fs: Arc<dyn FileSystem>,
    watcher: Option<Arc<dyn FileWatcher>>,
    ydoc: Arc<YDocManager>,
    documents: Arc<DocumentCrud>,
    event_bus: Arc<dyn EventBus>,
    /// Weak back-reference to the owning [`AppCore`] — avoids the obvious
    /// AppCore ↔ WorkspaceCore ownership cycle.
    _app: Weak<AppCore>,
    /// Per-workspace sync runtime. `None` until P2P starts; torn down when
    /// the workspace closes or P2P stops.
    sync: tokio::sync::RwLock<Option<Arc<WorkspaceSync>>>,
    /// This workspace's symmetric key history (`{key_version → keys}`), loaded
    /// on open. Used by the encrypted gossip codec for publish/receive.
    keys: tokio::sync::RwLock<crate::WorkspaceKeys>,
    /// Device identity material retained for reloading keys after a Lockbox is
    /// installed (e.g. one received from a peer during sync).
    dev_peer_id: String,
    dev_x25519_secret: x25519_dalek::StaticSecret,
    dev_x25519_public: x25519_dalek::PublicKey,
}

impl WorkspaceCore {
    /// Construct a new workspace runtime. Called by
    /// [`AppCore::open_workspace`] — not a public entry point.
    #[allow(clippy::too_many_arguments)] // single internal call site; injecting deps explicitly
    pub(crate) async fn new(
        info: WorkspaceInfo,
        db: DatabaseConnection,
        fs: Arc<dyn FileSystem>,
        watcher: Option<Arc<dyn FileWatcher>>,
        event_bus: Arc<dyn EventBus>,
        identity: &crate::identity::IdentityManager,
        init_keys: bool,
        app: Weak<AppCore>,
    ) -> AppResult<Arc<Self>> {
        let db = Arc::new(db);
        let peer_id = identity.peer_id()?;
        let my_x25519_secret = identity.x25519_secret()?;
        let my_x25519_public = identity.x25519_public()?;

        // Owner-opened workspaces self-initialize a key if absent; sync-joined
        // workspaces (init_keys = false) stay keyless until the owner's Lockbox
        // arrives via sync, so they don't fork a divergent key.
        let workspace_keys = if init_keys {
            let (workspace_keys, did_init) = keys::load_or_initialize_workspace_keys(
                db.as_ref(),
                info.id,
                &peer_id,
                &my_x25519_secret,
                &my_x25519_public,
            )
            .await?;
            // Owner-create moment (key freshly self-initialized): seed the
            // genesis Owner permission op so the chain has a root of trust.
            if did_init {
                permissions::ensure_genesis_owner(db.as_ref(), identity, info.id).await?;
            }
            workspace_keys
        } else {
            keys::load_workspace_keys(
                db.as_ref(),
                info.id,
                &peer_id,
                &my_x25519_secret,
                &my_x25519_public,
            )
            .await?
        };
        let dev_peer_id = peer_id.clone();
        let dev_x25519_secret = my_x25519_secret.clone();
        let dev_x25519_public = my_x25519_public;
        let documents = Arc::new(DocumentCrud::new(Arc::clone(&db), peer_id.clone()));
        let ydoc = YDocManager::new(
            info.id,
            Arc::clone(&fs),
            Arc::clone(&event_bus),
            Arc::clone(&db),
            peer_id,
        );

        // Start the watcher (if any) BEFORE we hand out `Arc<Self>` so
        // reload callbacks see a fully-formed workspace.
        if let Some(w) = watcher.as_ref() {
            let callback =
                build_watcher_callback(info.id, Arc::clone(&ydoc), Arc::clone(&event_bus));
            w.watch(callback).await?;
        }

        Ok(Arc::new(Self {
            info,
            db,
            fs,
            watcher,
            ydoc,
            documents,
            event_bus,
            _app: app,
            sync: tokio::sync::RwLock::new(None),
            keys: tokio::sync::RwLock::new(workspace_keys),
            dev_peer_id,
            dev_x25519_secret,
            dev_x25519_public,
        }))
    }

    pub fn id(&self) -> Uuid {
        self.info.id
    }

    pub fn info(&self) -> &WorkspaceInfo {
        &self.info
    }

    /// Return a `WorkspaceInfo` clone with `doc_count` populated from the
    /// current DB row count. Use this when the UI surface needs an accurate
    /// document count; `info()` returns the cached snapshot whose
    /// `doc_count` is always 0.
    pub async fn fresh_info(&self) -> AppResult<WorkspaceInfo> {
        let doc_count = documents::Entity::find().count(&*self.db).await? as u32;
        let mut info = self.info.clone();
        info.doc_count = doc_count;
        Ok(info)
    }

    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    pub fn fs(&self) -> &Arc<dyn FileSystem> {
        &self.fs
    }

    pub fn watcher(&self) -> Option<&Arc<dyn FileWatcher>> {
        self.watcher.as_ref()
    }

    pub fn ydoc(&self) -> &Arc<YDocManager> {
        &self.ydoc
    }

    pub fn documents(&self) -> &Arc<DocumentCrud> {
        &self.documents
    }

    pub fn event_bus(&self) -> &Arc<dyn EventBus> {
        &self.event_bus
    }

    /// Current per-workspace sync runtime (if P2P is running).
    pub async fn sync(&self) -> Option<Arc<WorkspaceSync>> {
        self.sync.read().await.clone()
    }

    /// This workspace's loaded symmetric key history (clone of the in-memory
    /// `{key_version → keys}` map). Used by the encrypted gossip codec.
    pub async fn keys(&self) -> crate::WorkspaceKeys {
        self.keys.read().await.clone()
    }

    /// Reload the key history from the DB. Called after installing a Lockbox
    /// received from a peer during sync, so subsequent gossip can decrypt.
    pub async fn reload_keys(&self) -> AppResult<()> {
        let loaded = keys::load_workspace_keys(
            self.db.as_ref(),
            self.info.id,
            &self.dev_peer_id,
            &self.dev_x25519_secret,
            &self.dev_x25519_public,
        )
        .await?;
        *self.keys.write().await = loaded;
        Ok(())
    }

    /// Broadcast an awareness (caret / presence) update for an open doc.
    /// Bytes are an opaque `y-protocols/awareness` encodeAwarenessUpdate
    /// payload — never decoded, never persisted. Silently no-op when P2P is
    /// down (awareness is best-effort by design).
    pub async fn broadcast_awareness(&self, doc_uuid: Uuid, update: Vec<u8>) {
        if let Some(sync) = self.sync().await {
            sync.publish_awareness(doc_uuid, update).await;
        }
    }

    /// Install or replace the per-workspace sync runtime. Closes the
    /// previous instance (if any) to avoid leaking flush tasks / GossipSub
    /// subscriptions.
    pub(crate) async fn set_sync(&self, sync: Option<Arc<WorkspaceSync>>) {
        let old = std::mem::replace(&mut *self.sync.write().await, sync);
        if let Some(prev) = old {
            prev.close().await;
        }
    }

    /// Take the sync runtime out (returns the Arc if present, leaves None).
    /// Called by [`AppCore::stop_network`].
    pub(crate) async fn take_sync(&self) -> Option<Arc<WorkspaceSync>> {
        self.sync.write().await.take()
    }

    /// Flush every open Y.Doc, tear down sync + file watcher. Must be
    /// called before the last `Arc<WorkspaceCore>` reference drops.
    ///
    /// Returns `Err(AppError::WorkspaceCloseFailed)` if one or more dirty
    /// Y.Docs failed to persist. Even on error, all resources (sync,
    /// watcher, writeback loop, doc registry) ARE torn down — the workspace
    /// is safe to drop. The host's only job on error is to surface the
    /// failure to the user (e.g. a toast) so unsaved edits are not silent.
    pub async fn close(&self) -> AppResult<()> {
        // Tear down sync first (unsubscribe GossipSub, abort buffer flush).
        if let Some(sync) = self.take_sync().await {
            sync.close().await;
        }
        let ydoc_failures = self.ydoc.close_all().await;
        if let Some(w) = &self.watcher {
            w.unwatch().await;
        }
        tracing::info!("WorkspaceCore closed: {}", self.info.id);

        if !ydoc_failures.is_empty() {
            return Err(AppError::WorkspaceCloseFailed {
                workspace_id: self.info.id,
                failures: ydoc_failures
                    .into_iter()
                    .map(|(uuid, err)| (uuid, err.to_string()))
                    .collect(),
            });
        }
        Ok(())
    }
}

impl Drop for WorkspaceCore {
    fn drop(&mut self) {
        // Best-effort warning: if we reach Drop with dirty docs, the host
        // forgot to call `close().await`. We can't run async cleanup here
        // (no reliable runtime handle), so just log — the writeback tasks
        // will be aborted when the `Arc<DocEntry>`s drop.
        let open = self.ydoc.list_open_doc_uuids();
        if !open.is_empty() {
            tracing::warn!(
                "WorkspaceCore {} dropped with {} open docs; host should have called close().await first",
                self.info.id,
                open.len()
            );
        }
    }
}

/// Build the callback handed to `FileWatcher::watch`:
///
/// 1. Emit [`AppEvent::FileTreeChanged`] so the frontend re-scans the tree.
/// 2. For each modified `.md` path, spawn a tokio task calling
///    [`YDocManager::reload_from_file`] — that handles self-write detection
///    and fires `ExternalUpdate` / `ExternalConflict` events as appropriate.
fn build_watcher_callback(
    workspace_id: Uuid,
    ydoc: Arc<YDocManager>,
    event_bus: Arc<dyn EventBus>,
) -> FileEventCallback {
    // Per the [`FileWatcher`] trait contract, implementations MUST invoke
    // this callback from a tokio runtime context, so `tokio::spawn` is safe.
    Arc::new(move |events: Vec<FileEvent>| {
        event_bus.emit(AppEvent::FileTreeChanged { workspace_id });

        for ev in events {
            let rel = match ev {
                FileEvent::Modified(r) | FileEvent::Created(r) | FileEvent::Deleted(r) => r,
                FileEvent::Renamed { to, .. } => to,
            };
            if !rel.ends_with(".md") {
                continue;
            }
            let ydoc = Arc::clone(&ydoc);
            tokio::spawn(async move {
                if let Err(e) = ydoc.reload_from_file(&rel).await {
                    tracing::warn!("reload_from_file({rel}) failed: {e}");
                }
            });
        }
    })
}

/// Read the workspace UUID from `{path}/.swarmnote/workspace.db` without
/// running migrations or keeping the connection open. Used by
/// [`AppCore::open_workspace`] to dedup concurrent opens of the same
/// workspace across multiple windows.
pub(crate) async fn peek_workspace_uuid(path: &Path) -> AppResult<Option<Uuid>> {
    let db_path = path.join(".swarmnote").join("workspace.db");
    if !db_path.exists() {
        return Ok(None);
    }
    let db = db::connect_sqlite(&db_path).await?;
    let row = WorkspacesEntity::find().one(&db).await?;
    // Sea-orm doesn't expose an explicit close — dropping the connection
    // closes the underlying pool.
    drop(db);
    Ok(row.map(|w| w.id))
}

/// Load or create the workspace row in `workspace.db`. Returns the
/// [`WorkspaceInfo`] populated with runtime fields (`path`).
pub(crate) async fn load_or_create_workspace_info(
    db: &DatabaseConnection,
    path: &Path,
    peer_id: &str,
) -> AppResult<WorkspaceInfo> {
    if let Some(row) = WorkspacesEntity::find().one(db).await? {
        return Ok(WorkspaceInfo {
            id: row.id,
            name: row.name,
            path: path.to_string_lossy().into_owned(),
            created_by: row.created_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
            doc_count: 0,
        });
    }

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled".to_string());
    let now = Utc::now();
    let id = Uuid::now_v7();

    let model = workspaces::ActiveModel {
        id: Set(id),
        name: Set(name.clone()),
        created_by: Set(peer_id.to_owned()),
        created_at: Set(now),
        updated_at: Set(now),
    };
    let created = model.insert(db).await?;
    Ok(WorkspaceInfo {
        id: created.id,
        name: created.name,
        path: path.to_string_lossy().into_owned(),
        created_by: created.created_by,
        created_at: created.created_at,
        updated_at: created.updated_at,
        doc_count: 0,
    })
}

/// Register (or overwrite) the workspace row with a specific UUID — used
/// when a sync peer tells us the authoritative workspace ID. Idempotent.
pub async fn ensure_workspace_row(
    db: &DatabaseConnection,
    id: Uuid,
    name: &str,
    peer_id: &str,
) -> AppResult<WorkspaceInfo> {
    if let Some(existing) = WorkspacesEntity::find_by_id(id).one(db).await? {
        return Ok(WorkspaceInfo {
            id: existing.id,
            name: existing.name,
            path: String::new(), // populated by caller
            created_by: existing.created_by,
            created_at: existing.created_at,
            updated_at: existing.updated_at,
            doc_count: 0,
        });
    }
    // Skip matching on name — the UUID is authoritative.
    let _ = WorkspacesEntity::find()
        .filter(workspaces::Column::Name.eq(name))
        .one(db)
        .await?;

    let now = Utc::now();
    let model = workspaces::ActiveModel {
        id: Set(id),
        name: Set(name.to_owned()),
        created_by: Set(peer_id.to_owned()),
        created_at: Set(now),
        updated_at: Set(now),
    };
    let created = model.insert(db).await?;
    Ok(WorkspaceInfo {
        id: created.id,
        name: created.name,
        path: String::new(),
        created_by: created.created_by,
        created_at: created.created_at,
        updated_at: created.updated_at,
        doc_count: 0,
    })
}

/// Pin the workspace's authoritative owner (`created_by`) to `owner_peer_id`.
///
/// A joiner creates its local workspace row with **itself** as `created_by`
/// (it doesn't know the owner at creation time). Once it receives the owner's
/// signed permission chain over the Noise-authenticated sync channel, it pins
/// `created_by` to the real owner so [`permissions::materialize`] binds the
/// genesis correctly (its own role then materializes, and forged genesis ops
/// are rejected the same way they are on the owner's device). No-op if already
/// set or if the row is missing.
pub async fn pin_workspace_owner(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    owner_peer_id: &str,
) -> AppResult<()> {
    if let Some(row) = WorkspacesEntity::find_by_id(workspace_id).one(db).await? {
        if row.created_by != owner_peer_id {
            let mut model: workspaces::ActiveModel = row.into();
            model.created_by = Set(owner_peer_id.to_owned());
            model.update(db).await?;
        }
    }
    Ok(())
}

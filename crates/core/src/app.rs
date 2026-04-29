//! [`AppCore`] — device-level singleton. Holds identity, event bus,
//! keychain, global config, devices DB, and (when P2P is running) the
//! network session manager + per-peer sync state. Also keeps a `Weak`
//! registry of open [`WorkspaceCore`] instances so desktop windows can
//! share one runtime per workspace.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};

use sea_orm::DatabaseConnection;
use swarm_p2p_core::libp2p::identity::Keypair;
use tokio::sync::Mutex;
use tracing::info;
use uuid::Uuid;

use crate::config::{apply_workspace_update, load_or_create_config, GlobalConfigState, RecentWorkspace};
use crate::error::{AppError, AppResult};
use crate::events::{AppEvent, EventBus};
use crate::fs::{FileSystem, FileWatcher, LocalFs};
use crate::identity::IdentityManager;
use crate::keychain::KeychainProvider;
use crate::network::config::create_node_config;
use crate::network::event_loop::spawn_event_loop;
use crate::network::{AppNetClient, NetManager, NodeStatus};
use crate::protocol::{AppRequest, AppResponse, OsInfo};
use crate::workspace::sync::{AppSyncCoordinator, WorkspaceSync};
use crate::workspace::{
    self, db::init_devices_db, db::init_workspace_db, load_or_create_workspace_info, WorkspaceCore,
    WorkspaceInfo,
};

/// Factory that produces a workspace-scoped [`FileSystem`] implementation
/// bound to `path`. Registered on [`AppCoreBuilder`]; invoked once per
/// [`AppCore::open_workspace`] call.
pub type FsFactory = Arc<dyn Fn(&Path) -> Arc<dyn FileSystem> + Send + Sync + 'static>;

/// Factory that produces a workspace-scoped [`FileWatcher`] implementation
/// bound to `path`. Optional — mobile hosts with no watcher pass `None`.
pub type WatcherFactory = Arc<dyn Fn(&Path) -> Arc<dyn FileWatcher> + Send + Sync + 'static>;

/// Device-level core. Construct via [`AppCoreBuilder`].
pub struct AppCore {
    pub(crate) identity: Arc<IdentityManager>,
    pub(crate) config: Arc<GlobalConfigState>,
    pub(crate) event_bus: Arc<dyn EventBus>,
    pub(crate) keychain: Arc<dyn KeychainProvider>,

    /// Shared connection to `devices.db` (paired devices table). Always open
    /// for the lifetime of `AppCore` — does not track P2P lifecycle.
    pub(crate) devices_db: Arc<DatabaseConnection>,

    /// Absolute path to the app data directory (`~/.swarmnote/` or
    /// platform-specific). Platform layers inject this; core stores for
    /// reference only.
    pub(crate) app_data_dir: PathBuf,

    /// Platform-supplied factory producing the per-workspace filesystem.
    fs_factory: FsFactory,

    /// Platform-supplied factory producing the per-workspace file watcher.
    /// `None` on hosts that don't support file watching (mobile sandbox).
    watcher_factory: Option<WatcherFactory>,

    /// Running P2P session. `None` until [`AppCore::start_network`]
    /// completes; reset to `None` by [`AppCore::stop_network`].
    net: Mutex<Option<Arc<NetManager>>>,

    /// Global sync coordinator. Populated together with `net` — starts /
    /// stops with the P2P session. Per-workspace sync state lives in
    /// [`WorkspaceCore::sync`].
    sync_coordinator: Mutex<Option<Arc<AppSyncCoordinator>>>,

    /// Registry of active workspace runtimes keyed by workspace UUID.
    /// Stored as `Weak` so dropping the last external `Arc<WorkspaceCore>`
    /// actually frees the workspace; the map is cleaned on demand in
    /// [`AppCore::open_workspace`].
    workspaces: Mutex<HashMap<Uuid, Weak<WorkspaceCore>>>,
}

/// Builder for [`AppCore`]. Collects the three required platform
/// dependencies (`keychain`, `event_bus`, `app_data_dir`) and lets the host
/// register workspace-scoped filesystem / watcher factories.
///
/// The default `fs_factory` produces [`LocalFs`] (works for both desktop
/// user-chosen paths and mobile sandbox paths). The default
/// `watcher_factory` is `None` — desktop hosts call
/// [`AppCoreBuilder::with_watcher_factory`] to register `NotifyFileWatcher`.
pub struct AppCoreBuilder {
    keychain: Arc<dyn KeychainProvider>,
    event_bus: Arc<dyn EventBus>,
    app_data_dir: PathBuf,
    fs_factory: FsFactory,
    watcher_factory: Option<WatcherFactory>,
}

impl AppCoreBuilder {
    /// Start configuring an `AppCore`. Supplies the three always-required
    /// platform dependencies; filesystem / watcher factories default to
    /// `LocalFs` / `None` respectively.
    pub fn new(
        keychain: Arc<dyn KeychainProvider>,
        event_bus: Arc<dyn EventBus>,
        app_data_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            keychain,
            event_bus,
            app_data_dir: app_data_dir.into(),
            fs_factory: Arc::new(|p: &Path| Arc::new(LocalFs::new(p)) as Arc<dyn FileSystem>),
            watcher_factory: None,
        }
    }

    /// Override the filesystem factory. Rare — default `LocalFs` covers
    /// both desktop and mobile sandbox use cases.
    pub fn with_fs_factory<F>(mut self, factory: F) -> Self
    where
        F: Fn(&Path) -> Arc<dyn FileSystem> + Send + Sync + 'static,
    {
        self.fs_factory = Arc::new(factory);
        self
    }

    /// Register a filesystem watcher factory. Desktop hosts pass a closure
    /// that constructs `NotifyFileWatcher`; mobile hosts omit this entirely.
    pub fn with_watcher_factory<F>(mut self, factory: F) -> Self
    where
        F: Fn(&Path) -> Arc<dyn FileWatcher> + Send + Sync + 'static,
    {
        self.watcher_factory = Some(Arc::new(factory));
        self
    }

    /// Bootstrap the device-level core: load config, initialize identity,
    /// open `devices.db`. P2P network stays stopped until the host calls
    /// [`AppCore::start_network`] explicitly.
    pub async fn build(self) -> AppResult<Arc<AppCore>> {
        let Self {
            keychain,
            event_bus,
            app_data_dir,
            fs_factory,
            watcher_factory,
        } = self;

        // 1. Load (or create default) global config.
        let config_data = load_or_create_config(&app_data_dir)?;
        let config_state = Arc::new(GlobalConfigState::new(
            config_data.clone(),
            app_data_dir.join("config.json"),
        ));

        // 2. Initialize device identity.
        let identity = Arc::new(IdentityManager::new(keychain.clone(), &config_data).await?);

        // 3. Open devices.db.
        let devices_db = Arc::new(init_devices_db(&app_data_dir).await?);

        Ok(Arc::new(AppCore {
            identity,
            config: config_state,
            event_bus,
            keychain,
            devices_db,
            app_data_dir,
            fs_factory,
            watcher_factory,
            net: Mutex::new(None),
            sync_coordinator: Mutex::new(None),
            workspaces: Mutex::new(HashMap::new()),
        }))
    }
}

impl AppCore {
    // ── Accessors ─────────────────────────────────────────────

    pub fn identity(&self) -> &Arc<IdentityManager> {
        &self.identity
    }

    pub fn config(&self) -> &Arc<GlobalConfigState> {
        &self.config
    }

    pub fn event_bus(&self) -> &Arc<dyn EventBus> {
        &self.event_bus
    }

    pub fn keychain(&self) -> &Arc<dyn KeychainProvider> {
        &self.keychain
    }

    pub fn devices_db(&self) -> &DatabaseConnection {
        &self.devices_db
    }

    pub fn app_data_dir(&self) -> &Path {
        &self.app_data_dir
    }

    /// Open (or return existing) workspace at `path`.
    ///
    /// Concurrency: uses a double-checked pattern so parallel callers
    /// (e.g. two windows opening different workspaces at once) don't
    /// serialize on DB / filesystem I/O. The `workspaces` lock is only
    /// held for the registry peek / insert — the DB open and
    /// [`WorkspaceCore::new`] run **without** the lock held.
    ///
    /// If another task wins the race to insert the same UUID, this
    /// method drops its own freshly-built core and returns the winner's
    /// instance so desktop windows still share one runtime per workspace.
    pub async fn open_workspace(
        self: &Arc<Self>,
        path: impl Into<PathBuf>,
    ) -> AppResult<Arc<WorkspaceCore>> {
        let path: PathBuf = path.into();
        if !path.is_dir() {
            return Err(AppError::InvalidPath(path.to_string_lossy().into_owned()));
        }

        let fs: Arc<dyn FileSystem> = (self.fs_factory)(&path);
        let watcher: Option<Arc<dyn FileWatcher>> = self.watcher_factory.as_ref().map(|f| f(&path));

        // Peek UUID without holding any lock, so two calls to different
        // paths never serialize.
        let peeked = workspace::peek_workspace_uuid(&path).await?;

        // Fast path: if the peeked UUID is already registered and live, reuse it.
        if let Some(uuid) = peeked {
            let guard = self.workspaces.lock().await;
            if let Some(weak) = guard.get(&uuid) {
                if let Some(arc) = weak.upgrade() {
                    return Ok(arc);
                }
            }
            // Fall through to the slow path below — either no entry or a
            // stale Weak. We deliberately drop the lock here so the I/O
            // under slow-path runs without blocking other open_workspace
            // calls.
            drop(guard);
        }

        // Slow path: run DB open + workspace row load WITHOUT holding the
        // registry lock.
        let db = init_workspace_db(&path).await?;
        let peer_id = self.identity.peer_id()?;
        let mut info = load_or_create_workspace_info(&db, &path, &peer_id).await?;
        info.path = path.to_string_lossy().into_owned();
        let workspace_id = info.id;

        let core = WorkspaceCore::new(
            info,
            db,
            fs,
            watcher,
            self.event_bus.clone(),
            peer_id,
            Arc::downgrade(self),
        )
        .await?;

        // Re-take the lock and double-check: another task may have won the
        // race while we were doing I/O.
        let winner = {
            let mut guard = self.workspaces.lock().await;
            if let Some(weak) = guard.get(&workspace_id) {
                if let Some(existing) = weak.upgrade() {
                    // Lost the race — drop our freshly-built core so its
                    // writeback tasks are aborted and fall through to return
                    // the existing Arc. The drop must happen outside the
                    // guard (close is async).
                    drop(guard);
                    // Close our orphan core so its YDocManager writeback
                    // tasks are cancelled and file watcher released.
                    // The orphan has no user-visible state, so we swallow
                    // any persistence error — logging only.
                    if let Err(e) = core.close().await {
                        tracing::warn!("orphan core close failed during open_workspace race: {e}");
                    }
                    existing
                } else {
                    guard.insert(workspace_id, Arc::downgrade(&core));
                    drop(guard);
                    core
                }
            } else {
                guard.insert(workspace_id, Arc::downgrade(&core));
                drop(guard);
                core
            }
        };

        // If P2P is running, install per-workspace sync + subscribe.
        if let Some(coordinator) = self.sync_coordinator().await {
            self.install_workspace_sync(&winner, coordinator.client(), true)
                .await;
        }

        // Persist to recent_workspaces so hosts can surface MRU lists without
        // needing their own book-keeping layer. Failure is logged but does
        // not fail the open — recent is auxiliary metadata.
        let path_str = winner.info.path.clone();
        let name = winner.info.name.clone();
        let uuid_str = winner.info.id.to_string();
        if let Err(e) = self
            .touch_recent_workspace(&path_str, &name, Some(&uuid_str))
            .await
        {
            tracing::warn!("touch_recent_workspace failed after open: {e}");
        }

        Ok(winner)
    }

    /// Close the workspace with the given UUID. `flush()` runs regardless
    /// of the number of outstanding `Arc` references — this is an
    /// authoritative shutdown hook used by the host when the last window
    /// referencing a workspace closes.
    pub async fn close_workspace(&self, uuid: Uuid) -> AppResult<()> {
        let mut guard = self.workspaces.lock().await;
        let Some(weak) = guard.remove(&uuid) else {
            return Ok(());
        };
        drop(guard);

        // WorkspaceCore::close() tears down its WorkspaceSync (if any)
        // including GossipSub unsubscribe — no explicit unsubscribe needed here.
        if let Some(arc) = weak.upgrade() {
            arc.close().await?;
        }
        Ok(())
    }

    /// Look up an active workspace by UUID. Returns `None` if the workspace
    /// is not open or if all external `Arc`s were dropped.
    pub async fn get_workspace(&self, uuid: &Uuid) -> Option<Arc<WorkspaceCore>> {
        let guard = self.workspaces.lock().await;
        guard.get(uuid).and_then(|w| w.upgrade())
    }

    /// Snapshot of every live workspace (active `Arc` upgrades only).
    /// Stale `Weak` entries are *not* pruned here — that happens on the
    /// next `open_workspace` call.
    pub async fn list_workspaces(&self) -> Vec<Arc<WorkspaceCore>> {
        let guard = self.workspaces.lock().await;
        guard.values().filter_map(|w| w.upgrade()).collect()
    }

    /// Resolve `WorkspaceInfo` for a workspace UUID without forcing the
    /// caller to hold an `Arc<WorkspaceCore>`. Returns `None` when not open.
    pub async fn workspace_info(&self, uuid: &Uuid) -> Option<WorkspaceInfo> {
        self.get_workspace(uuid).await.map(|w| w.info.clone())
    }

    /// Read access to the underlying app_data_dir-relative config path.
    pub fn config_path(&self) -> &Path {
        self.config.path()
    }

    // ── Recent workspaces (MRU persisted list) ──

    /// Snapshot of recent workspaces (MRU-ordered, capped at 10). Pure read;
    /// does not touch disk.
    pub async fn recent_workspaces(&self) -> Vec<RecentWorkspace> {
        self.config.read().await.recent_workspaces.clone()
    }

    /// Insert or bump a workspace to the top of the MRU list and persist.
    /// `open_workspace` calls this automatically; exposed publicly so hosts
    /// that bypass `open_workspace` (e.g. pre-seeded recent from config
    /// migration) can record entries too.
    pub async fn touch_recent_workspace(
        &self,
        path: &str,
        name: &str,
        uuid: Option<&str>,
    ) -> AppResult<()> {
        let mut guard = self.config.write().await;
        apply_workspace_update(&mut guard, path, name, uuid);
        crate::config::save_config(self.config.path(), &guard)
    }

    /// Remove a path from the MRU list and persist. No-op if the path is not
    /// present. Does not delete any files on disk — hosts that want to wipe
    /// the workspace directory must do so themselves.
    pub async fn remove_recent_workspace(&self, path: &str) -> AppResult<()> {
        let mut guard = self.config.write().await;
        let before = guard.recent_workspaces.len();
        guard.recent_workspaces.retain(|w| w.path != path);
        if guard.recent_workspaces.len() == before {
            return Ok(());
        }
        if guard.last_workspace_path.as_deref() == Some(path) {
            guard.last_workspace_path = guard.recent_workspaces.first().map(|w| w.path.clone());
        }
        crate::config::save_config(self.config.path(), &guard)
    }

    // ── Network lifecycle ──

    /// Current network session, if any.
    pub async fn net(&self) -> Option<Arc<NetManager>> {
        self.net.lock().await.clone()
    }

    /// Current global sync coordinator, if P2P is running.
    pub async fn sync_coordinator(&self) -> Option<Arc<AppSyncCoordinator>> {
        self.sync_coordinator.lock().await.clone()
    }

    /// Node-level status for frontend display.
    pub async fn network_status(&self) -> NodeStatus {
        if self.net.lock().await.is_some() {
            NodeStatus::Running
        } else {
            NodeStatus::Stopped
        }
    }

    /// Start the P2P node. Idempotent: returns
    /// [`AppError::NetworkAlreadyRunning`] if a session is already active.
    ///
    /// Concurrency: the `net` mutex is only held during a brief existence
    /// check and a final CAS-style install. Libp2p startup, GossipSub
    /// subscribe, workspace-sync injection, and background task spawns all
    /// run **without** holding the lock — so concurrent `net()` /
    /// `network_status()` / `client()` / `pairing()` / `devices()` calls
    /// never block for longer than a map lookup.
    ///
    /// If two tasks race here, the loser shuts down its freshly-built
    /// `NetManager` before returning `NetworkAlreadyRunning` so no
    /// libp2p session is leaked.
    pub async fn start_network(self: &Arc<Self>) -> AppResult<()> {
        // 1. Short-lived existence check — early-exit if clearly running.
        //    (The authoritative check is the CAS in step 3; this is just a
        //    fast-path so common callers don't waste an I/O round trip.)
        if self.net.lock().await.is_some() {
            return Err(AppError::NetworkAlreadyRunning);
        }

        // 2. I/O — all of this runs WITHOUT the `net` lock held.
        let keypair_bytes = self.identity.keypair_protobuf()?;
        let keypair = Keypair::from_protobuf_encoding(&keypair_bytes)
            .map_err(|e| AppError::KeypairDecode(e.to_string()))?;
        let peer_id = keypair.public().to_peer_id();

        // Build agent_version from current device name.
        let device_name = self.identity.device_info()?.device_name;
        let mut os_info = OsInfo::default();
        if device_name != os_info.hostname {
            os_info.name = Some(device_name.clone());
        }
        let agent_version = os_info.to_agent_version(env!("CARGO_PKG_VERSION"));
        let config = create_node_config(agent_version);

        // Start the libp2p swarm + req/resp channels.
        let (client, receiver): (AppNetClient, _) =
            swarm_p2p_core::start::<AppRequest, AppResponse>(keypair, config).map_err(|e| {
                AppError::SwarmIo {
                    context: "swarm_p2p_core::start",
                    reason: e.to_string(),
                }
            })?;

        let net_manager = Arc::new(NetManager::new(
            client.clone(),
            peer_id,
            (*self.devices_db).clone(),
            os_info.name,
        ));
        let cancel_token = net_manager.cancel_token();

        // Global sync coordinator (full-sync de-dup, SV compensation, inbound routing).
        let coordinator = Arc::new(AppSyncCoordinator::new(self.clone(), client.clone()));

        // Event loop.
        spawn_event_loop(
            receiver,
            self.clone(),
            net_manager.client.clone(),
            net_manager.device_manager.clone(),
            net_manager.pairing_manager.clone(),
            coordinator.clone(),
            cancel_token.clone(),
        );

        // Periodic SV compensation.
        coordinator.start_sv_compensation(cancel_token.clone());

        // Subscribe to global ctrl topic for workspace-opened notifications.
        if let Err(e) = client.subscribe(crate::workspace::sync::CTRL_TOPIC).await {
            tracing::warn!("Failed to subscribe to ctrl topic: {e}");
        }

        // 3. CAS install. If the `net` slot is already Some, another task won
        //    the race — shut down our locally-built NetManager (which also
        //    cancels the event loop + SV compensation via the shared token)
        //    before returning the "already running" error.
        {
            let mut guard = self.net.lock().await;
            if guard.is_some() {
                drop(guard);
                net_manager.shutdown().await;
                return Err(AppError::NetworkAlreadyRunning);
            }
            *guard = Some(net_manager.clone());
        }
        *self.sync_coordinator.lock().await = Some(coordinator);

        // 4. Post-install wiring. Locks are released; free to await.
        //    Retroactively inject WorkspaceSync into already-open workspaces.
        for ws in self.list_workspaces().await {
            self.install_workspace_sync(&ws, &client, false).await;
        }

        // Background: announce online, bootstrap DHT, reload paired devices,
        // reconnect to paired peers.
        let announcer = net_manager.online_announcer.clone();
        let bootstrap_client = client.clone();
        let pairing_for_bootstrap = net_manager.pairing_manager.clone();

        tokio::spawn(async move {
            if let Err(e) = announcer.announce_online().await {
                tracing::warn!("Failed to announce online: {e}");
            }

            match bootstrap_client.bootstrap().await {
                Ok(_) => info!("DHT bootstrap completed"),
                Err(e) => tracing::warn!("DHT bootstrap failed: {e}"),
            }

            if let Err(e) = pairing_for_bootstrap.load_paired_devices().await {
                tracing::warn!("Failed to load paired devices: {e}");
            }
            let paired_peer_ids = pairing_for_bootstrap.get_paired_peer_ids();
            announcer.check_paired_online(paired_peer_ids).await;
        });

        // Periodic DHT record renewal.
        net_manager
            .online_announcer
            .clone()
            .spawn_renewal_task(cancel_token);

        self.event_bus.emit(AppEvent::NodeStarted);
        info!("P2P node started, PeerId: {peer_id}");
        Ok(())
    }

    /// Stop the P2P node. No-op if already stopped.
    ///
    /// Concurrency: the `net` lock is released before the (potentially
    /// long-running) `NetManager::shutdown().await` call so concurrent
    /// readers see `None` immediately.
    pub async fn stop_network(&self) -> AppResult<()> {
        // Take the manager out under the lock, then release before awaiting.
        let manager = self.net.lock().await.take();
        let Some(manager) = manager else {
            return Ok(());
        };

        // Clear the coordinator slot too — same short-hold pattern.
        let _ = self.sync_coordinator.lock().await.take();

        manager.shutdown().await;

        // Tear down per-workspace sync runtimes now that the coordinator
        // is gone. Lock on `workspaces` is brief and no longer competes
        // with `net`.
        for ws in self.list_workspaces().await {
            if let Some(sync) = ws.take_sync().await {
                sync.close().await;
            }
        }

        self.event_bus.emit(AppEvent::NodeStopped);
        info!("P2P node stopped");
        Ok(())
    }

    /// Convenience: get a shared `AppNetClient` if P2P is running.
    pub async fn client(&self) -> AppResult<AppNetClient> {
        self.net()
            .await
            .map(|n| n.client.clone())
            .ok_or(AppError::NetworkNotRunning)
    }

    /// Convenience: get `PairingManager` if P2P is running.
    pub async fn pairing(&self) -> AppResult<Arc<crate::pairing::PairingManager>> {
        self.net()
            .await
            .map(|n| n.pairing_manager.clone())
            .ok_or(AppError::NetworkNotRunning)
    }

    /// Convenience: get `DeviceManager` if P2P is running.
    pub async fn devices(&self) -> AppResult<Arc<crate::device::DeviceManager>> {
        self.net()
            .await
            .map(|n| n.device_manager.clone())
            .ok_or(AppError::NetworkNotRunning)
    }

    /// Convenience: get `AppSyncCoordinator` if P2P is running.
    pub async fn sync_coordinator_or_err(&self) -> AppResult<Arc<AppSyncCoordinator>> {
        self.sync_coordinator()
            .await
            .ok_or(AppError::NetworkNotRunning)
    }

    /// Create a [`WorkspaceSync`], subscribe to GossipSub, and install it
    /// on the workspace. If `publish_opened` is true, also broadcast a
    /// `WorkspaceOpened` ctrl message to connected peers.
    async fn install_workspace_sync(
        self: &Arc<Self>,
        ws: &Arc<WorkspaceCore>,
        client: &AppNetClient,
        publish_opened: bool,
    ) {
        let ws_sync = Arc::new(WorkspaceSync::new(ws.info.id, self.clone(), client.clone()));
        ws_sync.subscribe().await;
        if publish_opened {
            ws_sync.publish_workspace_opened().await;
        }
        ws.set_sync(Some(ws_sync)).await;
    }
}

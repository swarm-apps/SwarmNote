//! [`AppSyncCoordinator`] — global (AppCore-level) sync orchestration.
//!
//! Manages full-sync session de-duplication, state-vector compensation,
//! ctrl-topic message routing, and inbound sync-request dispatch. Created
//! when the P2P node starts, destroyed when it stops.
//!
//! Per-workspace state (pending buffer, GossipSub subscription, asset-check
//! handles) lives in [`super::WorkspaceSync`]; this coordinator routes
//! incoming events to the relevant workspace instance.

use std::sync::Arc;

use dashmap::DashMap;
use swarm_p2p_core::libp2p::PeerId;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
use uuid::Uuid;

use crate::app::AppCore;
use crate::network::AppNetClient;
use crate::protocol::{AppResponse, SyncRequest, SyncResponse};

use super::{asset_sync, doc_sync, full_sync};

/// Global sync coordinator owned by [`AppCore`].
pub struct AppSyncCoordinator {
    core: Arc<AppCore>,
    client: AppNetClient,
    /// Prevents duplicate full-sync for the same (peer, workspace).
    active_syncs: DashMap<(PeerId, Uuid), CancellationToken>,
    /// Signal to trigger an urgent SV compensation round.
    sv_urgent: Arc<Notify>,
}

impl AppSyncCoordinator {
    pub fn new(core: Arc<AppCore>, client: AppNetClient) -> Self {
        Self {
            core,
            client,
            active_syncs: DashMap::new(),
            sv_urgent: Arc::new(Notify::new()),
        }
    }

    pub fn client(&self) -> &AppNetClient {
        &self.client
    }

    /// Called when a paired peer comes online. Spawns full-sync for each
    /// locally registered workspace.
    pub async fn on_paired_peer_connected(self: &Arc<Self>, peer_id: PeerId) {
        info!("Paired peer connected: {peer_id}, will trigger full sync");
        for ws in self.core.list_workspaces().await {
            self.ensure_subscribed_and_sync(peer_id, &ws).await;
        }
    }

    /// Handle an incoming ctrl-topic message from a peer.
    pub async fn handle_ctrl_message(self: &Arc<Self>, source: PeerId, msg: super::CtrlMessage) {
        match msg {
            super::CtrlMessage::WorkspaceOpened { uuid } => {
                if let Some(ws) = self.core.get_workspace(&uuid).await {
                    info!(
                        "Peer {source} opened workspace {uuid} which is also open locally — syncing"
                    );
                    self.ensure_subscribed_and_sync(source, &ws).await;
                }
            }
        }
    }

    /// Ensure the workspace's GossipSub topic is subscribed, then spawn a
    /// full-sync task. Used by both `on_paired_peer_connected` and
    /// `handle_ctrl_message`.
    async fn ensure_subscribed_and_sync(
        self: &Arc<Self>,
        peer_id: PeerId,
        ws: &std::sync::Arc<crate::workspace::WorkspaceCore>,
    ) {
        if let Some(ws_sync) = ws.sync().await {
            ws_sync.subscribe().await;
        }
        self.spawn_full_sync(peer_id, ws.info.id).await;
    }

    /// Spawn a full sync task if not already running for this (peer, workspace).
    pub async fn spawn_full_sync(self: &Arc<Self>, peer_id: PeerId, workspace_uuid: Uuid) {
        let key = (peer_id, workspace_uuid);

        let cancel = {
            use dashmap::mapref::entry::Entry;
            match self.active_syncs.entry(key) {
                Entry::Occupied(_) => {
                    info!("Full sync already active for {peer_id} / {workspace_uuid}, skipping");
                    return;
                }
                Entry::Vacant(e) => {
                    let cancel = CancellationToken::new();
                    e.insert(cancel.clone());
                    cancel
                }
            }
        };

        let this = Arc::clone(self);
        let core = Arc::clone(&self.core);
        let client = self.client.clone();

        tokio::spawn(async move {
            let result = full_sync::full_sync(core, client, peer_id, workspace_uuid, cancel).await;
            if let Err(e) = result {
                warn!("Full sync failed for {peer_id} / {workspace_uuid}: {e}");
            }
            this.active_syncs.remove(&key);
        });
    }

    /// Handle an inbound sync request from a remote peer.
    pub async fn handle_inbound_request(
        &self,
        peer_id: PeerId,
        pending_id: u64,
        request: SyncRequest,
    ) {
        match request {
            SyncRequest::DocList { workspace_uuid } => {
                info!("Inbound DocList request from {peer_id} for workspace {workspace_uuid}");
                match full_sync::build_local_doc_list(&self.core, workspace_uuid).await {
                    Ok(docs) => {
                        let resp = AppResponse::Sync(SyncResponse::DocList { docs });
                        if let Err(e) = self.client.send_response(pending_id, resp).await {
                            warn!("Failed to send DocList response to {peer_id}: {e}");
                        }
                    }
                    Err(e) => {
                        warn!("Failed to build DocList for workspace {workspace_uuid}: {e}");
                    }
                }
            }
            SyncRequest::StateVector { doc_id, sv } => {
                info!("Inbound StateVector request from {peer_id} for doc {doc_id}");
                match self.find_doc_context(doc_id).await.map(|(ws, _)| ws) {
                    Some(ws_uuid) => {
                        if let Err(e) = doc_sync::handle_state_vector_request(
                            &self.core,
                            &self.client,
                            pending_id,
                            doc_id,
                            &sv,
                            ws_uuid,
                        )
                        .await
                        {
                            warn!("SV response failed for {doc_id}: {e}");
                        }
                    }
                    None => warn!("No workspace found for doc {doc_id}"),
                }
            }
            SyncRequest::FullSync { doc_id } => {
                info!("Inbound FullSync request from {peer_id} for doc {doc_id}");
                match self.find_doc_context(doc_id).await.map(|(ws, _)| ws) {
                    Some(ws_uuid) => {
                        if let Err(e) = doc_sync::handle_full_sync_request(
                            &self.core,
                            &self.client,
                            pending_id,
                            doc_id,
                            ws_uuid,
                        )
                        .await
                        {
                            warn!("FullSync response failed for {doc_id}: {e}");
                        }
                    }
                    None => warn!("No workspace found for doc {doc_id}"),
                }
            }
            SyncRequest::AssetManifest { doc_id } => {
                info!("Inbound AssetManifest request from {peer_id} for doc {doc_id}");
                if let Some((ws_uuid, rel_path)) = self.find_doc_context(doc_id).await {
                    if let Err(e) = asset_sync::handle_asset_manifest_request(
                        &self.core,
                        &self.client,
                        pending_id,
                        doc_id,
                        ws_uuid,
                        &rel_path,
                    )
                    .await
                    {
                        warn!("AssetManifest response failed for {doc_id}: {e}");
                    }
                }
            }
            SyncRequest::AssetChunk {
                doc_id,
                name,
                chunk_index,
            } => {
                if let Some((ws_uuid, rel_path)) = self.find_doc_context(doc_id).await {
                    if let Err(e) = asset_sync::handle_asset_chunk_request(
                        &self.core,
                        &self.client,
                        pending_id,
                        doc_id,
                        &name,
                        chunk_index,
                        ws_uuid,
                        &rel_path,
                    )
                    .await
                    {
                        warn!("AssetChunk response failed for {name}#{chunk_index}: {e}");
                    }
                }
            }
        }
    }

    /// Handle an incoming workspace-level GossipSub message. Routes to the
    /// workspace's [`WorkspaceSync`] for open-doc apply or pending-buffer.
    pub async fn handle_ws_gossip_update(
        &self,
        source: Option<PeerId>,
        workspace_uuid: Uuid,
        doc_uuid: Uuid,
        data: Vec<u8>,
    ) {
        let Some(ws) = self.core.get_workspace(&workspace_uuid).await else {
            return;
        };
        if let Some(ws_sync) = ws.sync().await {
            ws_sync
                .handle_gossip_update(&ws, source, doc_uuid, data)
                .await;
        }
    }

    /// Find workspace UUID and rel_path for a document by searching all open
    /// workspaces. Cross-workspace operation — must live at AppCore level.
    async fn find_doc_context(&self, doc_id: Uuid) -> Option<(Uuid, String)> {
        use entity::workspace::documents;
        use sea_orm::EntityTrait;

        for ws in self.core.list_workspaces().await {
            if let Ok(Some(doc)) = documents::Entity::find_by_id(doc_id).one(ws.db()).await {
                return Some((ws.info.id, doc.rel_path));
            }
        }
        None
    }

    /// Start periodic SV compensation for open documents.
    pub fn start_sv_compensation(self: &Arc<Self>, cancel: CancellationToken) {
        let this = Arc::clone(self);
        let urgent = Arc::clone(&self.sv_urgent);

        tokio::spawn(async move {
            let period = std::time::Duration::from_secs(300);
            let urgent_debounce = std::time::Duration::from_secs(30);
            let mut interval = tokio::time::interval(period);

            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        info!("SV compensation task stopped");
                        return;
                    }
                    _ = interval.tick() => {}
                    _ = urgent.notified() => {
                        tokio::time::sleep(urgent_debounce).await;
                        interval.reset();
                    }
                }
                this.run_sv_compensation().await;
            }
        });
    }

    async fn run_sv_compensation(self: &Arc<Self>) {
        let workspaces = self.core.list_workspaces().await;
        let mut per_ws_open_docs: Vec<(Uuid, Vec<Uuid>)> = Vec::with_capacity(workspaces.len());
        for ws in &workspaces {
            let open = ws.ydoc().list_open_doc_uuids();
            if !open.is_empty() {
                per_ws_open_docs.push((ws.info.id, open));
            }
        }
        if per_ws_open_docs.is_empty() {
            return;
        }

        let Some(net) = self.core.net().await else {
            return;
        };
        let connected_peers = net.device_manager.connected_paired_peers();
        if connected_peers.is_empty() {
            return;
        }

        for (ws_uuid, open_docs) in &per_ws_open_docs {
            for doc_uuid in open_docs {
                for peer_id in &connected_peers {
                    if let Err(e) = doc_sync::sync_via_state_vector(
                        &self.core,
                        &self.client,
                        *peer_id,
                        *ws_uuid,
                        *doc_uuid,
                    )
                    .await
                    {
                        tracing::trace!(
                            "SV compensation failed for doc {doc_uuid} with {peer_id}: {e}"
                        );
                    }
                }
            }
        }
    }

    /// Notify the SV compensation loop to run an urgent round (called by
    /// [`WorkspaceSync::publish_doc_update`] on GossipSub publish failure).
    pub fn signal_sv_urgent(&self) {
        self.sv_urgent.notify_one();
    }
}

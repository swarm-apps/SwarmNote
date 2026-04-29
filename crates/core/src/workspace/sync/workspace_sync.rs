//! [`WorkspaceSync`] — per-workspace sync state and GossipSub operations.
//!
//! Created by [`AppCore::start_network`] (or `open_workspace` if P2P is
//! already running) and installed into [`WorkspaceCore`] via
//! [`WorkspaceCore::set_sync`]. Torn down by [`WorkspaceCore::close`] or
//! [`AppCore::stop_network`].
//!
//! Holds per-workspace resources: the pending-update buffer for closed docs,
//! debounced asset-check handles, and the GossipSub topic subscription.

use std::sync::Arc;

use dashmap::DashMap;
use swarm_p2p_core::libp2p::PeerId;
use tokio::task::AbortHandle;
use tracing::{info, warn};
use uuid::Uuid;

use crate::app::AppCore;
use crate::network::AppNetClient;

use super::pending_buffer::PendingUpdateBuffer;
use super::{asset_sync, doc_sync};

/// Per-workspace sync runtime. Lifecycle follows [`crate::workspace::WorkspaceCore`].
pub struct WorkspaceSync {
    workspace_id: Uuid,
    core: Arc<AppCore>,
    client: AppNetClient,
    /// Buffer for GossipSub updates targeting closed documents.
    pending_buffer: PendingUpdateBuffer,
    /// Debounce: pending asset-check tasks per doc_uuid.
    asset_check_handles: DashMap<Uuid, AbortHandle>,
    /// Abort handle for the pending buffer flush task.
    pending_flush_handle: std::sync::Mutex<Option<AbortHandle>>,
}

impl WorkspaceSync {
    pub fn new(workspace_id: Uuid, core: Arc<AppCore>, client: AppNetClient) -> Self {
        let pending_buffer = PendingUpdateBuffer::new();
        let flush_handle = pending_buffer.spawn_flush_task(workspace_id, Arc::clone(&core));
        Self {
            workspace_id,
            core,
            client,
            pending_buffer,
            asset_check_handles: DashMap::new(),
            pending_flush_handle: std::sync::Mutex::new(Some(flush_handle)),
        }
    }

    pub fn workspace_id(&self) -> Uuid {
        self.workspace_id
    }

    pub async fn subscribe(&self) {
        let topic = super::ws_topic(&self.workspace_id);
        match self.client.subscribe(&topic).await {
            Ok(_) => info!("Subscribed to workspace GossipSub topic: {topic}"),
            Err(e) => warn!("Failed to subscribe to {topic}: {e}"),
        }
    }

    pub async fn unsubscribe(&self) {
        let topic = super::ws_topic(&self.workspace_id);
        match self.client.unsubscribe(&topic).await {
            Ok(_) => info!("Unsubscribed from workspace GossipSub topic: {topic}"),
            Err(e) => warn!("Failed to unsubscribe from {topic}: {e}"),
        }
    }

    /// Notify connected peers that this workspace is open, triggering
    /// them to subscribe + start full sync.
    pub async fn publish_workspace_opened(&self) {
        let payload = super::encode_ctrl_message(&super::CtrlMessage::WorkspaceOpened {
            uuid: self.workspace_id,
        });
        if let Err(e) = self.client.publish(super::CTRL_TOPIC, payload).await {
            warn!(
                "Failed to publish WorkspaceOpened for {}: {e}",
                self.workspace_id
            );
        }
    }

    /// Broadcast a local edit to connected peers via GossipSub. On failure,
    /// signals the coordinator to run urgent SV compensation.
    pub async fn publish_doc_update(&self, doc_uuid: Uuid, update: Vec<u8>) {
        let topic = super::ws_topic(&self.workspace_id);
        let payload = super::encode_ws_gossip(&doc_uuid, &update);
        if let Err(e) = self.client.publish(&topic, payload).await {
            tracing::debug!("Failed to publish doc update to {topic}: {e}");
            // Schedule urgent SV compensation to ensure data consistency.
            if let Some(coordinator) = self.core.sync_coordinator().await {
                coordinator.signal_sv_urgent();
            }
        }
    }

    /// Route an incoming workspace GossipSub payload to an open doc or the
    /// pending buffer (for closed docs).
    ///
    /// `ws` is the owning [`WorkspaceCore`] — passed by the coordinator to
    /// avoid a redundant `AppCore::get_workspace` Mutex lookup on every
    /// incoming GossipSub message.
    pub async fn handle_gossip_update(
        &self,
        ws: &std::sync::Arc<crate::workspace::WorkspaceCore>,
        source: Option<PeerId>,
        doc_uuid: Uuid,
        data: Vec<u8>,
    ) {
        match ws.ydoc().apply_sync_update(&doc_uuid, &data).await {
            Some(Ok(())) => {
                self.schedule_asset_check(source, doc_uuid);
            }
            Some(Err(e)) => {
                warn!("Failed to apply ws gossip update for {doc_uuid}: {e}");
            }
            None => {
                // Doc not open — buffer for later flush.
                if let Some((overflow_peer, overflow)) =
                    self.pending_buffer.push(doc_uuid, data, source).await
                {
                    for update in &overflow {
                        if let Err(e) = doc_sync::apply_remote_update(
                            &self.core,
                            self.workspace_id,
                            doc_uuid,
                            update,
                        )
                        .await
                        {
                            warn!("Overflow flush failed for doc {doc_uuid}: {e}");
                            break;
                        }
                    }
                    // Schedule asset sync for the overflowed doc (previously
                    // lost because push() didn't return the source peer).
                    self.schedule_asset_check(overflow_peer, doc_uuid);
                }
            }
        }
    }

    /// Debounced asset check after a remote doc update.
    fn schedule_asset_check(&self, source: Option<PeerId>, doc_uuid: Uuid) {
        let Some(peer) = source else { return };

        if let Some((_, old_handle)) = self.asset_check_handles.remove(&doc_uuid) {
            old_handle.abort();
        }

        let core = Arc::clone(&self.core);
        let client = self.client.clone();
        let workspace_id = self.workspace_id;
        let handles = self.asset_check_handles.clone();

        let handle = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            handles.remove(&doc_uuid);

            // Look up rel_path from DB.
            use entity::workspace::documents;
            use sea_orm::EntityTrait;
            let Some(ws) = core.get_workspace(&workspace_id).await else {
                return;
            };
            let Ok(Some(doc)) = documents::Entity::find_by_id(doc_uuid).one(ws.db()).await else {
                return;
            };

            if let Err(e) = asset_sync::sync_doc_assets(
                &core,
                &client,
                peer,
                workspace_id,
                doc_uuid,
                &doc.rel_path,
            )
            .await
            {
                warn!("Incremental asset sync failed for {doc_uuid}: {e}");
            }
        });

        self.asset_check_handles
            .insert(doc_uuid, handle.abort_handle());
    }

    /// Tear down: abort flush task + unsubscribe from GossipSub.
    pub async fn close(&self) {
        // Abort pending buffer flush task.
        if let Some(handle) = self
            .pending_flush_handle
            .lock()
            .expect("pending_flush_handle mutex")
            .take()
        {
            handle.abort();
        }
        // Abort in-flight asset checks.
        for entry in self.asset_check_handles.iter() {
            entry.value().abort();
        }
        self.asset_check_handles.clear();

        self.unsubscribe().await;
    }
}

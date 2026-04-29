//! Per-workspace in-memory buffer for GossipSub updates targeting closed
//! documents.
//!
//! Accumulates raw yrs updates keyed by `doc_uuid` and flushes them in
//! batch after a configurable debounce interval (3 s). Enforces a per-doc
//! cap to prevent unbounded memory growth.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use swarm_p2p_core::libp2p::PeerId;
use tokio::sync::Mutex;
use tokio::task::AbortHandle;
use tracing::{info, warn};
use uuid::Uuid;

use super::{asset_sync, doc_sync};

/// Debounce interval: flush pending updates 3 s after the last write.
const FLUSH_DEBOUNCE: Duration = Duration::from_secs(3);
/// Tick interval for the background flush task.
const TICK_INTERVAL: Duration = Duration::from_millis(500);
/// Maximum number of buffered updates per document before forcing an early flush.
const MAX_UPDATES_PER_DOC: usize = 500;

#[derive(Debug)]
struct PendingEntry {
    source_peer: Option<PeerId>,
    updates: Vec<Vec<u8>>,
    last_write: Instant,
}

/// (doc_uuid, source_peer, updates) ready to flush.
type FlushBatch = Vec<(Uuid, Option<PeerId>, Vec<Vec<u8>>)>;

/// Thread-safe buffer scoped to a single workspace. One instance per
/// [`super::WorkspaceSync`].
pub struct PendingUpdateBuffer {
    entries: Arc<Mutex<HashMap<Uuid, PendingEntry>>>,
}

impl PendingUpdateBuffer {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Push a raw yrs update for a closed document.
    ///
    /// If the per-doc count exceeds [`MAX_UPDATES_PER_DOC`], the entry is
    /// drained and returned for immediate flushing by the caller.
    /// Returns `Some((source_peer, updates))` when the cap is exceeded,
    /// so the caller can flush immediately and schedule an asset check.
    pub async fn push(
        &self,
        doc_uuid: Uuid,
        update: Vec<u8>,
        source: Option<PeerId>,
    ) -> Option<(Option<PeerId>, Vec<Vec<u8>>)> {
        let mut map = self.entries.lock().await;
        let entry = map.entry(doc_uuid).or_insert_with(|| PendingEntry {
            source_peer: source,
            updates: Vec::new(),
            last_write: Instant::now(),
        });
        entry.updates.push(update);
        entry.last_write = Instant::now();
        if source.is_some() {
            entry.source_peer = source;
        }

        if entry.updates.len() >= MAX_UPDATES_PER_DOC {
            let drained = map.remove(&doc_uuid).unwrap();
            Some((drained.source_peer, drained.updates))
        } else {
            None
        }
    }

    /// Spawn a background task that periodically flushes stale entries.
    pub fn spawn_flush_task(
        &self,
        workspace_id: Uuid,
        core: Arc<crate::app::AppCore>,
    ) -> AbortHandle {
        let entries = Arc::clone(&self.entries);
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(TICK_INTERVAL);
            loop {
                interval.tick().await;

                let ready: FlushBatch = {
                    let mut map = entries.lock().await;
                    let now = Instant::now();
                    let mut to_flush = Vec::new();
                    let mut to_remove = Vec::new();

                    for (doc_uuid, entry) in map.iter() {
                        if now.duration_since(entry.last_write) >= FLUSH_DEBOUNCE {
                            to_remove.push(*doc_uuid);
                        }
                    }

                    for key in &to_remove {
                        if let Some(mut entry) = map.remove(key) {
                            to_flush.push((
                                *key,
                                entry.source_peer,
                                std::mem::take(&mut entry.updates),
                            ));
                        }
                    }
                    to_flush
                };

                for (doc_uuid, source_peer, updates) in ready {
                    let flushed =
                        flush_updates(&core, workspace_id, doc_uuid, updates, &entries).await;

                    if flushed {
                        schedule_closed_doc_asset_sync(&core, source_peer, workspace_id, doc_uuid)
                            .await;
                    }
                }
            }
        });
        handle.abort_handle()
    }
}

/// Flush a batch of updates for a single document. Re-buffers remaining
/// updates on failure for retry on the next tick.
async fn flush_updates(
    core: &Arc<crate::app::AppCore>,
    workspace_uuid: Uuid,
    doc_uuid: Uuid,
    updates: Vec<Vec<u8>>,
    entries: &Arc<Mutex<HashMap<Uuid, PendingEntry>>>,
) -> bool {
    let total = updates.len();
    for (i, update) in updates.iter().enumerate() {
        if let Err(e) = doc_sync::apply_remote_update(core, workspace_uuid, doc_uuid, update).await
        {
            warn!("Pending buffer flush failed for doc {doc_uuid} at update {i}/{total}: {e}");
            let remaining: Vec<Vec<u8>> = updates[i..].to_vec();
            let mut map = entries.lock().await;
            let entry = map.entry(doc_uuid).or_insert_with(|| PendingEntry {
                source_peer: None,
                updates: Vec::new(),
                last_write: Instant::now(),
            });
            let mut merged = remaining;
            merged.append(&mut entry.updates);
            entry.updates = merged;
            return i > 0;
        }
    }
    info!("Flushed {total} pending updates for closed doc {doc_uuid}");
    true
}

/// Trigger asset sync for a closed document after its pending updates are
/// flushed.
async fn schedule_closed_doc_asset_sync(
    core: &Arc<crate::app::AppCore>,
    source_peer: Option<PeerId>,
    workspace_uuid: Uuid,
    doc_uuid: Uuid,
) {
    let Some(peer) = source_peer else { return };
    let Some(ws) = core.get_workspace(&workspace_uuid).await else {
        return;
    };

    use entity::workspace::documents;
    use sea_orm::EntityTrait;

    let rel_path = match documents::Entity::find_by_id(doc_uuid).one(ws.db()).await {
        Ok(Some(doc)) => doc.rel_path,
        _ => return,
    };

    let Some(net) = core.net().await else { return };
    let client = net.client.clone();

    if let Err(e) =
        asset_sync::sync_doc_assets(core, &client, peer, workspace_uuid, doc_uuid, &rel_path).await
    {
        warn!("Asset sync after buffer flush failed for doc {doc_uuid}: {e}");
    }
}

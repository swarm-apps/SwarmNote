use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use sea_orm::EntityTrait;
use swarm_p2p_core::libp2p::PeerId;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
use uuid::Uuid;

use crate::app::AppCore;
use crate::error::{AppError, AppResult};
use crate::events::AppEvent;
use crate::network::AppNetClient;
use crate::protocol::{AppRequest, AppResponse, DocMeta, SyncRequest, SyncResponse};

use super::{asset_sync, doc_sync};

/// What to do with each document during full sync.
#[derive(Debug)]
pub enum SyncAction {
    /// Exchange StateVector with a peer for an existing document.
    StateVectorExchange { doc_id: Uuid },
    /// Pull a new document from the peer (never existed locally).
    FullPull { doc_id: Uuid, rel_path: String },
    /// Apply a remote deletion locally.
    ApplyDeletion {
        doc_id: Uuid,
        rel_path: String,
        lamport_clock: i64,
    },
    /// Resurrect a locally deleted document (remote has newer edits).
    Resurrect { doc_id: Uuid, rel_path: String },
    /// Rename the local loser document before pulling the remote winner.
    ResolvePathConflict {
        local_doc_id: Uuid,
        remote_doc_id: Uuid,
        rel_path: String,
        remote_clock: i64,
        local_clock: i64,
    },
}

/// Build the local DocList for a workspace by merging documents + deletion_log.
pub async fn build_local_doc_list(
    core: &Arc<AppCore>,
    workspace_uuid: Uuid,
) -> AppResult<Vec<DocMeta>> {
    let ws = core
        .get_workspace(&workspace_uuid)
        .await
        .ok_or(AppError::NoWorkspaceDb)?;
    let db: &sea_orm::DatabaseConnection = ws.db();

    // Active documents
    use entity::workspace::documents;
    let active = documents::Entity::find().all(db).await?;

    // Tombstones
    use entity::workspace::deletion_log;
    let deleted = deletion_log::Entity::find().all(db).await?;

    let mut list = Vec::with_capacity(active.len() + deleted.len());

    for doc in active {
        list.push(DocMeta {
            doc_id: doc.id,
            rel_path: doc.rel_path,
            title: doc.title,
            updated_at: doc.updated_at.timestamp_millis(),
            deleted_at: None,
            lamport_clock: doc.lamport_clock,
            workspace_uuid,
        });
    }

    for tomb in deleted {
        list.push(DocMeta {
            doc_id: tomb.doc_id,
            rel_path: tomb.rel_path,
            title: String::new(),
            updated_at: tomb.deleted_at.timestamp_millis(),
            deleted_at: Some(tomb.deleted_at.timestamp_millis()),
            lamport_clock: tomb.lamport_clock,
            workspace_uuid,
        });
    }

    Ok(list)
}

/// Send a DocList request to a remote peer with a timeout.
pub async fn request_doc_list(
    client: &AppNetClient,
    peer_id: PeerId,
    workspace_uuid: Uuid,
) -> AppResult<Vec<DocMeta>> {
    let request = AppRequest::Sync(SyncRequest::DocList { workspace_uuid });

    let response = tokio::time::timeout(
        Duration::from_secs(5),
        client.send_request(peer_id, request),
    )
    .await
    .map_err(|_| AppError::SwarmIo {
        context: "send_request DocList",
        reason: format!("timed out for peer {peer_id}"),
    })?
    .map_err(|e| AppError::SwarmIo {
        context: "send_request DocList",
        reason: format!("peer {peer_id}: {e}"),
    })?;

    match response {
        AppResponse::Sync(SyncResponse::DocList { docs }) => Ok(docs),
        other => Err(AppError::SwarmIo {
            context: "DocList response",
            reason: format!("unexpected response: {other:?}"),
        }),
    }
}

/// Ensure we hold this workspace's key before syncing. Joined workspaces start
/// keyless; request the owner's key (sealed to us) over the Noise-encrypted
/// request-response channel and install it. Best-effort: on failure, RR-based
/// full sync still pulls document state — only real-time gossip stays
/// undecryptable until a key arrives.
async fn ensure_workspace_key(
    core: &Arc<AppCore>,
    client: &AppNetClient,
    peer_id: PeerId,
    workspace_uuid: Uuid,
) {
    let Some(ws) = core.get_workspace(&workspace_uuid).await else {
        return;
    };
    if !ws.keys().await.is_empty() {
        return;
    }

    let request = AppRequest::Sync(SyncRequest::WorkspaceKey { workspace_uuid });
    let response = match tokio::time::timeout(
        Duration::from_secs(5),
        client.send_request(peer_id, request),
    )
    .await
    {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            warn!("WorkspaceKey request to {peer_id} failed: {e}");
            return;
        }
        Err(_) => {
            warn!("WorkspaceKey request to {peer_id} timed out");
            return;
        }
    };

    let AppResponse::Sync(SyncResponse::WorkspaceKey { sealed, ops, .. }) = response else {
        warn!("Peer {peer_id} returned unexpected response for workspace key");
        return;
    };

    // Persist the permission chain so we can materialize our own role, even if
    // the key itself was declined.
    for op in &ops {
        if op.verify() {
            let _ = crate::workspace::permissions::save_op(ws.db(), workspace_uuid, op).await;
        }
    }

    // Pin our local `created_by` to the chain's authoritative owner. A fresh
    // joiner's row was created with this device as `created_by`; without this
    // pin, `materialize` would reject the owner's genesis (issuer != our
    // `created_by`) and our role map would be empty. We trust the single
    // genesis in the chain we just received over the Noise-authenticated RR
    // channel from the peer we chose to sync from.
    if let Some(owner) = crate::workspace::permissions::genesis_owner(&ops) {
        if let Err(e) = crate::workspace::pin_workspace_owner(ws.db(), workspace_uuid, &owner).await
        {
            warn!("Failed to pin workspace owner for {workspace_uuid}: {e}");
        }
    }

    let Some(sk) = sealed else {
        warn!("Peer {peer_id} declined workspace key for {workspace_uuid} (not authorized?)");
        return;
    };

    let my_peer = match core.identity().peer_id() {
        Ok(p) => p,
        Err(e) => {
            warn!("Cannot install workspace key: {e}");
            return;
        }
    };

    if let Err(e) = crate::workspace::keys::install_received_key(
        ws.db(),
        workspace_uuid,
        &my_peer,
        &peer_id.to_string(),
        sk.key_version,
        sk.sealed_read,
        sk.sealed_write,
    )
    .await
    {
        warn!("Failed to install received workspace key: {e}");
        return;
    }

    match ws.reload_keys().await {
        Ok(()) => info!(
            "Installed workspace key v{} for {workspace_uuid} from {peer_id}",
            sk.key_version
        ),
        Err(e) => warn!("Failed to reload keys after install: {e}"),
    }
}

/// Diff remote DocList against local state to produce a sync plan.
///
/// Single-direction: decides what the **local** side needs to do based on
/// what the **remote** side has. The remote side runs the same algorithm
/// independently (symmetric design).
pub fn diff_doc_lists(local: &[DocMeta], remote: &[DocMeta]) -> Vec<SyncAction> {
    // Index local state
    let local_active: HashMap<Uuid, &DocMeta> = local
        .iter()
        .filter(|d| d.deleted_at.is_none())
        .map(|d| (d.doc_id, d))
        .collect();

    let local_tombstones: HashMap<Uuid, &DocMeta> = local
        .iter()
        .filter(|d| d.deleted_at.is_some())
        .map(|d| (d.doc_id, d))
        .collect();

    // Index local rel_path → doc_id for conflict detection
    let local_path_to_id: HashMap<&str, (Uuid, i64)> = local
        .iter()
        .filter(|d| d.deleted_at.is_none())
        .map(|d| (d.rel_path.as_str(), (d.doc_id, d.lamport_clock)))
        .collect();

    let mut actions = Vec::new();

    for remote_doc in remote {
        if let Some(_deleted_at) = remote_doc.deleted_at {
            // Remote is a tombstone
            if let Some(local_doc) = local_active.get(&remote_doc.doc_id) {
                if local_doc.lamport_clock < remote_doc.lamport_clock {
                    // Remote deletion is newer → apply deletion
                    actions.push(SyncAction::ApplyDeletion {
                        doc_id: remote_doc.doc_id,
                        rel_path: remote_doc.rel_path.clone(),
                        lamport_clock: remote_doc.lamport_clock,
                    });
                }
                // else: local edit is newer, skip
            }
            // If not in local_active or local_tombstones, ignore
        } else {
            // Remote is an active document
            if local_active.contains_key(&remote_doc.doc_id) {
                // Both sides have the document → StateVector exchange
                actions.push(SyncAction::StateVectorExchange {
                    doc_id: remote_doc.doc_id,
                });
            } else if let Some(local_tomb) = local_tombstones.get(&remote_doc.doc_id) {
                // We deleted it, remote still has it
                if remote_doc.lamport_clock > local_tomb.lamport_clock {
                    // Remote is newer → resurrect
                    actions.push(SyncAction::Resurrect {
                        doc_id: remote_doc.doc_id,
                        rel_path: remote_doc.rel_path.clone(),
                    });
                }
                // else: our deletion is authoritative, skip
            } else {
                // New document we don't have at all
                // Check for rel_path conflict (different UUID, same path)
                if let Some(&(local_id, local_clock)) =
                    local_path_to_id.get(remote_doc.rel_path.as_str())
                {
                    // Same path, different UUID → Lamport clock arbitration
                    actions.push(SyncAction::ResolvePathConflict {
                        local_doc_id: local_id,
                        remote_doc_id: remote_doc.doc_id,
                        rel_path: remote_doc.rel_path.clone(),
                        remote_clock: remote_doc.lamport_clock,
                        local_clock,
                    });
                } else {
                    actions.push(SyncAction::FullPull {
                        doc_id: remote_doc.doc_id,
                        rel_path: remote_doc.rel_path.clone(),
                    });
                }
            }
        }
    }

    actions
}

/// Priority key for sorting sync actions.
/// P0 = currently open, P1 = recently edited, P2 = everything else.
async fn priority_key(action: &SyncAction, core: &Arc<AppCore>, workspace_uuid: Uuid) -> (u8, i64) {
    let doc_id = match action {
        SyncAction::StateVectorExchange { doc_id } => *doc_id,
        SyncAction::FullPull { doc_id, .. } => *doc_id,
        SyncAction::Resurrect { doc_id, .. } => *doc_id,
        SyncAction::ResolvePathConflict { remote_doc_id, .. } => *remote_doc_id,
        SyncAction::ApplyDeletion { .. } => return (0, 0), // deletions are cheap, do first
    };

    // P0 if document is currently open
    if let Some(ws) = core.get_workspace(&workspace_uuid).await {
        if ws.ydoc().is_doc_open(&doc_id) {
            return (0, 0);
        }
    }
    (2, 0) // P2 for everything else; P1 would need updated_at lookup, defer for now
}

// ── Full sync orchestration ──

/// Execute a full sync session with a peer for a specific workspace.
///
/// Emits `SyncStarted` first, then `SyncCompleted` on every exit path —
/// including early-return errors from inner steps like `request_doc_list`.
/// Without this guarantee the frontend wizard can hang on the spinner forever
/// when, say, the DocList request times out before any progress event fires.
pub async fn full_sync(
    core: Arc<AppCore>,
    client: AppNetClient,
    peer_id: PeerId,
    workspace_uuid: Uuid,
    cancel: CancellationToken,
) -> AppResult<()> {
    info!("Starting full sync with {peer_id} for workspace {workspace_uuid}");

    core.event_bus.emit(AppEvent::SyncStarted {
        workspace_id: workspace_uuid,
        peer_id: peer_id.to_string(),
    });

    let result = run_full_sync(&core, &client, peer_id, workspace_uuid, &cancel).await;

    let cancelled = cancel.is_cancelled();
    let error = match &result {
        Ok(_) => None,
        Err(e) => {
            warn!("Full sync failed for {peer_id} / {workspace_uuid}: {e}");
            Some(e.to_string())
        }
    };

    core.event_bus.emit(AppEvent::SyncCompleted {
        workspace_id: workspace_uuid,
        peer_id: peer_id.to_string(),
        cancelled,
        error,
    });

    info!("Full sync with {peer_id} for workspace {workspace_uuid} ended (cancelled={cancelled})");

    result
}

/// Inner body of `full_sync`. Errors propagate via `?` and are surfaced as
/// `SyncCompleted { error: Some(...) }` by the outer `full_sync`.
async fn run_full_sync(
    core: &Arc<AppCore>,
    client: &AppNetClient,
    peer_id: PeerId,
    workspace_uuid: Uuid,
    cancel: &CancellationToken,
) -> AppResult<()> {
    // 0. Acquire the workspace key if we don't have one (joined workspaces
    //    start keyless). Best-effort — doesn't block doc sync.
    ensure_workspace_key(core, client, peer_id, workspace_uuid).await;

    // 1. Exchange DocLists
    let remote_docs = request_doc_list(client, peer_id, workspace_uuid).await?;
    let local_docs = build_local_doc_list(core, workspace_uuid).await?;

    // 2. Diff
    let mut actions = diff_doc_lists(&local_docs, &remote_docs);

    // 3. Sort by priority (async lookup into YDocManager → compute keys first)
    let mut keyed: Vec<((u8, i64), SyncAction)> = Vec::with_capacity(actions.len());
    for action in actions.drain(..) {
        let key = priority_key(&action, core, workspace_uuid).await;
        keyed.push((key, action));
    }
    keyed.sort_by_key(|(k, _)| *k);
    let actions: Vec<SyncAction> = keyed.into_iter().map(|(_, a)| a).collect();

    let total = actions.len() as u32;
    let mut completed = 0u32;

    // 4. Execute sequentially (each action may do internal I/O concurrency,
    //    serial at the action level avoids DB contention for temp Y.Doc ops)
    for action in &actions {
        if cancel.is_cancelled() {
            info!("Full sync cancelled for {peer_id}");
            break;
        }

        if let Err(e) = execute_action(core, client, peer_id, workspace_uuid, action).await {
            warn!("Sync action failed: {e}");
        }

        completed += 1;

        core.event_bus.emit(AppEvent::SyncProgress {
            workspace_id: workspace_uuid,
            peer_id: peer_id.to_string(),
            completed,
            total,
        });
    }

    info!("Full sync with {peer_id} for workspace {workspace_uuid} actions: {completed}/{total}");

    Ok(())
}

/// Execute a single sync action.
async fn execute_action(
    core: &Arc<AppCore>,
    client: &AppNetClient,
    peer_id: PeerId,
    workspace_uuid: Uuid,
    action: &SyncAction,
) -> AppResult<()> {
    match action {
        SyncAction::StateVectorExchange { doc_id } => {
            doc_sync::sync_via_state_vector(core, client, peer_id, workspace_uuid, *doc_id).await?;
            // Asset sync follows doc sync
            if let Some((_, rel_path)) = find_doc_rel_path(core, workspace_uuid, *doc_id).await {
                asset_sync::sync_doc_assets(
                    core,
                    client,
                    peer_id,
                    workspace_uuid,
                    *doc_id,
                    &rel_path,
                )
                .await
                .ok(); // asset failure non-blocking
            }
        }
        SyncAction::FullPull { doc_id, rel_path } => {
            doc_sync::sync_via_full_pull(core, client, peer_id, workspace_uuid, *doc_id, rel_path)
                .await?;
            asset_sync::sync_doc_assets(core, client, peer_id, workspace_uuid, *doc_id, rel_path)
                .await
                .ok();
        }
        SyncAction::Resurrect { doc_id, rel_path } => {
            doc_sync::sync_via_full_pull(core, client, peer_id, workspace_uuid, *doc_id, rel_path)
                .await?;
            asset_sync::sync_doc_assets(core, client, peer_id, workspace_uuid, *doc_id, rel_path)
                .await
                .ok();
        }
        SyncAction::ApplyDeletion {
            doc_id,
            rel_path,
            lamport_clock,
        } => {
            doc_sync::apply_deletion(core, workspace_uuid, *doc_id, rel_path, *lamport_clock)
                .await?;
        }
        SyncAction::ResolvePathConflict {
            local_doc_id,
            remote_doc_id,
            rel_path,
            remote_clock,
            local_clock,
        } => {
            doc_sync::resolve_path_conflict(
                core,
                client,
                peer_id,
                workspace_uuid,
                *local_doc_id,
                *remote_doc_id,
                rel_path,
                *local_clock,
                *remote_clock,
            )
            .await?;
        }
    }
    Ok(())
}

/// Helper: look up a doc's rel_path from DB.
async fn find_doc_rel_path(
    core: &Arc<AppCore>,
    workspace_uuid: Uuid,
    doc_id: Uuid,
) -> Option<(Uuid, String)> {
    use entity::workspace::documents;
    let ws = core.get_workspace(&workspace_uuid).await?;
    let doc = documents::Entity::find_by_id(doc_id)
        .one(ws.db())
        .await
        .ok()
        .flatten()?;
    Some((workspace_uuid, doc.rel_path))
}

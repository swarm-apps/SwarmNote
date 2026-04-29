//! Asset file sync: manifest exchange, chunked transfer, integrity verification.

use std::collections::HashSet;
use std::io::SeekFrom;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use swarm_p2p_core::libp2p::PeerId;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tracing::{info, warn};
use uuid::Uuid;

use crate::app::AppCore;
use crate::error::{AppError, AppResult};
use crate::network::AppNetClient;
use crate::protocol::{AppRequest, AppResponse, AssetMeta, SyncRequest, SyncResponse};

use super::doc_sync::{asset_dir_from_rel_path, get_workspace_path};

/// 256 KB per chunk (matches SwarmDrop).
const CHUNK_SIZE: usize = 256 * 1024;
/// Max concurrent chunk requests per asset.
const MAX_CONCURRENT_CHUNKS: usize = 4;
/// Max retry attempts per chunk.
const MAX_RETRIES: u32 = 3;

// ── Scanning ──

/// Scan a document's asset directory and return metadata for each file.
/// Runs on a blocking thread to avoid stalling the tokio runtime on large files.
pub async fn scan_asset_dir(workspace_path: &Path, rel_path: &str) -> Vec<AssetMeta> {
    let dir = workspace_path.join(asset_dir_from_rel_path(rel_path));
    tokio::task::spawn_blocking(move || scan_asset_dir_blocking(&dir))
        .await
        .unwrap_or_default()
}

fn scan_asset_dir_blocking(dir: &Path) -> Vec<AssetMeta> {
    if !dir.is_dir() {
        return vec![];
    }

    let mut assets = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![];
    };
    for entry in entries.flatten() {
        if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(data) = std::fs::read(entry.path()) else {
            continue;
        };
        let hash = blake3::hash(&data);
        assets.push(AssetMeta {
            name,
            hash: hash.as_bytes().to_vec(),
            size: data.len() as u64,
        });
    }
    assets
}

// ── Diff ──

/// Compare remote and local manifests. Returns names of assets that exist
/// remotely but not locally (need to be pulled).
pub fn diff_assets(local: &[AssetMeta], remote: &[AssetMeta]) -> Vec<AssetMeta> {
    let local_names: HashSet<&str> = local.iter().map(|a| a.name.as_str()).collect();
    remote
        .iter()
        .filter(|r| !local_names.contains(r.name.as_str()))
        .cloned()
        .collect()
}

// ── Sync orchestration ──

/// Sync assets for a single document: exchange manifest, diff, pull missing.
/// Returns the list of successfully pulled asset file names (empty if none).
pub async fn sync_doc_assets(
    core: &Arc<AppCore>,
    client: &AppNetClient,
    peer_id: PeerId,
    workspace_uuid: Uuid,
    doc_id: Uuid,
    rel_path: &str,
) -> AppResult<Vec<String>> {
    let workspace_path = get_workspace_path(core, workspace_uuid).await?;

    // Request remote manifest
    let request = AppRequest::Sync(SyncRequest::AssetManifest { doc_id });
    let response = tokio::time::timeout(
        Duration::from_secs(5),
        client.send_request(peer_id, request),
    )
    .await
    .map_err(|_| AppError::SwarmIo {
        context: "send_request AssetManifest",
        reason: "timed out".into(),
    })?
    .map_err(|e| AppError::SwarmIo {
        context: "send_request AssetManifest",
        reason: e.to_string(),
    })?;

    let remote_assets = match response {
        AppResponse::Sync(SyncResponse::AssetManifest { assets, .. }) => assets,
        _ => {
            warn!("Unexpected AssetManifest response for doc {doc_id}");
            return Ok(vec![]);
        }
    };

    if remote_assets.is_empty() {
        return Ok(vec![]);
    }

    // Scan local assets
    let local_assets = scan_asset_dir(&workspace_path, rel_path).await;

    // Diff
    let missing = diff_assets(&local_assets, &remote_assets);
    if missing.is_empty() {
        return Ok(vec![]);
    }

    info!("Doc {doc_id}: {} missing asset(s) to pull", missing.len());

    // Ensure asset directory exists
    let asset_dir = workspace_path.join(asset_dir_from_rel_path(rel_path));
    tokio::fs::create_dir_all(&asset_dir).await.ok();

    // Pull each missing asset, track successes
    let mut pulled = Vec::new();
    for asset in &missing {
        if let Err(e) = pull_asset(
            client,
            peer_id,
            doc_id,
            &asset.name,
            asset.size,
            &asset.hash,
            &asset_dir,
        )
        .await
        {
            // Single asset failure does not block overall sync
            warn!("Failed to pull asset {} for doc {doc_id}: {e}", asset.name);
        } else {
            pulled.push(asset.name.clone());
        }
    }

    // Note: asset-update notification previously emitted via Tauri event
    // `yjs:assets-updated`; in the core layer we rely on subsequent
    // sync progress events + the editor's asset re-resolution to pick up
    // the newly pulled files. Host shells MAY listen to the pulled list by
    // observing the `AppEvent::SyncProgress`/`SyncCompleted` hooks.

    Ok(pulled)
}

// ── Chunked pull ──

/// Pull a single asset file using chunked transfer.
async fn pull_asset(
    client: &AppNetClient,
    peer_id: PeerId,
    doc_id: Uuid,
    name: &str,
    file_size: u64,
    expected_hash: &[u8],
    asset_dir: &Path,
) -> AppResult<()> {
    let total_chunks = file_size.div_ceil(CHUNK_SIZE as u64).max(1) as u32;
    let part_path = asset_dir.join(format!("{name}.part"));
    let final_path = asset_dir.join(name);

    // Create .part file
    let file = tokio::fs::File::create(&part_path).await?;
    let file = tokio::sync::Mutex::new(file);

    // Pull chunks with limited concurrency
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_CHUNKS));
    let mut tasks = tokio::task::JoinSet::new();
    let file = std::sync::Arc::new(file);

    for chunk_index in 0..total_chunks {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let client = client.clone();
        let name = name.to_string();
        let file = file.clone();

        tasks.spawn(async move {
            let result = pull_single_chunk(&client, peer_id, doc_id, &name, chunk_index).await;
            drop(permit);
            match result {
                Ok(data) => {
                    // Write chunk at correct offset
                    let offset = chunk_index as u64 * CHUNK_SIZE as u64;
                    let mut f = file.lock().await;
                    f.seek(SeekFrom::Start(offset)).await?;
                    f.write_all(&data).await?;
                    Ok::<_, AppError>(())
                }
                Err(e) => Err(e),
            }
        });
    }

    // Collect results
    let mut any_error = false;
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                warn!("Chunk error for {name}: {e}");
                any_error = true;
            }
            Err(e) => {
                warn!("Chunk task panicked for {name}: {e}");
                any_error = true;
            }
        }
    }

    if any_error {
        tokio::fs::remove_file(&part_path).await.ok();
        return Err(AppError::SwarmIo {
            context: "pull asset chunks",
            reason: format!("failed to pull all chunks for {name}"),
        });
    }

    // Verify integrity
    let data = tokio::fs::read(&part_path).await?;
    let actual_hash = blake3::hash(&data);
    if actual_hash.as_bytes() != expected_hash {
        tokio::fs::remove_file(&part_path).await.ok();
        return Err(AppError::SwarmIo {
            context: "asset hash verify",
            reason: format!(
                "hash mismatch for {name}: expected {:?}, got {:?}",
                &expected_hash[..4],
                &actual_hash.as_bytes()[..4]
            ),
        });
    }

    // Rename .part → final
    tokio::fs::rename(&part_path, &final_path).await?;
    info!("Pulled asset: {name} ({file_size} bytes)");

    Ok(())
}

/// Pull a single chunk with retry.
async fn pull_single_chunk(
    client: &AppNetClient,
    peer_id: PeerId,
    doc_id: Uuid,
    name: &str,
    chunk_index: u32,
) -> AppResult<Vec<u8>> {
    let mut retries = 0;
    loop {
        let request = AppRequest::Sync(SyncRequest::AssetChunk {
            doc_id,
            name: name.to_string(),
            chunk_index,
        });

        let result = tokio::time::timeout(
            Duration::from_secs(30),
            client.send_request(peer_id, request),
        )
        .await;

        let err_msg = match result {
            Ok(Ok(AppResponse::Sync(SyncResponse::AssetChunk { data, .. }))) => {
                return Ok(data);
            }
            Ok(Ok(other)) => {
                return Err(AppError::SwarmIo {
                    context: "pull chunk response",
                    reason: format!("unexpected response: {other:?}"),
                });
            }
            Ok(Err(e)) => format!("{e}"),
            Err(_) => "timeout".to_string(),
        };

        retries += 1;
        if retries > MAX_RETRIES {
            return Err(AppError::SwarmIo {
                context: "pull chunk retry exhausted",
                reason: format!(
                    "chunk {chunk_index} of {name} failed after {MAX_RETRIES} retries: {err_msg}"
                ),
            });
        }

        let delay = Duration::from_millis(100 * 2u64.pow(retries));
        warn!(
            "Chunk {chunk_index} of {name} failed (attempt {retries}/{MAX_RETRIES}): {err_msg}, retrying in {delay:?}"
        );
        tokio::time::sleep(delay).await;
    }
}

// ── Inbound handlers ──

/// Handle inbound AssetManifest request: scan local assets and respond.
pub async fn handle_asset_manifest_request(
    core: &Arc<AppCore>,
    client: &AppNetClient,
    pending_id: u64,
    doc_id: Uuid,
    workspace_uuid: Uuid,
    rel_path: &str,
) -> AppResult<()> {
    let workspace_path = get_workspace_path(core, workspace_uuid).await?;
    let assets = scan_asset_dir(&workspace_path, rel_path).await;

    let resp = AppResponse::Sync(SyncResponse::AssetManifest { doc_id, assets });
    client
        .send_response(pending_id, resp)
        .await
        .map_err(|e| AppError::SwarmIo {
            context: "send_response AssetManifest",
            reason: e.to_string(),
        })?;

    Ok(())
}

/// Handle inbound AssetChunk request: read the requested chunk and respond.
#[allow(clippy::too_many_arguments)]
pub async fn handle_asset_chunk_request(
    core: &Arc<AppCore>,
    client: &AppNetClient,
    pending_id: u64,
    doc_id: Uuid,
    name: &str,
    chunk_index: u32,
    workspace_uuid: Uuid,
    rel_path: &str,
) -> AppResult<()> {
    let workspace_path = get_workspace_path(core, workspace_uuid).await?;
    let asset_dir = workspace_path.join(asset_dir_from_rel_path(rel_path));
    let file_path = asset_dir.join(name);

    let metadata = tokio::fs::metadata(&file_path).await?;
    let file_size = metadata.len();

    let offset = chunk_index as u64 * CHUNK_SIZE as u64;
    let read_size = ((file_size.saturating_sub(offset)) as usize).min(CHUNK_SIZE);

    let mut file = tokio::fs::File::open(&file_path).await?;
    file.seek(SeekFrom::Start(offset)).await?;
    let mut buf = vec![0u8; read_size];
    file.read_exact(&mut buf).await?;

    let is_last = offset + read_size as u64 >= file_size;

    let resp = AppResponse::Sync(SyncResponse::AssetChunk {
        doc_id,
        name: name.to_string(),
        chunk_index,
        data: buf,
        is_last,
    });
    client
        .send_response(pending_id, resp)
        .await
        .map_err(|e| AppError::SwarmIo {
            context: "send_response AssetChunk",
            reason: e.to_string(),
        })?;

    Ok(())
}

//! Per-workspace key management: generate the symmetric `read_key`/`write_key`
//! set, seal it to this device via an X25519 Lockbox, persist it, and reload
//! the full `{key_version → keys}` history on open.
//!
//! v1 only ever creates **self-Lockboxes** (sealed by this device, for this
//! device). Distributing keys to other devices (deriving a sender's X25519
//! public key from its PeerId) lands in the sharing phase. See
//! `dev-notes/design/{05-sharing,08-e2e-encryption}.md`.

use std::collections::BTreeMap;

use chrono::Utc;
use entity::workspace::workspace_key_lockboxes::{self, Entity as Lockboxes};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use uuid::Uuid;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::crypto::{open_lockbox, random_key, seal_lockbox, KEY_LEN};
use crate::error::{AppError, AppResult};

/// One key version's symmetric material. `write_key` is `None` for a future
/// read-only (Reader) device.
#[derive(Clone)]
pub struct WorkspaceKeySet {
    pub read_key: [u8; KEY_LEN],
    pub write_key: Option<[u8; KEY_LEN]>,
}

/// In-memory `{key_version → keys}` history. Old versions are retained so old
/// CRDT updates / ciphertexts remain decryptable (no forward secrecy — see
/// threat model).
#[derive(Default, Clone)]
pub struct WorkspaceKeys {
    versions: BTreeMap<u32, WorkspaceKeySet>,
}

impl WorkspaceKeys {
    /// Highest (current) key version, if any.
    pub fn current_version(&self) -> Option<u32> {
        self.versions.keys().next_back().copied()
    }

    /// The read key for `version`, if held.
    pub fn read_key(&self, version: u32) -> Option<&[u8; KEY_LEN]> {
        self.versions.get(&version).map(|s| &s.read_key)
    }

    /// The write key for `version`, if held (a Reader holds none).
    pub fn write_key(&self, version: u32) -> Option<&[u8; KEY_LEN]> {
        self.versions
            .get(&version)
            .and_then(|s| s.write_key.as_ref())
    }

    /// Current version + its key set.
    pub fn current(&self) -> Option<(u32, &WorkspaceKeySet)> {
        self.versions.iter().next_back().map(|(v, s)| (*v, s))
    }

    pub fn is_empty(&self) -> bool {
        self.versions.is_empty()
    }

    fn insert(&mut self, version: u32, set: WorkspaceKeySet) {
        self.versions.insert(version, set);
    }
}

/// Generate `key_version = 1` keys for a brand-new workspace, seal a
/// self-Lockbox to this device, persist it, and return the in-memory set.
pub async fn initialize_workspace_keys(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    my_peer_id: &str,
    my_secret: &StaticSecret,
    my_public: &PublicKey,
) -> AppResult<WorkspaceKeys> {
    let read = random_key();
    let write = random_key();
    let version: i32 = 1;

    let sealed_read = seal_lockbox(my_secret, my_public, &read)?;
    let sealed_write = seal_lockbox(my_secret, my_public, &write)?;

    workspace_key_lockboxes::ActiveModel {
        workspace_id: Set(workspace_id),
        key_version: Set(version),
        recipient_peer_id: Set(my_peer_id.to_string()),
        sealed_read_key: Set(sealed_read),
        sealed_write_key: Set(Some(sealed_write)),
        sealed_by_peer_id: Set(my_peer_id.to_string()),
        created_at: Set(Utc::now()),
    }
    .insert(db)
    .await?;

    let mut keys = WorkspaceKeys::default();
    keys.insert(
        version as u32,
        WorkspaceKeySet {
            read_key: read,
            write_key: Some(write),
        },
    );
    Ok(keys)
}

/// Load every key version sealed for this device from the workspace DB.
pub async fn load_workspace_keys(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    my_peer_id: &str,
    my_secret: &StaticSecret,
    my_public: &PublicKey,
) -> AppResult<WorkspaceKeys> {
    let rows = Lockboxes::find()
        .filter(workspace_key_lockboxes::Column::WorkspaceId.eq(workspace_id))
        .filter(workspace_key_lockboxes::Column::RecipientPeerId.eq(my_peer_id))
        .all(db)
        .await?;

    let mut keys = WorkspaceKeys::default();
    for row in rows {
        // v1: only self-sealed Lockboxes exist. Opening a Lockbox sealed by
        // another device (sender X25519 public derived from its PeerId) is
        // wired in the sharing phase.
        if row.sealed_by_peer_id != my_peer_id {
            tracing::warn!(
                workspace_id = %workspace_id,
                sealed_by = %row.sealed_by_peer_id,
                "skipping non-self Lockbox (peer-key derivation lands in sharing phase)"
            );
            continue;
        }
        let read = to_key(
            open_lockbox(my_secret, my_public, &row.sealed_read_key)?,
            "read_key",
        )?;
        let write = match row.sealed_write_key {
            Some(ref sealed) => Some(to_key(
                open_lockbox(my_secret, my_public, sealed)?,
                "write_key",
            )?),
            None => None,
        };
        keys.insert(
            row.key_version as u32,
            WorkspaceKeySet {
                read_key: read,
                write_key: write,
            },
        );
    }
    Ok(keys)
}

fn to_key(bytes: Vec<u8>, what: &'static str) -> AppResult<[u8; KEY_LEN]> {
    bytes.try_into().map_err(|_| AppError::Crypto {
        context: "workspace-keys",
        reason: format!("{what} has wrong length"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::keyx::derive_x25519_secret;
    use entity::workspace::workspaces;
    use migration::{MigratorTrait, WorkspaceMigrator};
    use sea_orm::Database;

    async fn mem_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        WorkspaceMigrator::up(&db, None).await.unwrap();
        db
    }

    fn device(seed: u8) -> (String, StaticSecret, PublicKey) {
        let sec = derive_x25519_secret(&[seed; 32]);
        let pubk = PublicKey::from(&sec);
        (format!("12D3KooW-test-{seed}"), sec, pubk)
    }

    async fn insert_workspace(db: &DatabaseConnection, id: Uuid) {
        // Insert via the entity so the Uuid PK serializes exactly as the
        // Lockbox FK does (avoids a raw-SQL vs sea-orm encoding mismatch).
        workspaces::ActiveModel {
            id: Set(id),
            name: Set("WS".to_string()),
            created_by: Set("me".to_string()),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
        }
        .insert(db)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn generate_persist_reload_round_trip() {
        let db = mem_db().await;
        let ws = Uuid::now_v7();
        insert_workspace(&db, ws).await;
        let (peer, sec, pubk) = device(1);

        let created = initialize_workspace_keys(&db, ws, &peer, &sec, &pubk)
            .await
            .unwrap();
        assert_eq!(created.current_version(), Some(1));
        assert!(created.write_key(1).is_some());

        // Simulate restart: reload purely from the DB.
        let loaded = load_workspace_keys(&db, ws, &peer, &sec, &pubk)
            .await
            .unwrap();
        assert_eq!(loaded.current_version(), Some(1));
        assert_eq!(loaded.read_key(1), created.read_key(1));
        assert_eq!(loaded.write_key(1), created.write_key(1));
    }

    #[tokio::test]
    async fn other_device_has_no_keys() {
        let db = mem_db().await;
        let ws = Uuid::now_v7();
        insert_workspace(&db, ws).await;
        let (peer_a, sec_a, pub_a) = device(1);
        initialize_workspace_keys(&db, ws, &peer_a, &sec_a, &pub_a)
            .await
            .unwrap();

        // Device B has no Lockbox sealed for it → no keys (until shared).
        let (peer_b, sec_b, pub_b) = device(2);
        let loaded_b = load_workspace_keys(&db, ws, &peer_b, &sec_b, &pub_b)
            .await
            .unwrap();
        assert!(loaded_b.is_empty());
    }
}

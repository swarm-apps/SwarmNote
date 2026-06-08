//! Per-workspace key management: generate the symmetric `read_key`/`write_key`
//! set, seal it to this device via an X25519 Lockbox, persist it, and reload
//! the full `{key_version → keys}` history on open.
//!
//! v1 only ever creates **self-Lockboxes** (sealed by this device, for this
//! device). Distributing keys to other devices (deriving a sender's X25519
//! public key from its PeerId) lands in the sharing phase. See
//! `dev-notes/design/{05-sharing,08-e2e-encryption}.md`.

use std::collections::BTreeMap;
use std::str::FromStr;

use chrono::Utc;
use entity::workspace::workspace_key_lockboxes::{self, Entity as Lockboxes};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use swarm_p2p_core::libp2p::PeerId;
use uuid::Uuid;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::crypto::{open_lockbox, random_key, seal_lockbox, KEY_LEN};
use crate::error::{AppError, AppResult};
use crate::identity::peer_id_to_x25519_public;

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

    /// Test-only constructor for a single key version.
    #[cfg(test)]
    pub(crate) fn test_single(
        version: u32,
        read_key: [u8; KEY_LEN],
        write_key: Option<[u8; KEY_LEN]>,
    ) -> Self {
        let mut keys = Self::default();
        keys.insert(
            version,
            WorkspaceKeySet {
                read_key,
                write_key,
            },
        );
        keys
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

/// Load this device's keys, generating `key_version = 1` (self-Lockbox) if the
/// workspace has none yet. Idempotent + race-safe: if a concurrent open wins the
/// init, this reloads the persisted keys instead of returning divergent ones.
///
/// NOTE: a workspace synced/joined from another device should receive its keys
/// via a shared Lockbox (sharing phase) rather than self-initializing — until
/// then a joined workspace self-inits its own (distinct) key, which only matters
/// once encrypted broadcast is switched on.
/// Returns `(keys, did_initialize)` where `did_initialize` is `true` only when
/// this call freshly self-initialized the workspace key — i.e. the owner-create
/// moment, used to seed the genesis permission op exactly once.
pub async fn load_or_initialize_workspace_keys(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    my_peer_id: &str,
    my_secret: &StaticSecret,
    my_public: &PublicKey,
) -> AppResult<(WorkspaceKeys, bool)> {
    let existing = load_workspace_keys(db, workspace_id, my_peer_id, my_secret, my_public).await?;
    if !existing.is_empty() {
        return Ok((existing, false));
    }
    match initialize_workspace_keys(db, workspace_id, my_peer_id, my_secret, my_public).await {
        Ok(keys) => Ok((keys, true)),
        // Lost an init race (PK conflict): use whatever was persisted.
        Err(_) => Ok((
            load_workspace_keys(db, workspace_id, my_peer_id, my_secret, my_public).await?,
            false,
        )),
    }
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
        // The sender's X25519 public key: our own for a self-Lockbox, otherwise
        // derived from the sealer's PeerId (ed25519 → X25519). Skip rows whose
        // sealer PeerId can't be parsed/derived rather than failing the load.
        let sender_public = if row.sealed_by_peer_id == my_peer_id {
            *my_public
        } else {
            match PeerId::from_str(&row.sealed_by_peer_id)
                .ok()
                .and_then(|pid| peer_id_to_x25519_public(&pid).ok())
            {
                Some(pk) => pk,
                None => {
                    tracing::warn!(
                        workspace_id = %workspace_id,
                        sealed_by = %row.sealed_by_peer_id,
                        "skipping Lockbox: cannot derive sealer X25519 public key"
                    );
                    continue;
                }
            }
        };
        let read = to_key(
            open_lockbox(my_secret, &sender_public, &row.sealed_read_key)?,
            "read_key",
        )?;
        let write = match row.sealed_write_key {
            Some(ref sealed) => Some(to_key(
                open_lockbox(my_secret, &sender_public, sealed)?,
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

/// Seal this device's current workspace keys to a `recipient` X25519 public key
/// (pure crypto, no DB). Returns `(key_version, sealed_read, sealed_write)`.
/// A read-only (Reader) recipient is given only the read key.
pub fn seal_keys_for_recipient(
    my_secret: &StaticSecret,
    recipient_public: &PublicKey,
    keys: &WorkspaceKeys,
    include_write_key: bool,
) -> AppResult<(u32, Vec<u8>, Option<Vec<u8>>)> {
    let (version, set) = keys.current().ok_or(AppError::Crypto {
        context: "seal-keys",
        reason: "workspace has no key to share".into(),
    })?;
    let sealed_read = seal_lockbox(my_secret, recipient_public, &set.read_key)?;
    let sealed_write = match (include_write_key, set.write_key) {
        (true, Some(write)) => Some(seal_lockbox(my_secret, recipient_public, &write)?),
        _ => None,
    };
    Ok((version, sealed_read, sealed_write))
}

/// Seal this device's current workspace keys to a paired `recipient` device
/// (X25519 public derived from its PeerId) and persist the Lockbox locally.
/// A read-only (Reader) recipient is given only the read key.
pub async fn share_workspace_keys_to_device(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    my_peer_id: &str,
    my_secret: &StaticSecret,
    recipient_peer_id: &str,
    keys: &WorkspaceKeys,
    include_write_key: bool,
) -> AppResult<()> {
    let recipient_pid = PeerId::from_str(recipient_peer_id).map_err(|e| AppError::Crypto {
        context: "share-keys",
        reason: format!("bad recipient peer id: {e}"),
    })?;
    let recipient_public = peer_id_to_x25519_public(&recipient_pid)?;
    let (version, sealed_read, sealed_write) =
        seal_keys_for_recipient(my_secret, &recipient_public, keys, include_write_key)?;

    install_received_key(
        db,
        workspace_id,
        recipient_peer_id,
        my_peer_id,
        version,
        sealed_read,
        sealed_write,
    )
    .await
}

/// Persist a received (or self-sealed) Lockbox row for `recipient_peer_id`.
/// Idempotent: a row already present for `(workspace, version, recipient)` is
/// left untouched.
pub async fn install_received_key(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    recipient_peer_id: &str,
    sealed_by_peer_id: &str,
    key_version: u32,
    sealed_read: Vec<u8>,
    sealed_write: Option<Vec<u8>>,
) -> AppResult<()> {
    let pk = (
        workspace_id,
        key_version as i32,
        recipient_peer_id.to_string(),
    );
    if Lockboxes::find_by_id(pk).one(db).await?.is_some() {
        return Ok(());
    }
    workspace_key_lockboxes::ActiveModel {
        workspace_id: Set(workspace_id),
        key_version: Set(key_version as i32),
        recipient_peer_id: Set(recipient_peer_id.to_string()),
        sealed_read_key: Set(sealed_read),
        sealed_write_key: Set(sealed_write),
        sealed_by_peer_id: Set(sealed_by_peer_id.to_string()),
        created_at: Set(Utc::now()),
    }
    .insert(db)
    .await?;
    Ok(())
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

    /// A real device with a genuine ed25519 PeerId (so `sealed_by` parses and
    /// `peer_id_to_x25519_public` can derive the sender key cross-device).
    fn real_device() -> (String, StaticSecret, PublicKey) {
        use swarm_p2p_core::libp2p::identity::Keypair;
        let kp = Keypair::generate_ed25519();
        let peer_id = kp.public().to_peer_id().to_string();
        let ed = kp.try_into_ed25519().unwrap();
        let seed: [u8; 32] = ed.to_bytes()[..32].try_into().unwrap();
        let sec = derive_x25519_secret(&seed);
        let pubk = PublicKey::from(&sec);
        (peer_id, sec, pubk)
    }

    #[tokio::test]
    async fn share_to_paired_device_round_trip() {
        let db = mem_db().await;
        let ws = Uuid::now_v7();
        insert_workspace(&db, ws).await;

        let (peer_a, sec_a, pub_a) = real_device();
        let (peer_b, sec_b, pub_b) = real_device();

        // A initializes its own keys (self-Lockbox).
        let a_keys = initialize_workspace_keys(&db, ws, &peer_a, &sec_a, &pub_a)
            .await
            .unwrap();

        // A shares its current keys to paired device B (sealed to B's PeerId).
        share_workspace_keys_to_device(&db, ws, &peer_a, &sec_a, &peer_b, &a_keys, true)
            .await
            .unwrap();

        // B loads → opens A's Lockbox (sender X25519 derived from A's PeerId) and
        // recovers the SAME read/write keys.
        let b_keys = load_workspace_keys(&db, ws, &peer_b, &sec_b, &pub_b)
            .await
            .unwrap();
        assert_eq!(b_keys.read_key(1), a_keys.read_key(1));
        assert_eq!(b_keys.write_key(1), a_keys.write_key(1));
    }

    /// End-to-end proof of the encrypted-sync core (everything except the
    /// libp2p transport itself): A initializes a key, distributes it to B via a
    /// peer-sealed Lockbox, B installs + loads it, A encrypts a gossip payload,
    /// **B decrypts it**, and an unauthorized device C (no key) cannot.
    #[tokio::test]
    async fn encrypted_sync_end_to_end_after_key_share() {
        use crate::workspace::sync::{
            decode_encrypted_gossip, encode_encrypted_gossip, MSG_TYPE_DOC,
        };

        // Device A owns the workspace; B is a paired joiner; C is unauthorized.
        let db_a = mem_db().await;
        let db_b = mem_db().await;
        let ws = Uuid::now_v7();
        insert_workspace(&db_a, ws).await;
        insert_workspace(&db_b, ws).await;

        let (peer_a, sec_a, pub_a) = real_device();
        let (peer_b, sec_b, pub_b) = real_device();
        let (_peer_c, sec_c, pub_c) = real_device();

        // 1. A initializes its workspace key (v1, self-Lockbox).
        let a_keys = initialize_workspace_keys(&db_a, ws, &peer_a, &sec_a, &pub_a)
            .await
            .unwrap();

        // 2. A seals its key to B's PeerId; B installs it into its own DB and loads.
        let b_pub = peer_id_to_x25519_public(&PeerId::from_str(&peer_b).unwrap()).unwrap();
        let (ver, sealed_read, sealed_write) =
            seal_keys_for_recipient(&sec_a, &b_pub, &a_keys, true).unwrap();
        install_received_key(&db_b, ws, &peer_b, &peer_a, ver, sealed_read, sealed_write)
            .await
            .unwrap();
        let b_keys = load_workspace_keys(&db_b, ws, &peer_b, &sec_b, &pub_b)
            .await
            .unwrap();
        assert_eq!(
            b_keys.read_key(1),
            a_keys.read_key(1),
            "B must hold A's key"
        );

        // 3. A encrypts a gossip doc-update; B decrypts it back to plaintext.
        let doc = Uuid::now_v7();
        let plaintext = b"yjs-update-\xf0\x9f\x90\x9d"; // arbitrary bytes incl. emoji
        let wire = encode_encrypted_gossip(&a_keys, &ws, &doc, MSG_TYPE_DOC, plaintext).unwrap();
        let (got_doc, got) = decode_encrypted_gossip(&b_keys, &ws, MSG_TYPE_DOC, &wire).unwrap();
        assert_eq!(got_doc, doc);
        assert_eq!(got, plaintext, "B must decrypt A's broadcast");

        // 4. Unauthorized device C (never received the key) cannot decrypt.
        let c_keys = load_workspace_keys(&mem_db().await, ws, "c", &sec_c, &pub_c)
            .await
            .unwrap();
        assert!(c_keys.is_empty());
        assert!(
            decode_encrypted_gossip(&c_keys, &ws, MSG_TYPE_DOC, &wire).is_err(),
            "device without the key must not decrypt"
        );
    }
}

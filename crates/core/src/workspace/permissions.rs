//! Signed, append-only permission operation chain (authorization DAG).
//!
//! Each [`PermissionOp`] is identified by its content hash (`op_id`) and signed
//! by the issuer device's Ed25519 key. Replaying the ops ([`materialize`])
//! yields the current `peer → role` map, validating these invariants per op:
//! signature valid, issuer currently Owner (non-genesis), (two-tier) only Owner
//! may grant/revoke, and the owner is never demoted. The genesis op (no
//! `prev_hash`) is a self-grant of Owner that bootstraps the workspace owner —
//! but it is **bound to the workspace's authoritative creator** (`created_by`):
//! only that peer's genesis is honored, so a forged self-signed genesis from any
//! other device cannot establish a second Owner. See
//! `dev-notes/design/04-permissions.md`.
//!
//! v1 is two-tier (Owner / Collaborator). The op carries everything needed to
//! add a real Reader role later without a schema change.

use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use chrono::Utc;
use entity::workspace::permission_ops::{self, Entity as PermissionOps};
use entity::workspace::workspaces::Entity as Workspaces;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use serde::{Deserialize, Serialize};
use swarm_p2p_core::libp2p::PeerId;
use uuid::Uuid;

use crate::error::AppResult;
use crate::identity::{verify_peer_signature, IdentityManager};

/// Workspace role. v1 two-tier; `Reader` is reserved for v2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Owner,
    Collaborator,
}

impl Role {
    fn as_str(self) -> &'static str {
        match self {
            Role::Owner => "owner",
            Role::Collaborator => "collaborator",
        }
    }
    fn parse(s: &str) -> Option<Role> {
        match s {
            "owner" => Some(Role::Owner),
            "collaborator" => Some(Role::Collaborator),
            _ => None,
        }
    }
}

/// Permission operation kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OpKind {
    Grant,
    Revoke,
}

impl OpKind {
    fn as_str(self) -> &'static str {
        match self {
            OpKind::Grant => "grant",
            OpKind::Revoke => "revoke",
        }
    }
    fn parse(s: &str) -> Option<OpKind> {
        match s {
            "grant" => Some(OpKind::Grant),
            "revoke" => Some(OpKind::Revoke),
            _ => None,
        }
    }
    fn tag(self) -> u8 {
        match self {
            OpKind::Grant => 1,
            OpKind::Revoke => 2,
        }
    }
}

/// One node in the signed permission chain. Serializable for broadcast over
/// the ctrl GossipSub topic (`CtrlMessage::PermissionOpsUpdate`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PermissionOp {
    pub op_id: String,
    pub op_kind: OpKind,
    pub target_peer_id: String,
    /// `Some` for `Grant`, `None` for `Revoke`.
    pub new_role: Option<Role>,
    pub issuer_peer_id: String,
    pub key_version: i32,
    pub prev_hash: Option<String>,
    pub signature: Vec<u8>,
}

/// Deterministic byte encoding signed by the issuer + hashed into `op_id`.
fn canonical_bytes(
    kind: OpKind,
    target: &str,
    role: Option<Role>,
    issuer: &str,
    key_version: i32,
    prev_hash: Option<&str>,
) -> Vec<u8> {
    let mut b = Vec::new();
    b.push(kind.tag());
    b.extend_from_slice(target.as_bytes());
    b.push(0);
    b.extend_from_slice(role.map(|r| r.as_str()).unwrap_or("none").as_bytes());
    b.push(0);
    b.extend_from_slice(issuer.as_bytes());
    b.push(0);
    b.extend_from_slice(&key_version.to_be_bytes());
    b.push(0);
    if let Some(ph) = prev_hash {
        b.extend_from_slice(ph.as_bytes());
    }
    b
}

impl PermissionOp {
    fn canonical(&self) -> Vec<u8> {
        canonical_bytes(
            self.op_kind,
            &self.target_peer_id,
            self.new_role,
            &self.issuer_peer_id,
            self.key_version,
            self.prev_hash.as_deref(),
        )
    }

    /// Verify the content hash (`op_id`) and the issuer's Ed25519 signature.
    pub fn verify(&self) -> bool {
        let canon = self.canonical();
        if blake3::hash(&canon).to_hex().to_string() != self.op_id {
            return false;
        }
        match PeerId::from_str(&self.issuer_peer_id) {
            Ok(pid) => verify_peer_signature(&pid, &canon, &self.signature),
            Err(_) => false,
        }
    }
}

/// Build + sign a permission op with this device as issuer.
pub fn build_signed_op(
    identity: &IdentityManager,
    op_kind: OpKind,
    target_peer_id: &str,
    new_role: Option<Role>,
    key_version: i32,
    prev_hash: Option<String>,
) -> AppResult<PermissionOp> {
    let issuer = identity.peer_id()?;
    let canon = canonical_bytes(
        op_kind,
        target_peer_id,
        new_role,
        &issuer,
        key_version,
        prev_hash.as_deref(),
    );
    let op_id = blake3::hash(&canon).to_hex().to_string();
    let signature = identity.sign(&canon)?;
    Ok(PermissionOp {
        op_id,
        op_kind,
        target_peer_id: target_peer_id.to_string(),
        new_role,
        issuer_peer_id: issuer,
        key_version,
        prev_hash,
        signature,
    })
}

/// Persist an op (idempotent on `op_id`).
pub async fn save_op(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    op: &PermissionOp,
) -> AppResult<()> {
    if PermissionOps::find_by_id(op.op_id.clone())
        .one(db)
        .await?
        .is_some()
    {
        return Ok(());
    }
    permission_ops::ActiveModel {
        op_id: Set(op.op_id.clone()),
        workspace_id: Set(workspace_id),
        op_kind: Set(op.op_kind.as_str().to_string()),
        target_peer_id: Set(op.target_peer_id.clone()),
        new_role: Set(op
            .new_role
            .map(|r| r.as_str())
            .unwrap_or("none")
            .to_string()),
        issuer_peer_id: Set(op.issuer_peer_id.clone()),
        key_version: Set(op.key_version),
        prev_hash: Set(op.prev_hash.clone()),
        signature: Set(op.signature.clone()),
        created_at: Set(Utc::now()),
    }
    .insert(db)
    .await?;
    Ok(())
}

/// Load all permission ops for a workspace.
pub async fn load_ops(db: &DatabaseConnection, workspace_id: Uuid) -> AppResult<Vec<PermissionOp>> {
    let rows = PermissionOps::find()
        .filter(permission_ops::Column::WorkspaceId.eq(workspace_id))
        .all(db)
        .await?;
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            Some(PermissionOp {
                op_id: r.op_id,
                op_kind: OpKind::parse(&r.op_kind)?,
                target_peer_id: r.target_peer_id,
                new_role: Role::parse(&r.new_role),
                issuer_peer_id: r.issuer_peer_id,
                key_version: r.key_version,
                prev_hash: r.prev_hash,
                signature: r.signature,
            })
        })
        .collect())
}

/// Load + replay this workspace's permission ops into a `peer → role` map.
///
/// The workspace row's `created_by` is the **authoritative owner** and is
/// passed to [`materialize`] as the genesis trust anchor — only that peer's
/// self-grant-Owner genesis is honored, so a forged genesis from any other
/// device cannot bootstrap a second Owner.
pub async fn load_and_materialize(
    db: &DatabaseConnection,
    workspace_id: Uuid,
) -> AppResult<HashMap<String, Role>> {
    let expected_owner = workspace_owner(db, workspace_id).await?;
    Ok(materialize(
        &load_ops(db, workspace_id).await?,
        &expected_owner,
    ))
}

/// The workspace's authoritative owner — the `created_by` peer id on the
/// workspace row. This binds the genesis op (see [`materialize`]). Returns an
/// empty string if the row is missing, which **fails closed**: no genesis is
/// accepted and the workspace materializes to an empty role map.
async fn workspace_owner(db: &DatabaseConnection, workspace_id: Uuid) -> AppResult<String> {
    Ok(Workspaces::find_by_id(workspace_id)
        .one(db)
        .await?
        .map(|w| w.created_by)
        .unwrap_or_default())
}

/// The sole authoritative-owner identity declared by a permission chain — the
/// issuer of its unique self-grant-Owner genesis op. A joiner uses this to pin
/// its local `created_by` to the real owner learned from the (Noise-
/// authenticated) chain it received, so its own genesis binding matches the
/// owner's. Returns `None` if there is zero or more than one such genesis
/// (ambiguous → caller should not pin).
pub fn genesis_owner(ops: &[PermissionOp]) -> Option<String> {
    let mut found: Option<String> = None;
    for op in ops.iter().filter(|o| o.verify()) {
        if op.prev_hash.is_none()
            && op.op_kind == OpKind::Grant
            && op.new_role == Some(Role::Owner)
            && op.issuer_peer_id == op.target_peer_id
        {
            if found.is_some() {
                return None; // ambiguous — multiple genesis claims
            }
            found = Some(op.issuer_peer_id.clone());
        }
    }
    found
}

/// This peer's current role in the workspace (if any).
pub async fn role_of(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    peer_id: &str,
) -> AppResult<Option<Role>> {
    Ok(load_and_materialize(db, workspace_id)
        .await?
        .get(peer_id)
        .copied())
}

/// The current tip (leaf) of the op chain — the op_id no other op lists as its
/// `prev_hash`. For a linear chain this is the latest op; used as `prev_hash`
/// for a newly-issued op. Deterministic (smallest op_id) on a fork.
pub fn chain_tip(ops: &[PermissionOp]) -> Option<String> {
    let prevs: HashSet<&str> = ops.iter().filter_map(|o| o.prev_hash.as_deref()).collect();
    ops.iter()
        .filter(|o| !prevs.contains(o.op_id.as_str()))
        .map(|o| o.op_id.clone())
        .min()
}

/// Seed the genesis Owner op for a freshly-created (owner) workspace if the
/// permission chain is empty. Idempotent. Called only on the owner's create
/// path (where this device self-initialized the workspace key).
pub async fn ensure_genesis_owner(
    db: &DatabaseConnection,
    identity: &IdentityManager,
    workspace_id: Uuid,
) -> AppResult<()> {
    if !load_ops(db, workspace_id).await?.is_empty() {
        return Ok(());
    }
    let me = identity.peer_id()?;
    let op = build_signed_op(identity, OpKind::Grant, &me, Some(Role::Owner), 1, None)?;
    save_op(db, workspace_id, &op).await?;
    tracing::info!(workspace_id = %workspace_id, "seeded genesis owner permission op");
    Ok(())
}

/// Replay ops into a `peer → role` map, dropping any op that fails the
/// invariants: valid signature, **genesis bound to the authoritative owner**,
/// issuer currently Owner (non-genesis), only Owner grants/revokes, and the
/// owner is never demoted/revoked. Deterministic across devices.
///
/// `expected_owner` is the workspace's authoritative creator (`created_by`).
/// A genesis op (no `prev_hash`) bootstraps Owner **only** when its
/// issuer == target == `expected_owner`. This is the trust anchor that closes
/// the second-genesis-forgery hole: any other device self-signing a genesis
/// (`issuer == target` but `!= expected_owner`) is rejected, so a paired-but-
/// unauthorized peer cannot inject a forged root, materialize itself as Owner,
/// and trick a key holder into sealing the workspace key to it.
pub fn materialize(ops: &[PermissionOp], expected_owner: &str) -> HashMap<String, Role> {
    let valid: Vec<&PermissionOp> = ops.iter().filter(|o| o.verify()).collect();
    let ordered = order_by_chain(&valid);

    let mut roles: HashMap<String, Role> = HashMap::new();
    for op in ordered {
        if op.prev_hash.is_none() {
            // Genesis bootstraps the owner: a self-grant of Owner — but only for
            // the workspace's authoritative creator. Any other self-signed
            // genesis is a forgery and is dropped.
            if op.op_kind == OpKind::Grant
                && op.new_role == Some(Role::Owner)
                && op.issuer_peer_id == op.target_peer_id
                && op.issuer_peer_id == expected_owner
            {
                roles.insert(op.target_peer_id.clone(), Role::Owner);
            }
            continue;
        }
        // Non-genesis: the issuer must currently be Owner.
        if roles.get(&op.issuer_peer_id).copied() != Some(Role::Owner) {
            continue;
        }
        // The authoritative owner can never be revoked or downgraded — protects
        // the founder and guarantees at least one Owner always survives. Without
        // this, a (forged or peer) Owner could revoke the real owner and seize
        // the chain.
        if op.target_peer_id == expected_owner {
            continue;
        }
        match op.op_kind {
            OpKind::Grant => {
                if let Some(role) = op.new_role {
                    roles.insert(op.target_peer_id.clone(), role);
                }
            }
            OpKind::Revoke => {
                roles.remove(&op.target_peer_id);
            }
        }
    }
    roles
}

/// Order ops by their `prev_hash` chain (BFS by causal level, deterministic by
/// `op_id` within a level). Orphan ops (predecessor missing) are dropped.
fn order_by_chain<'a>(ops: &[&'a PermissionOp]) -> Vec<&'a PermissionOp> {
    let mut applied: HashSet<String> = HashSet::new();
    let mut result: Vec<&PermissionOp> = Vec::with_capacity(ops.len());
    loop {
        let mut ready: Vec<&PermissionOp> = ops
            .iter()
            .copied()
            .filter(|o| {
                !applied.contains(&o.op_id)
                    && o.prev_hash
                        .as_ref()
                        .map(|p| applied.contains(p))
                        .unwrap_or(true)
            })
            .collect();
        if ready.is_empty() {
            break;
        }
        ready.sort_by(|a, b| a.op_id.cmp(&b.op_id));
        for op in ready {
            applied.insert(op.op_id.clone());
            result.push(op);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use swarm_p2p_core::libp2p::identity::Keypair;

    fn new_kp() -> Keypair {
        Keypair::generate_ed25519()
    }
    fn peer_of(kp: &Keypair) -> String {
        kp.public().to_peer_id().to_string()
    }
    fn signed(
        kp: &Keypair,
        kind: OpKind,
        target: &str,
        role: Option<Role>,
        kv: i32,
        prev: Option<String>,
    ) -> PermissionOp {
        let issuer = peer_of(kp);
        let canon = canonical_bytes(kind, target, role, &issuer, kv, prev.as_deref());
        let op_id = blake3::hash(&canon).to_hex().to_string();
        let signature = kp.sign(&canon).unwrap();
        PermissionOp {
            op_id,
            op_kind: kind,
            target_peer_id: target.to_string(),
            new_role: role,
            issuer_peer_id: issuer,
            key_version: kv,
            prev_hash: prev,
            signature,
        }
    }

    #[test]
    fn genesis_establishes_owner() {
        let owner = new_kp();
        let o = peer_of(&owner);
        let g = signed(&owner, OpKind::Grant, &o, Some(Role::Owner), 1, None);
        let roles = materialize(&[g], &o);
        assert_eq!(roles.get(&o), Some(&Role::Owner));
    }

    #[test]
    fn owner_grants_then_revokes_collaborator() {
        let owner = new_kp();
        let bob = new_kp();
        let (o, b) = (peer_of(&owner), peer_of(&bob));
        let g = signed(&owner, OpKind::Grant, &o, Some(Role::Owner), 1, None);
        let add = signed(
            &owner,
            OpKind::Grant,
            &b,
            Some(Role::Collaborator),
            1,
            Some(g.op_id.clone()),
        );
        let roles = materialize(&[g.clone(), add.clone()], &o);
        assert_eq!(roles.get(&b), Some(&Role::Collaborator));

        let rev = signed(&owner, OpKind::Revoke, &b, None, 2, Some(add.op_id.clone()));
        let roles = materialize(&[g, add, rev], &o);
        assert_eq!(roles.get(&b), None);
    }

    #[test]
    fn forged_self_escalation_rejected() {
        let owner = new_kp();
        let bob = new_kp();
        let (o, b) = (peer_of(&owner), peer_of(&bob));
        let g = signed(&owner, OpKind::Grant, &o, Some(Role::Owner), 1, None);
        let add = signed(
            &owner,
            OpKind::Grant,
            &b,
            Some(Role::Collaborator),
            1,
            Some(g.op_id.clone()),
        );
        // Bob validly signs an op making himself Owner — but Bob is not Owner.
        let forged = signed(
            &bob,
            OpKind::Grant,
            &b,
            Some(Role::Owner),
            1,
            Some(add.op_id.clone()),
        );
        let roles = materialize(&[g, add, forged], &o);
        assert_eq!(
            roles.get(&b),
            Some(&Role::Collaborator),
            "escalation must be rejected"
        );
    }

    #[test]
    fn tampered_signature_dropped() {
        let owner = new_kp();
        let o = peer_of(&owner);
        let mut g = signed(&owner, OpKind::Grant, &o, Some(Role::Owner), 1, None);
        g.signature[0] ^= 0xff; // tamper
        assert!(!g.verify());
        assert!(materialize(&[g], &o).is_empty());
    }

    #[test]
    fn forged_genesis_cannot_self_bootstrap_owner() {
        // The authoritative owner's genesis + an attacker's own self-signed
        // genesis. The attacker's signature/hash are internally valid, so
        // `verify()` passes — but it is not the workspace's `created_by`, so
        // `materialize` must drop it. This is the second-genesis-forgery hole.
        let owner = new_kp();
        let attacker = new_kp();
        let (o, a) = (peer_of(&owner), peer_of(&attacker));
        let g = signed(&owner, OpKind::Grant, &o, Some(Role::Owner), 1, None);
        let forged_genesis = signed(&attacker, OpKind::Grant, &a, Some(Role::Owner), 1, None);
        assert!(
            forged_genesis.verify(),
            "forged genesis is internally valid"
        );

        let roles = materialize(&[g, forged_genesis], &o);
        assert_eq!(roles.get(&o), Some(&Role::Owner), "real owner stands");
        assert_eq!(
            roles.get(&a),
            None,
            "forged self-grant genesis must NOT establish a second Owner"
        );
    }

    #[test]
    fn lone_forged_genesis_yields_no_owner() {
        // An attacker-only chain (no real owner present) still materializes to
        // empty — there is no `expected_owner` match. Fails closed.
        let attacker = new_kp();
        let owner = new_kp();
        let (a, o) = (peer_of(&attacker), peer_of(&owner));
        let forged = signed(&attacker, OpKind::Grant, &a, Some(Role::Owner), 1, None);
        let roles = materialize(&[forged], &o);
        assert!(
            roles.is_empty(),
            "no genesis matches the authoritative owner"
        );
    }

    #[test]
    fn owner_cannot_be_revoked_or_demoted() {
        // Even a validly-signed op (here issued by a second Owner) targeting the
        // authoritative owner must not remove/demote it — the founder is
        // protected and at least one Owner always survives.
        let owner = new_kp();
        let coowner = new_kp();
        let (o, c) = (peer_of(&owner), peer_of(&coowner));
        let g = signed(&owner, OpKind::Grant, &o, Some(Role::Owner), 1, None);
        // Owner promotes coowner to Owner (no legitimate v1 API does this, but
        // the DAG must stay sound even if such an op is constructed/injected).
        let promote = signed(
            &owner,
            OpKind::Grant,
            &c,
            Some(Role::Owner),
            1,
            Some(g.op_id.clone()),
        );
        // coowner (a current Owner) tries to revoke the founding owner.
        let revoke_owner = signed(
            &coowner,
            OpKind::Revoke,
            &o,
            None,
            1,
            Some(promote.op_id.clone()),
        );
        let roles = materialize(&[g, promote, revoke_owner], &o);
        assert_eq!(
            roles.get(&o),
            Some(&Role::Owner),
            "authoritative owner cannot be revoked"
        );
    }

    #[test]
    fn genesis_owner_extracts_unique_or_none() {
        let owner = new_kp();
        let attacker = new_kp();
        let bob = new_kp();
        let (o, a, b) = (peer_of(&owner), peer_of(&attacker), peer_of(&bob));
        let g = signed(&owner, OpKind::Grant, &o, Some(Role::Owner), 1, None);
        let add = signed(
            &owner,
            OpKind::Grant,
            &b,
            Some(Role::Collaborator),
            1,
            Some(g.op_id.clone()),
        );
        // Unique genesis → its issuer.
        assert_eq!(genesis_owner(&[g.clone(), add.clone()]), Some(o.clone()));
        // Two competing genesis claims → ambiguous → None (caller won't pin).
        let forged_genesis = signed(&attacker, OpKind::Grant, &a, Some(Role::Owner), 1, None);
        assert_eq!(genesis_owner(&[g, forged_genesis, add]), None);
    }

    #[tokio::test]
    async fn genesis_seeding_idempotent_and_db_round_trip() {
        use crate::identity::IdentityManager;
        use entity::workspace::workspaces;
        use migration::{MigratorTrait, WorkspaceMigrator};
        use sea_orm::Database;

        let db = Database::connect("sqlite::memory:").await.unwrap();
        WorkspaceMigrator::up(&db, None).await.unwrap();

        let identity = IdentityManager::for_tests().await;
        let me = identity.peer_id().unwrap();

        // The workspace's authoritative owner (`created_by`) is this device —
        // it binds the genesis op that `ensure_genesis_owner` seeds.
        let ws = Uuid::now_v7();
        workspaces::ActiveModel {
            id: Set(ws),
            name: Set("WS".to_string()),
            created_by: Set(me.clone()),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
        }
        .insert(&db)
        .await
        .unwrap();

        ensure_genesis_owner(&db, &identity, ws).await.unwrap();
        ensure_genesis_owner(&db, &identity, ws).await.unwrap(); // idempotent

        let ops = load_ops(&db, ws).await.unwrap();
        assert_eq!(ops.len(), 1, "genesis seeded exactly once");
        assert!(
            ops[0].verify(),
            "persisted genesis op signature round-trips"
        );

        let roles = load_and_materialize(&db, ws).await.unwrap();
        assert_eq!(roles.get(&me), Some(&Role::Owner), "creator is Owner");
    }

    #[tokio::test]
    async fn forged_genesis_rejected_against_db_created_by() {
        // End-to-end at the DB layer: a workspace owned by `owner`; an attacker
        // injects (persists) its own self-signed genesis op. `load_and_materialize`
        // reads `created_by` from the row and must reject the forged genesis.
        use entity::workspace::workspaces;
        use migration::{MigratorTrait, WorkspaceMigrator};
        use sea_orm::Database;

        let db = Database::connect("sqlite::memory:").await.unwrap();
        WorkspaceMigrator::up(&db, None).await.unwrap();

        let owner = new_kp();
        let attacker = new_kp();
        let (o, a) = (peer_of(&owner), peer_of(&attacker));

        let ws = Uuid::now_v7();
        workspaces::ActiveModel {
            id: Set(ws),
            name: Set("WS".to_string()),
            created_by: Set(o.clone()),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
        }
        .insert(&db)
        .await
        .unwrap();

        let g = signed(&owner, OpKind::Grant, &o, Some(Role::Owner), 1, None);
        let forged = signed(&attacker, OpKind::Grant, &a, Some(Role::Owner), 1, None);
        save_op(&db, ws, &g).await.unwrap();
        save_op(&db, ws, &forged).await.unwrap(); // attacker manages to persist it

        let roles = load_and_materialize(&db, ws).await.unwrap();
        assert_eq!(roles.get(&o), Some(&Role::Owner));
        assert_eq!(
            roles.get(&a),
            None,
            "forged genesis persisted in DB still rejected by created_by binding"
        );
        assert_eq!(
            role_of(&db, ws, &a).await.unwrap(),
            None,
            "attacker is not an authorized member"
        );
    }

    #[test]
    fn deterministic_across_replays() {
        let owner = new_kp();
        let bob = new_kp();
        let (o, b) = (peer_of(&owner), peer_of(&bob));
        let g = signed(&owner, OpKind::Grant, &o, Some(Role::Owner), 1, None);
        let add = signed(
            &owner,
            OpKind::Grant,
            &b,
            Some(Role::Collaborator),
            1,
            Some(g.op_id.clone()),
        );
        let a = materialize(&[g.clone(), add.clone()], &o);
        let c = materialize(&[add, g], &o); // different input order
        assert_eq!(a, c);
    }
}

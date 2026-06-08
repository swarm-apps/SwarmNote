//! E2E sharing schema: per-device key Lockboxes + signed permission op chain,
//! plus a `commitment` column on `share_invites` (key-committing link invites).
//! See `dev-notes/design/{04-permissions,05-sharing,08-e2e-encryption}.md`.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // Per-device key Lockboxes: one row per (workspace, key_version, recipient
        // device). `sealed_read_key` / `sealed_write_key` are independent X25519
        // sealed envelopes (each frame self-contains its nonce + commitment).
        // A v2 read-only recipient has `sealed_write_key = NULL`.
        db.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS workspace_key_lockboxes (
                workspace_id TEXT NOT NULL REFERENCES workspaces(id),
                key_version INTEGER NOT NULL,
                recipient_peer_id TEXT NOT NULL,
                sealed_read_key BLOB NOT NULL,
                sealed_write_key BLOB,
                sealed_by_peer_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                PRIMARY KEY (workspace_id, key_version, recipient_peer_id)
            )",
        )
        .await?;

        // Signed, append-only permission operation chain (auth DAG). Each op is
        // identified by its content hash (`op_id`) and signed by the issuer
        // device's Ed25519 key; `prev_hash` links the causal predecessor.
        db.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS permission_ops (
                op_id TEXT PRIMARY KEY,
                workspace_id TEXT NOT NULL REFERENCES workspaces(id),
                op_kind TEXT NOT NULL,
                target_peer_id TEXT NOT NULL,
                new_role TEXT NOT NULL,
                issuer_peer_id TEXT NOT NULL,
                key_version INTEGER NOT NULL,
                prev_hash TEXT,
                signature BLOB NOT NULL,
                created_at TEXT NOT NULL
            )",
        )
        .await?;
        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_permission_ops_ws ON permission_ops(workspace_id)",
        )
        .await?;

        // Link-invite key commitment (defends partitioning-oracle on the
        // password/secret-wrapped invite). Nullable: pre-existing rows have none.
        db.execute_unprepared("ALTER TABLE share_invites ADD COLUMN commitment BLOB")
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("DROP TABLE IF EXISTS permission_ops")
            .await?;
        db.execute_unprepared("DROP TABLE IF EXISTS workspace_key_lockboxes")
            .await?;
        // SQLite 3.35+ supports DROP COLUMN.
        db.execute_unprepared("ALTER TABLE share_invites DROP COLUMN commitment")
            .await?;
        Ok(())
    }
}

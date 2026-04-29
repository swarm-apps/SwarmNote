use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // --- workspaces ---
        db.execute_unprepared(
            "CREATE TABLE workspaces_new (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                created_by TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .await?;
        db.execute_unprepared(
            "INSERT INTO workspaces_new (id, name, created_by, created_at, updated_at)
             SELECT id, name, created_by,
                    strftime('%Y-%m-%dT%H:%M:%SZ', created_at, 'unixepoch'),
                    strftime('%Y-%m-%dT%H:%M:%SZ', updated_at, 'unixepoch')
             FROM workspaces",
        )
        .await?;
        db.execute_unprepared("DROP TABLE workspaces").await?;
        db.execute_unprepared("ALTER TABLE workspaces_new RENAME TO workspaces")
            .await?;

        // --- workspace_keys ---
        db.execute_unprepared(
            "CREATE TABLE workspace_keys_new (
                workspace_id TEXT PRIMARY KEY REFERENCES workspaces(id),
                read_key_enc BLOB NOT NULL,
                write_key_enc BLOB,
                admin_key_enc BLOB,
                key_version INTEGER NOT NULL DEFAULT 1,
                updated_at TEXT NOT NULL
            )",
        )
        .await?;
        db.execute_unprepared(
            "INSERT INTO workspace_keys_new (workspace_id, read_key_enc, write_key_enc, admin_key_enc, key_version, updated_at)
             SELECT workspace_id, read_key_enc, write_key_enc, admin_key_enc, key_version,
                    strftime('%Y-%m-%dT%H:%M:%SZ', updated_at, 'unixepoch')
             FROM workspace_keys",
        )
        .await?;
        db.execute_unprepared("DROP TABLE workspace_keys").await?;
        db.execute_unprepared("ALTER TABLE workspace_keys_new RENAME TO workspace_keys")
            .await?;

        // --- folders ---
        db.execute_unprepared(
            "CREATE TABLE folders_new (
                id TEXT PRIMARY KEY,
                workspace_id TEXT NOT NULL REFERENCES workspaces(id),
                parent_folder_id TEXT REFERENCES folders_new(id),
                name TEXT NOT NULL,
                rel_path TEXT NOT NULL,
                created_by TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .await?;
        db.execute_unprepared(
            "INSERT INTO folders_new (id, workspace_id, parent_folder_id, name, rel_path, created_by, created_at, updated_at)
             SELECT id, workspace_id, parent_folder_id, name, rel_path, created_by,
                    strftime('%Y-%m-%dT%H:%M:%SZ', created_at, 'unixepoch'),
                    strftime('%Y-%m-%dT%H:%M:%SZ', updated_at, 'unixepoch')
             FROM folders",
        )
        .await?;
        db.execute_unprepared("DROP TABLE folders").await?;
        db.execute_unprepared("ALTER TABLE folders_new RENAME TO folders")
            .await?;

        // --- documents ---
        db.execute_unprepared(
            "CREATE TABLE documents_new (
                id TEXT PRIMARY KEY,
                workspace_id TEXT NOT NULL REFERENCES workspaces(id),
                folder_id TEXT REFERENCES folders(id),
                title TEXT NOT NULL,
                rel_path TEXT NOT NULL,
                file_hash BLOB,
                yjs_state BLOB,
                state_vector BLOB,
                lamport_clock INTEGER NOT NULL DEFAULT 0,
                created_by TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .await?;
        db.execute_unprepared(
            "INSERT INTO documents_new (id, workspace_id, folder_id, title, rel_path, file_hash, yjs_state, state_vector, lamport_clock, created_by, created_at, updated_at)
             SELECT id, workspace_id, folder_id, title, rel_path, file_hash, yjs_state, state_vector, lamport_clock, created_by,
                    strftime('%Y-%m-%dT%H:%M:%SZ', created_at, 'unixepoch'),
                    strftime('%Y-%m-%dT%H:%M:%SZ', updated_at, 'unixepoch')
             FROM documents",
        )
        .await?;
        db.execute_unprepared("DROP TABLE documents").await?;
        db.execute_unprepared("ALTER TABLE documents_new RENAME TO documents")
            .await?;
        // Re-create unique index lost during table rebuild
        db.execute_unprepared(
            "CREATE UNIQUE INDEX idx_documents_ws_rel_path ON documents(workspace_id, rel_path)",
        )
        .await?;

        // --- deletion_log ---
        db.execute_unprepared(
            "CREATE TABLE deletion_log_new (
                doc_id TEXT PRIMARY KEY,
                rel_path TEXT NOT NULL,
                deleted_at TEXT NOT NULL,
                deleted_by TEXT NOT NULL,
                lamport_clock INTEGER NOT NULL
            )",
        )
        .await?;
        db.execute_unprepared(
            "INSERT INTO deletion_log_new (doc_id, rel_path, deleted_at, deleted_by, lamport_clock)
             SELECT doc_id, rel_path,
                    strftime('%Y-%m-%dT%H:%M:%SZ', deleted_at, 'unixepoch'),
                    deleted_by, lamport_clock
             FROM deletion_log",
        )
        .await?;
        db.execute_unprepared("DROP TABLE deletion_log").await?;
        db.execute_unprepared("ALTER TABLE deletion_log_new RENAME TO deletion_log")
            .await?;

        // --- permissions ---
        db.execute_unprepared(
            "CREATE TABLE permissions_new (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                resource_type TEXT NOT NULL,
                resource_id TEXT NOT NULL,
                peer_id TEXT NOT NULL,
                role TEXT NOT NULL,
                granted_by TEXT NOT NULL,
                granted_at TEXT NOT NULL,
                UNIQUE(resource_type, resource_id, peer_id)
            )",
        )
        .await?;
        db.execute_unprepared(
            "INSERT INTO permissions_new (id, resource_type, resource_id, peer_id, role, granted_by, granted_at)
             SELECT id, resource_type, resource_id, peer_id, role, granted_by,
                    strftime('%Y-%m-%dT%H:%M:%SZ', granted_at, 'unixepoch')
             FROM permissions",
        )
        .await?;
        db.execute_unprepared("DROP TABLE permissions").await?;
        db.execute_unprepared("ALTER TABLE permissions_new RENAME TO permissions")
            .await?;

        // --- share_invites ---
        db.execute_unprepared(
            "CREATE TABLE share_invites_new (
                token TEXT PRIMARY KEY,
                resource_type TEXT NOT NULL,
                resource_id TEXT NOT NULL,
                role TEXT NOT NULL,
                encrypted_keys BLOB NOT NULL,
                created_by TEXT NOT NULL,
                created_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                max_uses INTEGER,
                used_count INTEGER NOT NULL DEFAULT 0,
                password_hash TEXT
            )",
        )
        .await?;
        db.execute_unprepared(
            "INSERT INTO share_invites_new (token, resource_type, resource_id, role, encrypted_keys, created_by, created_at, expires_at, max_uses, used_count, password_hash)
             SELECT token, resource_type, resource_id, role, encrypted_keys, created_by,
                    strftime('%Y-%m-%dT%H:%M:%SZ', created_at, 'unixepoch'),
                    strftime('%Y-%m-%dT%H:%M:%SZ', expires_at, 'unixepoch'),
                    max_uses, used_count, password_hash
             FROM share_invites",
        )
        .await?;
        db.execute_unprepared("DROP TABLE share_invites").await?;
        db.execute_unprepared("ALTER TABLE share_invites_new RENAME TO share_invites")
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // Reverse: TEXT → INTEGER for all tables
        // Only workspaces shown as example; full reverse would mirror up() with
        // CAST(strftime('%s', col) AS INTEGER)

        db.execute_unprepared(
            "CREATE TABLE workspaces_old (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                created_by TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
        )
        .await?;
        db.execute_unprepared(
            "INSERT INTO workspaces_old (id, name, created_by, created_at, updated_at)
             SELECT id, name, created_by,
                    CAST(strftime('%s', created_at) AS INTEGER),
                    CAST(strftime('%s', updated_at) AS INTEGER)
             FROM workspaces",
        )
        .await?;
        db.execute_unprepared("DROP TABLE workspaces").await?;
        db.execute_unprepared("ALTER TABLE workspaces_old RENAME TO workspaces")
            .await?;

        // TODO: repeat for remaining tables if full rollback is needed
        Ok(())
    }
}

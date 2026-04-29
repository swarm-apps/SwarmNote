use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // workspaces
        db.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS workspaces (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                created_by TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
        )
        .await?;

        // workspace_keys
        db.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS workspace_keys (
                workspace_id TEXT PRIMARY KEY REFERENCES workspaces(id),
                read_key_enc BLOB NOT NULL,
                write_key_enc BLOB,
                admin_key_enc BLOB,
                key_version INTEGER NOT NULL DEFAULT 1,
                updated_at INTEGER NOT NULL
            )",
        )
        .await?;

        // folders (self-referencing)
        db.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS folders (
                id TEXT PRIMARY KEY,
                workspace_id TEXT NOT NULL REFERENCES workspaces(id),
                parent_folder_id TEXT REFERENCES folders(id),
                name TEXT NOT NULL,
                rel_path TEXT NOT NULL,
                created_by TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
        )
        .await?;

        // documents
        db.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS documents (
                id TEXT PRIMARY KEY,
                workspace_id TEXT NOT NULL REFERENCES workspaces(id),
                folder_id TEXT REFERENCES folders(id),
                title TEXT NOT NULL,
                rel_path TEXT NOT NULL,
                file_hash BLOB,
                yjs_state BLOB,
                state_vector BLOB,
                created_by TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
        )
        .await?;

        // doc_chunks
        db.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS doc_chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                doc_id TEXT NOT NULL REFERENCES documents(id),
                chunk_offset INTEGER NOT NULL,
                chunk_length INTEGER NOT NULL,
                chunk_hash BLOB NOT NULL,
                UNIQUE(doc_id, chunk_offset)
            )",
        )
        .await?;

        // permissions
        db.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS permissions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                resource_type TEXT NOT NULL,
                resource_id TEXT NOT NULL,
                peer_id TEXT NOT NULL,
                role TEXT NOT NULL,
                granted_by TEXT NOT NULL,
                granted_at INTEGER NOT NULL,
                UNIQUE(resource_type, resource_id, peer_id)
            )",
        )
        .await?;

        // share_invites
        db.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS share_invites (
                token TEXT PRIMARY KEY,
                resource_type TEXT NOT NULL,
                resource_id TEXT NOT NULL,
                role TEXT NOT NULL,
                encrypted_keys BLOB NOT NULL,
                created_by TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                max_uses INTEGER,
                used_count INTEGER NOT NULL DEFAULT 0,
                password_hash TEXT
            )",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        // Drop in reverse dependency order
        for table in [
            "share_invites",
            "permissions",
            "doc_chunks",
            "documents",
            "folders",
            "workspace_keys",
            "workspaces",
        ] {
            db.execute_unprepared(&format!("DROP TABLE IF EXISTS {table}"))
                .await?;
        }
        Ok(())
    }
}

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // Clean up legacy bug: frontend passed rel_path as document id.
        // Valid UUIDs are exactly 36 chars (xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx).
        db.execute_unprepared("DELETE FROM documents WHERE length(id) != 36")
            .await?;

        // Add lamport clock for sync ordering
        db.execute_unprepared(
            "ALTER TABLE documents ADD COLUMN lamport_clock INTEGER NOT NULL DEFAULT 0",
        )
        .await?;

        // Deletion log for tombstone-based sync (prevents resurrection)
        db.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS deletion_log (
                doc_id TEXT PRIMARY KEY,
                rel_path TEXT NOT NULL,
                deleted_at INTEGER NOT NULL,
                deleted_by TEXT NOT NULL,
                lamport_clock INTEGER NOT NULL
            )",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("DROP TABLE IF EXISTS deletion_log")
            .await?;
        // Note: SQLite does not support DROP COLUMN
        Ok(())
    }
}

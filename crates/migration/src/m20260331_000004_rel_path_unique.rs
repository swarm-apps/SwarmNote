use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // Remove duplicate (workspace_id, rel_path) rows, keeping the one with the earliest created_at.
        // SQLite supports window functions since 3.25.0.
        db.execute_unprepared(
            "DELETE FROM documents WHERE rowid NOT IN (
                SELECT MIN(rowid) FROM documents GROUP BY workspace_id, rel_path
            )",
        )
        .await?;

        // Now enforce uniqueness going forward.
        db.execute_unprepared(
            "CREATE UNIQUE INDEX idx_documents_ws_rel_path ON documents(workspace_id, rel_path)",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("DROP INDEX IF EXISTS idx_documents_ws_rel_path")
            .await?;
        Ok(())
    }
}

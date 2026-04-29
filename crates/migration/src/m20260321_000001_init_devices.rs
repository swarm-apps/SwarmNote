use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS paired_devices (
                peer_id     TEXT PRIMARY KEY,
                hostname    TEXT NOT NULL,
                os          TEXT,
                platform    TEXT,
                arch        TEXT,
                paired_at   INTEGER NOT NULL,
                last_seen   INTEGER
            )",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Alias::new("paired_devices")).to_owned())
            .await
    }
}

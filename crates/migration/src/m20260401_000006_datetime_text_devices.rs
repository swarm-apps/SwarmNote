use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared(
            "CREATE TABLE paired_devices_new (
                peer_id     TEXT PRIMARY KEY,
                hostname    TEXT NOT NULL,
                os          TEXT,
                platform    TEXT,
                arch        TEXT,
                paired_at   TEXT NOT NULL,
                last_seen   TEXT
            )",
        )
        .await?;
        db.execute_unprepared(
            "INSERT INTO paired_devices_new (peer_id, hostname, os, platform, arch, paired_at, last_seen)
             SELECT peer_id, hostname, os, platform, arch,
                    strftime('%Y-%m-%dT%H:%M:%SZ', paired_at, 'unixepoch'),
                    CASE WHEN last_seen IS NOT NULL
                         THEN strftime('%Y-%m-%dT%H:%M:%SZ', last_seen, 'unixepoch')
                         ELSE NULL END
             FROM paired_devices",
        )
        .await?;
        db.execute_unprepared("DROP TABLE paired_devices").await?;
        db.execute_unprepared("ALTER TABLE paired_devices_new RENAME TO paired_devices")
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared(
            "CREATE TABLE paired_devices_old (
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
        db.execute_unprepared(
            "INSERT INTO paired_devices_old (peer_id, hostname, os, platform, arch, paired_at, last_seen)
             SELECT peer_id, hostname, os, platform, arch,
                    CAST(strftime('%s', paired_at) AS INTEGER),
                    CASE WHEN last_seen IS NOT NULL
                         THEN CAST(strftime('%s', last_seen) AS INTEGER)
                         ELSE NULL END
             FROM paired_devices",
        )
        .await?;
        db.execute_unprepared("DROP TABLE paired_devices").await?;
        db.execute_unprepared("ALTER TABLE paired_devices_old RENAME TO paired_devices")
            .await?;

        Ok(())
    }
}
